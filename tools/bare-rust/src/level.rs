//! Levels (language 6.22, Y § 3.19): how much of its language a module uses.
//!
//! A module subject may carry a `level`, a ceiling that nothing in the module may rise above. `uses` finds what the
//! bare bodies of a crate use above `data`, each with the level it needs and the function and line it is at; `check`
//! holds every module that has a level to it, the modules inside it (Rust's `mod`, Y's facets) and the files under it
//! among them. A module's tests are not part of what it carries to a backend, so a `tests` module is not held to its
//! level.
//!
//! What each level adds, as bare Rust shows it:
//! - `calculations`: a function body;
//! - `structured`: a loop (`for`, `while`, `loop`), an assignment, `&mut`, a parameter the function changes (one
//!   whose access is `write`: Y's `mutable`), and recursion: a call of the function it is in, or of one that calls
//!   back to it;
//! - `shared`: a parameter the Y lowering marks `injected` whose type is `Shared<…>`, and a call into a library module
//!   at `shared` (`y_runtime::shared`, `y_runtime::arena`, `std::cell`, `std::sync`, …);
//! - `effects`: any other `injected` parameter, which is a capability; an `effects only …` clause; `async`, `.await`;
//!   a call into a library module at `effects` (`y_runtime::task`, `y_runtime::store`, `y_runtime::System`,
//!   `y_runtime::Deadline`, `std::fs`, …); a wait (`// y: waits until name`); and a region that waits `forever` or
//!   is detached.
//!
//! The library modules' levels are a table, `libraries/levels.json`: a call of a function under a module listed there,
//! or of a method of a type under it, needs the level of the longest module that matches, and a call into a library
//! the table does not list needs `effects`. `crate::facts::check_library` (`bare-rust levels --libraries`) checks the
//! table against what a library crate's code calls.
//!
//! A module's level is the highest of anything it declares or imports (Y § 3.19): a call of a function of the crate
//! needs the level that function needs, so a `calculations` module that calls a `structured` function is
//! `structured`. An import (`use`) of a module pinned at a higher level fails too.
//!
//! What a call calls is read one of two ways, and `uses_with` takes, file by file, the first it has:
//! - **From rustc** (`crate::facts`), when the woven crate builds: weave with cargo reads the crate's MIR, where every
//!   call names the item it resolved to. A function of the crate is found by its path; a method by its receiver's
//!   type, which MIR states (`<Tally as Count>::count` is `Tally`'s `count`, whatever names the receiver: a closure's
//!   parameter, a chained call); a library's function by where it is defined, so a re-export, a renamed import or a
//!   local that holds the function (`let run = atomic;`) is the function itself. A trait's method called on a type
//!   parameter may be any impl of the trait in the crate, and needs the most of them; a library's trait method needs
//!   the level of its type's module and of its trait's.
//! - **From the syntax**, when there is no build (a unit test, `bare-rust levels` without `--crate`, a crate that does
//!   not compile): a call is matched through the file's `use` declarations (`atomic(…)` after
//!   `use y_runtime::shared::atomic;` is `y_runtime::shared::atomic`). A method call whose receiver's type the
//!   constraint data does not give (a parameter or a typed local) may be any method of that name of the crate's types
//!   (or, on `self`, its own type's), and a name the crate has more than one function for, out of reach of the
//!   caller, may be any of them: such a call needs the most that any of them needs. A re-export, a field that holds a
//!   function and a local alias are not followed.
//!
//! Weave says which way it read them (`CrateWeave::checks_via`). Either way, what is not a call (a loop, `&mut`, a
//! marker, a parameter's type) is read from the syntax and the constraint data, and a macro's calls are not seen.
use crate::facts::{Callee, Fact, Facts};
use crate::{parse_bare, text};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

/// The levels, lowest first.
pub const LEVELS: &[&str] = &["data", "calculations", "structured", "shared", "effects"];

/// The rank of a level, or `None` for a word that is not one.
pub fn rank(level: &str) -> Option<usize> {
    LEVELS.iter().position(|l| *l == level)
}

/// One construct above `data`: the level it needs, what it is, the function it is in, its file and its line there.
#[derive(Debug, Clone, PartialEq)]
pub struct Use {
    pub level: usize,
    pub what: String,
    pub function: String,
    pub file: String,
    pub line: usize,
}

/// The levels of library modules (`libraries/levels.json`): a call into one needs its level.
const LIBRARY_LEVELS: &str = include_str!("../libraries/levels.json");

/// The calls `libraries/levels.json` trusts a library's function to make though they are above its module's level, by
/// the function: each a path, or a module or type the call is under (see `crate::facts::check_library`).
pub(crate) fn trusted_calls(function: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(LIBRARY_LEVELS).unwrap_or_default();
    v["trusted"][function]["calls"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(str::to_string)).collect()
}

/// Whether the call of `callee` is one of the `trusted` calls: the path itself, or under it.
pub(crate) fn is_trusted(trusted: &[String], callee: &str) -> bool {
    trusted.iter().any(|t| callee == t || callee.starts_with(&format!("{t}::")))
}

/// The level a call of the library path `path` needs, by the longest module of the table it is under.
pub(crate) fn library_level(path: &str) -> Option<usize> {
    thread_local! {
        static TABLE: Vec<(String, usize)> = {
            let v: Value = serde_json::from_str(LIBRARY_LEVELS).expect("libraries/levels.json");
            v["modules"].as_object().into_iter().flatten().filter_map(|(m, l)| Some((m.clone(), rank(l.as_str()?)?))).collect()
        };
    }
    TABLE.with(|t| t.iter().filter(|(m, _)| path == m || path.starts_with(&format!("{m}::"))).max_by_key(|(m, _)| m.len()).map(|(_, l)| *l))
}

