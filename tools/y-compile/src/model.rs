//! The names a Y module declares, resolved over its tree: its facets, types, constant groups, functions and rules, with
//! the types their documentation gives.
use crate::ty::{pascal, Ty};
use std::collections::{BTreeMap, BTreeSet};
use y_parse::Node;

#[derive(Clone, Debug)]
pub enum TypeDef {
    /// A structure: its fields, its derived values with their lines and expressions, and its invariants.
    Struct { fields: Vec<(String, Ty)>, derived: Vec<(String, Ty)>, derived_values: Vec<(usize, Node)>, invariants: Vec<(String, usize, Node)> },
    /// `type word is string:` with its `where` clauses: a newtype whose constructor checks them.
    Newtype { base: Ty, wheres: Vec<(String, usize, Node)> },
    /// Cases, each with its fields; a constant group used as a closed type has cases with none.
    Enum { cases: Vec<(String, Vec<(String, Ty)>)> },
}

#[derive(Clone, Debug)]
pub struct TypeInfo {
    pub name: String,
    pub facet: Option<String>,
    pub def: TypeDef,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct ConstGroup {
    pub name: String,
    pub facet: Option<String>,
    pub items: Vec<(String, Option<Ty>, Node)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Ref,
    Mut,
    Value,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
    pub mutable: bool,
    pub injected: bool,
    pub mode: Mode,
}

#[derive(Clone, Debug)]
pub struct FnInfo {
    pub name: String,
    pub facet: Option<String>,
    pub params: Vec<Param>,
    pub result: Option<(String, Ty)>,
    pub fails: Option<Ty>,
    pub rule: bool,
    pub line: usize,
}

impl FnInfo {
    pub fn injected(&self) -> Vec<&Param> {
        self.params.iter().filter(|p| p.injected).collect()
    }
}

pub struct Module<'t> {
    pub name: String,
    pub tree: &'t Node,
    pub types: BTreeMap<String, TypeInfo>,
    pub consts: BTreeMap<String, ConstGroup>,
    pub fns: BTreeMap<String, FnInfo>,
    /// Each facet, in order, with its line, visibility and members' names.
    pub facets: Vec<(String, usize, String, Vec<String>)>,
    /// The functions that may suspend (Y § 3.7): they wait for a capability that may, or call one that does.
    pub suspending: BTreeSet<String>,
}

/// The declarations of a file with the facet each is in.
pub fn declarations(tree: &Node) -> Vec<(Option<String>, &Node)> {
    let mut out = vec![];
    for d in tree.nodes("decls") {
        if d.kind == "facet" {
            for m in d.nodes("decls") {
                out.push((d.str("name").map(str::to_string), m));
            }
        } else {
            out.push((None, d));
        }
    }
    out
}

/// The type a documentation entry gives a name, from the `doc` node of a declaration.
pub fn doc_type(d: &Node, name: &str) -> Option<Ty> {
    let doc = d.node("doc")?;
    doc.nodes("entries").into_iter().find(|e| e.str("name") == Some(name)).and_then(|e| e.node("type")).map(Ty::from_node)
}

fn type_names(n: &Node, out: &mut BTreeSet<String>) {
    n.walk(&mut |x: &Node| {
        if x.kind == "type" {
            if let Some(t) = x.str("name") {
                out.insert(t.to_string());
            }
        }
    });
}

pub fn is_copy(m: &Module, t: &Ty) -> bool {
    copy_in(m, t, 0)
}

fn copy_in(m: &Module, t: &Ty, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    match t {
        Ty::U32 | Ty::I32 | Ty::U64 | Ty::Count | Ty::Int | Ty::Bool | Ty::StrLit => true,
        Ty::Named(n) if crate::ty::COPIED.contains(&n.as_str()) => true,
        Ty::Opt(x) => copy_in(m, x, depth + 1),
        Ty::Named(n) => match m.types.get(n).map(|t| &t.def) {
            Some(TypeDef::Struct { fields, .. }) => fields.iter().all(|(_, f)| copy_in(m, f, depth + 1)),
            Some(TypeDef::Enum { cases }) => cases.iter().all(|(_, fs)| fs.iter().all(|(_, f)| copy_in(m, f, depth + 1))),
            Some(TypeDef::Newtype { base, .. }) => copy_in(m, base, depth + 1),
            None => false,
        },
        _ => false,
    }
}

