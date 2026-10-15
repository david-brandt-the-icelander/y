//! The standard macros whose arguments are expressions, written as calls for a while, so that `bind` treats their
//! arguments as it treats a call's, and written back as macros after.
//!
//! ```text
//! println!("{}", x as u8)            __bm_println_p("{}", x as u8)
//! vec![0u8; n]                       __bm_vec_b_r(0u8, n)
//! assert!(c, "{}", f() as u8)        __bm_assert_p(c, "{}", (|__bmc| f() as u8)(()))
//! json!({ "n": x as u8, "a": [1] })  __bm_json_p(__bj_o("n", x as u8, "a", __bj_a(1)))
//! ```
//!
//! How each runs its arguments decides the form: the format macros, `vec!` and `dbg!` run every argument once, in
//! order, like a call. An assertion's message runs only when it fails, and every argument of `debug_assert!` only in a
//! debug build, so each such argument is the body of a closure, where `bind` puts a new scope rather than binding
//! before the statement. `json!` builds its value in order: an object is a call with its keys and values in turn, an
//! array a call with its elements.
use crate::{all_children, apply, first_error, named_children, parse_rust, text, Edit};
use tree_sitter::Node;

/// A call that stands for a macro: `__bm_<name>_<delimiter>`, and `_r` for `vec![x; n]`.
pub const CALL: &str = "__bm_";
/// A JSON object and array inside `json!`.
const OBJ: &str = "__bj_o";
const ARR: &str = "__bj_a";
/// The parameter of the closure around an argument that runs only sometimes.
const COND: &str = "__bmc";

enum How {
    /// Every argument, once, in order.
    All,
    /// The first `n` arguments always; the others only sometimes (an assertion's message, a debug build).
    Cond(usize),
    Json,
}

fn how(name: &str) -> Option<How> {
    Some(match name {
        "format" | "format_args" | "print" | "println" | "eprint" | "eprintln" | "write" | "writeln" | "panic" | "unreachable" | "todo"
        | "unimplemented" | "dbg" | "vec" => How::All,
        "assert" => How::Cond(1),
        "assert_eq" | "assert_ne" => How::Cond(2),
        "debug_assert" | "debug_assert_eq" | "debug_assert_ne" => How::Cond(0),
        "json" => How::Json,
        _ => return None,
    })
}

/// The macro a call stands for, if it stands for one: `__bm_println_p` is `println`.
pub fn macro_name(call: &str) -> Option<&str> {
    let rest = call.strip_prefix(CALL)?;
    let rest = rest.strip_suffix("_r").unwrap_or(rest);
    rest.rsplit_once('_').map(|(n, _)| n)
}

/// A call that stands for a JSON object or array.
pub fn is_json_part(n: Node, src: &str) -> bool {
    n.kind() == "call_expression" && n.child_by_field_name("function").is_some_and(|f| matches!(text(f, src), OBJ | ARR))
}

/// A closure around an argument that runs only sometimes, called: `(|__bmc| e)(())`.
pub fn is_cond(n: Node, src: &str) -> bool {
    cond_body(n, src).is_some()
}

fn cond_body<'a>(n: Node<'a>, src: &str) -> Option<Node<'a>> {
    if n.kind() != "call_expression" {
        return None;
    }
    let f = n.child_by_field_name("function").filter(|f| f.kind() == "parenthesized_expression")?;
    let c = named_children(f).into_iter().find(|c| c.kind() == "closure_expression")?;
    let ps = c.child_by_field_name("parameters")?;
    (text(ps, src) == format!("|{COND}|")).then(|| c.child_by_field_name("body")).flatten()
}

/// The arguments of a call that stands for a macro or a JSON part, as `bind` binds them: each is a value of its own.
pub fn macro_arg(n: Node, src: &str) -> bool {
    let Some(p) = n.parent() else { return false };
    if p.kind() == "closure_expression" && p.child_by_field_name("body") == Some(n) {
        return p.parent().and_then(|x| x.parent()).is_some_and(|call| is_cond(call, src));
    }
    if p.kind() == "assignment_expression" && p.child_by_field_name("right") == Some(n) {
        return p.parent().is_some_and(|a| a.kind() == "arguments" && a.parent().is_some_and(|c| stands_for_macro(c, src)));
    }
    p.kind() == "arguments" && p.parent().is_some_and(|c| stands_for_macro(c, src) || is_json_part(c, src))
}

/// The argument list of a call that stands for a macro.
pub fn macro_arg_list(n: Node, src: &str) -> bool {
    n.kind() == "arguments" && n.parent().is_some_and(|c| stands_for_macro(c, src))
}

fn stands_for_macro(call: Node, src: &str) -> bool {
    call.kind() == "call_expression" && call_name(call, src).is_some_and(|n| n.starts_with(CALL))
}