/// The names a file's `use` declarations bring in, to their paths: `use a::b::{c, d as e};` gives `c` → `a::b::c` and
/// `e` → `a::b::d`. Globs bring in nothing by name.
fn use_map(imports: &[Import], file: &str) -> BTreeMap<String, String> {
    fn go(prefix: &str, tree: &str, out: &mut BTreeMap<String, String>) {
        let tree = tree.trim();
        if let Some(open) = tree.find('{').filter(|_| tree.ends_with('}')) {
            let base = tree[..open].trim_end_matches("::");
            let inner = &tree[open + 1..tree.len() - 1];
            let (mut depth, mut start) = (0, 0);
            for (i, c) in inner.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    ',' if depth == 0 => {
                        go(&join(prefix, base), &inner[start..i], out);
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            go(&join(prefix, base), &inner[start..], out);
        } else if !tree.is_empty() && !tree.ends_with('*') {
            let (path, alias) = tree.split_once(" as ").map_or((tree, None), |(p, a)| (p.trim(), Some(a.trim())));
            let full = join(prefix, path);
            let name = alias.unwrap_or_else(|| full.rsplit("::").next().unwrap_or(&full)).to_string();
            if name != "self" {
                out.insert(name, full);
            }
        }
    }
    fn join(a: &str, b: &str) -> String {
        if a.is_empty() { b.to_string() } else if b.is_empty() { a.to_string() } else { format!("{a}::{b}") }
    }
    let mut out = BTreeMap::new();
    for im in imports.iter().filter(|i| i.file == file) {
        go("", &im.path, &mut out);
    }
    out
}

/// A call as the bare body writes it: a path (`f`, `a::b::f`) or a method (`x.f`, kept as `.f` with its receiver); or
/// as rustc resolved it (`crate::facts`).
#[derive(Clone)]
enum Call {
    Path(String),
    Method { name: String, receiver: String, ty: Option<String> },
    Resolved(Callee),
}

/// A function of the crate: its path, its file, what it uses itself, and the calls it makes, with their lines; and,
/// for a method of an impl of a trait, the trait's name.
struct Function {
    path: String,
    file: String,
    own: Vec<Use>,
    calls: Vec<(Call, usize)>,
    trait_name: Option<String>,
}

/// An import: the module it is in, the path as written, its file and line.
struct Import {
    module: String,
    path: String,
    file: String,
    line: usize,
}

fn crate_name(js: &Value) -> String {
    js["crate"].as_str().and_then(|k| k.split_whitespace().next()).unwrap_or("crate").replace('-', "_")
}

/// The last segment of a path, without its generic arguments: `std::ops::Add<u8>` is `Add`.
fn last_segment(p: &str) -> String {
    p.split('<').next().unwrap_or(p).trim().rsplit("::").next().unwrap_or("").to_string()
}

/// The function among `found` that a byte offset is in: the innermost one.
fn enclosing(found: &[(String, Node)], offset: usize) -> Option<usize> {
    found.iter().enumerate().filter(|(_, (_, n))| n.start_byte() <= offset && offset < n.end_byte()).min_by_key(|(_, (_, n))| n.end_byte() - n.start_byte()).map(|(i, _)| i)
}

/// The functions and imports of one bare body. With `facts`, the calls are the ones rustc resolved in it, each in the
/// innermost function it is in; without, the ones its syntax shows.
fn read_file(file: &str, brs: &str, js: &Value, facts: Option<&[Fact]>, fns: &mut Vec<Function>, imports: &mut Vec<Import>) {
    let base = crate::weave::base_path(js);
    let tree = parse_bare(brs);
    let subject = |f: &str| js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "function" && s["name"] == f).cloned();
    let mut found: Vec<(String, Node)> = vec![];
    collect(tree.root_node(), brs, &base, &mut found, imports, file);
    let first = fns.len();
    let mut resolved: Vec<Vec<(Call, usize)>> = vec![vec![]; found.len()];
    for f in facts.into_iter().flatten() {
        if let Some(i) = enclosing(&found, f.offset) {
            resolved[i].push((Call::Resolved(f.callee.clone()), f.line));
        }
    }
    for (k, (path, node)) in found.iter().enumerate() {
        let (path, node) = (path.clone(), *node);
        let line = node.start_position().row + 1;
        let mut own = vec![];
        let mut add = |level: &str, what: String, line: usize| own.push(Use { level: rank(level).unwrap(), what, function: path.clone(), file: file.to_string(), line });
        if node.child_by_field_name("body").is_some() {
            add("calculations", "a function body".into(), line);
        }
        if (0..node.child_count()).filter_map(|i| node.child(i)).any(|c| c.kind() == "function_modifiers" && text(c, brs).contains("async")) {
            add("effects", "an `async` function".into(), line);
        }
        if let Some(sub) = subject(&path) {
            // a parameter the function changes (`mutable` in Y: write access to what it refers to) is mutation
            for p in sub["params"].as_array().into_iter().flatten().filter(|p| p["injected"] != true && p["access"]["rights"] == "write") {
                add("structured", format!("the parameter `{}`, which it changes", p["name"].as_str().unwrap_or("")), line);
            }
            for p in sub["params"].as_array().into_iter().flatten().filter(|p| p["injected"] == true) {
                let ty = p["type"].as_str().unwrap_or("");
                let ty = ty.strip_prefix("ref<target_type=").and_then(|t| t.strip_suffix('>')).unwrap_or(ty);
                let n = p["name"].as_str().unwrap_or("");
                if ty.starts_with("Shared<") {
                    add("shared", format!("the shared state `{n}`"), line);
                } else {
                    add("effects", format!("the capability `{n}`"), line);
                }
            }
            for c in sub["clauses"].as_array().into_iter().flatten().filter(|c| c["kind"] == "effects") {
                if let Some(t) = c["text"].as_str().filter(|t| t.starts_with("effects only ") && *t != "effects only none") {
                    add("effects", format!("`{t}`"), line);
                }
            }
        }
        let mut calls = vec![];
        if let Some(body) = node.child_by_field_name("body") {
            walk(body, brs, &mut add, &mut calls);
        }
        // a method's receiver, when it is a parameter or a local whose type the constraint data gives: its type's name
        let typed = |name: &str| -> Option<String> {
            let param = subject(&path).and_then(|s| s["params"].as_array()?.iter().find(|p| p["name"] == name).and_then(|p| p["type"].as_str().map(str::to_string)));
            let local = js["holes"][path.as_str()]["locals"][name].as_str().map(str::to_string).or_else(|| js["holes"][path.as_str()]["locals"][name]["type"].as_str().map(str::to_string));
            param.or(local).map(|t| type_head(&t))
        };
        for (c, _) in calls.iter_mut() {
            if let Call::Method { receiver, ty, .. } = c {
                if receiver.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                    *ty = typed(receiver);
                }
            }
        }
        if facts.is_some() {
            calls = std::mem::take(&mut resolved[k]);
        }
        let trait_name = node.parent().and_then(|p| p.parent()).filter(|p| p.kind() == "impl_item").and_then(|p| p.child_by_field_name("trait")).map(|t| last_segment(text(t, brs)));
        fns.push(Function { path: path.clone(), file: file.to_string(), own, calls, trait_name });
    }
    if facts.is_some() {
        // what rustc resolved needs no imports to follow: its library calls are counted with the crate's (`uses_with`)
        return;
    }
    // a call into a library needs its module's level (`libraries/levels.json`), through the file's imports; a call into a
    // library the table does not know needs `effects`, as nothing says it has none
    let uses = use_map(imports, file);
    let local = local_names(&tree.root_node(), brs, &base);
    for f in fns[first..].iter_mut() {
        for (call, line) in &f.calls {
            let (shown, full) = match call {
                Call::Path(p) => {
                    let first = p.split("::").next().unwrap_or("");
                    match uses.get(first) {
                        Some(m) => (p.clone(), format!("{m}{}", &p[first.len()..])),
                        None if p.contains("::") => (p.clone(), p.clone()),
                        None => continue,
                    }
                }
                Call::Method { name, ty: Some(t), .. } => match uses.get(t.as_str()) {
                    Some(m) => (format!(".{name}"), format!("{m}::{name}")),
                    None => continue,
                },
                _ => continue,
            };
            let first = full.split("::").next().unwrap_or("");
            if local.contains(first) || (first.starts_with(|c: char| c.is_ascii_uppercase()) && !uses.contains_key(first)) {
                // the crate's own, or a type of the prelude (`String::from`, `Vec::new`)
                continue;
            }
            match library_level(&full) {
                Some(l) if l >= 2 => f.own.push(Use { level: l, what: format!("`{shown}` (`{full}`)"), function: f.path.clone(), file: file.to_string(), line: *line }),
                Some(_) => {}
                None => f.own.push(Use { level: 4, what: format!("`{shown}` (`{full}`), a library with no level in libraries/levels.json"), function: f.path.clone(), file: file.to_string(), line: *line }),
            }
        }
    }
}

