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
//!   and could have an effect (a call, an index, a macro) is bound first, in order. A method receiver that is a value
//!   is bound whole; a place (a receiver such as `self.rows[i]`, a borrow, an indexed base) stays where it is, and
//!   what it computes on the way is bound: `self.rows[next()].push(n as u32)` names `next()`. The right of an
//!   assignment runs before its left. Where the order depends on types (the two sides of `m[k()] += x as u64`) and
//!   the left has an effect, the binding goes into a block in the right's place, which runs where the right did; a
//!   cast in the left, where the right has an effect, is left as it is and reported.
//! - **Already named.** `let n = e as f64;` gets a type, `let n: f64 = e as f64;`, rather than a second name.
//! - **Closures.** A closure with typed parameters is bound to a name the same way (its creation has no effect, so
//!   nothing earlier is bound for it), so that its parameters are keyed `<name>.<parameter>`. A closure's return type
//!   goes onto a local in its body: `|s| -> usize { b }` becomes `|s| { let out: usize = { b }; out }`.
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

/// What releases something at its end (language 7.3, `lifetime self released at end`): the names of such types, and
/// the names of the calls whose result holds one. From the standard library's file and from the source itself.
#[derive(Default)]
pub struct Release {
    pub types: HashSet<String>,
    /// Methods (called as `x.lock()`) by name, and calls by path (`File::open`, or a free function's name).
    pub calls: HashSet<String>,
    pub paths: HashSet<String>,
    /// The source's own types with an `impl Drop`.
    pub drops: HashSet<String>,
}

fn last(path: &str) -> &str {
    let p = path.split('<').next().unwrap_or(path);
    p.rsplit("::").next().unwrap_or(p).trim()
}

/// Words of a type text, to see whether it mentions a type by name.
fn mentions(ty: &str, names: &HashSet<String>) -> bool {
    idents(ty).iter().any(|w| names.contains(w))
}

impl Release {
    /// From a constraint file: its types with the clause, and its functions whose result mentions one.
    pub fn add_file(&mut self, file: &serde_json::Value) {
        let subs = file["subjects"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        for s in subs.iter().filter(|s| s["kind"] == "type") {
            let releases = s["clauses"].as_array().into_iter().flatten().any(|c| c["text"] == "lifetime self released at end");
            if releases {
                self.types.insert(last(s["name"].as_str().unwrap_or("")).to_string());
            }
        }
        for s in subs.iter().filter(|s| s["kind"] == "function") {
            if s["result"]["type"].as_str().is_some_and(|t| mentions(t, &self.types)) {
                let name = s["name"].as_str().unwrap_or("");
                let segs: Vec<&str> = name.split("::").collect();
                let n = segs.len();
                if s["params"][0]["name"] == "self" {
                    self.calls.insert(segs[n - 1].to_string());
                }
                self.paths.insert(if n >= 2 { format!("{}::{}", segs[n - 2], segs[n - 1]) } else { name.to_string() });
            }
        }
    }

    /// The standard library's, from the library file built into the tool.
    pub fn std() -> &'static Release {
        static R: std::sync::OnceLock<Release> = std::sync::OnceLock::new();
        R.get_or_init(|| {
            let mut r = Release::default();
            if let Ok(v) = serde_json::from_str(include_str!("../libraries/std-1.97.0.constraints.json")) {
                r.add_file(&v);
            }
            r
        })
    }

    /// The standard library's, and the source's own: a type with an `impl Drop`, or with a field of a releasing type,
    /// and a function whose result type mentions one.
    pub fn of_source(root: Node, src: &str) -> Release {
        let std = Release::std();
        let mut r = Release { types: std.types.clone(), calls: std.calls.clone(), paths: std.paths.clone(), drops: HashSet::new() };
        let mut items = vec![];
        fn walk<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
            if matches!(n.kind(), "impl_item" | "struct_item" | "enum_item" | "function_item" | "function_signature_item" | "type_item") {
                out.push(n);
            }
            for c in named_children(n) {
                walk(c, out);
            }
        }
        walk(root, &mut items);
        // A type the source defines shadows a standard one of the same name (`Ref` here is not `std::cell::Ref`).
        for i in items.iter().filter(|i| matches!(i.kind(), "struct_item" | "enum_item" | "type_item")) {
            if let Some(n) = i.child_by_field_name("name") {
                r.types.remove(text(n, src));
            }
        }
        for i in items.iter().filter(|i| i.kind() == "impl_item") {
            let tr = i.child_by_field_name("trait").map(|t| text(t, src));
            if tr.is_some_and(|t| last(t) == "Drop") {
                if let Some(t) = i.child_by_field_name("type") {
                    r.types.insert(last(text(t, src)).to_string());
                    r.drops.insert(last(text(t, src)).to_string());
                }
            }
        }
        loop {
            let before = r.types.len();
            for i in items.iter().filter(|i| matches!(i.kind(), "struct_item" | "enum_item")) {
                let name = i.child_by_field_name("name").map(|n| text(n, src).to_string()).unwrap_or_default();
                // The types of its fields, not their names or the names of its variants.
                fn field_types<'a>(n: Node<'a>, src: &'a str, out: &mut Vec<&'a str>) {
                    if matches!(n.kind(), "field_declaration" | "ordered_field_declaration_list") {
                        let mut c = n.walk();
                        for (i, ch) in n.children(&mut c).enumerate() {
                            if n.field_name_for_child(i as u32) == Some("type") {
                                out.push(text(ch, src));
                            }
                        }
                    }
                    for c in named_children(n) {
                        field_types(c, src, out);
                    }
                }
                let mut tys = vec![];
                if let Some(b) = i.child_by_field_name("body") {
                    field_types(b, src, &mut tys);
                }
                if !r.types.contains(&name) && tys.iter().any(|t| mentions(t, &r.types)) {
                    r.types.insert(name);
                }
            }
            if r.types.len() == before {
                break;
            }
        }
        for f in items.iter().filter(|i| matches!(i.kind(), "function_item" | "function_signature_item")) {
            if let (Some(n), Some(t)) = (f.child_by_field_name("name"), f.child_by_field_name("return_type")) {
                if mentions(text(t, src), &r.types) {
                    let name = text(n, src).to_string();
                    let method = f.child_by_field_name("parameters").is_some_and(|ps| named_children(ps).iter().any(|p| p.kind() == "self_parameter"));
                    // A function of an impl is called by its type's path; a free one by its name.
                    let owner = {
                        let mut a = f.parent();
                        while let Some(x) = a.filter(|x| x.kind() != "impl_item") {
                            a = x.parent();
                        }
                        a.and_then(|i| i.child_by_field_name("type")).map(|t| last(text(t, src)).to_string())
                    };
                    if method {
                        r.calls.insert(name.clone());
                    }
                    r.paths.insert(match owner {
                        Some(o) => format!("{o}::{name}"),
                        None => name,
                    });
                }
            }
        }
        r
    }
}

