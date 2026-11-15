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
//! What a call calls is read one of two ways, and `uses_with` takes, function by function, the first it has:
//! - **From rustc** (`crate::facts`), when the woven crate builds: weave with cargo reads the crate's MIR, where every
//!   call names the item it resolved to. A function of the crate is found by its path; a method by its receiver's
//!   type, which MIR states (`<Tally as Count>::count` is `Tally`'s `count`, whatever names the receiver: a closure's
//!   parameter, a chained call); a library's function by where it is defined, so a re-export, a renamed import or a
//!   local that holds the function (`let run = atomic;`) is the function itself. A trait's method called on a type
//!   parameter may be any impl of the trait in the crate, and needs the most of them; a library's trait method needs
//!   the level of its type's module and of its trait's.
//!
//!   A call through a function pointer, a `dyn Fn…` or a type parameter bound by `Fn…` names no function in MIR, only
//!   the types of its arguments. It may be any function the crate names as a value (passes, binds, makes a pointer or a
//!   `dyn Fn`) or any closure the crate writes that takes those types (a type parameter takes any), and needs the most
//!   of them (`analyse`). A closure is read on its own for this, as `function::{closure@line:col}`; what it uses is its
//!   function's already, and is reported there.
//!
//!   rustc's MIR is read for every target of the crate (library, binaries, integration tests), for the library and
//!   binaries again with their unit tests (`cfg(test)`), and in every build whose check passed (`crate::builds`: its
//!   target, features, `--cfg` and profile), so code that only a test or another build compiles is read from rustc too.
//! - **From the syntax**, when there is no build (a unit test, `bare-rust levels` without `--crate`, a crate that does
//!   not compile), and for a function rustc said nothing of (one that no build compiles, or that makes no call): a call is matched through the file's `use` declarations (`atomic(…)` after
//!   `use y_runtime::shared::atomic;` is `y_runtime::shared::atomic`). A method call whose receiver's type the
//!   constraint data does not give (a parameter or a typed local) may be any method of that name of the crate's types
//!   (or, on `self`, its own type's), and a name the crate has more than one function for, out of reach of the
//!   caller, may be any of them: such a call needs the most that any of them needs. A re-export, a field that holds a
//!   function and a local alias are not followed.
//!
//! Weave says which way it read them (`CrateWeave::checks_via`). Either way, what is not a call (a loop, `&mut`, a
//! marker, a parameter's type) is read from the syntax and the constraint data.
//!
//! A macro is read from the syntax either way, as rustc places the calls of a macro's expansion in the library's
//! source, not at the invocation: an invocation needs the level the table gives the macro's path (`std::println` is
//! `effects`), named through the file's imports or, alone, as the prelude's (`macro_level`), and one the table cannot
//! place needs `effects`. The calls in a macro's arguments are the body's: rustc places them in the bare file, and the
//! syntax reads the arguments as a call's (`macro_arguments`).
//!
//! A macro the crate defines (`macro_rules! name`) is read as a function of the crate, `module::name!`, whose body is
//! what its rules expand to: rustc places the calls of its expansions in its definition, in the bare file, and the
//! syntax reads each rule's right side as code (`macro_body`, with `$x` as `x` and a repetition as a block of what it
//! repeats). An invocation of it is a call of it, and needs what it needs; one that expands to itself is not recursion
//! at run time. Its body counts in the module it is defined in, as a function's does. What a rule's right side builds
//! from tokens it is given (`$body` that is code) is the invocation's, through the arguments.
use crate::facts::{Callee, Fact, Facts, Kind};
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
    /// An invocation of a macro the crate defines, by its name.
    Macro(String),
    /// A call of a closure, as rustc resolved it: where the closure is written (bare file, offset).
    Closure((String, usize)),
}

/// A fact as a call: one of a closure is the closure's, by where it is written.
fn fact_call(f: &Fact) -> Call {
    match (&f.callee.kind, &f.def) {
        (Kind::CallClosure, Some(def)) => Call::Closure(def.clone()),
        _ => Call::Resolved(f.callee.clone()),
    }
}