/// A type's name, from the constraint data's form of it: `ref<target_type=Game>` is `Game`, `Shared<Holdings>` is
/// `Shared`, `list<element_type=Line> as 'List'` is `List`.
fn type_head(t: &str) -> String {
    let t = t.trim();
    if let Some(inner) = t.strip_prefix("ref<target_type=").and_then(|r| r.strip_suffix('>')) {
        return type_head(inner);
    }
    if let Some((_, facade)) = t.rsplit_once(" as '") {
        return facade.trim_end_matches('\'').rsplit("::").next().unwrap_or("").to_string();
    }
    t.split('<').next().unwrap_or(t).rsplit("::").next().unwrap_or(t).to_string()
}

/// The names a bare body declares, and the crate's own roots: what a path that starts with one of them is not a
/// library's.
fn local_names(root: &Node, src: &str, base: &str) -> BTreeSet<String> {
    fn go(n: Node, src: &str, out: &mut BTreeSet<String>) {
        if matches!(n.kind(), "mod_item" | "function_item" | "struct_item" | "enum_item" | "type_item" | "trait_item" | "const_item" | "static_item" | "union_item") {
            if let Some(name) = n.child_by_field_name("name") {
                out.insert(text(name, src).to_string());
            }
        }
        let mut c = n.walk();
        for ch in n.named_children(&mut c) {
            go(ch, src, out);
        }
    }
    let mut out: BTreeSet<String> = ["crate", "self", "super", "Self"].iter().map(|s| s.to_string()).collect();
    out.insert(base.split("::").next().unwrap_or("").to_string());
    go(*root, src, &mut out);
    out
}

/// The paths a bare body's `use` declarations bring in, by name: for the region checks (`crate::region`).
pub fn imports_of(brs: &str, js: &Value) -> BTreeMap<String, String> {
    let (mut fns, mut imports) = (vec![], vec![]);
    let tree = parse_bare(brs);
    collect(tree.root_node(), brs, &crate::weave::base_path(js), &mut fns, &mut imports, "");
    use_map(&imports, "")
}

/// The functions (with their full paths) and the imports under a node.
fn collect<'t>(n: Node<'t>, src: &str, path: &str, fns: &mut Vec<(String, Node<'t>)>, imports: &mut Vec<Import>, file: &str) {
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        match ch.kind() {
            "mod_item" => {
                if let (Some(name), Some(body)) = (ch.child_by_field_name("name"), ch.child_by_field_name("body")) {
                    collect(body, src, &format!("{path}::{}", text(name, src)), fns, imports, file);
                }
            }
            "impl_item" => {
                let ty = ch.child_by_field_name("type").map(|t| text(t, src).split('<').next().unwrap_or("").trim().to_string()).unwrap_or_default();
                if let Some(body) = ch.child_by_field_name("body") {
                    collect(body, src, &format!("{path}::{ty}"), fns, imports, file);
                }
            }
            "function_item" => {
                if let Some(name) = ch.child_by_field_name("name") {
                    let p = format!("{path}::{}", text(name, src));
                    fns.push((p.clone(), ch));
                    if let Some(body) = ch.child_by_field_name("body") {
                        collect(body, src, &p, fns, imports, file);
                    }
                }
            }
            "use_declaration" => {
                if let Some(arg) = ch.child_by_field_name("argument") {
                    imports.push(Import { module: path.to_string(), path: text(arg, src).to_string(), file: file.to_string(), line: ch.start_position().row + 1 });
                }
            }
            _ => collect(ch, src, path, fns, imports, file),
        }
    }
}

