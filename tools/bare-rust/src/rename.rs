//! rename: new names for names of a stripped crate, in the bare bodies and the constraint files together, and nothing
//! else.
//!
//! A map names what to rename, by the function whose body binds the name, and the crate's realizations by
//! themselves:
//!
//! ```json
//! { "strsim::jaro": { "a_len_f64": "a_total" }, "realizations": { "parse_u16": "parse_port" } }
//! ```
//!
//! A local is renamed at every use in the function's own body (not in an item declared inside it), and in the keys of
//! the function's `holes` that name it (`a_len_f64`, `by_len.a`, `(m, n)`, `x#2`). A realization is renamed at every
//! use in the crate and in the crate's table. The rename is refused when the new name is not a name, is already a name
//! in the function (or, for a realization, anywhere in the crate), or names an item of the crate; when the old name is
//! not bound in that body, or a format string captures it (`"{x}"`); and when the result does not weave as the
//! original did. A check after the edit compares the token streams: the same tokens, but for the renamed names.
use crate::crate_weave::crate_realizations;
use crate::weave::{weave_in_crate, Woven};
use crate::{all_children, apply, is_item_kind, parse_bare, text, Edit, R};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// One name renamed: under which function (or `realizations`), from and to, and at how many places.
#[derive(Debug, Clone, PartialEq)]
pub struct Renamed {
    pub subject: String,
    pub from: String,
    pub to: String,
    pub uses: usize,
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
    "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof",
    "unsized", "virtual", "yield", "try", "gen", "rust",
];

fn is_name(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && s.chars().all(|c| c.is_alphanumeric() || c == '_') && s != "_" && !KEYWORDS.contains(&s)
}

fn leaves<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
    if n.child_count() == 0 {
        out.push(n);
    }
    for c in all_children(n) {
        leaves(c, out);
    }
}

/// The leaves of a body, not counting the items declared in it.
fn own_leaves<'a>(n: Node<'a>, root: bool, out: &mut Vec<Node<'a>>) {
    if !root && is_item_kind(n.kind()) {
        return;
    }
    if n.child_count() == 0 {
        out.push(n);
    }
    for c in all_children(n) {
        own_leaves(c, false, out);
    }
}

/// Whether a name at `n` is where it is bound: in a pattern (a `let`, a closure's or a `for`'s, a match arm's).
fn binds(n: Node) -> bool {
    let mut cur = n;
    while let Some(p) = cur.parent() {
        if p.child_by_field_name("pattern") == Some(cur) || matches!(p.kind(), "closure_parameters" | "match_pattern") {
            return true;
        }
        if p.kind().ends_with("_expression") || p.kind() == "block" || is_item_kind(p.kind()) {
            return false;
        }
        cur = p;
    }
    false
}

