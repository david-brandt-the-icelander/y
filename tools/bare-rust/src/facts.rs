//! What rustc resolved in a woven crate, by position in its bare files: each call's callee, by its canonical path.
//!
//! The level check (`crate::level`) and the region check (`crate::region`) read a bare body's calls. From its syntax
//! alone, a call names a function only through the file's `use` declarations, and a method call names no type unless
//! the constraint data gives its receiver's. Where the woven crate compiles, rustc knows each callee: `cargo rustc
//! -- -Zunpretty=mir` (the same output `bare-rust externals` reads) prints every call and every function named as a
//! value with the path of the item it resolved to and the span of the call. A trait method called on a known type is
//! printed as `<Type as Trait>::method`, so its receiver's type, and the impl the call runs, are known too. A function
//! called through a local (`let run = atomic; run(…)`) is a call of the local, whose type names the function.
//!
//! Each call is mapped back from the woven file to the bare one through the weave's span map (`crate::trace`), and
//! kept as a `Fact`: the bare offset and line, and the `Callee`. What rustc does not resolve is left to the syntax:
//! a call through a function pointer or a closure, and a trait method called on a type parameter, whose impl the
//! caller does not choose (its trait is kept).
use crate::trace::SpanMap;
use crate::R;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

/// A function a call or a function value resolved to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callee {
    /// The function by where it is defined. One of the crate's starts with the crate's name (`warehouse::stock::reserve`,
    /// `warehouse::stock::Holdings::count`); a library's with its crate's (`y_runtime::shared::atomic`), with `core` and
    /// `alloc` read as `std`. A method of a trait is the method of its receiver's type (`m::Tally::count`); called on a
    /// trait object, it is the trait's.
    pub path: String,
    /// For a method of a trait or a type: the receiver's type, without its arguments (`warehouse::stock::Holdings`,
    /// `std::primitive` for a primitive, a slice or a tuple), and whether it is the crate's.
    pub self_ty: Option<(String, bool)>,
    /// For a method of a trait: the trait, and whether it is the crate's.
    pub trait_path: Option<(String, bool)>,
    /// The function's name: the last segment of `path`.
    pub name: String,
    /// Whether `path` is the crate's.
    pub local: bool,
    /// Whether the receiver is named by one segment of the crate: a type parameter of the function the call is in,
    /// whose impl the caller does not choose, or a type of the crate's root module, which MIR prints the same way.
    pub param: bool,
}

/// A call in a bare file, as rustc resolved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    /// The byte offset in the bare file where the call's function is named (the method's name, for a method call).
    pub offset: usize,
    pub line: usize,
    pub callee: Callee,
}

/// The facts of a crate's bare files, by bare file (`src/m.brs`).
pub type Facts = BTreeMap<String, Vec<Fact>>;

/// A call as MIR prints it: its file, line and column (1-based) in the woven crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirCall {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub callee: Callee,
}

/// The crates that are not the one being read: the standard library's, and the dependencies'.
const SYSROOT: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];

/// A path as rustc prints it, by its segments: generic arguments dropped, and `<impl [T]>` (an inherent impl of a
/// primitive in `core`) left out.
fn segments(p: &str) -> Vec<String> {
    crate::externals::read_path(p, 0).into_iter().filter(|s| !s.starts_with("<impl ")).collect()
}

/// A path of a crate (`krate`) or a library: `core` and `alloc` read as `std`, and a path of the crate gets the crate's
/// name in front, as rustc prints the items of the crate it compiles without it. `None` for a path not of an item.
fn canonical(p: &str, krate: &str, externs: &BTreeSet<String>) -> Option<(String, bool)> {
    let mut segs = segments(p.trim());
    let first = segs.first()?.clone();
    if first == "core" || first == "alloc" {
        segs[0] = "std".into();
    }
    if crate::externals::is_prim(&first) || first == "str" {
        return Some((format!("std::primitive::{first}"), false));
    }
    if first == krate {
        return Some((segs.join("::"), true));
    }
    if externs.contains(&first) || SYSROOT.contains(&first.as_str()) {
        return Some((segs.join("::"), false));
    }
    Some((format!("{krate}::{}", segs.join("::")), true))
}

