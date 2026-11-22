//! The full path of a name as a file writes it: through the file's `use` items, the items it defines, and the
//! standard prelude. Used to give a template one name however a file spells it (language 3.5).
use crate::{named_children, text};
use std::collections::HashMap;
use tree_sitter::Node;

/// Names of the standard prelude a template may be written with (Rust 2021, with `size_of` and friends since 1.80).
const PRELUDE: &[(&str, &str)] = &[
    ("Vec", "std::vec::Vec"),
    ("String", "std::string::String"),
    ("Box", "std::boxed::Box"),
    ("Option", "std::option::Option"),
    ("Some", "std::option::Option::Some"),
    ("None", "std::option::Option::None"),
    ("Result", "std::result::Result"),
    ("Ok", "std::result::Result::Ok"),
    ("Err", "std::result::Result::Err"),
    ("ToString", "std::string::ToString"),
    ("ToOwned", "std::borrow::ToOwned"),
    ("Iterator", "std::iter::Iterator"),
    ("IntoIterator", "std::iter::IntoIterator"),
    ("FromIterator", "std::iter::FromIterator"),
    ("Default", "std::default::Default"),
    ("From", "std::convert::From"),
    ("Into", "std::convert::Into"),
    ("TryFrom", "std::convert::TryFrom"),
    ("TryInto", "std::convert::TryInto"),
    ("Clone", "std::clone::Clone"),
    ("drop", "std::mem::drop"),
    ("size_of", "std::mem::size_of"),
    ("size_of_val", "std::mem::size_of_val"),
    ("align_of", "std::mem::align_of"),
    ("align_of_val", "std::mem::align_of_val"),
];

/// What the names of one file stand for.
pub struct Paths {
    /// The file's module, as `crate::a::b`.
    module: Vec<String>,
    /// A name the file brings in or defines -> its full path.
    names: HashMap<String, String>,
}

impl Paths {
    /// The names of a file: its top-level `use` items and the items it defines. `module` is the file's module path
    /// (`["a", "b"]` for `src/a/b.rs`).
    pub fn of_file(root: Node, src: &str, module: &[String]) -> Paths {
        let mut p = Paths { module: std::iter::once("crate".to_string()).chain(module.iter().cloned()).collect(), names: HashMap::new() };
        for n in named_children(root) {
            match n.kind() {
                "use_declaration" => {
                    if let Some(a) = n.child_by_field_name("argument") {
                        p.add_use(a, src, "");
                    }
                }
                "function_item" | "struct_item" | "enum_item" | "type_item" | "trait_item" | "const_item" | "static_item" | "mod_item"
                | "union_item" => {
                    if let Some(name) = n.child_by_field_name("name") {
                        let name = text(name, src).to_string();
                        let full = format!("{}::{name}", p.module.join("::"));
                        p.names.entry(name).or_insert(full);
                    }
                }
                _ => {}
            }
        }
        p
    }

    fn add_use(&mut self, n: Node, src: &str, prefix: &str) {
        let join = |a: &str, b: &str| if a.is_empty() { b.to_string() } else { format!("{a}::{b}") };
        match n.kind() {
            "identifier" | "scoped_identifier" => {
                let full = join(prefix, text(n, src));
                let last = full.rsplit("::").next().unwrap_or(&full).to_string();
                if last != "self" {
                    self.names.insert(last, self.absolute(&full));
                } else if let Some(parent) = full.strip_suffix("::self") {
                    let last = parent.rsplit("::").next().unwrap_or(parent).to_string();
                    self.names.insert(last, self.absolute(parent));
                }
            }
            "use_as_clause" => {
                if let (Some(path), Some(alias)) = (n.child_by_field_name("path"), n.child_by_field_name("alias")) {
                    let full = join(prefix, text(path, src));
                    let full = full.strip_suffix("::self").unwrap_or(&full).to_string();
                    if text(alias, src) != "_" {
                        self.names.insert(text(alias, src).to_string(), self.absolute(&full));
                    }
                }
            }
            "scoped_use_list" => {
                let path = n.child_by_field_name("path").map(|p| text(p, src)).unwrap_or("");
                let prefix = join(prefix, path);
                if let Some(list) = n.child_by_field_name("list") {
                    for c in named_children(list) {
                        self.add_use(c, src, &prefix);
                    }
                }
            }
            "use_list" => {
                for c in named_children(n) {
                    self.add_use(c, src, prefix);
                }
            }
            "self" if !prefix.is_empty() => {
                let last = prefix.rsplit("::").next().unwrap_or(prefix).to_string();
                self.names.insert(last, self.absolute(prefix));
            }
            _ => {}
        }
    }

    /// A path that starts at the crate or at a crate in the build: `self::x` and `super::x` made absolute, a
    /// leading `::` dropped.
    fn absolute(&self, path: &str) -> String {
        let path = path.strip_prefix("::").unwrap_or(path);
        let mut segs: Vec<&str> = path.split("::").collect();
        let mut base: Vec<String> = vec![];
        match segs.first().copied() {
            Some("self") => {
                base = self.module.clone();
                segs.remove(0);
            }
            Some("super") => {
                base = self.module.clone();
                while segs.first() == Some(&"super") {
                    segs.remove(0);
                    if base.len() > 1 {
                        base.pop();
                    }
                }
            }
            _ => {}
        }
        base.into_iter().chain(segs.into_iter().map(str::to_string)).collect::<Vec<_>>().join("::")
    }

    /// The full path of a path as the file writes it: `size_of` -> `std::mem::size_of`, `mem::swap` with `use std::mem`
    /// -> `std::mem::swap`, `HashMap` with `use std::collections::HashMap` -> `std::collections::HashMap`. A path the
    /// file does not say where it comes from (a glob import, `Self`) is kept as written.
    pub fn full(&self, written: &str) -> String {
        let written: String = written.chars().filter(|c| !c.is_whitespace()).collect();
        let path = written.strip_prefix("::").unwrap_or(&written);
        let (first, rest) = match path.split_once("::") {
            Some((f, r)) => (f, Some(r)),
            None => (path, None),
        };
        let head = match first {
            "crate" | "self" | "super" => return self.absolute(path),
            _ => self.names.get(first).cloned().or_else(|| PRELUDE.iter().find(|(n, _)| *n == first).map(|(_, p)| p.to_string())),
        };
        match (head, rest) {
            (Some(h), Some(r)) => format!("{h}::{r}"),
            (Some(h), None) => h,
            (None, _) => path.to_string(),
        }
    }
}