/// A cast target that a local can carry: any type written out, `_` aside. A primitive, a pointer (`ptr<target type=T>`, with its
/// access), a type alias, a `fn` pointer or a reference (`&x as &dyn Display`) alike: the local's type is the target.
pub fn carried_type(t: &str) -> bool {
    !t.trim().is_empty() && t != "_"
}

fn is_pointer(t: &str) -> bool {
    t.strip_prefix('*').is_some_and(|r| {
        let r = r.trim_start();
        ["const", "mut"].iter().any(|k| r.strip_prefix(k).is_some_and(|x| x.starts_with(char::is_whitespace)))
    })
}

/// The word a cast's name ends with: the primitive (`h_u64`), `ptr` for a pointer (`p_ptr`), `ref` for a reference,
/// `fn` for a function pointer, or the last segment of a path (`x as Word` is `x_word`).
fn type_word(t: &str) -> &str {
    let t = t.trim();
    if is_pointer(t) {
        "ptr"
    } else if t.starts_with('&') {
        "ref"
    } else if t.starts_with("fn") || t.starts_with("unsafe ") || t.starts_with("extern ") {
        "fn"
    } else {
        let head = t.split('<').next().unwrap_or(t);
        head.rsplit("::").next().unwrap_or(head).trim()
    }
}

/// The source with every cast and suffixed literal that can be bound, bound.
pub fn bind(src: &str) -> R<String> {
    bind_report(src).map(|r| r.0)
}