/// A type passed by value: a number, a truth value, or a type whose values are its cases alone.
pub fn by_value(m: &Module, t: &Ty) -> bool {
    match t {
        Ty::U32 | Ty::I32 | Ty::U64 | Ty::Count | Ty::Int | Ty::Bool | Ty::Rune => true,
        Ty::Named(n) if crate::ty::COPIED.contains(&n.as_str()) => true,
        Ty::Named(n) => matches!(m.types.get(n).map(|t| &t.def), Some(TypeDef::Enum { cases }) if cases.iter().all(|(_, f)| f.is_empty())),
        _ => false,
    }
}

impl<'t> Module<'t> {
    pub fn new(tree: &'t Node) -> Module<'t> {
        Module::new_in(tree, None)
    }

    /// A module that sees the types and constants of another: a test file, which is in the module it tests.
    pub fn new_in(tree: &'t Node, outer: Option<&Module>) -> Module<'t> {
        let name = tree.nodes("decls").iter().find(|d| d.kind == "module").and_then(|d| d.str("name")).unwrap_or("main").to_string();
        let decls = declarations(tree);
        // a constant group is a closed type when its name is used as a type
        let mut used_as_type = BTreeSet::new();
        type_names(tree, &mut used_as_type);
        for (_, d) in &decls {
            if d.kind == "function" {
                for f in d.str("fails").into_iter() {
                    used_as_type.insert(f.to_string());
                }
                if let Some(y_parse::Value::List(fs)) = d.get("fails") {
                    for f in fs {
                        if let y_parse::Value::Str(s) = f {
                            used_as_type.insert(s.split('.').next().unwrap().to_string());
                        }
                    }
                }
            }
        }
        let mut m = Module { name, tree, types: BTreeMap::new(), consts: BTreeMap::new(), fns: BTreeMap::new(), facets: vec![], suspending: BTreeSet::new() };
        if let Some(o) = outer {
            m.types = o.types.clone();
            m.consts = o.consts.clone();
        }
        for d in tree.nodes("decls") {
            if d.kind == "facet" {
                let members = d.nodes("decls").iter().filter_map(|x| x.str("name").map(str::to_string)).collect();
                m.facets.push((d.str("name").unwrap().to_string(), d.line, d.str("visibility").unwrap_or("private").to_string(), members));
            }
        }
        for (facet, d) in &decls {
            let name = d.str("name").unwrap_or("").to_string();
            match d.kind {
                "structure" => {
                    let mut fields = vec![];
                    let mut derived = vec![];
                    let mut derived_values = vec![];
                    let mut invariants = vec![];
                    for mem in d.nodes("members") {
                        let t = mem.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown);
                        match mem.kind {
                            "field" => fields.push((mem.str("name").unwrap().to_string(), t)),
                            "derived" => {
                                derived.push((mem.str("name").unwrap().to_string(), t));
                                derived_values.push((mem.line, mem.node("value").cloned().unwrap_or_else(|| Node::new("literal", mem.line))));
                            }
                            "invariant" => invariants.push((mem.str("name").unwrap().to_string(), mem.line, mem.node("condition").cloned().unwrap())),
                            _ => {}
                        }
                    }
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Struct { fields, derived, derived_values, invariants }, line: d.line });
                }
                "type_decl" if d.node("type").is_some() && d.nodes("cases").is_empty() => {
                    let base = Ty::from_node(d.node("type").unwrap());
                    let wheres = d.nodes("where").iter().map(|w| (w.str("name").unwrap().to_string(), w.line, w.node("condition").cloned().unwrap())).collect();
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Newtype { base, wheres }, line: d.line });
                }
                "type_decl" => {
                    let cases = d
                        .nodes("cases")
                        .iter()
                        .map(|c| {
                            let fs = c.nodes("fields").iter().map(|f| (f.str("name").unwrap().to_string(), f.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown))).collect();
                            (c.str("name").unwrap().to_string(), fs)
                        })
                        .collect();
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Enum { cases }, line: d.line });
                }
                "constants" => {
                    let items: Vec<(String, Option<Ty>, Node)> =
                        d.nodes("items").iter().map(|i| (i.str("name").unwrap().to_string(), i.node("type").map(Ty::from_node), i.node("value").cloned().unwrap_or_else(|| Node::new("literal", i.line)))).collect();
                    if used_as_type.contains(&name) {
                        let cases = items.iter().map(|(n, _, _)| (n.clone(), vec![])).collect();
                        m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Enum { cases }, line: d.line });
                    } else {
                        m.consts.insert(name.clone(), ConstGroup { name, facet: facet.clone(), items });
                    }
                }
                _ => {}
            }
        }
        for (facet, d) in &decls {
            if d.kind == "function" || d.kind == "rule" {
                let f = m.function(d, facet.clone());
                m.fns.insert(f.name.clone(), f);
            }
        }
        if let Some(o) = outer {
            m.suspending = o.suspending.clone();
        }
        m.suspending.extend(suspending(&decls, outer.map(|o| &o.suspending)));
        m
    }

    fn function(&self, d: &Node, facet: Option<String>) -> FnInfo {
        let name = d.str("name").unwrap().to_string();
        let rule = d.kind == "rule";
        let result = if rule { Some(("holds".to_string(), Ty::Bool)) } else { d.str("returns").map(|r| (r.to_string(), doc_type(d, r).unwrap_or(Ty::Unknown))) };
        let fails = match d.get("fails") {
            Some(y_parse::Value::List(fs)) if !fs.is_empty() => match &fs[0] {
                y_parse::Value::Str(s) => Some(Ty::Named(s.split('.').next().unwrap().to_string())),
                _ => None,
            },
            _ => None,
        };
        let kept = kept_params(d);
        let params = d
            .nodes("params")
            .iter()
            .map(|p| {
                let pname = p.str("name").unwrap().to_string();
                let ty = doc_type(d, &pname).or_else(|| p.node("type").map(Ty::from_node)).unwrap_or(Ty::Unknown);
                let (mutable, injected) = (p.flag("mutable"), p.flag("injected"));
                let mode = if injected {
                    if matches!(&ty, Ty::Cap(c) if c == "forever" || c == "system") { Mode::Ref } else { Mode::Mut }
                } else if mutable {
                    Mode::Mut
                } else if by_value(self, &ty) || kept.contains(&pname) {
                    Mode::Value
                } else {
                    Mode::Ref
                };
                Param { name: pname, ty, mutable, injected, mode }
            })
            .collect();
        FnInfo { name, facet, params, result, fails, rule, line: d.line }
    }

    pub fn rust_type(&self, t: &Ty) -> String {
        match t {
            Ty::U32 => "u32".into(),
            Ty::I32 => "i32".into(),
            Ty::U64 => "u64".into(),
            Ty::Count | Ty::Int => "usize".into(),
            Ty::Bool => "bool".into(),
            Ty::Str => "YString".into(),
            Ty::HostStr => "String".into(),
            Ty::StrLit => "&'static str".into(),
            Ty::Rune => "Rune".into(),
            Ty::List(e) => format!("List<{}>", self.rust_type(e)),
            Ty::Array(e, n) => format!("[{}; {n}]", self.rust_type(e)),
            Ty::Set(e) => format!("Set<{}>", self.rust_type(e)),
            Ty::Opt(e) => format!("Option<{}>", self.rust_type(e)),
            Ty::Iter(e) => format!("impl Iterator<Item = {}>", self.rust_type(e)),
            Ty::Res(a, b) => format!("Result<{}, {}>", self.rust_type(a), self.rust_type(b)),
            Ty::Named(n) | Ty::Cap(n) => crate::ty::external(n).map(|(r, _)| r.to_string()).unwrap_or_else(|| pascal(n)),
            Ty::Unit => "()".into(),
            Ty::Unknown => "_".into(),
        }
    }

    pub fn field_ty(&self, t: &Ty, field: &str) -> Option<(Ty, bool)> {
        let Ty::Named(n) = t else { return None };
        match &self.types.get(n)?.def {
            TypeDef::Struct { fields, derived, .. } => {
                fields.iter().find(|(f, _)| f == field).map(|(_, t)| (t.clone(), false)).or_else(|| derived.iter().find(|(f, _)| f == field).map(|(_, t)| (t.clone(), true)))
            }
            _ => None,
        }
    }

    /// The Rust path of a member of the module, from the module's own file: its facet's module when it has one.
    pub fn facet_of(&self, name: &str) -> Option<&str> {
        self.types.get(name).and_then(|t| t.facet.as_deref()).or_else(|| self.fns.get(name).and_then(|f| f.facet.as_deref())).or_else(|| self.consts.get(name).and_then(|c| c.facet.as_deref()))
    }
}