fn walk(n: Node, src: &str, add: &mut impl FnMut(&str, String, usize), calls: &mut Vec<(Call, usize)>) {
    let line = n.start_position().row + 1;
    match n.kind() {
        // a function inside this one is read as its own
        "function_item" => return,
        "for_expression" | "while_expression" | "loop_expression" => add("structured", "a loop".into(), line),
        "assignment_expression" | "compound_assignment_expr" => add("structured", "an assignment".into(), line),
        "reference_expression" if text(n, src).starts_with("&mut") => add("structured", "`&mut`".into(), line),
        "await_expression" => add("effects", "`.await`".into(), line),
        "line_comment" => {
            let t = text(n, src);
            if t.starts_with("// y:") && (t.contains(" forever because \"") || t.contains(" detach because \"")) {
                add("effects", "a region that waits forever or is detached".into(), line);
            }
            if t.starts_with("// y: waits until ") {
                add("effects", format!("a wait, `{}`", t.trim_start_matches("// y: ")), line);
            }
        }
        "call_expression" => {
            if let Some(f) = n.child_by_field_name("function") {
                match f.kind() {
                    "identifier" | "scoped_identifier" => calls.push((Call::Path(text(f, src).to_string()), line)),
                    "field_expression" => {
                        if let (Some(v), Some(m)) = (f.child_by_field_name("value"), f.child_by_field_name("field")) {
                            calls.push((Call::Method { name: text(m, src).to_string(), receiver: text(v, src).to_string(), ty: None }, line));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        walk(ch, src, add, calls);
    }
}

/// The functions of the crate a call may be, from the function `caller`, and whether it can be only one. A path names
/// the functions whose paths end with it; a name, the function of that name nearest the caller (in its module, then in
/// each module around it), or else every function of the crate of that name. A method (`x.f()`), whose receiver's type
/// the bare body does not say, is the caller's own type's `f` when the receiver is `self`, and else every method `f`
/// of a type of the crate. A call that may be several needs the most any of them needs; only a call that can be one
/// counts as recursion.
fn resolve(call: &Call, caller: &str, krate: &str, fns: &BTreeMap<String, usize>) -> (Vec<usize>, bool) {
    let by_last = |name: &str, methods: bool| -> Vec<usize> {
        fns.iter()
            .filter(|(p, _)| {
                let mut segs = p.rsplit("::");
                segs.next() == Some(name) && (!methods || segs.next().is_some_and(|t| t.starts_with(|c: char| c.is_ascii_uppercase())))
            })
            .map(|(_, i)| *i)
            .collect()
    };
    match call {
        Call::Path(callee) => {
            let callee = callee.strip_prefix("crate::").map(|c| format!("{krate}::{c}")).unwrap_or_else(|| callee.clone());
            if callee.contains("::") {
                let hits: Vec<usize> = fns.iter().filter(|(p, _)| **p == callee || p.ends_with(&format!("::{callee}"))).map(|(_, i)| *i).collect();
                let one = hits.len() == 1;
                return (hits, one);
            }
            let mut scope: Vec<&str> = caller.split("::").collect();
            while !scope.is_empty() {
                if let Some(i) = fns.get(&format!("{}::{callee}", scope.join("::"))) {
                    return (vec![*i], true);
                }
                scope.pop();
            }
            let hits = by_last(&callee, false);
            let one = hits.len() == 1;
            (hits, one)
        }
        Call::Resolved(_) => (vec![], false),
        Call::Method { name, receiver, ty } => {
            if let Some(t) = ty {
                // the receiver's type is known: its method, if the type is the crate's; else none of the crate's
                let hits: Vec<usize> = fns.iter().filter(|(p, _)| p.ends_with(&format!("::{t}::{name}"))).map(|(_, i)| *i).collect();
                let one = hits.len() == 1;
                return (hits, one);
            }
            if receiver == "self" {
                let mut segs: Vec<&str> = caller.split("::").collect();
                segs.pop();
                if let Some(i) = fns.get(&format!("{}::{name}", segs.join("::"))) {
                    return (vec![*i], true);
                }
            }
            let hits = by_last(name, true);
            let one = hits.len() == 1;
            (hits, one)
        }
    }
}

/// The functions of the crate a call may be, and whether it can be only one.
type Callees = (Vec<usize>, bool);

/// Where a call rustc resolved goes: functions of the crate (and whether it can be only one), or a library's.
enum Target {
    Crate(Vec<usize>, bool),
    Library(Vec<String>),
}

/// The functions of the crate, or the library item, a call rustc resolved is a call of. A function of the crate is
/// found by its path; a method of an impl written in a module other than its type's, by its type and name. A method
/// called on a type parameter (`<T as Count>::count`), whose impl the caller does not choose, may be the method of any
/// impl of the trait in the crate, or, for a library's trait, is the trait's. A library's method is counted at the
/// level of its type's module, and of its trait's.
fn resolve_fact(c: &Callee, fns: &[Function], index: &BTreeMap<String, usize>, types: &BTreeSet<String>) -> Target {
    let local_self = c.self_ty.as_ref().is_some_and(|(_, l)| *l);
    let local_trait = c.trait_path.as_ref().is_some_and(|(_, l)| *l);
    if !(c.local || local_self || local_trait) {
        let mut paths = vec![c.path.clone()];
        if let Some((t, _)) = &c.trait_path {
            paths.push(format!("{t}::{}", c.name));
        }
        return Target::Library(paths);
    }
    if let Some(i) = index.get(&c.path) {
        return Target::Crate(vec![*i], true);
    }
    let head = c.self_ty.as_ref().map(|(t, _)| last_segment(t)).unwrap_or_default();
    let of_trait = |want: &str| -> Vec<usize> {
        fns.iter().enumerate().filter(|(_, f)| f.trait_name.as_deref() == Some(want) && short(&f.path) == c.name).map(|(i, _)| i).collect()
    };
    if local_self && (!c.param || types.contains(&head)) {
        // a type of the crate: its method, wherever its impl is
        let hits: Vec<usize> = index.iter().filter(|(p, _)| p.ends_with(&format!("::{head}::{}", c.name))).map(|(_, i)| *i).collect();
        let one = hits.len() == 1;
        return Target::Crate(hits, one);
    }
    match &c.trait_path {
        // a type parameter's method: any impl of the crate's trait
        Some((t, true)) if c.param => Target::Crate(of_trait(&last_segment(t)), false),
        Some((t, false)) if c.param => Target::Library(vec![format!("{t}::{}", c.name)]),
        // the crate's trait, for a library's type: the crate's impls of it for that type
        Some((t, true)) => {
            let hits: Vec<usize> = of_trait(&last_segment(t)).into_iter().filter(|i| fns[*i].path.ends_with(&format!("::{head}::{}", c.name))).collect();
            let one = hits.len() == 1;
            Target::Crate(hits, one)
        }
        _ => Target::Crate(vec![], false),
    }
}

fn short(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

/// The names of the types a bare body declares or implements: what a receiver MIR names by one segment may be.
fn type_names(n: Node, src: &str, out: &mut BTreeSet<String>) {
    if matches!(n.kind(), "struct_item" | "enum_item" | "type_item" | "union_item" | "trait_item") {
        if let Some(name) = n.child_by_field_name("name") {
            out.insert(text(name, src).to_string());
        }
    }
    if n.kind() == "impl_item" {
        if let Some(t) = n.child_by_field_name("type") {
            out.insert(last_segment(text(t, src)));
        }
    }
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        type_names(ch, src, out);
    }
}

/// The facts of a file, when rustc gave some: a file it gave none for (its crate did not build, or the file is not in
/// a target it read) is read from its syntax.
fn facts_of<'f>(facts: Option<&'f Facts>, file: &str) -> Option<&'f [Fact]> {
    facts.and_then(|m| m.get(file)).filter(|v| !v.is_empty()).map(Vec::as_slice)
}

/// What the bare bodies of a crate (`(file, bare body, constraint file)`) use above `data`, calls of the crate's own
/// functions among them, each at the level its callee needs, and imports of pinned modules. The calls are read from
/// the bodies' syntax: see `uses_with` for the calls rustc resolved.
pub fn uses(files: &[(String, String, Value)]) -> Vec<Use> {
    uses_with(files, None)
}

/// As `uses`, with the calls rustc resolved in the files it gave facts for (`crate::facts`), and the syntax's in the
/// others.
pub fn uses_with(files: &[(String, String, Value)], facts: Option<&Facts>) -> Vec<Use> {
    let (mut fns, mut imports) = (vec![], vec![]);
    let mut types = BTreeSet::new();
    for (file, brs, js) in files {
        read_file(file, brs, js, facts_of(facts, file), &mut fns, &mut imports);
        type_names(parse_bare(brs).root_node(), brs, &mut types);
    }
    let krate = files.first().map(|f| crate_name(&f.2)).unwrap_or_default();
    let index: BTreeMap<String, usize> = fns.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
    // a call rustc resolved into a library needs the level of the library's module, and one into a library the table
    // does not list needs `effects`
    let mut targets: Vec<Vec<Option<Callees>>> = vec![];
    let mut library: Vec<Vec<Use>> = vec![];
    for f in &fns {
        let (mut t, mut lib) = (vec![], vec![]);
        for (c, line) in &f.calls {
            let Call::Resolved(c) = c else {
                t.push(None);
                continue;
            };
            match resolve_fact(c, &fns, &index, &types) {
                Target::Crate(hits, one) => t.push(Some((hits, one))),
                Target::Library(paths) => {
                    t.push(Some((vec![], false)));
                    // a method of a trait needs the level of its type's module and of its trait's
                    let shown = &c.name;
                    if let Some(p) = paths.iter().find(|p| library_level(p).is_none()) {
                        lib.push(Use { level: 4, what: format!("`{shown}` (`{p}`), a library with no level in libraries/levels.json"), function: f.path.clone(), file: f.file.clone(), line: *line });
                    } else if let Some((l, p)) = paths.iter().filter_map(|p| Some((library_level(p)?, p))).max_by_key(|x| x.0).filter(|x| x.0 >= 2) {
                        lib.push(Use { level: l, what: format!("`{shown}` (`{p}`)"), function: f.path.clone(), file: f.file.clone(), line: *line });
                    }
                }
            }
        }
        targets.push(t);
        library.push(lib);
    }
    // each call's possible callees, with its line and whether it can be only one
    let calls: Vec<Vec<(Vec<usize>, usize, bool, String)>> = fns
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.calls
                .iter()
                .enumerate()
                .map(|(k, (c, line))| {
                    let (hits, one) = targets[i][k].clone().unwrap_or_else(|| resolve(c, &f.path, &krate, &index));
                    let shown = match c {
                        Call::Path(p) => p.clone(),
                        Call::Method { name, .. } => format!(".{name}"),
                        Call::Resolved(c) => c.name.clone(),
                    };
                    (hits, *line, one, shown)
                })
                .filter(|(hits, ..)| !hits.is_empty())
                .collect()
        })
        .collect();
    let edges: Vec<Vec<(usize, usize)>> = calls.iter().map(|cs| cs.iter().filter(|c| c.2).map(|c| (c.0[0], c.1)).collect()).collect();
    // recursion: a call of a function that reaches back to the caller
    let reaches = |from: usize, to: usize| -> bool {
        let (mut seen, mut stack) = (BTreeSet::new(), vec![from]);
        while let Some(x) = stack.pop() {
            if x == to {
                return true;
            }
            if seen.insert(x) {
                stack.extend(edges[x].iter().map(|e| e.0));
            }
        }
        false
    };
    let mut own: Vec<Vec<Use>> = fns.iter().zip(library).map(|(f, lib)| f.own.iter().cloned().chain(lib).collect()).collect();
    for (i, f) in fns.iter().enumerate() {
        for &(j, line) in &edges[i] {
            if reaches(j, i) {
                let what = if i == j { format!("a call of `{}` inside it", short(&f.path)) } else { format!("recursion through `{}`", short(&fns[j].path)) };
                own[i].push(Use { level: 2, what, function: f.path.clone(), file: f.file.clone(), line });
            }
        }
    }
    // each function needs the most of what it uses and of what its callees need: to a fixed point
    let mut level: Vec<usize> = own.iter().map(|u| u.iter().map(|u| u.level).max().unwrap_or(0)).collect();
    loop {
        let mut changed = false;
        for i in 0..fns.len() {
            for (hits, ..) in &calls[i] {
                let most = hits.iter().map(|j| level[*j]).max().unwrap_or(0);
                if most > level[i] {
                    level[i] = most;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut out = vec![];
    for (i, f) in fns.iter().enumerate() {
        out.extend(own[i].iter().cloned());
        for (hits, line, one, shown) in &calls[i] {
            let (j, most) = hits.iter().map(|j| (*j, level[*j])).max_by_key(|x| x.1).unwrap();
            if most >= 2 && !(hits.len() == 1 && j == i) {
                let which = if *one { format!("`{}`", short(&fns[j].path)) } else { format!("`{shown}` (it may be `{}`, which needs the most of the {} it may be)", fns[j].path, hits.len()) };
                out.push(Use { level: most, what: format!("a call of {which}, which needs `{}`,", LEVELS[most]), function: f.path.clone(), file: f.file.clone(), line: *line });
            }
        }
    }
    // an import of a pinned module, from outside it, needs that module's level
    for im in &imports {
        let path = im.path.strip_prefix("crate::").map(|c| format!("{krate}::{c}")).unwrap_or_else(|| im.path.clone());
        for (m, l) in &pinned(files) {
            if (path == *m || path.starts_with(&format!("{m}::"))) && !im.module.starts_with(m.as_str()) {
                out.push(Use { level: *l, what: format!("the import of `{}`, from module `{m}` at `{}`,", im.path, LEVELS[*l]), function: format!("{}::use", im.module), file: im.file.clone(), line: im.line });
            }
        }
    }
    out
}

/// The modules with a `level`, and their levels.
fn pinned(files: &[(String, String, Value)]) -> Vec<(String, usize)> {
    files
        .iter()
        .flat_map(|(_, _, js)| js["subjects"].as_array().into_iter().flatten())
        .filter(|s| s["kind"] == "module")
        .filter_map(|s| Some((s["name"].as_str()?.to_string(), rank(s["level"].as_str()?)?)))
        .collect()
}

fn in_module(u: &Use, m: &str) -> bool {
    u.function.starts_with(&format!("{m}::")) && !u.function.split("::").any(|seg| seg == "tests")
}

/// The modules with a `level` in the files of a crate (`(bare file, bare body, constraint file)`), and everything in
/// them that needs more: one message for each, naming the construct, where it is, the level it needs and the module's.
pub fn check(files: &[(String, String, Value)]) -> Vec<String> {
    check_with(files, None)
}

/// As `check`, with the calls rustc resolved (`uses_with`).
pub fn check_with(files: &[(String, String, Value)], facts: Option<&Facts>) -> Vec<String> {
    let pinned = pinned(files);
    let mut errs = vec![];
    for u in uses_with(files, facts) {
        for (m, level) in &pinned {
            if in_module(&u, m) && u.level > *level {
                errs.push(format!("{}:{}: {} in `{}` needs level `{}`, and module `{m}` is at `{}` (language 6.22)", u.file, u.line, u.what, u.function, LEVELS[u.level], LEVELS[*level]));
            }
        }
    }
    errs.sort();
    errs.dedup();
    errs
}

/// The level a module of the crate needs, the highest of the uses under it (`data` if none), with the first use that
/// needs it. For proposing a level to pin (language 6.22, harvested).
pub fn needed(files: &[(String, String, Value)], module: &str) -> (&'static str, Option<Use>) {
    needed_with(files, None, module)
}

/// As `needed`, with the calls rustc resolved (`uses_with`).
pub fn needed_with(files: &[(String, String, Value)], facts: Option<&Facts>, module: &str) -> (&'static str, Option<Use>) {
    let mut top: Option<Use> = None;
    for u in uses_with(files, facts) {
        if in_module(&u, module) && top.as_ref().is_none_or(|t| u.level > t.level) {
            top = Some(u);
        }
    }
    (LEVELS[top.as_ref().map_or(0, |u| u.level)], top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(brs: &str, subjects: Value) -> (String, String, Value) {
        ("src/m.brs".into(), brs.into(), json!({"crate": "c 0.0.0", "module": "src/m.rs", "subjects": subjects}))
    }

    fn pure(name: &str) -> Value {
        json!({"kind": "function", "name": name, "params": [], "clauses": [{"kind": "effects", "text": "pure", "source": "y/src/m.y:1"}]})
    }

    fn pin(f: &(String, String, Value), module: &str, level: &str) -> (String, String, Value) {
        let mut g = f.clone();
        g.2["subjects"].as_array_mut().unwrap().push(json!({"kind": "module", "name": module, "level": level}));
        g
    }

    #[test]
    fn each_construct_needs_its_level() {
        let brs = "mod sums {\n    fn total(xs) {\n        let t = 0;\n        for x in xs {\n            t += x;\n        }\n        t\n    }\n    fn twice(x) {\n        x * 2\n    }\n}\n";
        let f = file(brs, json!([pure("c::m::sums::total"), pure("c::m::sums::twice")]));
        let levels: Vec<(String, &str)> = uses(&[f.clone()]).into_iter().map(|u| (u.what, LEVELS[u.level])).collect();
        assert!(levels.contains(&("a loop".into(), "structured")));
        assert!(levels.contains(&("an assignment".into(), "structured")));
        assert_eq!(needed(&[f.clone()], "c::m::sums").0, "structured");
        // pinned at `calculations`, the loop and the assignment fail; at `structured`, nothing does
        let errs = check(&[pin(&f, "c::m::sums", "calculations")]);
        assert_eq!(errs.len(), 2, "{errs:#?}");
        assert!(errs.iter().any(|e| e.contains("a loop in `c::m::sums::total` needs level `structured`, and module `c::m::sums` is at `calculations`")), "{errs:#?}");
        assert!(check(&[pin(&f, "c::m::sums", "structured")]).is_empty());
        assert_eq!(check(&[pin(&f, "c::m::sums", "data")]).len(), 4);
    }

    #[test]
    fn shared_state_and_effects() {
        let brs = "use y_runtime::shared::{atomic, Step};\nfn count(sold) {\n    atomic(|tx| tx.get(sold))\n}\nfn show(console) {\n    console.write_line(\"hi\")\n}\nasync fn wait(w) {\n    w.next().await\n}\n";
        let show = json!({"kind": "function", "name": "c::m::show", "params": [{"name": "console", "type": "ref<target_type=Console>", "injected": true}]});
        let f = file(brs, json!([pure("c::m::count"), show, pure("c::m::wait")]));
        let found: Vec<(String, &str)> = uses(&[f]).into_iter().filter(|u| u.level >= 3).map(|u| (u.function, LEVELS[u.level])).collect();
        assert!(found.contains(&("c::m::count".into(), "shared")));
        // `show` is given the console; `wait` is async and awaits
        assert!(found.contains(&("c::m::show".into(), "effects")));
        assert_eq!(found.iter().filter(|(f, l)| f == "c::m::wait" && *l == "effects").count(), 2);
    }

    #[test]
    fn a_call_needs_what_its_callee_needs_and_recursion_is_structured() {
        // `price` is a calculation, but calls `total`, which loops; `even` and `odd` call each other
        let brs = "mod pricing {\n    fn price(xs) {\n        total(xs) * 2\n    }\n}\nmod sums {\n    fn total(xs) {\n        for x in xs {\n            x;\n        }\n    }\n    fn even(n) {\n        if n == 0 { true } else { odd(n - 1) }\n    }\n    fn odd(n) {\n        if n == 0 { false } else { even(n - 1) }\n    }\n}\n";
        let f = file(brs, json!([pure("c::m::pricing::price"), pure("c::m::sums::total"), pure("c::m::sums::even"), pure("c::m::sums::odd")]));
        let errs = check(&[pin(&f, "c::m::pricing", "calculations")]);
        assert_eq!(errs.len(), 1, "{errs:#?}");
        assert!(errs[0].contains("a call of `total`, which needs `structured`, in `c::m::pricing::price`"), "{errs:#?}");
        let found: Vec<String> = uses(&[f]).into_iter().filter(|u| u.function.ends_with("::even")).map(|u| u.what).collect();
        assert!(found.contains(&"recursion through `odd`".to_string()), "{found:#?}");
    }

    #[test]
    fn an_import_of_a_higher_module_fails() {
        let brs = "mod pricing {\n    use crate::m::console_play::show;\n    fn price(x) {\n        x * 2\n    }\n}\nmod console_play {\n    fn show(console) {\n        console.write_line(\"hi\")\n    }\n}\n";
        let show = json!({"kind": "function", "name": "c::m::console_play::show", "params": [{"name": "console", "type": "Console", "injected": true}]});
        let f = file(brs, json!([pure("c::m::pricing::price"), show]));
        let f = pin(&pin(&f, "c::m::pricing", "calculations"), "c::m::console_play", "effects");
        let errs = check(&[f]);
        assert_eq!(errs.len(), 1, "{errs:#?}");
        assert!(errs[0].contains("the import of `crate::m::console_play::show`, from module `c::m::console_play` at `effects`,"), "{errs:#?}");
    }

    #[test]
    fn a_modules_tests_are_not_held_to_its_level() {
        let brs = "mod tests {\n    fn t() {\n        loop {}\n    }\n}\n";
        let f = pin(&file(brs, json!([pure("c::m::tests::t")])), "c::m", "calculations");
        assert!(check(&[f]).is_empty());
    }

    #[test]
    fn method_calls_ambiguous_names_and_library_calls_count() {
        // `Game::play` loops; `show` calls it as a method; `start` calls `step`, which two modules define; `wait` calls
        // the runtime's deadline through its import
        let brs = "use y_runtime::Deadline;\nmod rules {\n    fn step(x) {\n        x\n    }\n}\nmod moves {\n    fn step(x) {\n        for y in x {\n            y;\n        }\n    }\n}\nmod board {\n    impl Game {\n        fn play(self) {\n            loop {}\n        }\n    }\n    fn show(game) {\n        game.play()\n    }\n    fn start(x) {\n        step(x)\n    }\n    fn wait() {\n        Deadline::after(FIVE)\n    }\n}\n";
        let f = file(brs, json!([pure("c::m::rules::step"), pure("c::m::moves::step"), pure("c::m::board::Game::play"), pure("c::m::board::show"), pure("c::m::board::start"), pure("c::m::board::wait")]));
        let all = uses(&[f]);
        let of = |f: &str| all.iter().filter(|u| u.function == format!("c::m::board::{f}")).map(|u| (u.what.clone(), LEVELS[u.level])).collect::<Vec<_>>();
        assert!(of("show").iter().any(|(w, l)| w.contains("a call of `play`") && *l == "structured"), "{:#?}", of("show"));
        assert!(of("start").iter().any(|(w, l)| w.contains("`step` (it may be `c::m::moves::step`") && *l == "structured"), "{:#?}", of("start"));
        assert!(of("wait").iter().any(|(w, l)| w.contains("`y_runtime::Deadline::after`") && *l == "effects"), "{:#?}", of("wait"));
    }

    #[test]
    fn a_receivers_type_picks_its_method_and_an_unknown_library_needs_effects() {
        // two types have `count`; `total`'s receiver is a `Tally`, whose `count` is a calculation, while `Ledger`'s loops
        let brs = "use rand::random;\nmod m2 {\n    impl Tally {\n        fn count(self) {\n            1\n        }\n    }\n    impl Ledger {\n        fn count(self) {\n            loop {}\n        }\n    }\n    fn total(tally) {\n        tally.count()\n    }\n    fn guess() {\n        random()\n    }\n}\n";
        let total = json!({"kind": "function", "name": "c::m::m2::total", "params": [{"name": "tally", "type": "ref<target_type=Tally>"}], "clauses": [{"kind": "effects", "text": "pure", "source": "y/src/m.y:1"}]});
        let f = file(brs, json!([pure("c::m::m2::Tally::count"), pure("c::m::m2::Ledger::count"), total, pure("c::m::m2::guess")]));
        let all = uses(&[f]);
        let of = |f: &str| all.iter().filter(|u| u.function == format!("c::m::m2::{f}")).map(|u| (u.what.clone(), LEVELS[u.level])).collect::<Vec<_>>();
        assert!(of("total").iter().all(|(_, l)| *l == "calculations"), "{:#?}", of("total"));
        assert!(of("guess").iter().any(|(w, l)| w.contains("`rand::random`), a library with no level") && *l == "effects"), "{:#?}", of("guess"));
    }

    /// A fact as rustc gives it: the call of `mir` (a path as MIR prints it) where `at` first stands in the body.
    fn fact(brs: &str, at: &str, mir: &str) -> Fact {
        let offset = brs.find(at).unwrap_or_else(|| panic!("{at}"));
        let externs = ["y_runtime".to_string(), "rand".to_string()].into_iter().collect();
        Fact { offset, line: crate::trace::line_col(brs, offset).0, callee: crate::facts::callee(mir, "c", &externs).unwrap() }
    }

    #[test]
    fn rustc_resolves_a_method_on_a_closures_parameter_to_its_impl() {
        // `t.count()` on a closure's parameter: its syntax names no type, so it may be `Ledger`'s, which loops; rustc
        // says it is `Tally`'s
        let brs = "mod counts {\n    impl Count for Tally {\n        fn count(self) {\n            self.0\n        }\n    }\n    impl Count for Ledger {\n        fn count(self) {\n            loop {}\n        }\n    }\n}\nmod sums {\n    fn total(xs) {\n        xs.iter().map(|t| t.count()).sum()\n    }\n}\n";
        let f = pin(&file(brs, json!([pure("c::m::counts::Tally::count"), pure("c::m::counts::Ledger::count"), pure("c::m::sums::total")])), "c::m::sums", "calculations");
        let errs = check(std::slice::from_ref(&f));
        assert!(errs.len() == 1 && errs[0].contains("a call of `.count` (it may be `c::m::counts::Ledger::count`"), "{errs:#?}");
        let facts: Facts = [(
            "src/m.brs".to_string(),
            vec![
                fact(brs, "iter()", "core::slice::<impl [m::counts::Tally]>::iter"),
                fact(brs, "map(", "<std::slice::Iter<'_, m::counts::Tally> as std::iter::Iterator>::map::<u32, {closure@src/m.rs:15:23: 15:26}>"),
                fact(brs, "count()", "<m::counts::Tally as m::counts::Count>::count"),
                fact(brs, "sum()", "<std::iter::Map<std::slice::Iter<'_, m::counts::Tally>, {closure@src/m.rs:15:23: 15:26}> as std::iter::Iterator>::sum::<u32>"),
            ],
        )]
        .into_iter()
        .collect();
        assert_eq!(check_with(std::slice::from_ref(&f), Some(&facts)), Vec::<String>::new());
        // called on a type parameter, it may be the method of any impl of the trait in the crate
        let generic: Facts = [("src/m.brs".to_string(), vec![fact(brs, "count()", "<T as m::counts::Count>::count")])].into_iter().collect();
        let errs = check_with(&[f], Some(&generic));
        assert!(errs.len() == 1 && errs[0].contains("a call of `count` (it may be `c::m::counts::Ledger::count`"), "{errs:#?}");
    }

    #[test]
    fn rustc_sees_a_reexported_runtime_function_where_it_is_defined() {
        // `start` is `y_runtime::task::detach` under another name: by its syntax, a function of the crate, which has none
        let brs = "mod rt {\n    use y_runtime::task::detach as start;\n}\nmod ticks {\n    fn begin(d) {\n        crate::m::rt::start(d, || 1)\n    }\n}\n";
        let f = pin(&file(brs, json!([pure("c::m::ticks::begin")])), "c::m::ticks", "calculations");
        assert_eq!(check(std::slice::from_ref(&f)), Vec::<String>::new());
        let facts: Facts = [("src/m.brs".to_string(), vec![fact(brs, "crate::m::rt::start", "y_runtime::task::detach::<i32, {closure@src/m.rs:6:32: 6:34}>")])].into_iter().collect();
        let errs = check_with(&[f], Some(&facts));
        assert!(errs.len() == 1 && errs[0].contains("`detach` (`y_runtime::task::detach`) in `c::m::ticks::begin` needs level `effects`"), "{errs:#?}");
        // a library the table does not list needs `effects`, however it is reached
        let unknown: Facts = [("src/m.brs".to_string(), vec![fact(brs, "crate::m::rt::start", "<rand::rngs::ThreadRng as rand::RngCore>::next_u32")])].into_iter().collect();
        let f2 = pin(&file(brs, json!([pure("c::m::ticks::begin")])), "c::m::ticks", "calculations");
        let errs = check_with(&[f2], Some(&unknown));
        assert!(errs.len() == 1 && errs[0].contains("(`rand::rngs::ThreadRng::next_u32`), a library with no level in libraries/levels.json"), "{errs:#?}");
    }
}