/// `old` replaced by `new` where it is a whole word of a key: `a_len`, `by_len.a`, `(m, n)`, `x#2`.
fn rename_key(key: &str, old: &str, new: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut String| {
        out.push_str(if w == old { new } else { w });
        w.clear();
    };
    for c in key.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A table's keys renamed, in their order.
fn rename_keys(m: &Map<String, Value>, f: &dyn Fn(&str) -> String) -> Map<String, Value> {
    m.iter().map(|(k, v)| (f(k), v.clone())).collect()
}

struct File {
    brs: PathBuf,
    bare: String,
    json: Value,
}

/// Rename in the stripped crate at `dir` (its `.brs` and `.constraints.json` files) what `map` says. With `write`, the
/// files are rewritten; without, only checked.
pub fn rename_dir(dir: &Path, map: &Value, write: bool) -> R<Vec<Renamed>> {
    rename_dir_with(dir, map, write, false).map(|(r, _)| r)
}

/// As `rename_dir`; with `lenient`, an entry that does not apply (no such function or name, or a new name already
/// taken) is skipped and returned with why, rather than refusing the map. For a map made on another version of the
/// crate.
pub fn rename_dir_with(dir: &Path, map: &Value, write: bool, lenient: bool) -> R<(Vec<Renamed>, Vec<String>)> {
    let mut brs = vec![];
    crate::corpus::find_brs(dir, &mut brs);
    if brs.is_empty() {
        return Err(format!("{}: no .brs files", dir.display()));
    }
    let mut files = vec![];
    for b in brs {
        let bare = fs::read_to_string(&b).map_err(|e| format!("{}: {e}", b.display()))?;
        let cj = b.with_extension("constraints.json");
        let js = fs::read_to_string(&cj).map_err(|e| format!("{}: {e}", cj.display()))?;
        let json: Value = serde_json::from_str(&js).map_err(|e| format!("{}: {e}", cj.display()))?;
        files.push(File { brs: b, bare, json });
    }
    let (out, renamed, skipped) = rename_files(&files, map, lenient)?;
    if write {
        for (f, (bare, json)) in files.iter().zip(out) {
            if bare != f.bare {
                fs::write(&f.brs, bare).map_err(|e| format!("{}: {e}", f.brs.display()))?;
            }
            if json != f.json {
                let cj = f.brs.with_extension("constraints.json");
                fs::write(&cj, crate::json::canonical(&json)).map_err(|e| format!("{}: {e}", cj.display()))?;
            }
        }
    }
    Ok((renamed, skipped))
}

fn weave_all(files: &[(PathBuf, String, Value)]) -> R<Vec<Woven>> {
    let listed: Vec<(PathBuf, Value)> = files.iter().map(|f| (f.0.clone(), f.2.clone())).collect();
    let (reals, _) = crate_realizations(&listed)?;
    files.iter().map(|(p, b, j)| weave_in_crate(b, j, Some(&reals)).map_err(|e| format!("{}: {}", p.display(), e.msg))).collect()
}

fn rename_files(files: &[File], map: &Value, lenient: bool) -> R<(Vec<(String, Value)>, Vec<Renamed>, Vec<String>)> {
    let usage = "the map is a JSON object: {\"<function>\": {\"<old>\": \"<new>\"}, \"realizations\": {\"<old>\": \"<new>\"}}";
    let obj = map.as_object().ok_or(usage)?;
    let triples: Vec<(PathBuf, String, Value)> = files.iter().map(|f| (f.brs.clone(), f.bare.clone(), f.json.clone())).collect();
    let before = weave_all(&triples)?;
    let listed: Vec<(PathBuf, Value)> = files.iter().map(|f| (f.brs.clone(), f.json.clone())).collect();
    let (reals, _) = crate_realizations(&listed)?;
    // Names of the crate's items: a local may not take one.
    let items: HashSet<String> = files
        .iter()
        .flat_map(|f| f.json["subjects"].as_array().into_iter().flatten())
        .filter_map(|s| s["name"].as_str().map(|n| n.rsplit("::").next().unwrap_or(n).to_string()))
        .collect();
    let trees: Vec<tree_sitter::Tree> = files.iter().map(|f| parse_bare(&f.bare)).collect();
    let mut edits: Vec<Vec<Edit>> = vec![vec![]; files.len()];
    // Per file: the leaf (start byte) a rename changes, and to what (for the check after).
    let mut planned: Vec<Vec<(usize, String, bool)>> = vec![vec![]; files.len()];
    let mut jsons: Vec<Value> = files.iter().map(|f| f.json.clone()).collect();
    let mut renamed = vec![];
    let mut skipped = vec![];
    // New names given so far, by function (and `realizations`): two entries may not give the same one.
    let mut given: HashSet<(String, String)> = HashSet::new();
    // A check that refuses the map, or with `lenient` skips the entry.
    macro_rules! refuse {
        ($($t:tt)*) => {{
            let why = format!($($t)*);
            if lenient {
                skipped.push(why);
                continue;
            }
            return Err(why);
        }};
    }
    for (subject, pairs) in obj {
        let pairs = pairs.as_object().ok_or(usage)?;
        for (old, new) in pairs {
            let new = new.as_str().ok_or(usage)?;
            if !is_name(new) {
                refuse!("`{new}` is not a name");
            }
            let clash = if subject == "realizations" {
                given.iter().any(|(_, n)| n == new)
            } else {
                given.contains(&(subject.clone(), new.to_string())) || given.contains(&("realizations".to_string(), new.to_string()))
            };
            if clash {
                refuse!("`{new}` is given twice in `{subject}`");
            }
            if subject == "realizations" {
                if !reals.contains_key(old) {
                    refuse!("no realization `{old}` in the crate");
                }
                if reals.contains_key(new) || files.iter().any(|f| crate::bind::idents(&f.bare).contains(new)) || items.contains(new) {
                    refuse!("`{new}` is already a name in the crate");
                }
                let mut uses = 0;
                for (i, t) in trees.iter().enumerate() {
                    let mut ls = vec![];
                    leaves(t.root_node(), &mut ls);
                    for l in ls.into_iter().filter(|l| matches!(l.kind(), "identifier" | "field_identifier") && text(*l, &files[i].bare) == old) {
                        edits[i].push((l.start_byte(), l.end_byte(), new.to_string()));
                        planned[i].push((l.start_byte(), new.to_string(), false));
                        uses += 1;
                    }
                    if let Some(t) = jsons[i]["realizations"].as_object() {
                        if t.contains_key(old) {
                            jsons[i]["realizations"] = Value::Object(rename_keys(t, &|k| if k == old { new.to_string() } else { k.to_string() }));
                        }
                    }
                }
                given.insert((subject.clone(), new.to_string()));
                renamed.push(Renamed { subject: subject.clone(), from: old.clone(), to: new.to_string(), uses });
                continue;
            }
            // A local of a function: its body, in the file whose weave found it.
            let found = before
                .iter()
                .enumerate()
                .find_map(|(i, w)| w.bodies.iter().find(|(s, _)| s == subject).map(|(_, r)| (i, *r)))
                .ok_or_else(|| format!("no body of `{subject}` in the bare files"));
            let (fi, range) = match found {
                Ok(x) => x,
                Err(e) => refuse!("{e}"),
            };
            let src = &files[fi].bare;
            let body = trees[fi].root_node().descendant_for_byte_range(range.0, range.1).ok_or("a body that does not parse")?;
            let item = body.parent().unwrap_or(body);
            if crate::bind::idents(text(item, src)).contains(new) || items.contains(new) || reals.contains_key(new) {
                refuse!("`{new}` is already a name in `{subject}` or in the crate");
            }
            let mut ls = vec![];
            own_leaves(body, true, &mut ls);
            let at: Vec<Node> = ls.into_iter().filter(|l| matches!(l.kind(), "identifier" | "shorthand_field_identifier") && text(*l, src) == old).collect();
            if at.is_empty() {
                refuse!("`{old}` is not a name in the body of `{subject}`");
            }
            if !at.iter().any(|l| binds(*l)) {
                refuse!("`{old}` is not bound in the body of `{subject}` (a parameter or an item: rename it where it is declared)");
            }
            let mut strs = vec![];
            own_leaves(body, true, &mut strs);
            if strs.iter().any(|s| s.kind() == "string_content" && (text(*s, src).contains(&format!("{{{old}}}")) || text(*s, src).contains(&format!("{{{old}:")))) {
                refuse!("a format string in `{subject}` captures `{old}`: rename it by hand");
            }
            for l in &at {
                // A shorthand field keeps its field: `S { x }` becomes `S { x: y }`.
                let short = l.kind() == "shorthand_field_identifier" || l.parent().is_some_and(|p| p.kind() == "shorthand_field_initializer");
                let to = if short { format!("{old}: {new}") } else { new.to_string() };
                edits[fi].push((l.start_byte(), l.end_byte(), to));
                planned[fi].push((l.start_byte(), new.to_string(), short));
            }
            if let Some(h) = jsons[fi]["holes"][subject.as_str()].as_object_mut() {
                if let Some(Value::Object(locals)) = h.get("locals") {
                    let r = rename_keys(locals, &|k| rename_key(k, old, new));
                    h.insert("locals".into(), Value::Object(r));
                }
            }
            given.insert((subject.clone(), new.to_string()));
            renamed.push(Renamed { subject: subject.clone(), from: old.clone(), to: new.to_string(), uses: at.len() });
        }
    }
    let out: Vec<(String, Value)> = files.iter().zip(edits).zip(jsons).map(|((f, e), j)| (apply(&f.bare, e), j)).collect();
    // The check after: the same tokens but for the renamed ones, and the same weave.
    for (i, (bare, _)) in out.iter().enumerate() {
        let (mut a, mut b) = (vec![], vec![]);
        leaves(trees[i].root_node(), &mut a);
        let t2 = parse_bare(bare);
        if let Some(e) = crate::first_error(&t2) {
            return Err(format!("{}: the renamed body does not parse, at {e}", files[i].brs.display()));
        }
        leaves(t2.root_node(), &mut b);
        let mut want: Vec<(String, String)> = vec![];
        for l in &a {
            let (k, t) = (l.kind().to_string(), text(*l, &files[i].bare).to_string());
            match planned[i].iter().find(|p| p.0 == l.start_byte()) {
                Some((_, new, true)) => {
                    want.push(("field_identifier".into(), t));
                    want.push((":".into(), ":".into()));
                    want.push(("identifier".into(), new.clone()));
                }
                Some((_, new, false)) => want.push((k, new.clone())),
                None => want.push((k, t)),
            }
        }
        let got: Vec<(String, String)> = b.iter().map(|l| (l.kind().to_string(), text(*l, bare).to_string())).collect();
        if want != got {
            let at = want.iter().zip(&got).position(|(x, y)| x != y).unwrap_or(want.len().min(got.len()));
            return Err(format!("{}: the renamed body differs in more than the names, at token {at}", files[i].brs.display()));
        }
    }
    let after_files: Vec<(PathBuf, String, Value)> = files.iter().zip(&out).map(|(f, (b, j))| (f.brs.clone(), b.clone(), j.clone())).collect();
    let after = weave_all(&after_files)?;
    for (i, (x, y)) in before.iter().zip(&after).enumerate() {
        if x.unbound.len() != y.unbound.len() {
            let shown: Vec<String> = y.unbound.iter().map(|u| u.show()).collect();
            return Err(format!("{}: renamed, the file binds differently: {}", files[i].brs.display(), shown.join(", ")));
        }
    }
    Ok((out, renamed, skipped))
}

/// The names each body binds (in its own patterns, not in items declared inside it), by function, with the line of
/// the first binding: what `rename` can rename. And the crate's realizations, under `realizations`.
pub fn bound_names(dir: &Path) -> R<Value> {
    let mut brs = vec![];
    crate::corpus::find_brs(dir, &mut brs);
    let mut triples = vec![];
    for b in brs {
        let bare = fs::read_to_string(&b).map_err(|e| format!("{}: {e}", b.display()))?;
        let cj = b.with_extension("constraints.json");
        let json: Value = serde_json::from_str(&fs::read_to_string(&cj).map_err(|e| format!("{}: {e}", cj.display()))?).map_err(|e| format!("{}: {e}", cj.display()))?;
        triples.push((b, bare, json));
    }
    let woven = weave_all(&triples)?;
    let mut out = Map::new();
    for ((path, bare, json), w) in triples.iter().zip(&woven) {
        let tree = parse_bare(bare);
        let rel = path.strip_prefix(dir).unwrap_or(path).display().to_string();
        for (subject, (s, e)) in &w.bodies {
            let Some(body) = tree.root_node().descendant_for_byte_range(*s, *e) else { continue };
            let mut ls = vec![];
            own_leaves(body, true, &mut ls);
            let mut names = Map::new();
            for l in ls.into_iter().filter(|l| l.kind() == "identifier" && binds(*l)) {
                let n = text(l, bare).to_string();
                if !names.contains_key(&n) {
                    names.insert(n, json!({"line": l.start_position().row + 1}));
                }
            }
            if !names.is_empty() {
                out.insert(subject.clone(), json!({"file": rel, "names": names}));
            }
        }
        if let Some(t) = json["realizations"].as_object() {
            let r = out.entry("realizations").or_insert_with(|| json!({}));
            for (k, v) in t {
                r[k] = v.clone();
            }
        }
    }
    Ok(Value::Object(out))
}
