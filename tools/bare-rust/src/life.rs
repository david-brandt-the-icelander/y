//! Syntax analyses shared by strip, weave and the check-2 normaliser: lifetime elision, `mut` on bindings,
//! and the sites in a body where a concrete type becomes a hole.
use crate::{all_children, is_comment, is_item_kind, named_children, text, Edit};
use std::collections::HashSet;
use tree_sitter::Node;

/// Lifetime parameters of a function that Rust's elision rules can express without a name.
pub struct Life {
    pub elide: HashSet<String>,
    pub cuts: Vec<Edit>,
}

struct Mention<'a> {
    name: String,
    node: Node<'a>,
    on_ref: bool,
    input: bool,
    output: bool,
    decl: bool,
}

fn mentions<'a>(n: Node<'a>, src: &str, region: u8, out: &mut Vec<Mention<'a>>) {
    if n.kind() == "lifetime" {
        let p = n.parent();
        let decl = p.is_some_and(|p| {
            p.kind() == "lifetime_parameter" && p.child_by_field_name("name").is_some_and(|x| x.id() == n.id())
        });
        let on_ref = p.is_some_and(|p| matches!(p.kind(), "reference_type" | "self_parameter"));
        out.push(Mention { name: text(n, src).to_string(), node: n, on_ref, input: region == 1, output: region == 2, decl });
        return;
    }
    for c in all_children(n) {
        mentions(c, src, region, out);
    }
}

/// Which of a function's lifetime parameters may be elided, and the edits that elide them in Rust source.
pub fn analyse(f: Node, src: &str) -> Life {
    let mut ms = vec![];
    let tp = f.child_by_field_name("type_parameters");
    let params = f.child_by_field_name("parameters");
    if let Some(t) = tp {
        mentions(t, src, 0, &mut ms);
    }
    if let Some(p) = params {
        mentions(p, src, 1, &mut ms);
    }
    if let Some(r) = f.child_by_field_name("return_type") {
        mentions(r, src, 2, &mut ms);
    }
    for c in named_children(f) {
        if c.kind() == "where_clause" {
            mentions(c, src, 0, &mut ms);
        }
    }
    let mut bad: HashSet<String> = HashSet::new();
    let mut fnl: Vec<String> = vec![];
    if let Some(t) = tp {
        for c in named_children(t) {
            if c.kind() == "lifetime_parameter" {
                let name = text(c.child_by_field_name("name").unwrap(), src).to_string();
                if c.child_by_field_name("bounds").is_some() {
                    bad.insert(name.clone());
                }
                fnl.push(name);
            }
        }
    }
    for m in &ms {
        if !m.decl && (!m.on_ref || !(m.input || m.output)) {
            bad.insert(m.name.clone());
        }
    }
    // The lifetime the elision rules would give the result: `&self`'s, or the only one among the inputs.
    let self_ref = params
        .and_then(|p| named_children(p).into_iter().find(|c| c.kind() == "self_parameter"))
        .filter(|s| text(*s, src).starts_with('&'));
    let source: Option<Option<String>> = match self_ref {
        Some(s) => Some(named_children(s).iter().find(|c| c.kind() == "lifetime").map(|l| text(*l, src).to_string())),
        None => {
            let mut pos: Vec<Option<String>> = vec![];
            if let Some(p) = params {
                count_positions(p, src, &mut pos);
            }
            // A path type such as `Node` may carry a lifetime the source does not show; then the single visible
            // lifetime is not the only input lifetime, and Rust would not give it to the result.
            let tparams: Vec<&str> = tp
                .map(|t| {
                    named_children(t)
                        .into_iter()
                        .filter_map(|c| match c.kind() {
                            "type_identifier" => Some(c),
                            "type_parameter" => c.child_by_field_name("name"),
                            "constrained_type_parameter" => c.child_by_field_name("left"),
                            "optional_type_parameter" => c.child_by_field_name("name"),
                            _ => None,
                        })
                        .map(|n| text(n, src))
                        .collect()
                })
                .unwrap_or_default();
            let hides = params.is_some_and(|p| named_children(p).into_iter().filter(|c| c.kind() == "parameter").filter_map(|c| c.child_by_field_name("type")).any(|t| may_hide(t, src, &tparams)));
            if pos.len() == 1 && !hides {
                Some(pos.remove(0))
            } else {
                None
            }
        }
    };
    let mut elide = HashSet::new();
    for l in fnl {
        if bad.contains(&l) || l == "'_" {
            continue;
        }
        let ins = ms.iter().filter(|m| m.on_ref && m.input && m.name == l).count();
        let outs = ms.iter().any(|m| m.on_ref && m.output && m.name == l);
        if ins != 1 || (outs && source != Some(Some(l.clone()))) {
            continue;
        }
        elide.insert(l);
    }
    let mut cuts = vec![];
    for m in &ms {
        if m.on_ref && elide.contains(&m.name) {
            if let Some(next) = m.node.next_sibling() {
                cuts.push((m.node.start_byte(), next.start_byte(), String::new()));
            }
        }
    }
    if let Some(t) = tp {
        let kids = named_children(t);
        let gone = |c: &Node| {
            c.kind() == "lifetime_parameter"
                && elide.contains(text(c.child_by_field_name("name").unwrap(), src))
        };
        if !elide.is_empty() {
            let rest: Vec<&str> = kids.iter().filter(|c| !gone(c)).map(|c| text(*c, src)).collect();
            let new = if rest.is_empty() { String::new() } else { format!("<{}>", rest.join(", ")) };
            cuts.push((t.start_byte(), t.end_byte(), new));
        }
    }
    Life { elide, cuts }
}