/// The receiver's type of a method: `None` when the call is of a closure or a function pointer (whose body is where it
/// is written, or unknown), `Some(None)` for a trait object (the trait says what it is).
fn self_type(t: &str, krate: &str, externs: &BTreeSet<String>) -> Option<Option<(String, bool)>> {
    let mut t = t.trim();
    loop {
        let before = t;
        for p in ["&mut ", "&", "*const ", "*mut ", "mut "] {
            t = t.strip_prefix(p).unwrap_or(t).trim_start();
        }
        // a lifetime: `&'a T`
        if let Some(rest) = t.strip_prefix('\'') {
            t = rest.split_once(' ').map_or("", |(_, r)| r).trim_start();
        }
        if t == before {
            break;
        }
    }
    if t.starts_with('{') || t.starts_with("fn(") || t.starts_with("for<") || t.starts_with("unsafe ") || t.starts_with("extern ") || t.starts_with("impl ") {
        return None;
    }
    if let Some(rest) = t.strip_prefix("dyn ") {
        let _ = rest;
        return Some(None);
    }
    if t.starts_with('[') || t.starts_with('(') || t.starts_with('!') || t.starts_with('<') {
        return Some(Some(("std::primitive".into(), false)));
    }
    Some(canonical(t, krate, externs))
}

/// The callee of a function path as MIR prints it in a call or a constant (`y_runtime::shared::atomic::<…>`,
/// `<m::Tally as m::Count>::count`, `<[T]>::len`). `None` for what is not a function of an item: a closure's call.
pub fn callee(v: &str, krate: &str, externs: &BTreeSet<String>) -> Option<Callee> {
    let v = v.trim();
    if v.starts_with('<') {
        let end = crate::externals::skip_balanced(v, 0);
        let inner = &v[1..end - 1];
        let after = v[end..].strip_prefix("::")?;
        let name: String = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if name.is_empty() {
            return None;
        }
        let (s, tr) = match crate::externals::split_as(inner) {
            Some((s, tr)) => (s, Some(tr)),
            None => (inner, None),
        };
        let self_ty = self_type(s, krate, externs)?;
        let trait_path = match tr {
            Some(tr) => Some(canonical(tr, krate, externs)?),
            None => None,
        };
        // a receiver MIR names by one segment of the crate is a type parameter, or a type of the crate's root module
        let param = matches!(&self_ty, Some((_, true))) && segments(s.trim_start_matches(['&', '*', ' '])).len() == 1;
        let callee = match (&self_ty, &trait_path) {
            (Some((t, l)), _) => Callee { path: format!("{t}::{name}"), self_ty: self_ty.clone(), trait_path, name, local: *l, param },
            // a trait object's method: the trait's
            (None, Some((tr, l))) => Callee { path: format!("{tr}::{name}"), self_ty: None, trait_path: trait_path.clone(), name, local: *l, param },
            (None, None) => return None,
        };
        return Some(callee);
    }
    let (path, local) = canonical(v, krate, externs)?;
    let name = path.rsplit("::").next().unwrap_or("").to_string();
    Some(Callee { path, self_ty: None, trait_path: None, name, local, param: false })
}

/// `file:line:col: line:col`, the start of a span. `None` for a span outside the crate (in a library's source).
fn span_start(s: &str) -> Option<(String, usize, usize)> {
    let mut parts = s.trim().split(':');
    let file = parts.next()?;
    let line = parts.next()?.trim().parse().ok()?;
    let col = parts.next()?.trim().parse().ok()?;
    if file.is_empty() || file.starts_with('/') || file.starts_with('<') {
        return None;
    }
    Some((file.to_string(), line, col))
}

/// The function a local's type names, if it is a function item: `fn(u32) -> u32 {rt::shared::atomic}`.
fn fn_item(ty: &str) -> Option<&str> {
    let ty = ty.trim();
    if !(ty.starts_with("fn(") || ty.starts_with("for<") || ty.starts_with("unsafe fn(")) || !ty.ends_with('}') {
        return None;
    }
    // the `{` that matches the last `}`: the item's generic arguments may hold closures, `{closure@…}`
    let mut depth = 0;
    let open = ty.char_indices().rev().find(|(_, c)| {
        depth += match c {
            '}' => 1,
            '{' => -1,
            _ => 0,
        };
        depth == 0
    })?.0;
    let item = &ty[open + 1..ty.len() - 1];
    (!item.starts_with("closure@") && !item.starts_with("coroutine@")).then_some(item)
}

