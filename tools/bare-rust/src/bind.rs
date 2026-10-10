//! bind: give every value that carries a written type a name to carry it.
//!
//! A cast (`e as u64`) or a suffixed literal (`1u8`) states a type. Where it stands in the middle of an expression,
//! nothing in the bare form is named for that type to belong to, and it would have to go in a positional hint. `bind`
//! rewrites the Rust so that each one initialises a typed `let`:
//!
//! ```text
//! (h as u64).wrapping_mul(P)        let h_u64: u64 = h as u64;
//!                                   h_u64.wrapping_mul(P)
//! ```
//!
//! after which `strip` carries the type on the local (a cast local, `{"type": "u64", "cast": true}`). The rewrite keeps
//! what the program does:
//!
//! - **Where it runs.** A value is bound just before the statement it is in. Where that statement would run it when
//!   the original might not (the right of `&&` or `||`, a match arm or guard, an `else if`, a `while` condition, a
//!   closure body), the binding goes into a new block at that place instead: `a && { let b_u8: u8 = b as u8; f(b_u8) }`.
//! - **Order.** A bound value runs before the rest of its statement. Anything in the statement that runs before it
//!   and could have an effect (a call, an index, a macro) is bound first, in order. Where that is not a plain value
//!   (a method receiver, a borrowed place, the left of an assignment), the site is left as it is and reported.
//! - **Already named.** `let n = e as f64;` gets a type, `let n: f64 = e as f64;`, rather than a second name.
//!
//! The names are mechanical (`h_u64`, `k1_u8`, `len_v`), and unique in their function. They are meant to be renamed
//! once by someone who knows what the values are. Sites `bind` cannot name are reported with the reason; `strip`
//! leaves them as `as _` with a hint.
use crate::life::{self, SK};
use crate::{apply, first_error, is_comment, named_children, parse_rust, text, Edit, R};
use std::collections::HashSet;
use tree_sitter::Node;

/// A site that `bind` left as it was.
#[derive(Clone, Debug, PartialEq)]
pub struct Left {
    pub line: usize,
    pub col: usize,
    /// `cast` or `literal`.
    pub what: &'static str,
    pub why: &'static str,
}

/// A cast target that a local can carry: a primitive written as such.
pub fn carried_type(t: &str) -> bool {
    t != "_" && (crate::types::is_prim(t) || matches!(t, "usize" | "isize" | "char"))
}

/// The source with every cast and suffixed literal that can be bound, bound.
pub fn bind(src: &str) -> R<String> {
    bind_report(src).map(|r| r.0)
}

/// As `bind`, with the sites it left.
pub fn bind_report(src: &str) -> R<(String, Vec<Left>)> {
    let mut s = src.to_string();
    let (mut fi, mut left, mut steps) = (0, vec![], 0);
    loop {
        let tree = parse_rust(&s);
        if let Some(e) = first_error(&tree) {
            return Err(format!("bind: parse-error at {e}"));
        }
        let mut fns = vec![];
        functions(tree.root_node(), &mut fns);
        let Some(&body) = fns.get(fi) else { break };
        match step(body, &s) {
            Step::Edit(edits) => {
                steps += 1;
                if steps > 100_000 {
                    return Err("bind: no fixpoint".into());
                }
                s = apply(&s, edits);
            }
            Step::Done(l) => {
                left.extend(l);
                fi += 1;
            }
        }
    }
    Ok((s, left))
}

/// Function bodies, outer before inner.
fn functions<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
    if n.kind() == "function_item" {
        if let Some(b) = n.child_by_field_name("body") {
            out.push(b);
        }
    }
    for c in named_children(n) {
        functions(c, out);
    }
}

enum Step {
    Edit(Vec<Edit>),
    Done(Vec<Left>),
}