/// The lifetime positions of the inputs: one entry per reference or lifetime, with its name if it has one.
fn count_positions(n: Node, src: &str, out: &mut Vec<Option<String>>) {
    match n.kind() {
        "reference_type" => {
            let l = named_children(n).into_iter().find(|c| c.kind() == "lifetime");
            out.push(l.map(|l| text(l, src).to_string()));
            if let Some(inner) = n.child_by_field_name("type") {
                count_positions(inner, src, out);
            }
        }
        "lifetime" => out.push(Some(text(n, src).to_string())),
        _ => {
            for c in all_children(n) {
                count_positions(c, src, out);
            }
        }
    }
}

/// Whether a type may contain a lifetime that its text does not show: a path type that is not a primitive, a
/// type parameter of the function or a well-known type without lifetimes (the fully known ones are looked into).
fn may_hide(n: Node, src: &str, tparams: &[&str]) -> bool {
    const FREE: [&str; 17] = [
        "String", "Vec", "Option", "Box", "Rc", "Arc", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Result", "VecDeque", "Cell", "RefCell",
        "PathBuf", "Duration", "Ordering",
    ];
    match n.kind() {
        "function_type" => false,
        "type_identifier" => {
            let t = text(n, src);
            !(FREE.contains(&t) || tparams.contains(&t))
        }
        "scoped_type_identifier" => !FREE.contains(&text(n, src).rsplit("::").next().unwrap_or("")),
        "abstract_type" | "dynamic_type" | "bounded_type" => true,
        _ => all_children(n).into_iter().any(|c| may_hide(c, src, tparams)),
    }
}

/// The lifetime positions of a parameter type, for deciding what a result borrows from.
pub fn positions(n: Node, src: &str) -> Vec<Option<String>> {
    let mut v = vec![];
    count_positions(n, src, &mut v);
    v
}

// ---------- mut ----------

/// Edits that remove `mut` from the bindings of a pattern.
pub fn pat_mut_edits(pat: Node, out: &mut Vec<Edit>) {
    match pat.kind() {
        "token_tree" | "closure_expression" => return,
        "mut_pattern" if pat.parent().is_none_or(|p| p.kind() != "ref_pattern") => {
            if let Some(inner) = pat.named_child(1) {
                out.push((pat.start_byte(), inner.start_byte(), String::new()));
            }
        }
        "field_pattern" => {
            let kids = all_children(pat);
            if !kids.iter().any(|k| k.kind() == "ref") {
                if let Some(i) = kids.iter().position(|k| k.kind() == "mutable_specifier") {
                    out.push((kids[i].start_byte(), kids[i + 1].start_byte(), String::new()));
                }
            }
        }
        _ => {}
    }
    for c in named_children(pat) {
        pat_mut_edits(c, out);
    }
}

fn text_starts_with_ref(n: Node) -> bool {
    n.child(0).is_some_and(|c| c.kind() == "&")
}