/// Every call and function value in the MIR of one target of the crate `krate`, with where it is.
pub fn read_mir(mir: &str, krate: &str, externs: &BTreeSet<String>) -> Vec<MirCall> {
    let mut out = vec![];
    let mut pending: Option<(String, usize, usize)> = None;
    // the function items the body's locals hold, by local
    let mut locals: BTreeMap<String, String> = BTreeMap::new();
    for line in mir.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            locals.clear();
            pending = None;
            continue;
        }
        let t = line.trim();
        if t == "// mir::ConstOperand" {
            pending = None;
        } else if let Some(sp) = t.strip_prefix("// + span: ") {
            pending = span_start(sp);
        } else if let Some(c) = t.strip_prefix("// + const_: ") {
            let Some(at) = pending.take() else { continue };
            let Some(i) = c.find("val: Value(") else { continue };
            let Some(v) = c[i + 11..].strip_suffix(") }") else { continue };
            // a function item's constant: its type ends with `{path}`
            if !c[..i].trim_end().trim_end_matches(',').ends_with(&format!("{{{v}}}")) {
                continue;
            }
            if let Some(callee) = callee(v, krate, externs) {
                out.push(MirCall { file: at.0, line: at.1, col: at.2, callee });
            }
        } else if let Some(rest) = t.strip_prefix("let ") {
            // `let mut _6: fn(u32) -> u32 {rt::shared::atomic}; // in scope 0 at …`
            let rest = rest.strip_prefix("mut ").unwrap_or(rest);
            let Some((name, ty)) = rest.split_once(": ") else { continue };
            let ty = ty.split("; //").next().unwrap_or("").trim_end_matches(';');
            if let Some(item) = fn_item(ty) {
                locals.insert(name.to_string(), item.to_string());
            }
        } else if let Some(eq) = t.find(" = ") {
            // a call through a local: `_5 = move _6(const 1_u32) -> [return: bb1, …]; // scope 2 at src/lib.rs:13:88: 13:92`
            let rhs = &t[eq + 3..];
            let rhs = rhs.strip_prefix("move ").or_else(|| rhs.strip_prefix("copy ")).unwrap_or(rhs);
            let Some(open) = rhs.find('(') else { continue };
            let local = &rhs[..open];
            if !(local.starts_with('_') && local[1..].bytes().all(|b| b.is_ascii_digit()) && local.len() > 1 && rhs.contains(") -> [")) {
                continue;
            }
            let (Some(item), Some(i)) = (locals.get(local), t.rfind(" at ")) else { continue };
            let (Some(at), Some(callee)) = (span_start(&t[i + 4..]), callee(item, krate, externs)) else { continue };
            out.push(MirCall { file: at.0, line: at.1, col: at.2, callee });
        }
    }
    out
}

/// The byte offset of a 1-based line and column (in characters) in a text.
fn offset_of(text: &str, line: usize, col: usize) -> Option<usize> {
    let start = if line <= 1 { 0 } else { text.match_indices('\n').nth(line - 2)?.0 + 1 };
    let rest = &text[start..];
    let within = rest.char_indices().nth(col.saturating_sub(1)).map_or(rest.len(), |(i, _)| i);
    Some(start + within)
}

/// One woven file: its path in the crate (`src/m.rs`), its text and span map, and its bare file's name and text.
pub struct Woven<'a> {
    pub rel: String,
    pub text: &'a str,
    pub map: &'a SpanMap,
    pub bare_name: String,
    pub bare: &'a str,
}

/// The calls of the MIR, by bare file: each mapped from the woven file through its span map. A call in a file with no
/// bare file, or that weave printed from the constraint data rather than copied from the bare body, maps where the
/// printed text stands.
pub fn to_bare(calls: &[MirCall], files: &[Woven]) -> Facts {
    let mut out: Facts = BTreeMap::new();
    for f in files {
        out.entry(f.bare_name.clone()).or_default();
    }
    for c in calls {
        let Some(f) = files.iter().find(|f| f.rel == c.file) else { continue };
        let Some(w) = offset_of(f.text, c.line, c.col) else { continue };
        let offset = f.map.to_bare(w, false).min(f.bare.len());
        let line = crate::trace::line_col(f.bare, offset).0;
        let fact = Fact { offset, line, callee: c.callee.clone() };
        let v = out.entry(f.bare_name.clone()).or_default();
        if !v.contains(&fact) {
            v.push(fact);
        }
    }
    for v in out.values_mut() {
        v.sort_by(|a, b| (a.offset, &a.callee.path).cmp(&(b.offset, &b.callee.path)));
    }
    out
}

