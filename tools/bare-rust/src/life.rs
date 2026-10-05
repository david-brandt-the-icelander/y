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
            if pos.len() == 1 {
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

/// Edits for the check-2 normaliser, over a whole tree.
pub fn normalise_edits(n: Node, src: &str, out: &mut Vec<Edit>) {
    match n.kind() {
        "token_tree" => return,
        "function_item" | "function_signature_item" => out.extend(analyse(n, src).cuts),
        "let_declaration" | "parameter" | "closure_parameters" => mut_edits(n, out),
        _ => {}
    }
    for c in named_children(n) {
        normalise_edits(c, src, out);
    }
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
                for c in named_children(p).into_iter().filter(|c| !is_comment(*c)) {
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