/// Edits that remove `mut` from a `let`, a `parameter` or the parameters of a closure.
pub fn mut_edits(n: Node, out: &mut Vec<Edit>) {
    match n.kind() {
        "let_declaration" | "parameter" => {
            let Some(pat) = n.child_by_field_name("pattern") else { return };
            if let Some(ms) = named_children(n).into_iter().find(|c| c.kind() == "mutable_specifier") {
                out.push((ms.start_byte(), pat.start_byte(), String::new()));
            }
            pat_mut_edits(pat, out);
        }
        // `mut self`, by value: a binding like any other. (`&mut self` is access, which the file carries.)
        "self_parameter" if !text_starts_with_ref(n) => {
            if let (Some(ms), Some(s)) = (
                named_children(n).into_iter().find(|c| c.kind() == "mutable_specifier"),
                named_children(n).into_iter().find(|c| c.kind() == "self"),
            ) {
                out.push((ms.start_byte(), s.start_byte(), String::new()));
            }
        }
        "closure_parameters" => {
            for c in named_children(n) {
                if c.kind() != "parameter" && !is_comment(c) {
                    pat_mut_edits(c, out);
                }
            }
        }
        _ => {}
    }
}

/// Edits for the check-2 normaliser, over a whole tree: lifetimes elided where Rust allows, and, only if
/// `strip_mut`, `mut` removed from bindings (for weaves that did not derive `mut` through rustc).
pub fn normalise_edits(n: Node, src: &str, out: &mut Vec<Edit>, strip_mut: bool) {
    match n.kind() {
        "token_tree" => return,
        "function_item" | "function_signature_item" => out.extend(analyse(n, src).cuts),
        "let_declaration" | "parameter" | "closure_parameters" | "self_parameter" if strip_mut => mut_edits(n, out),
        _ => {}
    }
    for c in named_children(n) {
        normalise_edits(c, src, out, strip_mut);
    }
}

/// How many bindings carry `mut` in a tree (the ones `mut_edits` would cut).
pub fn count_mut(n: Node) -> usize {
    if n.kind() == "token_tree" {
        return 0;
    }
    let mut v = vec![];
    if matches!(n.kind(), "let_declaration" | "parameter" | "closure_parameters" | "self_parameter") {
        mut_edits(n, &mut v);
    }
    v.len() + named_children(n).into_iter().map(count_mut).sum::<usize>()
}

// ---------- sites ----------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SK {
    Cast,
    Targ,
    Lit,
    ClParam,
    ClRet,
    Let,
}

/// A place in a body where a concrete type is, or can be, a hole. `n` counts sites of the same kind.
pub struct Site<'a> {
    pub kind: SK,
    pub n: usize,
    pub node: Node<'a>,
}

fn is_type_arg(c: Node) -> bool {
    !is_comment(c)
        && !matches!(
            c.kind(),
            "lifetime"
                | "type_binding"
                | "block"
                | "string_literal"
                | "integer_literal"
                | "float_literal"
                | "boolean_literal"
                | "char_literal"
                | "negative_literal"
                | "raw_string_literal"
        )
}

fn push<'a>(out: &mut Vec<Site<'a>>, cnt: &mut [usize; 6], kind: SK, node: Node<'a>) {
    let i = kind as usize;
    out.push(Site { kind, n: cnt[i], node });
    cnt[i] += 1;
}

fn walk<'a>(n: Node<'a>, out: &mut Vec<Site<'a>>, cnt: &mut [usize; 6]) {
    match n.kind() {
        "token_tree" => {}
        k if is_item_kind(k) => {}
        "integer_literal" | "float_literal" => push(out, cnt, SK::Lit, n),
        "type_cast_expression" => {
            if let Some(t) = n.child_by_field_name("type") {
                push(out, cnt, SK::Cast, t);
            }
            if let Some(v) = n.child_by_field_name("value") {
                walk(v, out, cnt);
            }
        }
        "type_arguments" => {
            for c in named_children(n).into_iter().filter(|c| is_type_arg(*c)) {
                push(out, cnt, SK::Targ, c);
            }
        }
        "closure_expression" => {
            push(out, cnt, SK::ClRet, n);
            if let Some(p) = n.child_by_field_name("parameters") {
                // `|_|`: the `_` is an anonymous node in tree-sitter-rust and a named one in the bare grammar.
                for c in all_children(p).into_iter().filter(|c| !is_comment(*c) && !matches!(c.kind(), "|" | ",")) {
                    push(out, cnt, SK::ClParam, c);
                }
            }
            if let Some(b) = n.child_by_field_name("body") {
                walk(b, out, cnt);
            }
        }
        "let_declaration" => {
            push(out, cnt, SK::Let, n);
            let ty = n.child_by_field_name("type").map(|t| t.id());
            for c in named_children(n) {
                if Some(c.id()) != ty {
                    walk(c, out, cnt);
                }
            }
        }
        _ => {
            for c in named_children(n) {
                walk(c, out, cnt);
            }
        }
    }
}