/// As `bind`, with the sites it left.
pub fn bind_report(src: &str) -> R<(String, Vec<Left>)> {
    if let Some(e) = first_error(&parse_rust(src)) {
        return Err(format!("bind: parse-error at {e}"));
    }
    // The standard macros' arguments are bound as a call's are (`macros`).
    let mut s = crate::macros::encode(src);
    let (mut fi, mut left, mut steps) = (0, vec![], 0);
    loop {
        let tree = parse_rust(&s);
        if let Some(e) = first_error(&tree) {
            return Err(format!("bind: parse-error at {e}"));
        }
        let mut fns = vec![];
        functions(tree.root_node(), &mut fns);
        let Some(&body) = fns.get(fi) else { break };
        let rel = Release::of_source(tree.root_node(), &s);
        match step(body, &s, &rel) {
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
    Ok((crate::macros::decode(&s), left))
}

/// Function bodies and the values of `const` and `static` items, outer before inner.
fn functions<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
    let field = match n.kind() {
        "function_item" => Some("body"),
        "const_item" | "static_item" => Some("value"),
        _ => None,
    };
    if let Some(b) = field.and_then(|f| n.child_by_field_name(f)) {
        out.push(b);
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
fn step(body: Node, src: &str, rel: &Release) -> Step {
    let mut sites: Vec<(Node, &'static str, String)> = vec![];
    for s in life::sites(body) {
        match s.kind {
            // `as _` states no type.
            SK::Cast if text(s.node, src) == "_" => {}
            SK::Cast => {
                if let Some(e) = s.node.parent() {
                    sites.push((e, "cast", text(s.node, src).to_string()));
                }
            }
            SK::Lit => {
                if let Some((_, _, suffix)) = life::lit_suffix(text(s.node, src)) {
                    let neg = s.node.parent().filter(|p| matches!(p.kind(), "unary_expression" | "negative_literal") && text(*p, src).starts_with('-'));
                    sites.push((neg.unwrap_or(s.node), "literal", suffix.to_string()));
                }
            }
            _ => {}
        }
    }
    // Closures: a return type moves onto a local inside the body, and a closure with typed parameters gets a name, so
    // that its parameters are keyed by it (`by_len.a`).
    fn closures<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
        if crate::is_item_kind(n.kind()) || n.kind() == "token_tree" {
            return;
        }
        if n.kind() == "closure_expression" {
            out.push(n);
        }
        for c in named_children(n) {
            closures(c, out);
        }
    }
    let mut cls = vec![];
    for c in named_children(body) {
        closures(c, &mut cls);
    }
    for c in cls {
        if c.child_by_field_name("return_type").is_some() {
            sites.push((c, "closure_return", String::new()));
        }
        let typed = c.child_by_field_name("parameters").is_some_and(|ps| {
            named_children(ps).into_iter().any(|p| p.kind() == "parameter" && p.child_by_field_name("type").is_some())
        });
        if typed {
            sites.push((c, "closure", String::new()));
        }
    }
    // A macro argument that states a type in a turbofish (`format!("{}", v.iter().sum::<u64>())`): the argument is
    // named before the macro, where `strip` names the realization.
    fn turbofish<'a>(n: Node<'a>, src: &str, out: &mut Vec<Node<'a>>) {
        if crate::is_item_kind(n.kind()) || n.kind() == "token_tree" {
            return;
        }
        if n.kind() == "type_arguments" && src[..n.start_byte()].trim_end().ends_with("::") {
            let typed = named_children(n).into_iter().any(|a| !is_comment(a) && text(a, src) != "_");
            if typed {
                let mut cur = n;
                loop {
                    if crate::macros::macro_arg(cur, src) {
                        if !out.iter().any(|o| o.id() == cur.id()) {
                            out.push(cur);
                        }
                        break;
                    }
                    match cur.parent() {
                        Some(p) if !matches!(p.kind(), "block" | "function_item") => cur = p,
                        _ => break,
                    }
                }
            }
        }
        for c in named_children(n) {
            turbofish(c, src, out);
        }
    }
    let mut tf = vec![];
    for c in named_children(body) {
        turbofish(c, src, &mut tf);
    }
    for a in tf {
        sites.push((a, "macro_arg", String::new()));
    }
    // Innermost first, then in order: the order the values are computed in. The left of an assignment is computed
    // after its right, and the pattern of an `if let` is matched after its value is.
    let after_right = |e: Node| {
        let (mut end, mut left, mut cur) = (e.end_byte(), false, e);
        while let Some(p) = cur.parent() {
            let assignee = matches!(p.kind(), "assignment_expression" | "compound_assignment_expr") && p.child_by_field_name("left") == Some(cur);
            let pattern = matches!(p.kind(), "let_condition" | "let_declaration") && p.child_by_field_name("pattern") == Some(cur);
            if assignee || pattern {
                (end, left) = (p.end_byte(), true);
            }
            cur = p;
        }
        (end, left)
    };
    sites.sort_by_key(|(e, _, _)| (after_right(*e), e.end_byte(), std::cmp::Reverse(e.start_byte())));
    let mut used = idents(text(body, src));
    let mut left = vec![];
    for (e, what, ty) in sites {
        match plan(e, what, &ty, src, &mut used, rel) {
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

pub(crate) fn idents(s: &str) -> HashSet<String> {
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
        "identifier" => crate::macros::macro_name(text(n, src)).unwrap_or(text(n, src)).to_string(),
        "self" => "this".into(),
        "parenthesized_expression" | "reference_expression" | "unary_expression" | "try_expression" | "await_expression" => {
            named_children(n).into_iter().find(|c| !is_comment(*c)).map_or("value".into(), |c| base_name(c, src))
        }
        "field_expression" => n.child_by_field_name("field").map_or("value".into(), |f| text(f, src).to_string()),
        "scoped_identifier" => n.child_by_field_name("name").map_or("value".into(), |f| crate::macros::macro_name(text(f, src)).unwrap_or(text(f, src)).to_string()),
        "call_expression" => match n.child_by_field_name("function") {
            // `x.parse().unwrap()` is named for what was unwrapped.
            Some(f) if f.kind() == "field_expression"
                && f.child_by_field_name("field").is_some_and(|m| matches!(text(m, src), "unwrap" | "expect")) =>
            {
                f.child_by_field_name("value").map_or("value".into(), |v| base_name(v, src))
            }
            Some(f) => base_name(f, src),
            None => "value".into(),
        },
        "index_expression" => named_children(n).first().map_or("item".into(), |v| format!("{}_item", base_name(*v, src))),
        "type_cast_expression" => n.child_by_field_name("value").map_or("value".into(), |v| base_name(v, src)),
        "generic_function" => n.child_by_field_name("function").map_or("value".into(), |f| base_name(f, src)),
        _ => "value".into(),
    }
}

/// Free of effects, as far as the order of evaluation goes: reading places and computing on what was read. With the
/// source, a JSON value of `json!` is as pure as its parts, and an argument a macro runs only sometimes is never bound
/// before it.
fn pure_in(n: Node, src: Option<&str>) -> bool {
    if let Some(src) = src {
        if crate::macros::is_json_part(n, src) {
            return n.child_by_field_name("arguments").is_some_and(|a| named_children(a).into_iter().all(|c| is_comment(c) || pure_in(c, Some(src))));
        }
        if crate::macros::is_cond(n, src) {
            return true;
        }
    }
    match n.kind() {
        "identifier" | "self" | "scoped_identifier" | "field_identifier" | "integer_literal" | "float_literal"
        | "boolean_literal" | "char_literal" | "string_literal" | "raw_string_literal" | "unit_expression" | "crate"
        | "super" | "closure_expression" | "shorthand_field_initializer" => true,
        "field_expression" => n.child_by_field_name("value").is_some_and(|v| pure_in(v, src)),
        "reference_expression" | "unary_expression" | "parenthesized_expression" | "type_cast_expression"
        | "binary_expression" | "tuple_expression" | "array_expression" | "range_expression" | "negative_literal" => {
            named_children(n).into_iter().filter(|c| !is_comment(*c)).all(|c| pure_in(c, src) || is_type_node(c))
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
fn anchor<'a>(e: Node<'a>, src: &str) -> Result<Anchor<'a>, &'static str> {
    let mut cur = e;
    loop {
        let Some(p) = cur.parent() else { return Err("not in a function body") };
        let is = |f: &str| p.child_by_field_name(f) == Some(cur);
        match p.kind() {
            "block" => return Ok(Anchor::Stmt(cur)),
            "match_pattern" if is("condition") => return Ok(Anchor::Boundary(cur)),
            "match_pattern" | "let_declaration" | "let_condition" | "for_expression" if is("pattern") => return Err("in a pattern"),
            "match_pattern" => return Err("in a pattern"),
            k if k.ends_with("_pattern") => return Err("in a pattern"),
            k if k.ends_with("_type") || matches!(k, "type_arguments" | "attribute_item" | "token_tree" | "macro_invocation") => {
                return Err("in a type or macro")
            }
            // A constant context runs once, when the program is built: the binding goes into a block in its place,
            // which every constant context allows (`[0; { let n_usize: usize = N as usize; n_usize }]`).
            "array_expression" if is("length") => return Ok(Anchor::Boundary(cur)),
            "const_item" | "static_item" if is("value") => return Ok(Anchor::Boundary(cur)),
            "const_item" | "static_item" | "function_item" | "const_block" => return Err("in a constant context"),
            // `m[k()] += x as u64`: whether the right runs before the left depends on the types (both primitives: the
            // right first). Where the left has an effect, the binding goes into a block in the right's place, which
            // runs where the right did either way.
            // A cast in the left, where the right has an effect: a block in the place of the part of the left it is in
            // (an index, what is dereferenced), which runs where that part did: `v[{ let i_usize: usize = i as
            // usize; i_usize }] += next()`.
            "compound_assignment_expr" if is("left") => {
                if p.child_by_field_name("right").is_some_and(|r| !pure_in(r, Some(src))) {
                    let mut ps = vec![];
                    place_parts(cur, src, &mut ps);
                    return match ps.into_iter().find(|x| x.start_byte() <= e.start_byte() && e.end_byte() <= x.end_byte()) {
                        Some(part) => Ok(Anchor::Boundary(part)),
                        None => Err("in the left of a compound assignment whose right has an effect"),
                    };
                }
            }
            "compound_assignment_expr" if is("right") => {
                let mut ps = vec![];
                if let Some(l) = p.child_by_field_name("left") {
                    place_parts(l, src, &mut ps);
                }
                if ps.iter().any(|x| !pure_in(*x, Some(src))) {
                    return Ok(Anchor::Boundary(cur));
                }
            }
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
            // A `while let` runs its value each time round; a let chain runs each part only if the parts before it
            // matched, and a part may use the names they bind: the binding goes into a block in the part's place.
            "let_condition" if is("value") => {
                if p.parent().is_some_and(|g| matches!(g.kind(), "while_expression" | "let_chain")) {
                    return Ok(Anchor::Boundary(cur));
                }
            }
            "let_chain" => return Ok(Anchor::Boundary(cur)),
            "closure_expression" if is("body") => return Ok(Anchor::Boundary(cur)),
            _ => {}
        }
        cur = p;
    }
}

/// What runs before a value, as `bind` sees it.
enum Before<'a> {
    /// A value that runs before it: bound first, whole, if it has an effect.
    Value(Node<'a>),
    /// A value that may run before it or after it, by types `bind` does not know (the other side of a compound
    /// assignment, `a[i] += x as u32`): it must have no effect.
    Either(Node<'a>),
}

/// A place as Rust writes one: a name, a field, an element, or what a reference points to.
fn is_place(n: Node, src: &str) -> bool {
    match n.kind() {
        "identifier" | "self" | "scoped_identifier" | "field_expression" | "index_expression" => true,
        "unary_expression" => text(n, src).starts_with('*'),
        "parenthesized_expression" => named_children(n).into_iter().filter(|c| !is_comment(*c)).collect::<Vec<_>>().first().is_some_and(|c| is_place(*c, src)),
        _ => false,
    }
}

/// The values a place computes on its way, in order: an element's index, what a reference points to, a temporary
/// it is a part of. A place itself is kept where it is (binding it would move out of it, or borrow it longer); these
/// are bound in its stead: `self.rows[next()].push(x)` keeps `self.rows[..]` and names `next()`.
fn place_parts<'a>(n: Node<'a>, src: &str, out: &mut Vec<Node<'a>>) {
    let kids: Vec<Node> = named_children(n).into_iter().filter(|c| !is_comment(*c)).collect();
    match n.kind() {
        "identifier" | "self" | "scoped_identifier" => {}
        "parenthesized_expression" if kids.len() == 1 => place_parts(kids[0], src, out),
        "field_expression" => {
            if let Some(v) = n.child_by_field_name("value") {
                place_parts(v, src, out);
            }
        }
        "index_expression" if kids.len() == 2 => {
            place_parts(kids[0], src, out);
            out.push(kids[1]);
        }
        "unary_expression" if text(n, src).starts_with('*') && kids.len() == 1 => out.push(kids[0]),
        // A borrow of a place keeps the place: `f(&mut cells[pick()], ..)` names `pick()`.
        "reference_expression" => {
            if let Some(v) = n.child_by_field_name("value") {
                if is_place(v, src) {
                    place_parts(v, src, out);
                } else {
                    out.push(v);
                }
            }
        }
        // A value where a place is wanted is a temporary: it is the value that runs.
        _ => out.push(n),
    }
}

/// What runs before `e` in `region`, outermost first.
fn earlier<'a>(e: Node<'a>, region: Node<'a>, src: &str) -> Vec<Before<'a>> {
    // A JSON value of `json!` runs as its parts, in order.
    fn parts<'a>(n: Node<'a>, src: &str, out: &mut Vec<Node<'a>>) {
        match n.child_by_field_name("arguments").filter(|_| crate::macros::is_json_part(n, src)) {
            Some(a) => named_children(a).into_iter().filter(|c| !is_comment(*c)).for_each(|c| parts(c, src, out)),
            None => out.push(n),
        }
    }
    // One group per level, innermost level first; the outermost level runs first, and each level runs in order.
    let mut levels: Vec<Vec<Before<'a>>> = vec![];
    let mut cur = e;
    while cur.id() != region.id() {
        let Some(p) = cur.parent() else { break };
        let mut out = vec![];
        match p.kind() {
            // `place = value` computes the value first, then the place (the Rust reference, 8.2.14).
            "assignment_expression" => {
                if p.child_by_field_name("left") == Some(cur) {
                    if let Some(r) = p.child_by_field_name("right") {
                        out.push(Before::Value(r));
                    }
                }
            }
            // `place op= value` computes the value first when both are primitives, and the place first otherwise.
            "compound_assignment_expr" => {
                let other = if p.child_by_field_name("left") == Some(cur) { p.child_by_field_name("right") } else { p.child_by_field_name("left") };
                if let Some(o) = other {
                    if p.child_by_field_name("right") == Some(cur) {
                        let mut ps = vec![];
                        place_parts(o, src, &mut ps);
                        out.extend(ps.into_iter().map(Before::Either));
                    } else {
                        out.push(Before::Either(o));
                    }
                }
            }
            _ => {
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
                        // A named argument of a format macro (`n = e`): what runs is its value.
                        "assignment_expression" if crate::macros::macro_arg_list(p, src) => match c.child_by_field_name("right") {
                            Some(v) => v,
                            None => continue,
                        },
                        _ => c,
                    };
                    let skip = is_comment(c)
                        || is_type_node(c)
                        || matches!(c.kind(), "field_identifier" | "label" | "mutable_specifier" | "type_arguments")
                        || ["pattern", "type", "name", "field"].iter().any(|f| p.child_by_field_name(f) == Some(c));
                    if skip {
                        continue;
                    }
                    let mut ps = vec![];
                    match (p.kind(), c.kind()) {
                        // A method call runs its receiver first: a value is bound whole, a place keeps its place.
                        ("call_expression", "field_expression" | "generic_function") => {
                            let callee = if c.kind() == "generic_function" { c.child_by_field_name("function") } else { Some(c) };
                            match callee.filter(|f| f.kind() == "field_expression").and_then(|f| f.child_by_field_name("value")) {
                                Some(recv) if is_place(recv, src) => place_parts(recv, src, &mut ps),
                                Some(recv) => ps.push(recv),
                                None => ps.push(c),
                            }
                        }
                        // `a[i as usize]`: the base is a place.
                        ("index_expression", _) => place_parts(c, src, &mut ps),
                        // `f(&mut cells[pick()], x as u32)`: a borrow keeps its place.
                        (_, "reference_expression") => place_parts(c, src, &mut ps),
                        _ => parts(c, src, &mut ps),
                    }
                    out.extend(ps.into_iter().map(Before::Value));
                }
            }
        }
        levels.push(out);
        cur = p;
    }
    levels.into_iter().rev().flatten().collect()
}

/// The argument of a standard macro (`macros`) that holds `n`, the nearest, if any.
fn in_macro_arg<'a>(n: Node<'a>, src: &str) -> Option<Node<'a>> {
    let mut cur = Some(n);
    while let Some(c) = cur {
        if crate::macros::macro_arg(c, src) {
            return Some(c);
        }
        if c.kind() == "function_item" {
            return None;
        }
        cur = c.parent();
    }
    None
}

/// A binding never goes inside a macro's arguments, where `strip` cannot take its type out: the argument is named
/// first, before the macro, and what is inside it is bound there. An argument the macro runs only sometimes is left.
fn name_macro_arg_first(a: Node, src: &str, used: &mut HashSet<String>, rel: &Release) -> Result<Vec<Edit>, &'static str> {
    if crate::macros::is_cond_arg(a, src) {
        return Err("in a macro argument that runs only sometimes (an assertion's message, `debug_assert!`)");
    }
    plan(a, "macro_arg", "", src, used, rel)
}