/// One rewrite in a function body, or the sites it leaves.
fn step(body: Node, src: &str) -> Step {
    let mut sites: Vec<(Node, &'static str, String)> = vec![];
    for s in life::sites(body) {
        match s.kind {
            SK::Cast => {
                if let Some(e) = s.node.parent() {
                    sites.push((e, "cast", text(s.node, src).to_string()));
                }
            }
            SK::Lit => {
                if let Some((_, _, suffix)) = life::lit_suffix(text(s.node, src)) {
                    let neg = s.node.parent().filter(|p| p.kind() == "unary_expression" && text(*p, src).starts_with('-'));
                    sites.push((neg.unwrap_or(s.node), "literal", suffix.to_string()));
                }
            }
            _ => {}
        }
    }
    // Innermost first, then in order: the order the values are computed in.
    sites.sort_by_key(|(e, _, _)| (e.end_byte(), std::cmp::Reverse(e.start_byte())));
    let mut used = idents(text(body, src));
    let mut left = vec![];
    for (e, what, ty) in sites {
        match plan(e, what, &ty, src, &mut used) {
            Ok(edits) if edits.is_empty() => {}
            Ok(edits) => return Step::Edit(edits),
            Err(why) => {
                let p = e.start_position();
                left.push(Left { line: p.row + 1, col: p.column + 1, what, why });
            }
        }
    }
    Step::Done(left)
}

fn idents(s: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
        } else if !cur.is_empty() {
            out.insert(std::mem::take(&mut cur));
        }
    }
    out
}

fn fresh(base: &str, used: &mut HashSet<String>) -> String {
    let base: String = base.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    let base = if base.starts_with(|c: char| c.is_ascii_digit()) { format!("v{base}") } else { base };
    let mut name = base.clone();
    let mut i = 2;
    while used.contains(&name) {
        name = format!("{base}_{i}");
        i += 1;
    }
    used.insert(name.clone());
    name
}

/// A short name for the value of an expression: the variable, field, function or method it comes from.
fn base_name(n: Node, src: &str) -> String {
    match n.kind() {
        "identifier" => text(n, src).to_string(),
        "self" => "this".into(),
        "parenthesized_expression" | "reference_expression" | "unary_expression" | "try_expression" | "await_expression" => {
            named_children(n).into_iter().find(|c| !is_comment(*c)).map_or("value".into(), |c| base_name(c, src))
        }
        "field_expression" => n.child_by_field_name("field").map_or("value".into(), |f| text(f, src).to_string()),
        "scoped_identifier" => n.child_by_field_name("name").map_or("value".into(), |f| text(f, src).to_string()),
        "call_expression" => n.child_by_field_name("function").map_or("value".into(), |f| base_name(f, src)),
        "index_expression" => named_children(n).first().map_or("item".into(), |v| format!("{}_item", base_name(*v, src))),
        "type_cast_expression" => n.child_by_field_name("value").map_or("value".into(), |v| base_name(v, src)),
        "generic_function" => n.child_by_field_name("function").map_or("value".into(), |f| base_name(f, src)),
        _ => "value".into(),
    }
}

/// Free of effects, as far as the order of evaluation goes: reading places and computing on what was read.
fn pure(n: Node) -> bool {
    match n.kind() {
        "identifier" | "self" | "scoped_identifier" | "field_identifier" | "integer_literal" | "float_literal"
        | "boolean_literal" | "char_literal" | "string_literal" | "raw_string_literal" | "unit_expression" | "crate"
        | "super" | "closure_expression" => true,
        "field_expression" => n.child_by_field_name("value").is_some_and(pure),
        "reference_expression" | "unary_expression" | "parenthesized_expression" | "type_cast_expression"
        | "binary_expression" | "tuple_expression" | "array_expression" | "range_expression" | "negative_literal" => {
            named_children(n).into_iter().filter(|c| !is_comment(*c)).all(|c| pure(c) || is_type_node(c))
        }
        _ => false,
    }
}

fn is_type_node(n: Node) -> bool {
    n.kind().ends_with("_type") || matches!(n.kind(), "primitive_type" | "type_identifier" | "type_arguments")
}