/// An expression that binds tighter than `as`, so `<e> as T` needs no brackets around `<e>`.
pub fn binds_tighter_than_as(n: Node) -> bool {
    matches!(
        n.kind(),
        "identifier"
            | "scoped_identifier"
            | "self"
            | "field_expression"
            | "call_expression"
            | "index_expression"
            | "parenthesized_expression"
            | "integer_literal"
            | "float_literal"
            | "boolean_literal"
            | "char_literal"
            | "unary_expression"
            | "type_cast_expression"
            | "try_expression"
            | "await_expression"
            | "macro_invocation"
    )
}

/// The macro calls of a body (not of the items nested in it) whose tokens state a type: a cast, a suffixed literal or
/// a turbofish. `strip` cannot reach into them, so it marks them `rust! { ... }`.
pub fn typed_macro_calls<'a>(body: Node<'a>, src: &str) -> Vec<Node<'a>> {
    fn typed(tt: Node, src: &str) -> bool {
        let toks = all_children(tt);
        toks.iter().enumerate().any(|(i, t)| match t.kind() {
            "token_tree" => typed(*t, src),
            "as" => true,
            "integer_literal" | "float_literal" => lit_suffix(text(*t, src)).is_some(),
            "::" => toks.get(i + 1).is_some_and(|n| n.kind() == "<"),
            _ => false,
        })
    }
    fn walk<'a>(n: Node<'a>, src: &str, out: &mut Vec<Node<'a>>) {
        if is_item_kind(n.kind()) {
            return;
        }
        if n.kind() == "macro_invocation" {
            if all_children(n).into_iter().any(|c| c.kind() == "token_tree" && typed(c, src)) {
                out.push(n);
            }
            return;
        }
        for c in named_children(n) {
            walk(c, src, out);
        }
    }
    let mut out = vec![];
    for c in named_children(body) {
        walk(c, src, &mut out);
    }
    out
}

/// `rust! { <text> }`: the Rust text it carries, if `n` is that marker.
pub fn rust_marker<'a>(n: Node, src: &'a str) -> Option<&'a str> {
    if n.kind() != "macro_invocation" || n.child_by_field_name("macro").map(|m| text(m, src)) != Some("rust") {
        return None;
    }
    let tt = all_children(n).into_iter().find(|c| c.kind() == "token_tree")?;
    let t = text(tt, src);
    Some(t.get(1..t.len().saturating_sub(1))?.trim())
}

/// The sites of a body (or of an initialiser), in source order.
pub fn sites(body: Node) -> Vec<Site> {
    let mut out = vec![];
    walk(body, &mut out, &mut [0; 6]);
    out
}

/// Split a numeric literal into its digits and its type suffix: (digits, separator `_`?, suffix).
pub fn lit_suffix(t: &str) -> Option<(&str, bool, &str)> {
    const SUF: [&str; 14] =
        ["u128", "i128", "usize", "isize", "u16", "u32", "u64", "i16", "i32", "i64", "u8", "i8", "f32", "f64"];
    let radix = t.starts_with("0x") || t.starts_with("0b") || t.starts_with("0o");
    for s in SUF {
        if let Some(body) = t.strip_suffix(s) {
            let sep = body.ends_with('_');
            let digits = body.trim_end_matches('_');
            if digits.is_empty() || (radix && !sep && s.starts_with('f')) || (digits == "0x" || digits == "0b" || digits == "0o") {
                continue;
            }
            return Some((digits, sep, s));
        }
    }
    None
}