/// The last segment of a call's function, when it is a name or a path: `serde_json::__bm_json_p` gives `__bm_json_p`.
fn call_name<'a>(call: Node<'a>, src: &'a str) -> Option<&'a str> {
    let f = call.child_by_field_name("function")?;
    match f.kind() {
        "identifier" => Some(text(f, src)),
        "scoped_identifier" => f.child_by_field_name("name").map(|n| text(n, src)),
        _ => None,
    }
}

/// An argument that a macro runs only sometimes: an assertion's message, every argument of `debug_assert!`.
pub fn is_cond_arg(n: Node, src: &str) -> bool {
    n.parent()
        .filter(|p| p.kind() == "closure_expression" && p.child_by_field_name("body") == Some(n))
        .and_then(|p| p.parent())
        .and_then(|x| x.parent())
        .is_some_and(|call| is_cond(call, src))
}

/// The tokens between a token tree's delimiters, split at top-level tokens of kind `sep`; empty pieces are dropped.
/// A turbofish's arguments are not a token tree, and their commas do not split: `BTreeMap::<String, usize>::new()`.
fn split<'a>(toks: &[Node<'a>], sep: &str) -> Vec<Vec<Node<'a>>> {
    let mut out = vec![vec![]];
    let mut angle = 0usize;
    for (i, t) in toks.iter().enumerate() {
        match t.kind() {
            "<" if angle > 0 || (i > 0 && toks[i - 1].kind() == "::") => angle += 1,
            ">" if angle > 0 => angle -= 1,
            ">>" if angle > 0 => angle = angle.saturating_sub(2),
            _ => {}
        }
        if t.kind() == sep && angle == 0 {
            out.push(vec![]);
        } else {
            out.last_mut().unwrap().push(*t);
        }
    }
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

fn inner<'a>(tt: Node<'a>) -> Vec<Node<'a>> {
    let toks = all_children(tt);
    if toks.len() < 2 {
        return vec![];
    }
    toks[1..toks.len() - 1].to_vec()
}

/// The edits that write one JSON value of `json!` as calls, or `None` if it is not one `json!` takes.
fn json_value(toks: &[Node], edits: &mut Vec<Edit>) -> Option<()> {
    if toks.is_empty() {
        return None;
    }
    if toks.len() != 1 || toks[0].kind() != "token_tree" {
        return Some(());
    }
    let tt = toks[0];
    let all = all_children(tt);
    let (open, close) = (*all.first()?, *all.last()?);
    match open.kind() {
        "{" => {
            edits.push((open.start_byte(), open.end_byte(), format!("{OBJ}(")));
            edits.push((close.start_byte(), close.end_byte(), ")".into()));
            for entry in split(&inner(tt), ",") {
                let colon = entry.iter().position(|t| t.kind() == ":")?;
                if colon == 0 {
                    return None;
                }
                edits.push((entry[colon].start_byte(), entry[colon].end_byte(), ",".into()));
                json_value(&entry[colon + 1..], edits)?;
            }
            Some(())
        }
        "[" => {
            edits.push((open.start_byte(), open.end_byte(), format!("{ARR}(")));
            edits.push((close.start_byte(), close.end_byte(), ")".into()));
            for el in split(&inner(tt), ",") {
                json_value(&el, edits)?;
            }
            Some(())
        }
        _ => Some(()),
    }
}

fn encode_one(m: Node, src: &str) -> Option<Vec<Edit>> {
    // A path stays: `serde_json::json!(..)` is `serde_json::__bm_json_p(..)`.
    let mac = m.child_by_field_name("macro")?;
    let name_node = match mac.kind() {
        "identifier" => mac,
        "scoped_identifier" => mac.child_by_field_name("name")?,
        _ => return None,
    };
    let name = text(name_node, src);
    let how = how(name)?;
    let start = name_node.start_byte();
    let tt = named_children(m).into_iter().find(|c| c.kind() == "token_tree")?;
    let all = all_children(tt);
    let (open, close) = (*all.first()?, *all.last()?);
    let d = match open.kind() {
        "(" => "p",
        "[" => "b",
        "{" => "c",
        _ => return None,
    };
    let toks = inner(tt);
    let mut edits: Vec<Edit> = vec![];
    match how {
        How::Json => {
            edits.push((start, open.end_byte(), format!("{CALL}{name}_{d}(")));
            json_value(&toks, &mut edits)?;
        }
        How::All => {
            let semis: Vec<&Node> = toks.iter().filter(|t| t.kind() == ";").collect();
            match semis.as_slice() {
                [] => edits.push((start, open.end_byte(), format!("{CALL}{name}_{d}("))),
                [s] if name == "vec" => {
                    edits.push((start, open.end_byte(), format!("{CALL}{name}_{d}_r(")));
                    edits.push((s.start_byte(), s.end_byte(), ",".into()));
                }
                _ => return None,
            }
        }
        How::Cond(n) => {
            edits.push((start, open.end_byte(), format!("{CALL}{name}_{d}(")));
            for arg in split(&toks, ",").into_iter().skip(n) {
                // A named argument (`x = e`) runs its value.
                let from = arg.iter().position(|t| t.kind() == "=").map_or(0, |i| i + 1);
                let (Some(a), Some(b)) = (arg.get(from), arg.last()) else { return None };
                edits.push((a.start_byte(), a.start_byte(), format!("(|{COND}| ")));
                edits.push((b.end_byte(), b.end_byte(), ")(())".into()));
            }
        }
    }
    // Last: an insertion at the same place goes before it.
    edits.push((close.start_byte(), close.end_byte(), ")".into()));
    Some(edits)
}