/// The edits that bind one site, none if it is already bound, or why it cannot be.
fn plan(e: Node, what: &'static str, ty: &str, src: &str, used: &mut HashSet<String>, rel: &Release) -> Result<Vec<Edit>, &'static str> {
    match what {
        "closure_return" | "closure" => {
            if let Some(a) = in_macro_arg(e, src) {
                return name_macro_arg_first(a, src, used, rel);
            }
            return if what == "closure" { plan_closure_name(e, src, used) } else { plan_closure_return(e, src, used) };
        }
        _ => {}
    }
    let lit_value = |e: Node| -> String {
        let t = text(e, src);
        let (digits, _, suffix) = life::lit_suffix(t.trim_start_matches('-').trim()).unwrap_or((t, false, ""));
        let _ = suffix;
        if t.starts_with('-') { format!("-{digits}") } else { digits.to_string() }
    };
    // The value of a `let`: the local is the name. The whole value of a `const` or `static`: the item is.
    let named = |p: &Node| matches!(p.kind(), "let_declaration" | "const_item" | "static_item") && p.child_by_field_name("value") == Some(e);
    if let Some(p) = e.parent().filter(named) {
        if let Some(a) = in_macro_arg(p, src) {
            return name_macro_arg_first(a, src, used, rel);
        }
        let pat = p.child_by_field_name("pattern").or_else(|| p.child_by_field_name("name")).ok_or("let without a pattern")?;
        let declared = p.child_by_field_name("type");
        let fits = declared.is_none_or(|t| text(t, src) == ty);
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
    let anchor = match anchor(e, src) {
        Err("in a pattern") if what == "literal" => return plan_pattern_literal(e, ty, src, used),
        a => a?,
    };
    let region = match anchor {
        Anchor::Stmt(n) | Anchor::Boundary(n) => n,
    };
    if let Some(a) = in_macro_arg(region, src) {
        return name_macro_arg_first(a, src, used, rel);
    }
    let mut lets = vec![];
    let mut inner: Vec<Edit> = vec![];
    // What runs before it and has an effect: each such value is bound first, in order.
    let mut before: Vec<Node> = vec![];
    if what != "literal" {
        for b in earlier(e, region, src) {
            match b {
                Before::Value(x) if !pure_in(x, Some(src)) => before.push(x),
                Before::Either(x) if !pure_in(x, Some(src)) => {
                    return Err("the other side of a compound assignment has an effect, and which side runs first depends on their types")
                }
                _ => {}
            }
        }
        for x in &before {
            let name = fresh(&format!("{}_v", base_name(*x, src)), used);
            lets.push(format!("let {name} = {};", text(unparen(*x), src)));
            inner.push((x.start_byte(), x.end_byte(), name));
        }
    }
    // A value inside the cast that releases something at its end (a lock guard) keeps its end where it was: it is
    // named, and the statement goes into a block that ends where the statement did (language 7.3).
    let held = if what != "literal" { releasing(e, src, rel) } else { vec![] };
    if !held.is_empty() && matches!(anchor, Anchor::Boundary(_)) {
        return Err("a value that releases something at its end, where the statement might not run");
    }
    // A literal is a constant: binding it first changes no order and ends nothing.
    if before.iter().any(|x| !releasing(*x, src, rel).is_empty()) {
        return Err("something before it in the statement holds a value that releases something at its end");
    }
    let mut cast_text = text(e, src).to_string();
    if !held.is_empty() {
        let mut ed = vec![];
        for g in &held {
            let gname = fresh(&format!("{}_guard", base_name(*g, src)), used);
            lets.push(format!("let {gname} = {};", text(*g, src)));
            ed.push((g.start_byte() - e.start_byte(), g.end_byte() - e.start_byte(), gname));
        }
        cast_text = apply(&cast_text, ed);
    }
    let (name, value) = if what == "macro_arg" {
        (fresh(&format!("{}_v", base_name(e, src)), used), cast_text)
    } else if what == "cast" {
        let op = e.child_by_field_name("value").ok_or("cast without a value")?;
        (fresh(&format!("{}_{}", base_name(op, src), type_word(ty)), used), cast_text)
    } else {
        let v = lit_value(e);
        let base = format!("k{}_{ty}", v.replace('-', "neg"));
        (fresh(&base, used), v)
    };
    lets.push(if ty.is_empty() { format!("let {name} = {value};") } else { format!("let {name}: {ty} = {value};") });
    // `(h as u64).f()` becomes `h_u64.f()`, not `(h_u64).f()`.
    let target = e.parent().filter(|p| p.kind() == "parenthesized_expression" && named_children(*p).len() == 1).unwrap_or(e);
    inner.push((target.start_byte(), target.end_byte(), name));
    match anchor {
        Anchor::Stmt(s) if !held.is_empty() => {
            // The block ends where the statement ended: a `let` keeps its name outside, around its value.
            let (outer, wrap) = match s.kind() {
                "let_declaration" => (s, s.child_by_field_name("value").ok_or("let without a value")?),
                _ => (s, s),
            };
            let _ = outer;
            let base = wrap.start_byte();
            let shifted = inner.into_iter().map(|(a, b, t)| (a - base, b - base, t)).collect();
            let body = apply(text(wrap, src), shifted);
            Ok(vec![(wrap.start_byte(), wrap.end_byte(), format!("{{ {} {body} }}", lets.join(" ")))])
        }
        Anchor::Stmt(s) => {
            let sep = line_sep(s, src);
            let mut edits = vec![(s.start_byte(), s.start_byte(), lets.iter().map(|l| format!("{l}{sep}")).collect::<String>())];
            edits.extend(inner);
            Ok(edits)
        }
        Anchor::Boundary(b) => {
            // `|| (a > n as usize)`: the block takes the brackets' place, `|| { let ..; a > n_usize }`.
            let c = unparen(b);
            let c = if inner.iter().all(|(s, e, _)| c.start_byte() <= *s && *e <= c.end_byte()) { c } else { b };
            let base = c.start_byte();
            let shifted = inner.into_iter().map(|(s, e, t)| (s - base, e - base, t)).collect();
            let body = apply(text(c, src), shifted);
            Ok(vec![(b.start_byte(), b.end_byte(), format!("{{ {} {body} }}", lets.join(" ")))])
        }
    }
}

/// What is inside brackets that hold one expression: a `let`'s value and a block's tail need none.
fn unparen(n: Node) -> Node {
    if n.kind() == "parenthesized_expression" {
        let inner: Vec<Node> = named_children(n).into_iter().collect();
        if let [i] = inner.as_slice() {
            if !is_comment(*i) {
                return unparen(*i);
            }
        }
    }
    n
}

/// What goes between a binding and the statement after it: a new line at the statement's indent, or a space where the
/// statement does not start its line (`{ let a: u8 = 1; a }`).
fn line_sep(stmt: Node, src: &str) -> String {
    let line_start = src[..stmt.start_byte()].rfind('\n').map_or(0, |i| i + 1);
    let before = &src[line_start..stmt.start_byte()];
    if before.chars().all(char::is_whitespace) {
        format!("\n{before}")
    } else {
        " ".into()
    }
}

/// A suffixed literal in a pattern (`1u8 => ..`), where no `let` can carry its type. In order of preference:
///
/// - Where the value it is matched against is a name whose declared type is the literal's (a parameter `x: u8`, a
///   typed `let`; through `&`, `*` and the elements of a tuple, `match (a, b) { (1u8, _) => .. }`), the suffix says
///   nothing the declaration does not, and goes.
/// - A literal already under a name (`n @ 1u8..=9u8`) is carried by that name; the literals under it all take the
///   suffix, which `strip` keys by the name's pattern.
/// - Otherwise the literal, with the range or the alternatives it is part of, gets a name that does nothing else:
///   `_k1_u8 @ 1u8` (the `_` says that nothing uses it).
fn plan_pattern_literal(e: Node, ty: &str, src: &str, used: &mut HashSet<String>) -> Result<Vec<Edit>, &'static str> {
    let lit = if e.kind() == "negative_literal" { named_children(e).into_iter().find(|c| !is_comment(*c)).unwrap_or(e) } else { e };
    if matched_type(e, src).is_some_and(|t| t == ty) {
        let digits = life::lit_suffix(text(lit, src)).map_or(0, |(d, _, _)| d.len());
        return Ok(vec![(lit.start_byte() + digits, lit.end_byte(), String::new())]);
    }
    let mut target = e;
    if let Some(p) = target.parent().filter(|p| p.kind() == "range_pattern") {
        target = p;
    }
    while let Some(p) = target.parent().filter(|p| p.kind() == "or_pattern") {
        target = p;
    }
    let mut up = target;
    while let Some(p) = up.parent().filter(|p| is_parens_pattern(*p)) {
        up = p;
    }
    // Every literal under the name takes the suffix, so that the name's one type is each one's.
    let suffixed = |n: Node| -> Result<(Vec<Edit>, Vec<String>), &'static str> {
        let (mut edits, mut values) = (vec![], vec![]);
        for l in pattern_literals(n) {
            let t = text(l, src);
            let (digits, sep, suffix) = life::lit_suffix(t).unwrap_or((t, false, ty));
            if suffix != ty {
                return Err("in a pattern, with literals of different types under one name");
            }
            if sep || digits.len() == t.len() {
                edits.push((l.start_byte(), l.end_byte(), format!("{digits}{ty}")));
            }
            let neg = l.parent().is_some_and(|p| p.kind() == "negative_literal");
            values.push(format!("{}{}", if neg { "neg" } else { "" }, digits.replace('.', "_")));
        }
        Ok((edits, values))
    };
    if let Some(at) = up.parent().filter(|p| p.kind() == "captured_pattern") {
        return Ok(suffixed(at)?.0);
    }
    let (edits, values) = suffixed(target)?;
    let base = target.start_byte();
    let shifted = edits.into_iter().map(|(a, b, t)| (a - base, b - base, t)).collect();
    let pat = apply(text(target, src), shifted);
    let pat = if target.kind() == "or_pattern" { format!("({pat})") } else { pat };
    let name = fresh(&format!("_k{}_{ty}", values.join("_")), used);
    Ok(vec![(target.start_byte(), target.end_byte(), format!("{name} @ {pat}"))])
}

