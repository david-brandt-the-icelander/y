//! The names a Y module declares, resolved over its tree: its facets, types, constant groups, functions and rules, with
//! the types their documentation gives.
use crate::ty::{pascal, Ty};
use std::collections::{BTreeMap, BTreeSet};
use y_parse::Node;

#[derive(Clone, Debug)]
pub enum TypeDef {
    Struct { fields: Vec<(String, Ty)>, derived: Vec<(String, Ty)> },
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
        Ty::Opt(x) => copy_in(m, x, depth + 1),
        Ty::Named(n) => match m.types.get(n).map(|t| &t.def) {
            Some(TypeDef::Struct { fields, .. }) => fields.iter().all(|(_, f)| copy_in(m, f, depth + 1)),
            Some(TypeDef::Enum { cases }) => cases.iter().all(|(_, fs)| fs.iter().all(|(_, f)| copy_in(m, f, depth + 1))),
            None => false,
        },
        _ => false,
    }
}

/// A type passed by value: a number, a truth value, or a type whose values are its cases alone.
pub fn by_value(m: &Module, t: &Ty) -> bool {
    match t {
        Ty::U32 | Ty::I32 | Ty::U64 | Ty::Count | Ty::Int | Ty::Bool | Ty::Rune => true,
        Ty::Named(n) => matches!(m.types.get(n).map(|t| &t.def), Some(TypeDef::Enum { cases }) if cases.iter().all(|(_, f)| f.is_empty())),
        _ => false,
    }
}

impl<'t> Module<'t> {
    pub fn new(tree: &'t Node) -> Module<'t> {
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
        let mut m = Module { name, tree, types: BTreeMap::new(), consts: BTreeMap::new(), fns: BTreeMap::new(), facets: vec![] };
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
                    for mem in d.nodes("members") {
                        let t = mem.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown);
                        match mem.kind {
                            "field" => fields.push((mem.str("name").unwrap().to_string(), t)),
                            "derived" => derived.push((mem.str("name").unwrap().to_string(), t)),
                            _ => {}
                        }
                    }
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Struct { fields, derived }, line: d.line });
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
            Ty::Res(a, b) => format!("Result<{}, {}>", self.rust_type(a), self.rust_type(b)),
            Ty::Named(n) | Ty::Cap(n) => crate::ty::external(n).map(|(r, _)| r.to_string()).unwrap_or_else(|| pascal(n)),
            Ty::Unit => "()".into(),
            Ty::Unknown => "_".into(),
        }
    }

    pub fn field_ty(&self, t: &Ty, field: &str) -> Option<(Ty, bool)> {
        let Ty::Named(n) = t else { return None };
        match &self.types.get(n)?.def {
            TypeDef::Struct { fields, derived } => {
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

/// The parameters a function keeps: bound whole to a `mutable` name, or put whole into a structure it builds.
fn kept_params(d: &Node) -> BTreeSet<String> {
    let params: BTreeSet<String> = d.nodes("params").iter().map(|p| p.str("name").unwrap().to_string()).collect();
    let mut out = BTreeSet::new();
    for b in d.nodes("body") {
        b.walk(&mut |n: &Node| {
            if n.kind == "let" && n.flag("mutable") {
                if let Some(v) = n.node("value").filter(|v| v.kind == "name") {
                    if params.contains(v.str("name").unwrap()) {
                        out.insert(v.str("name").unwrap().to_string());
                    }
                }
            }
        });
    }
    out
}
