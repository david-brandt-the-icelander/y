//! What the runtime crates offer, read from the crates themselves: each public method and function of y-collections,
//! y-string, y-runtime, y-window and y-http, with its parameters and result, from `bare-rust`'s strip of their sources
//! (the constraint data of each, `subjects` of kind `function` whose `visible` is `public`).
//!
//! The lowering uses it two ways. A Y method that the lowering has no rule of its own for is a call of the method of
//! that name on the receiver's Rust type, and is lowered by the parameters and result the crate gives it; one the
//! crate does not have is a compile error that names it. And the runtime operations the lowering's own rules call
//! (`list::at_number`, `List::push_front`) are listed in [`LOWERED`], which a test checks against the crates.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A public method or function of a runtime crate.
#[derive(Clone, Debug)]
pub struct Method {
    /// Each parameter but `self`: its name and its type in the constraint language (`ref<target_type=T>`, `u64 as
    /// 'usize'`).
    pub params: Vec<(String, String)>,
    /// How it takes `self`: `None` for an associated function, `Some(true)` for `&mut self`, `Some(false)` for `&self`
    /// or `self`.
    pub receiver: Option<bool>,
    /// Its result's type, if it has one.
    pub result: Option<String>,
}

#[derive(Default, Debug)]
pub struct Library {
    /// Each type's methods, by the type's Rust path (`y_collections::List`) and the method's name.
    pub methods: BTreeMap<String, BTreeMap<String, Method>>,
    /// Each module's functions, by the module's Rust path (`y_runtime::list`) and the function's name.
    pub functions: BTreeMap<String, BTreeMap<String, Method>>,
    /// Why a crate could not be read, if one could not.
    pub problems: Vec<String>,
}

/// The crates, by folder under `examples/`.
pub const CRATES: [&str; 5] = ["y-collections", "y-string", "y-runtime", "y-window", "y-http"];

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn sources(dir: &Path, rel: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(rd) = std::fs::read_dir(dir.join(rel)) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let r = rel.join(p.file_name().unwrap());
        if p.is_dir() {
            sources(dir, &r, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            if let Ok(s) = std::fs::read_to_string(&p) {
                out.push((r, s));
            }
        }
    }
}

/// Reads one crate into the library.
fn read_crate(lib: &mut Library, name: &str) {
    let dir = examples().join(name);
    let mut files = vec![];
    sources(&dir, Path::new("src"), &mut files);
    if files.is_empty() {
        lib.problems.push(format!("{name}: no sources in {}", dir.display()));
        return;
    }
    let all: Vec<PathBuf> = files.iter().map(|f| f.0.clone()).collect();
    let trees = vec![(PathBuf::from("src/lib.rs"), all)];
    for (rel, r) in bare_rust::corpus::strip_sources(&format!("{name} 0.0.0"), &files, &trees) {
        let js = match r {
            Ok((_, js)) => js,
            Err(e) => {
                lib.problems.push(format!("{name}/{}: {e}", rel.display()));
                continue;
            }
        };
        for s in js["subjects"].as_array().into_iter().flatten() {
            if s["kind"] != "function" || s["visible"] != "public" {
                continue;
            }
            let full = s["name"].as_str().unwrap_or("");
            let Some((owner, method)) = full.rsplit_once("::") else { continue };
            let mut params = vec![];
            let mut receiver = None;
            for p in s["params"].as_array().into_iter().flatten() {
                let (pn, pt) = (p["name"].as_str().unwrap_or("").to_string(), p["type"].as_str().unwrap_or("").to_string());
                if pn == "self" {
                    receiver = Some(p["access"]["rights"] == "write" || p["access"]["rights"] == "read write" || pt.starts_with("mut ") || p["access"]["guarantee"] == "exclusive");
                } else {
                    params.push((pn, pt));
                }
            }
            let result = s["result"]["type"].as_str().map(str::to_string);
            let m = Method { params, receiver, result };
            let is_type = owner.rsplit("::").next().is_some_and(|o| o.chars().next().is_some_and(char::is_uppercase));
            let table = if is_type { &mut lib.methods } else { &mut lib.functions };
            table.entry(owner.to_string()).or_default().insert(method.to_string(), m);
        }
    }
}

/// The runtime crates' public API, read once.
pub fn library() -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(|| {
        let mut lib = Library::default();
        for c in CRATES {
            read_crate(&mut lib, c);
        }
        lib
    })
}