/// The literals under a pattern.
fn pattern_literals(n: Node) -> Vec<Node> {
    let mut out = vec![];
    fn walk<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
        if matches!(n.kind(), "integer_literal" | "float_literal") {
            out.push(n);
        }
        for c in named_children(n) {
            walk(c, out);
        }
    }
    walk(n, &mut out);
    out
}

/// `(p)`: brackets around a pattern, which tree-sitter reads as a tuple pattern without a comma.
fn is_parens_pattern(p: Node) -> bool {
    p.kind() == "tuple_pattern"
        && named_children(p).into_iter().filter(|c| !is_comment(*c)).count() == 1
        && !crate::all_children(p).iter().any(|c| matches!(c.kind(), "," | "_"))
}

/// The declared type of what a pattern literal is matched against, where the pattern's structure reaches a name
/// through tuples and references: the `u8` of `x` in `fn f(x: &u8) { match (x, y) { (1u8, _) => .. } }`.
fn matched_type(e: Node, src: &str) -> Option<String> {
    let mut path = vec![];
    let mut cur = e;
    let matched = loop {
        let p = cur.parent()?;
        match p.kind() {
            "range_pattern" | "or_pattern" | "reference_pattern" | "captured_pattern" | "negative_literal" => {}
            "tuple_pattern" if is_parens_pattern(p) => {}
            "tuple_pattern" => {
                let elems: Vec<Node> =
                    crate::all_children(p).into_iter().filter(|c| !is_comment(*c) && !matches!(c.kind(), "(" | ")" | ",")).collect();
                if elems.iter().any(|c| c.kind() == "remaining_field_pattern") {
                    return None;
                }
                path.push(elems.iter().position(|c| c.id() == cur.id())?);
            }
            "match_pattern" if p.child_by_field_name("condition") != Some(cur) => {
                break p.parent()?.parent()?.parent()?.child_by_field_name("value")?;
            }
            "let_condition" | "let_declaration" if p.child_by_field_name("pattern") == Some(cur) => break p.child_by_field_name("value")?,
            _ => return None,
        }
        cur = p;
    };
    fn peel<'a>(mut n: Node<'a>, src: &str) -> Node<'a> {
        loop {
            let inner = match n.kind() {
                "parenthesized_expression" | "reference_expression" => named_children(n).into_iter().rfind(|c| !is_comment(*c)),
                "unary_expression" if text(n, src).starts_with('*') => named_children(n).into_iter().find(|c| !is_comment(*c)),
                _ => None,
            };
            match inner {
                Some(i) => n = i,
                None => return n,
            }
        }
    }
    let mut m = matched;
    for i in path.into_iter().rev() {
        m = peel(m, src);
        if m.kind() != "tuple_expression" {
            return None;
        }
        m = *named_children(m).into_iter().filter(|c| !is_comment(*c)).collect::<Vec<_>>().get(i)?;
    }
    let m = peel(m, src);
    (m.kind() == "identifier").then(|| declared_type(text(m, src), m, src)).flatten()
}