/// The macros a function body calls whose arguments `encode` writes as a call's, outer before inner.
fn known<'a>(n: Node<'a>, src: &str, in_fn: bool, out: &mut Vec<Node<'a>>) {
    if n.kind() == "token_tree" {
        return;
    }
    if in_fn && n.kind() == "macro_invocation" {
        let name = n.child_by_field_name("macro").and_then(|m| match m.kind() {
            "identifier" => Some(m),
            "scoped_identifier" => m.child_by_field_name("name"),
            _ => None,
        });
        if name.is_some_and(|m| how(text(m, src)).is_some()) {
            out.push(n);
        }
        return;
    }
    let in_fn = in_fn || n.kind() == "function_item";
    for c in named_children(n) {
        known(c, src, in_fn, out);
    }
}

/// Write the standard macros of every function body as calls (outer before inner; a macro whose arguments do not
/// read as `json!` or a call's, or that would not parse, is left as it is).
pub fn encode(src: &str) -> String {
    let mut s = src.to_string();
    let mut skip = 0;
    loop {
        let tree = parse_rust(&s);
        let mut ms = vec![];
        known(tree.root_node(), &s, false, &mut ms);
        let Some(m) = ms.get(skip) else { break };
        let next = encode_one(*m, &s).map(|e| apply(&s, e)).filter(|n| first_error(&parse_rust(n)).is_none());
        match next {
            Some(n) => s = n,
            None => skip += 1,
        }
    }
    s
}

/// Write the calls `encode` made back as the macros they stand for.
pub fn decode(src: &str) -> String {
    let tree = parse_rust(src);
    let mut edits: Vec<Edit> = vec![];
    fn walk(n: Node, src: &str, edits: &mut Vec<Edit>) {
        if n.kind() == "call_expression" {
            let f = n.child_by_field_name("function");
            let args = n.child_by_field_name("arguments");
            let ft = call_name(n, src).unwrap_or("");
            // Where the name starts: a path before it stays.
            let at = f.map(|f| if f.kind() == "scoped_identifier" { f.child_by_field_name("name").unwrap_or(f) } else { f }).map_or(0, |f| f.start_byte());
            if let (Some(args), true) = (args, f.is_some_and(|f| matches!(f.kind(), "identifier" | "scoped_identifier"))) {
                let open_end = args.start_byte() + 1;
                let close = (args.end_byte() - 1, args.end_byte());
                let commas: Vec<Node> = all_children(args).into_iter().filter(|c| c.kind() == ",").collect();
                if let Some(rest) = ft.strip_prefix(CALL) {
                    let (rest, rep) = match rest.strip_suffix("_r") {
                        Some(r) => (r, true),
                        None => (rest, false),
                    };
                    if let Some((name, d)) = rest.rsplit_once('_') {
                        let (o, c) = match d {
                            "b" => ("[", "]"),
                            "c" => ("{", "}"),
                            _ => ("(", ")"),
                        };
                        edits.push((at, open_end, format!("{name}!{o}")));
                        edits.push((close.0, close.1, c.into()));
                        if let (true, Some(c)) = (rep, commas.first()) {
                            edits.push((c.start_byte(), c.end_byte(), ";".into()));
                        }
                    }
                } else if ft == OBJ {
                    edits.push((n.start_byte(), open_end, "{".into()));
                    edits.push((close.0, close.1, "}".into()));
                    for c in commas.iter().step_by(2) {
                        edits.push((c.start_byte(), c.end_byte(), ":".into()));
                    }
                } else if ft == ARR {
                    edits.push((n.start_byte(), open_end, "[".into()));
                    edits.push((close.0, close.1, "]".into()));
                }
            } else if let Some(body) = cond_body(n, src) {
                edits.push((n.start_byte(), body.start_byte(), String::new()));
                edits.push((body.end_byte(), n.end_byte(), String::new()));
            }
        }
        for c in named_children(n) {
            walk(c, src, edits);
        }
    }
    walk(tree.root_node(), src, &mut edits);
    apply(src, edits)
}