/// A function of the crate: its path, its file, what it uses itself, and the calls it makes, with their lines; and,
/// for a method of an impl of a trait, the trait's name.
struct Function {
    path: String,
    file: String,
    own: Vec<Use>,
    calls: Vec<(Call, usize)>,
    trait_name: Option<String>,
    /// The macros it invokes, by the path it names them by (`println`, `y_collections::list`), with their lines.
    macros: Vec<(String, usize)>,
    /// Whether its calls are the ones rustc resolved: rustc gave facts for its file, and some for it.
    resolved: bool,
    /// Whether it is a macro the crate defines (`macro_rules! name`, kept as `module::name!`): what its rules expand to.
    is_macro: bool,
    /// For a closure (`function::{closure@line:col}`): where it is written, as its bare file and the offset it starts
    /// at. A closure is read on its own only as what a call through a pointer or an `Fn` bound may call: what it uses is
    /// its function's already, and is reported there.
    closure: Option<(String, usize)>,
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
fn read_file(file: &str, brs: &str, js: &Value, facts: Option<&[Fact]>, crate_macros: &BTreeSet<String>, fns: &mut Vec<Function>, imports: &mut Vec<Import>) {
    let base = crate::weave::base_path(js);
    let tree = parse_bare(brs);
    let subject = |f: &str| js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "function" && s["name"] == f).cloned();
    let mut found: Vec<(String, Node)> = vec![];
    collect(tree.root_node(), brs, &base, &mut found, imports, file);
    let first = fns.len();
    let mut resolved: Vec<Vec<(Call, usize)>> = vec![vec![]; found.len()];
    for f in facts.into_iter().flatten() {
        if let Some(i) = enclosing(&found, f.offset) {
            resolved[i].push((fact_call(f), f.line));
        }
    }
    for (k, (path, node)) in found.iter().enumerate() {
        let (path, node) = (path.clone(), *node);
        let line = node.start_position().row + 1;
        let mut own = vec![];
        let mut add = |level: &str, what: String, line: usize| own.push(Use { level: rank(level).unwrap(), what, function: path.clone(), file: file.to_string(), line });
        let is_macro = node.kind() == "macro_definition";
        if is_macro {
            // what each rule expands to, read as a function's body
            let (mut calls, mut macros) = (vec![], vec![]);
            let mut c = node.walk();
            for rule in node.named_children(&mut c).filter(|r| r.kind() == "macro_rule") {
                if let Some(tt) = rule.child_by_field_name("right") {
                    macro_body(tt, brs, &mut add, &mut calls, &mut macros);
                }
            }
            let from_rustc = facts.is_some() && !resolved[k].is_empty();
            if from_rustc {
                calls = std::mem::take(&mut resolved[k]);
            }
            fns.push(Function { path: path.clone(), file: file.to_string(), own, calls, trait_name: None, macros, resolved: from_rustc, is_macro, closure: None });
            continue;
        }
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
        let (mut calls, mut macros) = (vec![], vec![]);
        if let Some(body) = node.child_by_field_name("body") {
            walk(body, brs, 0, &mut add, &mut calls, &mut macros);
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
        // the calls rustc resolved in it, if it gave some: a function of a file it read that no build compiles (or that
        // makes no call) keeps the calls its syntax shows
        let from_rustc = facts.is_some() && !resolved[k].is_empty();
        if from_rustc {
            calls = std::mem::take(&mut resolved[k]);
        }
        let trait_name = node.parent().and_then(|p| p.parent()).filter(|p| p.kind() == "impl_item").and_then(|p| p.child_by_field_name("trait")).map(|t| last_segment(text(t, brs)));
        fns.push(Function { path: path.clone(), file: file.to_string(), own, calls, trait_name, macros, resolved: from_rustc, is_macro, closure: None });
    }
    // each closure of a function, read on its own: what a call through a pointer or an `Fn` bound may call
    for (path, node) in found.iter().filter(|(_, n)| n.kind() == "function_item") {
        let mut stack = vec![];
        if let Some(body) = node.child_by_field_name("body") {
            stack.push(body);
        }
        while let Some(n) = stack.pop() {
            if n.kind() == "function_item" {
                continue;
            }
            if n.kind() == "closure_expression" {
                let (l, c) = (n.start_position().row + 1, n.start_position().column + 1);
                let cpath = format!("{path}::{{closure@{l}:{c}}}");
                let mut own = vec![];
                let mut add = |level: &str, what: String, line: usize| own.push(Use { level: rank(level).unwrap(), what, function: cpath.clone(), file: file.to_string(), line });
                let (mut calls, mut macros) = (vec![], vec![]);
                if let Some(body) = n.child_by_field_name("body") {
                    walk(body, brs, 0, &mut add, &mut calls, &mut macros);
                }
                let inside: Vec<(Call, usize)> = facts.into_iter().flatten().filter(|f| f.offset >= n.start_byte() && f.offset < n.end_byte()).map(|f| (fact_call(f), f.line)).collect();
                let from_rustc = facts.is_some() && !inside.is_empty();
                if from_rustc {
                    calls = inside;
                }
                fns.push(Function { path: cpath, file: file.to_string(), own, calls, trait_name: None, macros, resolved: from_rustc, is_macro: false, closure: Some((file.to_string(), n.start_byte())) });
            }
            let mut c = n.walk();
            stack.extend(n.named_children(&mut c));
        }
    }
    // a macro needs the level of the library it is from, read from its invocation in either way (`macro_level`); one the
    // crate defines is called, as a function is, and needs what its rules expand to
    let uses = use_map(imports, file);
    for f in fns[first..].iter_mut() {
        for (m, line) in f.macros.clone() {
            let name = ["::", "$crate::", "crate::", "self::"].iter().fold(m.as_str(), |n, p| n.strip_prefix(p).unwrap_or(n));
            if !name.contains("::") && crate_macros.contains(name) && !uses.contains_key(name) {
                f.calls.push((Call::Macro(name.to_string()), line));
            } else if let Some((l, what)) = macro_level(&m, &uses, crate_macros) {
                f.own.push(Use { level: l, what, function: f.path.clone(), file: file.to_string(), line });
            }
        }
    }
    // a call into a library needs its module's level (`libraries/levels.json`), through the file's imports; a call into a
    // library the table does not know needs `effects`, as nothing says it has none
    let local = local_names(&tree.root_node(), brs, &base);
    // what rustc resolved needs no imports to follow: its library calls are counted with the crate's (`uses_with`)
    for f in fns[first..].iter_mut().filter(|f| !f.resolved) {
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

/// The macros of the standard library's prelude: a macro named by one of these alone is `std`'s.
const STD_MACROS: &[&str] = &[
    "assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne", "format", "format_args", "vec", "write", "writeln",
    "print", "println", "eprint", "eprintln", "dbg", "panic", "unreachable", "unimplemented", "todo", "matches", "concat", "stringify", "include",
    "include_str", "include_bytes", "env", "option_env", "line", "column", "file", "module_path", "cfg", "compile_error", "thread_local",
];

/// The names of the macros a bare body defines (`macro_rules! name`).
fn macro_names(n: Node, src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if n.kind() == "macro_definition" {
        if let Some(name) = n.child_by_field_name("name") {
            out.insert(text(name, src).to_string());
        }
    }
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        out.extend(macro_names(ch, src));
    }
    out
}

/// The level a macro's invocation needs, and what to call it, when above `calculations`: the level of the library it
/// is from (`libraries/levels.json`, where `std::println` is `effects`). The macro is named through the file's imports
/// (`list!` after `use y_collections::list;`), by its path, or, alone, as one of the prelude's (`STD_MACROS`); one the
/// crate defines is not a library's, and one that is none of these needs `effects`, as nothing says it has none.
/// rustc says nothing of a macro (its expansion's calls are in the library's source), so this is read from the syntax
/// in either way the calls are read; the calls in its arguments are the body's.
fn macro_level(m: &str, uses: &BTreeMap<String, String>, defined: &BTreeSet<String>) -> Option<(usize, String)> {
    let m = m.trim_start_matches("::").trim_start_matches("$crate::");
    let first = m.split("::").next().unwrap_or("");
    let full = match uses.get(first) {
        Some(p) => format!("{p}{}", &m[first.len()..]),
        None if m.contains("::") => m.to_string(),
        None if defined.contains(m) => return None,
        None if STD_MACROS.contains(&m) => format!("std::{m}"),
        None => return Some((4, format!("the macro `{m}!`, from a library with no level in libraries/levels.json"))),
    };
    let full = ["core::", "alloc::"].iter().find_map(|r| full.strip_prefix(r)).map_or(full.clone(), |rest| format!("std::{rest}"));
    match library_level(&full) {
        Some(l) if l >= 2 => Some((l, format!("the macro `{m}!` (`{full}`)"))),
        Some(_) => None,
        None => Some((4, format!("the macro `{m}!` (`{full}`), from a library with no level in libraries/levels.json"))),
    }
}

/// What one rule of a macro the crate defines expands to (its right side), read as a function's body: a metavariable
/// (`$x`) stands for a name, a repetition (`$( … ),*`) for what it repeats, and `$crate` for `crate`. What does not
/// parse after that is not read.
fn macro_body(tt: Node, src: &str, add: &mut impl FnMut(&str, String, usize), calls: &mut Vec<(Call, usize)>, macros: &mut Vec<(String, usize)>) {
    let t = text(tt, src);
    if t.len() < 2 {
        return;
    }
    let inner = unrepeat(&t[1..t.len() - 1].replace("$crate", "crate"));
    let frag = format!("fn __m() {{ {inner} }}");
    let tree = parse_bare(&frag);
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.kind() == "function_item" {
            if let Some(body) = n.child_by_field_name("body") {
                let mut c = body.walk();
                for ch in body.children(&mut c) {
                    walk(ch, &frag, tt.start_position().row, add, calls, macros);
                }
            }
            return;
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
    }
}

/// A macro rule's right side as code: `$( … )` with its separator and operator (`$( f($x); )*`, `$( $x ),+`) as a
/// block of what it repeats, and `$x` as `x`. Lines stay where they were.
fn unrepeat(t: &str) -> String {
    let b: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '$' && b.get(i + 1) == Some(&'(') {
            // the group, to its matching `)`
            let (mut depth, mut j) = (0, i + 1);
            while j < b.len() {
                match b[j] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            out.push('{');
            out.push_str(&unrepeat(&b[i + 2..j.min(b.len())].iter().collect::<String>()));
            out.push('}');
            i = j + 1;
            // a separator and the operator: `,*`, `;+`, `*`
            if b.get(i).is_some_and(|c| !"*+?".contains(*c) && !c.is_whitespace()) && b.get(i + 1).is_some_and(|c| "*+?".contains(*c)) {
                i += 2;
            } else if b.get(i).is_some_and(|c| "*+?".contains(*c)) {
                i += 1;
            }
            continue;
        }
        if b[i] != '$' {
            out.push(b[i]);
        }
        i += 1;
    }
    out
}

/// The arguments of a macro, read as the arguments of a call (`f(a, b)`), so that the calls in them are seen: a
/// format string, expressions, `name = value` (whose value is read), and `x; n` (as `x, n`). What does not parse as
/// arguments is not read.
fn macro_arguments(tt: Node, src: &str, base: usize, add: &mut impl FnMut(&str, String, usize), calls: &mut Vec<(Call, usize)>, macros: &mut Vec<(String, usize)>) {
    let t = text(tt, src);
    if t.len() < 2 || !(t.starts_with('(') || t.starts_with('[')) {
        return;
    }
    let mut depth = 0;
    let inner: String = t[1..t.len() - 1]
        .chars()
        .map(|c| {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            }
            if c == ';' && depth == 0 { ',' } else { c }
        })
        .collect();
    let frag = format!("fn __m() {{ __m({inner}); }}");
    let tree = parse_bare(&frag);
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.kind() == "call_expression" && n.child_by_field_name("function").is_some_and(|f| text(f, &frag) == "__m") {
            let Some(args) = n.child_by_field_name("arguments") else { return };
            let mut c = args.walk();
            for a in args.named_children(&mut c) {
                // `name = value` names a format argument: only its value is code
                let a = if a.kind() == "assignment_expression" { a.child_by_field_name("right").unwrap_or(a) } else { a };
                walk(a, &frag, base + tt.start_position().row, add, calls, macros);
            }
            return;
        }
        let mut c = n.walk();
        stack.extend(n.named_children(&mut c));
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
            "macro_definition" => {
                if let Some(name) = ch.child_by_field_name("name") {
                    fns.push((format!("{path}::{}!", text(name, src)), ch));
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

/// What a function's body uses, from its syntax: `base` is the line the text `src` starts at (0 for a file; a macro's
/// arguments are read from a text of their own).
fn walk(n: Node, src: &str, base: usize, add: &mut impl FnMut(&str, String, usize), calls: &mut Vec<(Call, usize)>, macros: &mut Vec<(String, usize)>) {
    let line = base + n.start_position().row + 1;
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
        "macro_invocation" => {
            if let Some(m) = n.child_by_field_name("macro") {
                macros.push((text(m, src).to_string(), line));
            }
            let mut c = n.walk();
            if let Some(tt) = n.named_children(&mut c).find(|c| c.kind() == "token_tree") {
                macro_arguments(tt, src, base, add, calls, macros);
            }
            return;
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
        walk(ch, src, base, add, calls, macros);
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
        Call::Resolved(_) | Call::Closure(_) => (vec![], false),
        Call::Macro(name) => {
            let hits: Vec<usize> = fns.iter().filter(|(p, _)| p.ends_with(&format!("::{name}!"))).map(|(_, i)| *i).collect();
            let one = hits.len() == 1;
            (hits, one)
        }
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
    analyse(files, facts).uses
}

/// What a crate's bodies use (`uses_with`), and the argument types of everything a call through a function pointer or
/// an `Fn` bound may call: for the region check (`crate::region::check_body_with`), which holds a region to the whole
/// crate's.
pub struct CrateLevels {
    pub uses: Vec<Use>,
    pub candidates: Vec<Vec<String>>,
}

impl CrateLevels {
    /// How many functions and closures of the crate a call through a pointer or an `Fn` bound taking `args` may be.
    pub fn candidates_for(&self, args: &[String]) -> usize {
        self.candidates.iter().filter(|a| fits(args, a)).count()
    }
}

/// Whether a candidate that takes `have` fits a call that passes `want`: the same types, a type parameter (`_`) fitting
/// any.
fn fits(want: &[String], have: &[String]) -> bool {
    want.len() == have.len() && want.iter().zip(have).all(|(a, b)| a == b || a == "_" || b == "_")
}

/// `uses_with` and the candidates of indirect calls, together (`CrateLevels`).
pub fn analyse(files: &[(String, String, Value)], facts: Option<&Facts>) -> CrateLevels {
    let (mut fns, mut imports) = (vec![], vec![]);
    let mut types = BTreeSet::new();
    let crate_macros: BTreeSet<String> = files.iter().flat_map(|(_, brs, _)| macro_names(parse_bare(brs).root_node(), brs)).collect();
    for (file, brs, js) in files {
        read_file(file, brs, js, facts_of(facts, file), &crate_macros, &mut fns, &mut imports);
        type_names(parse_bare(brs).root_node(), brs, &mut types);
    }
    let krate = files.first().map(|f| crate_name(&f.2)).unwrap_or_default();
    let index: BTreeMap<String, usize> = fns.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
    // a call rustc resolved into a library needs the level of the library's module, and one into a library the table
    // does not list needs `effects`
    let mut targets: Vec<Vec<Option<Callees>>> = vec![];
    let mut library: Vec<Vec<Use>> = vec![];
    // what a call through a pointer or an `Fn` bound may call: the functions named as values, and the closures, by the
    // types of their arguments
    let closures: BTreeMap<(String, usize), usize> = fns.iter().enumerate().filter_map(|(i, f)| Some((f.closure.clone()?, i))).collect();
    let mut values: Vec<(Vec<String>, Callee)> = vec![];
    let mut closure_args: Vec<(Vec<String>, usize)> = vec![];
    for f in &fns {
        for (c, _) in &f.calls {
            if let Call::Resolved(c @ Callee { kind: Kind::Value(args), .. }) = c {
                values.push((args.clone(), c.clone()));
            }
        }
    }
    for (file, list) in facts.into_iter().flatten() {
        for x in list {
            if let (Kind::Closure(args), Some(def)) = (&x.callee.kind, &x.def) {
                if let Some(i) = closures.get(&(file.clone(), def.1)).or_else(|| closures.get(def)) {
                    closure_args.push((args.clone(), *i));
                }
            }
        }
    }
    for f in &fns {
        let (mut t, mut lib) = (vec![], vec![]);
        for (c, line) in &f.calls {
            if let Call::Closure(def) = c {
                t.push(Some((closures.get(def).into_iter().copied().collect(), true)));
                continue;
            }
            let Call::Resolved(c) = c else {
                t.push(None);
                continue;
            };
            match &c.kind {
                Kind::Closure(_) => {
                    // where a closure is written: its body is the function's already
                    t.push(Some((vec![], false)));
                    continue;
                }
                Kind::CallClosure => {
                    // a closure rustc could not place: nothing to follow
                    t.push(Some((vec![], false)));
                    continue;
                }
                Kind::Indirect(args) => {
                    // any closure, or function named as a value, that takes these arguments
                    let mut hits: Vec<usize> = closure_args.iter().filter(|(a, _)| fits(args, a)).map(|(_, i)| *i).collect();
                    for (_, v) in values.iter().filter(|(a, _)| fits(args, a)) {
                        match resolve_fact(v, &fns, &index, &types) {
                            Target::Crate(h, _) => hits.extend(h),
                            Target::Library(paths) => {
                                if let Some((l, p)) = paths.iter().map(|p| (library_level(p).unwrap_or(4), p)).max_by_key(|x| x.0).filter(|x| x.0 >= 2) {
                                    lib.push(Use { level: l, what: format!("a call through a function pointer or an `Fn` bound, which may be `{p}`"), function: f.path.clone(), file: f.file.clone(), line: *line });
                                }
                            }
                        }
                    }
                    hits.sort_unstable();
                    hits.dedup();
                    t.push(Some((hits, false)));
                    continue;
                }
                _ => {}
            }
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
                        Call::Resolved(Callee { kind: Kind::Indirect(_), .. }) => "a function pointer or an `Fn` bound".to_string(),
                        Call::Resolved(Callee { kind: Kind::CallClosure, .. }) => "a closure".to_string(),
                        Call::Resolved(c) => c.name.clone(),
                        Call::Macro(m) => format!("{m}!"),
                        Call::Closure(_) => "a closure".to_string(),
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
            // a macro that expands to itself is not recursion at run time, and a closure's recursion is its function's
            if !f.is_macro && !fns[j].is_macro && f.closure.is_none() && fns[j].closure.is_none() && reaches(j, i) {
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
    // a closure is reported in its function, which uses all it uses
    for (i, f) in fns.iter().enumerate().filter(|(_, f)| f.closure.is_none()) {
        out.extend(own[i].iter().cloned());
        for (hits, line, one, shown) in &calls[i] {
            let (j, most) = hits.iter().map(|j| (*j, level[*j])).max_by_key(|x| x.1).unwrap();
            // a closure the function writes and calls is in its body already
            let own_closure = hits.len() == 1 && fns[j].closure.is_some() && fns[j].path.starts_with(&format!("{}::{{closure", f.path));
            if most >= 2 && !(hits.len() == 1 && j == i) && !own_closure {
                let which = if *one {
                    format!("`{}`", short(&fns[j].path))
                } else if shown.starts_with("a function pointer") {
                    format!("{shown} (it may be `{}`, which needs the most of the {} it may be)", fns[j].path, hits.len())
                } else {
                    format!("`{shown}` (it may be `{}`, which needs the most of the {} it may be)", fns[j].path, hits.len())
                };
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
    let candidates = values.into_iter().map(|(a, _)| a).chain(closure_args.into_iter().map(|(a, _)| a)).collect();
    CrateLevels { uses: out, candidates }
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
        Fact { offset, line: crate::trace::line_col(brs, offset).0, callee: crate::facts::callee(mir, "c", &externs).unwrap(), def: None }
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
    fn a_macro_needs_the_level_of_its_library_and_its_arguments_are_read() {
        // `show` prints, which is an effect; `label` formats, which is a calculation, but calls `total`, which loops,
        // in the format's arguments (as a named argument, which is not an assignment)
        let brs = "use y_collections::list;\nmod sums {\n    fn total(xs) {\n        for x in xs {\n            x;\n        }\n    }\n}\nmod show {\n    macro_rules! twice {\n        ($x:expr) => { $x * 2 };\n    }\n    fn show(x) {\n        println!(\"{}\", x)\n    }\n    fn label(xs) {\n        format!(\"{n}\", n = crate::m::sums::total(xs))\n    }\n    fn plain(x) {\n        let v = vec![x; 3];\n        let l = list![x, twice!(x)];\n        assert!(v.len() == 3, \"{}\", x);\n        v\n    }\n    fn odd(x) {\n        sparkle!(x)\n    }\n}\n";
        let f = file(brs, json!([pure("c::m::sums::total"), pure("c::m::show::show"), pure("c::m::show::label"), pure("c::m::show::plain"), pure("c::m::show::odd")]));
        let all = uses(std::slice::from_ref(&f));
        let of = |g: &str| all.iter().filter(|u| u.function == format!("c::m::show::{g}") && u.level >= 2).map(|u| (u.what.clone(), LEVELS[u.level], u.line)).collect::<Vec<_>>();
        assert_eq!(of("show"), vec![("the macro `println!` (`std::println`)".to_string(), "effects", 14)]);
        assert_eq!(of("label"), vec![("a call of `total`, which needs `structured`,".to_string(), "structured", 17)]);
        // the standard library's `vec!` and `assert!`, an imported `list!` and the crate's own `twice!` are calculations
        assert_eq!(of("plain"), vec![]);
        assert_eq!(of("odd"), vec![("the macro `sparkle!`, from a library with no level in libraries/levels.json".to_string(), "effects", 26)]);
        // with what rustc resolved, the macro is still read from the syntax
        let facts: Facts = [("src/m.brs".to_string(), vec![fact(brs, "for x", "<std::ops::Range<u32> as std::iter::IntoIterator>::into_iter")])].into_iter().collect();
        let errs = check_with(&[pin(&f, "c::m::show", "calculations")], Some(&facts));
        assert!(errs.iter().any(|e| e.contains("the macro `println!` (`std::println`) in `c::m::show::show` needs level `effects`")), "{errs:#?}");
    }

    #[test]
    fn a_function_rustc_said_nothing_of_keeps_the_calls_its_syntax_shows() {
        // rustc read `shown` (a build compiles it) and not `gated` (no build does): `gated`'s call of `rand::random` is
        // still read from its syntax, through the file's import
        let brs = "use rand::random;\nmod m2 {\n    fn shown(x) {\n        twice(x)\n    }\n    fn twice(x) {\n        x\n    }\n    fn gated() {\n        random()\n    }\n}\n";
        let f = pin(&file(brs, json!([pure("c::m::m2::shown"), pure("c::m::m2::twice"), pure("c::m::m2::gated")])), "c::m::m2", "calculations");
        let facts: Facts = [("src/m.brs".to_string(), vec![fact(brs, "twice(x)", "m::m2::twice")])].into_iter().collect();
        let errs = check_with(&[f], Some(&facts));
        assert!(errs.len() == 1 && errs[0].contains("`random` (`rand::random`), a library with no level in libraries/levels.json in `c::m::m2::gated`"), "{errs:#?}");
    }

    #[test]
    fn a_macro_the_crate_defines_is_followed_into_its_body() {
        // `count_up!` loops, `shout!` prints, `price!` calls a function that loops, `munch!` expands to itself, and
        // `scaled!` multiplies, in a group that is not a repetition
        let brs = "macro_rules! count_up {\n    ($n:expr) => {{\n        let i = 0;\n        while i < $n {\n            i += 1;\n        }\n        i\n    }};\n}\nmacro_rules! shout {\n    ($($x:expr),+) => {\n        $( println!(\"{}\", $x); )+\n    };\n}\nmacro_rules! price {\n    ($x:expr) => {\n        $crate::m::sums::total($x)\n    };\n}\nmacro_rules! munch {\n    () => { 0 };\n    ($x:tt $($rest:tt)*) => { 1 + munch!($($rest)*) };\n}\nmacro_rules! scaled {\n    ($x:expr) => { ($x)*2 };\n}\nmod sums {\n    fn total(xs) {\n        for x in xs {\n            x;\n        }\n    }\n}\nmod calc {\n    fn a(n) {\n        count_up!(n)\n    }\n    fn b(n) {\n        shout!(n, n)\n    }\n    fn c(xs) {\n        price!(xs)\n    }\n    fn d() {\n        munch!(a b c) + scaled!(3)\n    }\n}\n";
        let names = ["sums::total", "calc::a", "calc::b", "calc::c", "calc::d"];
        let f = pin(&file(brs, json!(names.iter().map(|n| pure(&format!("c::m::{n}"))).collect::<Vec<_>>())), "c::m::calc", "calculations");
        let errs = check(std::slice::from_ref(&f));
        let of = |g: &str| errs.iter().filter(|e| e.contains(&format!("in `c::m::calc::{g}`"))).cloned().collect::<Vec<_>>();
        assert!(of("a").len() == 1 && of("a")[0].contains("a call of `count_up!`, which needs `structured`"), "{errs:#?}");
        assert!(of("b").len() == 1 && of("b")[0].contains("a call of `shout!`, which needs `effects`"), "{errs:#?}");
        assert!(of("c").len() == 1 && of("c")[0].contains("a call of `price!`, which needs `structured`"), "{errs:#?}");
        assert_eq!(of("d"), Vec::<String>::new());
        // what a macro's own body uses is at its line, in the macro
        let all = uses(std::slice::from_ref(&f));
        assert!(all.iter().any(|u| u.function == "c::m::count_up!" && u.what == "a loop" && u.line == 4), "{all:#?}");
        assert!(all.iter().any(|u| u.function == "c::m::shout!" && u.what.contains("`println!`") && u.line == 12), "{all:#?}");
        // with what rustc resolved in an expansion: a call in the macro's body is the macro's
        let brs2 = "macro_rules! later {\n    ($d:expr) => { crate::m::rt::start($d, || 1) };\n}\nmod calc {\n    fn e(d) {\n        later!(d)\n    }\n}\n";
        let f2 = pin(&file(brs2, json!([pure("c::m::calc::e")])), "c::m::calc", "calculations");
        assert_eq!(check(std::slice::from_ref(&f2)), Vec::<String>::new());
        let facts: Facts = [("src/m.brs".to_string(), vec![fact(brs2, "crate::m::rt::start", "y_runtime::task::detach::<i32, {closure@src/m.rs:2:46: 2:48}>")])].into_iter().collect();
        let errs = check_with(&[f2], Some(&facts));
        assert!(errs.len() == 1 && errs[0].contains("a call of `later!`, which needs `effects`, in `c::m::calc::e`"), "{errs:#?}");
        assert_eq!(unrepeat("$( f($x); )* ($y)*2 $( $z ),+"), "{ f(x); } (y)*2 { z }");
    }

    /// A fact of a kind other than a plain call: `def_at`, where the closure it is of or calls is written.
    fn kind_fact(brs: &str, at: &str, kind: Kind, path: &str, def_at: Option<&str>) -> Fact {
        let offset = brs.find(at).unwrap_or_else(|| panic!("{at}"));
        let mut callee = if path.is_empty() { crate::facts::callee("x", "c", &Default::default()).unwrap() } else { crate::facts::callee(path, "c", &Default::default()).unwrap() };
        callee.kind = kind;
        let def = def_at.map(|d| ("src/m.brs".to_string(), brs.find(d).unwrap()));
        Fact { offset, line: crate::trace::line_col(brs, offset).0, callee, def }
    }

    #[test]
    fn a_call_through_a_pointer_or_an_fn_bound_may_be_any_value_that_fits() {
        // `apply` calls `f`, a pointer; what may be passed are `calm` and the closure `slow`, which take a `u32`, and the
        // closure `named`, which takes a `&str` and does not fit
        let brs = "mod ops {\n    fn calm(x) {\n        x\n    }\n    fn apply(f) {\n        f(1)\n    }\n}\nmod main_m {\n    fn run() {\n        let slow = |n| {\n            loop {}\n        };\n        let named = |s| {\n            for c in s {\n                c;\n            }\n        };\n        crate::m::ops::apply(crate::m::ops::calm) + slow(2) + named(\"x\")\n    }\n}\n";
        let f = pin(&file(brs, json!([pure("c::m::ops::calm"), pure("c::m::ops::apply"), pure("c::m::main_m::run")])), "c::m::ops", "calculations");
        // from the syntax alone, `f(1)` names no function
        assert_eq!(check(std::slice::from_ref(&f)), Vec::<String>::new());
        let u32s = vec!["u32".to_string()];
        let facts: Facts = [(
            "src/m.brs".to_string(),
            vec![
                kind_fact(brs, "f(1)", Kind::Indirect(u32s.clone()), "", None),
                kind_fact(brs, "crate::m::ops::calm", Kind::Value(u32s.clone()), "m::ops::calm", None),
                kind_fact(brs, "crate::m::ops::apply", Kind::Call, "m::ops::apply", None),
                kind_fact(brs, "|n|", Kind::Closure(u32s.clone()), "", Some("|n|")),
                kind_fact(brs, "|s|", Kind::Closure(vec!["&str".into()]), "", Some("|s|")),
                kind_fact(brs, "slow(2)", Kind::CallClosure, "", Some("|n|")),
                kind_fact(brs, "named(", Kind::CallClosure, "", Some("|s|")),
            ],
        )]
        .into_iter()
        .collect();
        let errs = check_with(std::slice::from_ref(&f), Some(&facts));
        assert_eq!(errs.len(), 1, "{errs:#?}");
        assert!(errs[0].contains("a call of a function pointer or an `Fn` bound (it may be `c::m::main_m::run::{closure@11:20}`, which needs the most of the 2 it may be), which needs `structured`, in `c::m::ops::apply`"), "{errs:#?}");
        // `run` writes and calls its closures: their loops are its own, reported once
        let all = uses_with(std::slice::from_ref(&f), Some(&facts));
        let run: Vec<&str> = all.iter().filter(|u| u.function == "c::m::main_m::run" && u.level >= 2).map(|u| u.what.as_str()).collect();
        assert_eq!(run, ["a loop", "a loop", "a call of `apply`, which needs `structured`,"], "{all:#?}");
        // a type parameter fits any argument: all three take one
        assert_eq!(analyse(std::slice::from_ref(&f), Some(&facts)).candidates_for(&["_".into()]), 3);
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