enum Anchor<'a> {
    /// A statement (or the tail expression) of a block: bind before it.
    Stmt(Node<'a>),
    /// An expression that runs conditionally or repeatedly: bind in a block put in its place.
    Boundary(Node<'a>),
}

/// Where the bindings for a value at `e` go.
fn anchor(e: Node) -> Result<Anchor, &'static str> {
    let mut cur = e;
    loop {
        let Some(p) = cur.parent() else { return Err("not in a function body") };
        let is = |f: &str| p.child_by_field_name(f) == Some(cur);
        match p.kind() {
            "block" => return Ok(Anchor::Stmt(cur)),
            "match_pattern" if is("condition") => return Ok(Anchor::Boundary(cur)),
            "match_pattern" | "let_declaration" if is("pattern") => return Err("in a pattern"),
            "match_pattern" => return Err("in a pattern"),
            k if k.ends_with("_pattern") => return Err("in a pattern"),
            k if k.ends_with("_type") || matches!(k, "type_arguments" | "attribute_item" | "token_tree" | "macro_invocation") => {
                return Err("in a type or macro")
            }
            "array_expression" if is("length") => return Err("in a constant context"),
            "const_item" | "static_item" | "function_item" | "const_block" => return Err("in a constant context"),
            "binary_expression" if is("right") => {
                let op = p.child_by_field_name("operator").map(|o| o.kind());
                if matches!(op, Some("&&") | Some("||")) {
                    return Ok(Anchor::Boundary(cur));
                }
            }
            "else_clause" if cur.kind() == "if_expression" => return Ok(Anchor::Boundary(cur)),
            "match_arm" if is("value") => return Ok(Anchor::Boundary(cur)),
            "while_expression" if is("condition") => {
                if cur.kind() == "let_condition" || cur.kind() == "let_chain" {
                    return Err("in a `while let` condition");
                }
                return Ok(Anchor::Boundary(cur));
            }
            "let_condition" if is("value") => {
                if p.parent().is_some_and(|g| g.kind() == "while_expression") {
                    return Ok(Anchor::Boundary(cur));
                }
            }
            "let_chain" => return Err("in a let chain"),
            "closure_expression" if is("body") => return Ok(Anchor::Boundary(cur)),
            _ => {}
        }
        cur = p;
    }
}

/// The expressions in `region` that run before `e`, outermost first, with whether each can be bound by value.
fn earlier<'a>(e: Node<'a>, region: Node<'a>) -> Vec<(Node<'a>, bool)> {
    let mut out = vec![];
    let mut cur = e;
    while cur.id() != region.id() {
        let Some(p) = cur.parent() else { break };
        let assigns = matches!(p.kind(), "assignment_expression" | "compound_assignment_expr");
        if !(assigns && p.child_by_field_name("right") == Some(cur)) {
            for c in named_children(p).into_iter().take_while(|c| c.id() != cur.id()) {
                // `S { a: f(), b: x as u8 }`: what runs is the field's value.
                let c = match c.kind() {
                    "field_initializer" => match c.child_by_field_name("value") {
                        Some(v) => v,
                        None => continue,
                    },
                    "base_field_initializer" => match named_children(c).into_iter().next() {
                        Some(v) => v,
                        None => continue,
                    },
                    _ => c,
                };
                let skip = is_comment(c)
                    || is_type_node(c)
                    || matches!(c.kind(), "field_identifier" | "label" | "mutable_specifier" | "type_arguments")
                    || ["pattern", "type", "name", "field"].iter().any(|f| p.child_by_field_name(f) == Some(c));
                if !skip {
                    // A plain operand can be bound by value; a receiver, a borrowed place or an assignee cannot.
                    let place = matches!(p.kind(), "field_expression" | "reference_expression" | "index_expression")
                        || (assigns && p.child_by_field_name("left") == Some(c))
                        || (p.kind() == "call_expression" && c.kind() == "field_expression");
                    out.push((c, !place));
                }
            }
        }
        cur = p;
    }
    out.reverse();
    out
}

