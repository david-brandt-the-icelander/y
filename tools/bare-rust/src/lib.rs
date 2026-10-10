//! Bare Rust: strip Rust into a bare body plus constraint JSON, and weave them back (proposal/bare-rust.md).
use tree_sitter::{Language, Node, Parser, Tree};

pub mod bind;
pub mod cargo;
pub mod check;
pub mod clause;
pub mod corpus;
pub mod derive;
pub mod e2;
pub mod e3;
pub mod errmap;
pub mod crate_weave;
pub mod externals;
pub mod hints;
pub mod host;
pub mod scaffold;
pub mod json;
pub mod libcheck;
pub mod libstats;
pub mod callback;
pub mod conformance;
pub mod seq;
pub mod seqexpr;
pub mod life;
pub mod report;
pub mod runcheck;
pub mod stats;
pub mod strip;
pub mod trace;
pub mod types;
pub mod weave;

pub use strip::strip;
pub use weave::weave;

extern "C" {
    fn tree_sitter_bare_rust() -> *const ();
}

pub type R<T> = Result<T, String>;

/// A text edit: replace `start..end` with the text.
pub type Edit = (usize, usize, String);

pub fn bare_language() -> Language {
    let f = unsafe { tree_sitter_language::LanguageFn::from_raw(tree_sitter_bare_rust) };
    Language::from(f)
}

fn parse_with(lang: Language, src: &str) -> Tree {
    let mut p = Parser::new();
    p.set_language(&lang).expect("language");
    p.parse(src, None).expect("parse")
}

/// Parse with the bare parser; the tree may contain ERROR or MISSING nodes.
pub fn parse_bare(src: &str) -> Tree {
    parse_with(bare_language(), src)
}

/// Parse with tree-sitter-rust.
pub fn parse_rust(src: &str) -> Tree {
    parse_with(tree_sitter_rust::LANGUAGE.into(), src)
}

/// The first ERROR or MISSING node of a tree, as "line:column", if any.
pub fn first_error(tree: &Tree) -> Option<String> {
    fn go(n: Node) -> Option<String> {
        if n.is_error() || n.is_missing() {
            let p = n.start_position();
            return Some(format!("{}:{}", p.row + 1, p.column + 1));
        }
        if !n.has_error() {
            return None;
        }
        let mut c = n.walk();
        let found = n.children(&mut c).find_map(go);
        found
    }
    go(tree.root_node())
}

pub fn text<'a>(n: Node, src: &'a str) -> &'a str {
    &src[n.byte_range()]
}

pub fn named_children<'a>(n: Node<'a>) -> Vec<Node<'a>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

pub fn all_children<'a>(n: Node<'a>) -> Vec<Node<'a>> {
    let mut c = n.walk();
    n.children(&mut c).collect()
}

pub fn is_comment(n: Node) -> bool {
    matches!(n.kind(), "line_comment" | "block_comment")
}

/// Named children that are not comments or attributes.
pub fn significant<'a>(n: Node<'a>) -> Vec<Node<'a>> {
    named_children(n).into_iter().filter(|c| !is_comment(*c) && c.kind() != "attribute_item").collect()
}

/// Apply edits in order of position; edits at the same position keep their given order.
pub fn apply(src: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| e.0);
    let (mut out, mut at) = (String::new(), 0);
    for (s, e, t) in edits {
        if s < at {
            continue; // nested inside an earlier cut
        }
        out.push_str(&src[at..s]);
        out.push_str(&t);
        at = e;
    }
    out.push_str(&src[at..]);
    out
}

/// Collapse runs of whitespace to one space.
pub fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The module path a source file contributes to subject names: `src/a/b.rs` is `a::b`.
pub fn module_path(module: &str) -> Vec<String> {
    let Some(rest) = module.strip_prefix("src/") else { return vec![] };
    let rest = rest.strip_suffix(".rs").unwrap_or(rest);
    let mut parts: Vec<String> = rest.split('/').map(str::to_string).collect();
    if matches!(parts.last().map(String::as_str), Some("lib" | "main" | "mod")) {
        parts.pop();
    }
    parts
}

/// Node kinds that are items: they are subjects, and they are skipped when scanning a body for holes.
pub fn is_item_kind(k: &str) -> bool {
    matches!(
        k,
        "function_item"
            | "function_signature_item"
            | "struct_item"
            | "enum_item"
            | "union_item"
            | "trait_item"
            | "impl_item"
            | "const_item"
            | "static_item"
            | "type_item"
            | "associated_type"
            | "mod_item"
            | "macro_definition"
    )
}

/// Normalise Rust source for check 2: elide lifetimes where Rust allows. `mut` is left alone, because the
/// woven side derives it through rustc.
pub fn normalise(src: &str) -> R<String> {
    normalise_with(src, false)
}

/// As `normalise`, and also remove `mut` on bindings: for weaves that did not run rustc (`--no-cargo`).
pub fn normalise_nomut(src: &str) -> R<String> {
    normalise_with(src, true)
}

/// Round trips compare bound sources: `weave(strip(s))` is `bind(s)` (see `bind`), up to the rest of `normalise`.
fn normalise_with(src: &str, strip_mut: bool) -> R<String> {
    if let Some(e) = first_error(&parse_rust(src)) {
        return Err(format!("parse-error: {e}"));
    }
    let bound = bind::bind(src)?;
    let src = bound.as_str();
    let tree = parse_rust(src);
    let mut edits = vec![];
    life::normalise_edits(tree.root_node(), src, &mut edits, strip_mut);
    Ok(apply(src, edits))
}

/// How many bindings in a source text carry `mut`.
pub fn count_mut(src: &str) -> usize {
    life::count_mut(parse_rust(src).root_node())
}

/// Run `rustfmt` on a source text.
pub fn rustfmt(src: &str) -> R<String> {
    use std::io::Write;
    let mut child = std::process::Command::new("rustfmt")
        .args(["--edition", "2021", "--config", "max_width=100"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("rustfmt: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("rustfmt: no stdin")?;
    let input = src.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("rustfmt: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(format!("rustfmt: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("failed")));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

/// Gives every subject a unique name: a repeated name gets `#2`, `#3`, in source order.
#[derive(Default)]
pub struct Namer(std::collections::HashMap<String, usize>);

impl Namer {
    pub fn unique(&mut self, name: String) -> String {
        let c = self.0.entry(name.clone()).or_insert(0);
        *c += 1;
        if *c == 1 {
            name
        } else {
            format!("{name}#{c}")
        }
    }
}