/// The type a name is declared with in its function, when the function binds it once, as a parameter or a typed
/// `let`, with the references taken off: `&'a mut u8` is `u8`. Any other binding of the name makes it unknown.
fn declared_type(name: &str, at: Node, src: &str) -> Option<String> {
    let mut f = at;
    while f.kind() != "function_item" {
        f = f.parent()?;
    }
    let mut found: Vec<Option<String>> = vec![];
    // Every name a pattern binds; an identifier that is a path's or a struct's is none.
    fn binds(p: Node, name: &str, src: &str, found: &mut Vec<Option<String>>) {
        let is_head = p.parent().is_some_and(|q| matches!(q.kind(), "tuple_struct_pattern" | "struct_pattern") && q.child_by_field_name("type") == Some(p));
        if matches!(p.kind(), "identifier" | "shorthand_field_identifier") && text(p, src) == name && !is_head {
            found.push(None);
        }
        if p.kind() == "scoped_identifier" {
            return;
        }
        for c in named_children(p) {
            binds(c, name, src, found);
        }
    }
    fn walk(n: Node, name: &str, src: &str, found: &mut Vec<Option<String>>) {
        if crate::is_item_kind(n.kind()) {
            return;
        }
        let typed = |ty: Option<Node>| ty.map(|t| text(t, src).to_string());
        match n.kind() {
            "parameter" | "let_declaration" => {
                if let Some(p) = n.child_by_field_name("pattern") {
                    if p.kind() == "identifier" && text(p, src) == name {
                        found.push(typed(n.child_by_field_name("type")));
                    } else {
                        binds(p, name, src, found);
                    }
                }
            }
            "closure_parameters" | "for_expression" | "let_condition" | "match_pattern" => {
                let pats: Vec<Node> = match n.kind() {
                    "closure_parameters" => named_children(n),
                    "match_pattern" => named_children(n).into_iter().filter(|c| n.child_by_field_name("condition") != Some(*c)).collect(),
                    _ => n.child_by_field_name("pattern").into_iter().collect(),
                };
                for p in pats {
                    binds(p, name, src, found);
                }
            }
            _ => {}
        }
        for c in named_children(n) {
            if !matches!(n.kind(), "parameter" | "let_declaration") || Some(c) != n.child_by_field_name("pattern") {
                walk(c, name, src, found);
            }
        }
    }
    if let Some(ps) = f.child_by_field_name("parameters") {
        walk(ps, name, src, &mut found);
    }
    if let Some(b) = f.child_by_field_name("body") {
        walk(b, name, src, &mut found);
    }
    let [Some(t)] = found.as_slice() else { return None };
    let mut t = t.trim();
    while let Some(r) = t.strip_prefix('&') {
        let r = r.trim_start();
        let r = if r.starts_with('\'') { r.split_once(char::is_whitespace).map_or("", |x| x.1) } else { r };
        t = r.trim_start().strip_prefix("mut ").unwrap_or(r).trim_start();
    }
    Some(t.to_string())
}