/// The names of the crates a package depends on, as its code names them (`y-runtime` is `y_runtime`).
fn dependencies(dir: &Path) -> R<BTreeSet<String>> {
    let o = Command::new("cargo").args(["metadata", "--no-deps", "--format-version", "1"]).current_dir(dir).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !o.status.success() {
        return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("")));
    }
    let v: Value = serde_json::from_slice(&o.stdout).map_err(|e| e.to_string())?;
    let root = std::fs::canonicalize(dir).map_err(|e| e.to_string())?;
    let mut out = BTreeSet::new();
    for p in v["packages"].as_array().into_iter().flatten() {
        let mf = p["manifest_path"].as_str().unwrap_or("");
        if std::fs::canonicalize(mf).ok().and_then(|m| m.parent().map(Path::to_path_buf)) != Some(root.clone()) {
            continue;
        }
        for d in p["dependencies"].as_array().into_iter().flatten() {
            if let Some(n) = d["rename"].as_str().or(d["name"].as_str()) {
                out.insert(n.replace('-', "_"));
            }
        }
    }
    Ok(out)
}

/// The facts of a woven crate (at `dir`, built in `target`) whose crate is named `krate`: the MIR of its library and
/// binaries, without their tests, mapped to the bare files. With the targets it read, as `lib`, `bin name`.
pub fn of_crate(dir: &Path, target: &Path, krate: &str, files: &[Woven]) -> R<(Facts, Vec<String>)> {
    let externs = dependencies(dir)?;
    let mut calls = vec![];
    let mut read = vec![];
    for t in crate::externals::targets(dir, false)? {
        let mir = crate::externals::mir_of(dir, target, &t, false)?;
        calls.extend(read_mir(&mir, krate, &externs));
        read.push(if t.kind == "lib" { "lib".to_string() } else { format!("{} {}", t.kind, t.name) });
    }
    Ok((to_bare(&calls, files), read))
}

/// A function of a library crate, by its path (`y_runtime::shared::atomic`, `y_runtime::System::console`): whether
/// another crate can call it (`pub`, or a method of an impl of a trait), and the byte range of its node.
struct LibFn {
    path: String,
    public: bool,
    file: String,
    range: (usize, usize),
}