/// The points where a function may suspend: a capability's wait (`window.next_frame()`, `site.get(…)`), or a call of
/// a function that may suspend.
pub fn suspends_at(call: &Node, suspending: &BTreeSet<String>) -> Option<String> {
    let callee = call.node("callee")?;
    match callee.kind {
        "member" => {
            let obj = callee.node("object")?;
            let m = callee.str("name")?;
            let o = obj.str("name").filter(|_| obj.kind == "name")?;
            ((o == "window" && m == "next_frame") || (o == "site" && m == "get")).then(|| format!("{o}.{m}"))
        }
        "name" => callee.str("name").filter(|n| suspending.contains(*n)).map(str::to_string),
        _ => None,
    }
}

fn suspending(decls: &[(Option<String>, &Node)], outer: Option<&BTreeSet<String>>) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = outer.cloned().unwrap_or_default();
    loop {
        let before = out.len();
        for (_, d) in decls.iter().filter(|(_, d)| d.kind == "function") {
            let mut found = false;
            for b in d.nodes("body") {
                b.walk(&mut |n: &Node| {
                    if n.kind == "call" && suspends_at(n, &out).is_some() {
                        found = true;
                    }
                });
            }
            if found {
                out.insert(d.str("name").unwrap().to_string());
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The parameters a function keeps: bound whole to a `mutable` name and not read again.
fn kept_params(d: &Node) -> BTreeSet<String> {
    let params: BTreeSet<String> = d.nodes("params").iter().map(|p| p.str("name").unwrap().to_string()).collect();
    let mut bound = vec![];
    let mut reads: Vec<(String, usize)> = vec![];
    let mut stored = BTreeSet::new();
    for b in d.nodes("body") {
        b.walk(&mut |n: &Node| {
            if n.kind == "let" && n.flag("mutable") {
                if let Some(v) = n.node("value").filter(|v| v.kind == "name") {
                    if params.contains(v.str("name").unwrap()) {
                        bound.push((v.str("name").unwrap().to_string(), n.line));
                    }
                }
            }
            if n.kind == "name" {
                reads.push((n.str("name").unwrap().to_string(), n.line));
            }
            // `game(word: word, …)`: a named argument of a call whose value is the parameter whole
            if n.kind == "call" && n.node("callee").is_some_and(|c| c.kind == "name") {
                for a in n.nodes("args") {
                    if let Some(v) = a.node("value").filter(|v| v.kind == "name") {
                        if a.str("name").is_some() && params.contains(v.str("name").unwrap()) {
                            stored.insert(v.str("name").unwrap().to_string());
                        }
                    }
                }
            }
        });
    }
    let mut out: BTreeSet<String> = bound.into_iter().filter(|(p, line)| !reads.iter().any(|(r, l)| r == p && l != line)).map(|(p, _)| p).collect();
    out.extend(stored);
    out
}