impl Library {
    pub fn method(&self, owner: &str, name: &str) -> Option<&Method> {
        self.methods.get(owner).and_then(|m| m.get(name))
    }

    pub fn function(&self, module: &str, name: &str) -> Option<&Method> {
        self.functions.get(module).and_then(|m| m.get(name))
    }

    /// The methods a type offers, for a diagnostic.
    pub fn offered(&self, owner: &str) -> Vec<String> {
        self.methods.get(owner).map(|m| m.keys().cloned().collect()).unwrap_or_default()
    }

    /// Whether an operation the lowering names exists: `y_collections::List::push_front`, `y_runtime::list::count`.
    pub fn has(&self, path: &str) -> bool {
        let Some((owner, name)) = path.rsplit_once("::") else { return false };
        self.method(owner, name).is_some() || self.function(owner, name).is_some()
    }
}

/// The runtime operations the lowering's own rules call, for the Y operations whose lowering is not a call of the
/// method of the same name: what `tests/library.rs` checks against the crates, so that a rule cannot name an operation
/// a crate has lost or never had. Each is the Y receiver, the Y method, and the Rust operation it lowers to.
pub const LOWERED: &[(&str, &str, &str)] = &[
    ("list", "count", "y_runtime::list::count"),
    ("list", "count", "y_collections::List::count"),
    ("list", "to_int32", "y_runtime::list::count_int32"),
    ("list", "without_last", "y_runtime::list::without_last"),
    ("list", "insert_first", "y_collections::List::push_front"),
    ("list", "remove_last", "y_collections::List::take_back"),
    ("list", "remove_first", "y_collections::List::take_front"),
    ("list", "at_number", "y_runtime::list::at_number"),
    ("list", "at_number", "y_runtime::list::at_number_ref"),
    ("list", "sorted", "y_runtime::list::sorted"),
    ("list", "set_number", "y_runtime::list::set_number"),
    ("list", "number_of", "y_runtime::list::number_of"),
    ("list", "largest", "y_runtime::list::largest"),
    ("list", "smallest", "y_runtime::list::smallest"),
    ("list", "last", "y_runtime::list::last"),
    ("list", "repeated", "y_runtime::list::repeated"),
    ("list", "join", "y_string::YString::joined"),
    ("list", "join", "y_string::YString::from_runes"),
    ("set", "add", "y_collections::Set::add"),
    ("set", "contains", "y_collections::Set::contains"),
    ("string", "single_rune", "y_string::Rune::single"),
    ("string", "trim", "y_string::YString::trimmed"),
    ("string", "parse_uint32", "y_string::YString::parse_uint32"),
    ("string", "runes", "y_string::YString::runes"),
    ("string", "split_whitespace", "y_string::YString::split_whitespace"),
    ("uint32", "to_int32", "y_runtime::number::to_int32"),
    ("int32", "to_uint32", "y_runtime::number::to_uint32"),
    ("duration", "whole_seconds", "y_runtime::time::whole_seconds"),
    ("bag", "remove", "y_collections::Bag::remove_times"),
    ("bag", "add", "y_collections::Bag::add_times"),
    ("bag", "count_of", "y_collections::Bag::count_of"),
    ("queue", "add", "y_collections::Queue::add"),
    ("sorted_map", "set", "y_collections::SortedMap::set"),
    ("sorted_map", "get", "y_collections::SortedMap::get"),
    ("decimal", "to_rational", "y_runtime::decimal::Decimal::to_rational"),
    ("decimal", "to_decimal_rounded", "y_runtime::decimal::Decimal::to_decimal_rounded"),
    ("decimal", "divide_rounded", "y_runtime::decimal::Decimal::divide_rounded"),
    ("decimal", "divide_exact", "y_runtime::decimal::Decimal::divide_exact"),
    ("decimal", "to_integer_rounded", "y_runtime::decimal::Decimal::to_integer_rounded"),
    ("decimal", "to_float64_nearest", "y_runtime::decimal::Decimal::to_float64_nearest"),
    ("decimal", "*", "y_runtime::decimal::Decimal::multiply"),
    ("decimal", "*", "y_runtime::decimal::Decimal::times"),
    ("rational", "new", "y_runtime::decimal::Rational::new"),
    ("rational", "to_decimal_rounded", "y_runtime::decimal::Rational::to_decimal_rounded"),
    ("rational", "to_decimal_exact", "y_runtime::decimal::Rational::to_decimal_exact"),
    ("integer", "to_rational", "y_runtime::decimal::Integer::to_i128"),
    ("integer", "to_usize", "y_runtime::decimal::Integer::to_usize"),
    ("integer", "+", "y_runtime::decimal::Integer::from_i128"),
    ("shared", "snapshot", "y_runtime::shared::Shared::snapshot"),
    ("arena", "allocate", "y_runtime::arena::Arena::allocate"),
    ("store", "read_all", "y_runtime::store::Store::read_all"),
    ("store", "batch", "y_runtime::store::Store::batch"),
    ("console", "read_line", "y_runtime::Console::read_line"),
    ("console", "write_line", "y_runtime::Console::write_line"),
    ("random", "index_below", "y_runtime::Random::index_below"),
    ("random", "chosen", "y_runtime::Random::chosen"),
    ("random", "one_of", "y_runtime::Random::one_of"),
    ("random", "shuffled", "y_runtime::Random::shuffled"),
    ("clock", "now", "y_runtime::Clock::now"),
    ("window", "next_frame", "y_window::Window::next_frame"),
    ("window", "keys_pressed", "y_window::Window::keys_pressed"),
    ("window", "clicks", "y_window::Window::clicks"),
    ("http", "get", "y_http::Http::get"),
    ("pattern", "matches", "y_runtime::pattern::matches"),
];