/// The functions of a library crate's sources, by the module each file is (`src/shared.rs` is `shared`).
fn library_functions(dir: &Path, krate: &str) -> R<(Vec<LibFn>, BTreeMap<String, String>)> {
    let mut files = vec![];
    fn walk(d: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    walk(&dir.join("src"), &mut files);
    files.sort();
    let (mut fns, mut texts) = (vec![], BTreeMap::new());
    for f in files {
        let rel = f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        let src = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let tree = crate::parse_rust(&src);
        let base = std::iter::once(krate.to_string()).chain(crate::module_path(&rel)).collect::<Vec<_>>().join("::");
        for (path, node) in crate::region::functions(tree.root_node(), &src, &base) {
            let pubv = (0..node.named_child_count()).filter_map(|i| node.named_child(i)).any(|c| c.kind() == "visibility_modifier" && crate::text(c, &src).starts_with("pub"));
            let of_trait = node.parent().and_then(|p| p.parent()).filter(|p| p.kind() == "impl_item").is_some_and(|p| p.child_by_field_name("trait").is_some());
            fns.push(LibFn { path, public: pubv || of_trait, file: rel.clone(), range: (node.start_byte(), node.end_byte()) });
        }
        texts.insert(rel, src);
    }
    Ok((fns, texts))
}

/// The table of library levels (`libraries/levels.json`) against what a library crate's code calls, from its MIR:
/// each public function of the crate needs the most of the calls it makes into other libraries (at the level the
/// table gives them; a library the table does not list needs `effects`) and of the crate's own functions it calls. One
/// message for each public function that needs more than the table gives it, naming the call that needs it. What
/// the crate does itself (its loops, its own state) is not counted: the table says what a call into the library needs,
/// and the library is trusted for the rest. A function the table's `trusted` names is held at its module's level,
/// whatever it calls.
pub fn check_library(dir: &Path, target: &Path) -> R<Vec<String>> {
    let info = crate::corpus::crate_info(dir)?;
    let krate = info.name.replace('-', "_");
    let externs = dependencies(dir)?;
    let (fns, texts) = library_functions(dir, &krate)?;
    let lib = crate::externals::targets(dir, false)?.into_iter().find(|t| t.kind == "lib").ok_or_else(|| format!("{}: no library target", dir.display()))?;
    let calls = read_mir(&crate::externals::mir_of(dir, target, &lib, false)?, &krate, &externs);
    let index: BTreeMap<&str, usize> = fns.iter().enumerate().map(|(i, f)| (f.path.as_str(), i)).collect();
    // each function's own calls into other libraries, and the crate's functions it calls
    let mut own: Vec<(usize, String)> = vec![(0, String::new()); fns.len()];
    let mut edges: Vec<Vec<usize>> = vec![vec![]; fns.len()];
    for c in &calls {
        let Some(off) = texts.get(&c.file).and_then(|t| offset_of(t, c.line, c.col)) else { continue };
        let Some(i) = fns.iter().enumerate().filter(|(_, f)| f.file == c.file && f.range.0 <= off && off < f.range.1).min_by_key(|(_, f)| f.range.1 - f.range.0).map(|(i, _)| i) else { continue };
        let k = &c.callee;
        if k.local || k.self_ty.as_ref().is_some_and(|t| t.1) {
            let head = k.self_ty.as_ref().map(|(t, _)| t.rsplit("::").next().unwrap_or("").to_string()).unwrap_or_default();
            match index.get(k.path.as_str()) {
                Some(j) => edges[i].push(*j),
                None => edges[i].extend(fns.iter().enumerate().filter(|(_, f)| !head.is_empty() && f.path.ends_with(&format!("::{head}::{}", k.name))).map(|(j, _)| j)),
            }
            continue;
        }
        let mut paths = vec![k.path.clone()];
        if let Some((t, _)) = &k.trait_path {
            paths.push(format!("{t}::{}", k.name));
        }
        let (level, why) = match paths.iter().find(|p| crate::level::library_level(p).is_none()) {
            Some(p) => (4, format!("it calls `{p}`, of a library with no level in libraries/levels.json")),
            None => paths.iter().filter_map(|p| Some((crate::level::library_level(p)?, p))).max_by_key(|x| x.0).map(|(l, p)| (l, format!("it calls `{p}`"))).unwrap_or_default(),
        };
        if level > own[i].0 && crate::level::trusted(&fns[i].path).is_none() {
            own[i] = (level, why);
        }
    }
    // a function the table trusts is held at its module's level: nothing it calls counts
    for (i, f) in fns.iter().enumerate() {
        if crate::level::trusted(&f.path).is_some() {
            edges[i].clear();
        }
    }
    // to a fixed point: a function needs what the crate's functions it calls need
    let mut need = own.clone();
    loop {
        let mut changed = false;
        for i in 0..fns.len() {
            for &j in &edges[i] {
                if need[j].0 > need[i].0 {
                    need[i] = (need[j].0, format!("it calls `{}`, which needs `{}` ({})", fns[j].path, crate::level::LEVELS[need[j].0], need[j].1));
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut out = vec![];
    for (i, f) in fns.iter().enumerate().filter(|(_, f)| f.public && !f.path.split("::").any(|s| s == "tests")) {
        let table = crate::level::library_level(&f.path).unwrap_or(4);
        if need[i].0 > table {
            out.push(format!("{}: `{}` needs `{}`, and libraries/levels.json gives it `{}`: {}", f.file, f.path, crate::level::LEVELS[need[i].0], crate::level::LEVELS[table], need[i].1));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext() -> BTreeSet<String> {
        ["y_runtime".to_string()].into_iter().collect()
    }

    #[test]
    fn paths_are_canonical() {
        let c = callee("y_runtime::shared::atomic::<(), std::convert::Infallible, {closure@src/m.rs:3:9: 3:13}>", "c", &ext()).unwrap();
        assert_eq!((c.path.as_str(), c.local, c.name.as_str()), ("y_runtime::shared::atomic", false, "atomic"));
        let c = callee("<m::Tally as m::Count>::count", "c", &ext()).unwrap();
        assert_eq!(c.path, "c::m::Tally::count");
        assert_eq!(c.self_ty, Some(("c::m::Tally".into(), true)));
        assert_eq!(c.trait_path, Some(("c::m::Count".into(), true)));
        // on a type parameter, the trait's method
        let c = callee("<C as m::Count>::count", "c", &ext()).unwrap();
        assert_eq!((c.path.as_str(), c.param), ("c::C::count", true));
        let c = callee("<std::vec::Vec<m::Tally> as std::ops::Deref>::deref", "c", &ext()).unwrap();
        assert_eq!((c.path.as_str(), c.local), ("std::vec::Vec::deref", false));
        let c = callee("core::slice::<impl [m::Tally]>::iter", "c", &ext()).unwrap();
        assert_eq!(c.path, "std::slice::iter");
        let c = callee("alloc::rc::Rc::<u8>::new", "c", &ext()).unwrap();
        assert_eq!(c.path, "std::rc::Rc::new");
        let c = callee("<&y_runtime::shared::Shared<u32> as std::clone::Clone>::clone", "c", &ext()).unwrap();
        assert_eq!(c.path, "y_runtime::shared::Shared::clone");
        assert_eq!(callee("<{closure@src/m.rs:1:1: 1:4} as std::ops::Fn<(u8,)>>::call", "c", &ext()), None);
        // the crate's own items, named from a binary through the library's name
        assert_eq!(callee("c::m::f", "c", &ext()).unwrap().path, "c::m::f");
    }

    #[test]
    fn calls_values_and_calls_through_locals_are_read() {
        let mir = "fn m::h() -> u32 {\n    let _1: fn(u32) -> u32 {rt::shared::atomic}; // in scope 0 at src/lib.rs:13:29: 13:30\n    let mut _6: for<'a> fn(&'a str, {closure@src/lib.rs:12:22: 12:35}) -> u32 {rt::shared::atomic::<{closure@src/lib.rs:12:22: 12:35}>}; // in scope 0 at src/lib.rs:13:88: 13:89\n    bb0: {\n        _1 = rt::shared::atomic as fn(u32) -> u32; // scope 0 at src/lib.rs:13:33: 13:50\n                                         // mir::ConstOperand\n                                         // + span: src/lib.rs:13:33: 13:50\n                                         // + const_: Const { ty: fn(u32) -> u32 {rt::shared::atomic}, val: Value(rt::shared::atomic) }\n        _5 = move _6(const 1_u32) -> [return: bb1, unwind continue]; // scope 2 at src/lib.rs:13:88: 13:92\n        _7 = const \"sku\"; // scope 0 at src/lib.rs:14:1: 14:5\n                                         // mir::ConstOperand\n                                         // + span: src/lib.rs:14:1: 14:5\n                                         // + const_: Const { ty: &str, val: Value(\"sku\") }\n    }\n}\n";
        let calls = read_mir(mir, "c", &ext());
        assert_eq!(calls.len(), 2, "{calls:#?}");
        assert!(calls.iter().all(|c| c.callee.path == "c::rt::shared::atomic"));
        assert_eq!((calls[1].line, calls[1].col), (13, 88));
    }

    #[test]
    fn calls_map_back_to_the_bare_file() {
        let bare = "fn f() {\n    g()\n}\n";
        let woven = "fn f() -> u8 {\n    g()\n}\n";
        let map = SpanMap {
            spans: vec![crate::trace::Span { start: 6, end: 12, bare_start: 6, bare_end: 6, origin: crate::trace::Origin { kind: crate::trace::Kind::Slot, subject: "c::f".into(), slot: "ret".into() } }],
        };
        let calls = vec![MirCall { file: "src/lib.rs".into(), line: 2, col: 5, callee: callee("g", "c", &ext()).unwrap() }];
        let facts = to_bare(&calls, &[Woven { rel: "src/lib.rs".into(), text: woven, map: &map, bare_name: "src/lib.brs".into(), bare }]);
        let f = &facts["src/lib.brs"][0];
        assert_eq!((f.offset, f.line, f.callee.path.as_str()), (bare.find("g()").unwrap(), 2, "c::g"));
    }
}
