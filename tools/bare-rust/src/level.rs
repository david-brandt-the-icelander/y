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
//! The library modules' levels are a table, `libraries/levels.json`, and a call is matched to it through the file's
//! `use` declarations: `atomic(…)` after `use y_runtime::shared::atomic;` is a call of `y_runtime::shared::atomic`.
//!
//! A module's level is the highest of anything it declares or imports (Y § 3.19): a call of a function of the crate
//! needs the level that function needs, so a `calculations` module that calls a `structured` function is
//! `structured`. A method call (`x.f()`), whose receiver's type the bare body does not say, may be any method `f` of
//! the crate's types (or, on `self`, its own type's), and a name the crate has more than one function for, out of
//! reach of the caller, may be any of them: such a call needs the most that any of them needs. An import (`use`) of a
//! module pinned at a higher level fails too.
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

/// The level a call of the library path `path` needs, by the longest module of the table it is under.
fn library_level(path: &str) -> Option<usize> {
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

/// A call as the bare body writes it: a path (`f`, `a::b::f`) or a method (`x.f`, kept as `.f` with its receiver).
#[derive(Clone)]
enum Call {
    Path(String),
    Method { name: String, receiver: String },
}

/// A function of the crate: its path, its file, what it uses itself, and the calls it makes, with their lines.
struct Function {
    path: String,
    file: String,
    own: Vec<Use>,
    calls: Vec<(Call, usize)>,
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

/// The functions and imports of one bare body.
fn read_file(file: &str, brs: &str, js: &Value, fns: &mut Vec<Function>, imports: &mut Vec<Import>) {
    let base = crate::weave::base_path(js);
    let tree = parse_bare(brs);
    let subject = |f: &str| js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "function" && s["name"] == f).cloned();
    let mut found: Vec<(String, Node)> = vec![];
    collect(tree.root_node(), brs, &base, &mut found, imports, file);
    for (path, node) in found {
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
        fns.push(Function { path: path.clone(), file: file.to_string(), own, calls });
    }
    // a call into a library module needs that module's level (`libraries/levels.json`), through the file's imports
    let uses = use_map(imports, file);
    for f in fns.iter_mut().filter(|f| f.file == file) {
        for (call, line) in &f.calls {
            if let Call::Path(p) = call {
                let first = p.split("::").next().unwrap_or("");
                let full = uses.get(first).map_or(p.clone(), |m| format!("{m}{}", &p[first.len()..]));
                if let Some(l) = library_level(&full).filter(|l| *l >= 2) {
                    f.own.push(Use { level: l, what: format!("`{p}` (`{full}`)"), function: f.path.clone(), file: file.to_string(), line: *line });
                }
            }
        }
    }
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
                            calls.push((Call::Method { name: text(m, src).to_string(), receiver: text(v, src).to_string() }, line));
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
        Call::Method { name, receiver } => {
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

fn short(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

/// What the bare bodies of a crate (`(file, bare body, constraint file)`) use above `data`, calls of the crate's own
/// functions among them, each at the level its callee needs, and imports of pinned modules.
pub fn uses(files: &[(String, String, Value)]) -> Vec<Use> {
    let (mut fns, mut imports) = (vec![], vec![]);
    for (file, brs, js) in files {
        read_file(file, brs, js, &mut fns, &mut imports);
    }
    let krate = files.first().map(|f| crate_name(&f.2)).unwrap_or_default();
    let index: BTreeMap<String, usize> = fns.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
    // each call's possible callees, with its line and whether it can be only one
    let calls: Vec<Vec<(Vec<usize>, usize, bool, String)>> = fns
        .iter()
        .map(|f| {
            f.calls
                .iter()
                .map(|(c, line)| {
                    let (hits, one) = resolve(c, &f.path, &krate, &index);
                    let shown = match c {
                        Call::Path(p) => p.clone(),
                        Call::Method { name, .. } => format!(".{name}"),
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
    let mut own: Vec<Vec<Use>> = fns.iter().map(|f| f.own.clone()).collect();
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
    let pinned = pinned(files);
    let mut errs = vec![];
    for u in uses(files) {
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
    let mut top: Option<Use> = None;
    for u in uses(files) {
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
}