/// The Rust type a Y type lowers to, among the runtime's, and the Y name a diagnostic gives it.
pub fn owner_of(t: &crate::ty::Ty) -> Option<(&'static str, &'static str)> {
    use crate::ty::Ty;
    Some(match t {
        Ty::List(_) | Ty::Array(..) => ("y_collections::List", "list"),
        Ty::Set(_) => ("y_collections::Set", "set"),
        Ty::Bag(_) => ("y_collections::Bag", "bag"),
        Ty::Queue(_) => ("y_collections::Queue", "queue"),
        Ty::SortedMap(..) => ("y_collections::SortedMap", "sorted_map"),
        Ty::Coll(k, _) => crate::ty::coll_kind(k).map(|(_, y, p)| (p, y))?,
        Ty::Str => ("y_string::YString", "string"),
        Ty::Ascii => ("y_string::AsciiString", "string<character_set=ascii>"),
        Ty::Rune => ("y_string::Rune", "rune"),
        Ty::Decimal(..) => ("y_runtime::decimal::Decimal", "decimal"),
        Ty::Rational(_) => ("y_runtime::decimal::Rational", "rational"),
        Ty::Integer(..) => ("y_runtime::decimal::Integer", "integer"),
        Ty::Shared(_) => ("y_runtime::shared::Shared", "shared"),
        Ty::Store(_) => ("y_runtime::store::Store", "store"),
        Ty::Arena(_) => ("y_runtime::arena::Arena", "arena"),
        Ty::Cap(c) => match c.as_str() {
            "console" => ("y_runtime::Console", "console"),
            "random" => ("y_runtime::Random", "random"),
            "clock" => ("y_runtime::Clock", "clock"),
            "system" => ("y_runtime::System", "system"),
            "forever" => ("y_runtime::Forever", "forever"),
            "window" => ("y_window::Window", "window"),
            "http" => ("y_http::Http", "http"),
            _ => return None,
        },
        _ => return None,
    })
}

/// A Y type for a type the constraint data gives a parameter or a result: `T` is the receiver's element, a reference
/// is what it refers to, and the scalars and collections are theirs.
pub fn y_type(t: &str, elem: &crate::ty::Ty) -> crate::ty::Ty {
    use crate::ty::Ty;
    let t = t.trim();
    if let Some(inner) = t.strip_suffix('?') {
        return Ty::Opt(Box::new(y_type(inner, elem)));
    }
    if let Some(inner) = t.strip_prefix("ref<target_type=").and_then(|r| r.strip_suffix('>')) {
        return y_type(inner, elem);
    }
    let (base, host) = match t.split_once(" as '") {
        Some((b, h)) => (b, h.trim_end_matches('\'')),
        None => (t, ""),
    };
    match (base, host) {
        ("T", _) => elem.clone(),
        ("bool", _) => Ty::Bool,
        ("u32", _) => Ty::U32,
        ("i32", _) => Ty::I32,
        ("u64", "usize") => Ty::Count,
        ("u64", _) => Ty::U64,
        ("string", _) | (_, "YString") => Ty::Str,
        (b, _) if b.starts_with("list<") || host.starts_with("List") => Ty::List(Box::new(elem.clone())),
        _ => Ty::Unknown,
    }
}