/// `|x| -> u32 { body }` becomes `|x| { let out: u32 = { body }; out }`: the return type is carried by a local.
fn plan_closure_return(c: Node, src: &str, used: &mut HashSet<String>) -> Result<Vec<Edit>, &'static str> {
    let (Some(ps), Some(rt), Some(body)) =
        (c.child_by_field_name("parameters"), c.child_by_field_name("return_type"), c.child_by_field_name("body"))
    else {
        return Err("closure without parameters or body");
    };
    let name = fresh("out", used);
    Ok(vec![
        (ps.end_byte(), rt.end_byte(), String::new()),
        (body.start_byte(), body.end_byte(), format!("{{ let {name}: {} = {}; {name} }}", text(rt, src), text(body, src))),
    ])
}

/// A closure with typed parameters, not yet the value of a `let`, is bound to a name before its statement (or in a
/// new block where its statement would run it conditionally), so that its parameters are keyed by that name.
fn plan_closure_name(c: Node, src: &str, used: &mut HashSet<String>) -> Result<Vec<Edit>, &'static str> {
    if let Some(p) = c.parent().filter(|p| p.kind() == "let_declaration" && p.child_by_field_name("value") == Some(c)) {
        return if p.child_by_field_name("pattern").is_some_and(|x| x.kind() == "identifier") {
            Ok(vec![])
        } else {
            Err("the closure is the value of a `let` whose pattern is not a name")
        };
    }
    let base = match c.parent().filter(|p| p.kind() == "arguments").and_then(|a| a.parent()) {
        Some(call) => call.child_by_field_name("function").map_or("closure".to_string(), |f| format!("{}_fn", base_name(f, src))),
        None => "closure".into(),
    };
    let name = fresh(&base, used);
    let line = format!("let {name} = {};", text(c, src));
    match anchor(c, src)? {
        Anchor::Stmt(s) => {
            Ok(vec![(s.start_byte(), s.start_byte(), format!("{line}{}", line_sep(s, src))), (c.start_byte(), c.end_byte(), name)])
        }
        Anchor::Boundary(b) => {
            let base = b.start_byte();
            let body = apply(text(b, src), vec![(c.start_byte() - base, c.end_byte() - base, name)]);
            Ok(vec![(b.start_byte(), b.end_byte(), format!("{{ {line} {body} }}"))])
        }
    }
}