/// The edits that bind one site, none if it is already bound, or why it cannot be.
fn plan(e: Node, what: &'static str, ty: &str, src: &str, used: &mut HashSet<String>) -> Result<Vec<Edit>, &'static str> {
    if what == "cast" && !carried_type(ty) {
        return Err("the target is not a primitive type");
    }
    let lit_value = |e: Node| -> String {
        let t = text(e, src);
        let (digits, _, suffix) = life::lit_suffix(t.trim_start_matches('-').trim()).unwrap_or((t, false, ""));
        let _ = suffix;
        if t.starts_with('-') { format!("-{digits}") } else { digits.to_string() }
    };
    // The value of a `let`: the local is the name.
    if let Some(p) = e.parent().filter(|p| p.kind() == "let_declaration" && p.child_by_field_name("value") == Some(e)) {
        let pat = p.child_by_field_name("pattern").ok_or("let without a pattern")?;
        let declared = p.child_by_field_name("type");
        let fits = declared.map_or(true, |t| text(t, src) == ty);
        if fits && (declared.is_some() || pat.kind() == "identifier") {
            let mut edits = vec![];
            if declared.is_none() {
                edits.push((pat.end_byte(), pat.end_byte(), format!(": {ty}")));
            }
            if what == "literal" {
                edits.push((e.start_byte(), e.end_byte(), lit_value(e)));
            } else if let Some(op) = e.child_by_field_name("value") {
                // Brackets weave would not put back: `(x) as u16`.
                if op.kind() == "parenthesized_expression" {
                    let inner: Vec<Node> = named_children(op).into_iter().filter(|c| !is_comment(*c)).collect();
                    if inner.len() == 1 && life::binds_tighter_than_as(inner[0]) && named_children(op).len() == 1 {
                        edits.push((op.start_byte(), inner[0].start_byte(), String::new()));
                        edits.push((inner[0].end_byte(), op.end_byte(), String::new()));
                    }
                }
            }
            return Ok(edits);
        }
    }
    let anchor = anchor(e)?;
    let region = match anchor {
        Anchor::Stmt(n) | Anchor::Boundary(n) => n,
    };
    let mut lets = vec![];
    let mut inner: Vec<Edit> = vec![];
    if what == "cast" {
        for (x, by_value) in earlier(e, region) {
            if pure(x) {
                continue;
            }
            if !by_value {
                return Err("something before it in the statement has an effect, and is not a plain value");
            }
            let name = fresh(&format!("{}_v", base_name(x, src)), used);
            lets.push(format!("let {name} = {};", text(x, src)));
            inner.push((x.start_byte(), x.end_byte(), name));
        }
    }
    let (name, value) = if what == "cast" {
        let op = e.child_by_field_name("value").ok_or("cast without a value")?;
        (fresh(&format!("{}_{ty}", base_name(op, src)), used), text(e, src).to_string())
    } else {
        let v = lit_value(e);
        let base = format!("k{}_{ty}", v.replace('-', "neg"));
        (fresh(&base, used), v)
    };
    lets.push(format!("let {name}: {ty} = {value};"));
    // `(h as u64).f()` becomes `h_u64.f()`, not `(h_u64).f()`.
    let target = e.parent().filter(|p| p.kind() == "parenthesized_expression" && named_children(*p).len() == 1).unwrap_or(e);
    inner.push((target.start_byte(), target.end_byte(), name));
    match anchor {
        Anchor::Stmt(s) => {
            let line_start = src[..s.start_byte()].rfind('\n').map_or(0, |i| i + 1);
            let indent: String = src[line_start..s.start_byte()].chars().take_while(|c| c.is_whitespace()).collect();
            let mut edits = vec![(s.start_byte(), s.start_byte(), lets.iter().map(|l| format!("{l}\n{indent}")).collect::<String>())];
            edits.extend(inner);
            Ok(edits)
        }
        Anchor::Boundary(b) => {
            let base = b.start_byte();
            let shifted = inner.into_iter().map(|(s, e, t)| (s - base, e - base, t)).collect();
            let body = apply(text(b, src), shifted);
            Ok(vec![(b.start_byte(), b.end_byte(), format!("{{ {} {body} }}", lets.join(" ")))])
        }
    }
}