/// The values inside `e` that hold something released at their end: each call whose result is one, extended through
/// `?`, `.unwrap()` and `.expect(..)` to the value that is then used.
fn releasing<'a>(e: Node<'a>, src: &str, rel: &Release) -> Vec<Node<'a>> {
    fn calls<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
        if n.kind() == "closure_expression" || crate::is_item_kind(n.kind()) {
            return;
        }
        if n.kind() == "call_expression" {
            out.push(n);
        }
        for c in named_children(n) {
            calls(c, out);
        }
    }
    let mut cs = vec![];
    calls(e, &mut cs);
    let mut out: Vec<Node> = vec![];
    for c in cs {
        let f = c.child_by_field_name("function");
        // A method by its name; a call by path by its last two segments (`Box::new` is not `BufWriter::new`).
        let by_path = |t: &str| {
            let segs: Vec<&str> = t.split("::").map(|x| x.split('<').next().unwrap_or(x).trim()).collect();
            let n = segs.len();
            rel.paths.contains(&if n >= 2 { format!("{}::{}", segs[n - 2], segs[n - 1]) } else { segs[0].to_string() })
        };
        let hit = match f.map(|f| f.kind()) {
            Some("field_expression") => f.and_then(|f| f.child_by_field_name("field")).is_some_and(|n| rel.calls.contains(text(n, src))),
            Some("identifier") | Some("scoped_identifier") => f.is_some_and(|f| by_path(text(f, src))),
            Some("generic_function") => f.and_then(|f| f.child_by_field_name("function")).is_some_and(|n| match n.kind() {
                "field_expression" => n.child_by_field_name("field").is_some_and(|m| rel.calls.contains(text(m, src))),
                _ => by_path(text(n, src)),
            }),
            _ => false,
        };
        if !hit {
            continue;
        }
        let mut g = c;
        while let Some(p) = g.parent() {
            if p.kind() == "try_expression" {
                g = p;
                continue;
            }
            let unwraps = p.kind() == "field_expression"
                && p.child_by_field_name("value") == Some(g)
                && p.child_by_field_name("field").is_some_and(|m| matches!(text(m, src), "unwrap" | "expect"))
                && p.parent().is_some_and(|call| call.kind() == "call_expression");
            if unwraps {
                g = p.parent().unwrap();
                continue;
            }
            break;
        }
        // Inner values first is not needed: a value inside another one is named with it.
        if !out.iter().any(|o| o.start_byte() <= g.start_byte() && g.end_byte() <= o.end_byte()) {
            out.retain(|o| !(g.start_byte() <= o.start_byte() && o.end_byte() <= g.end_byte()));
            out.push(g);
        }
    }
    out.sort_by_key(|n| n.start_byte());
    out
}
