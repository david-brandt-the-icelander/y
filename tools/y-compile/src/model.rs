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
    /// `type sku is string<character_set=ascii>`: another name for a type the runtime gives.
    Alias(Ty),
    /// Cases, each with its fields; a constant group used as a closed type has cases with none.
    Enum { cases: Vec<(String, Vec<(String, Ty)>)> },
    /// `enumeration name [is type]:` its members in order, each with its value when it has one, and the type of the
    /// values.
    Enumeration { items: Vec<(String, Option<Node>)>, value: Option<Ty> },
    /// `state name:` the fields every state shares, and each state's own.
    State { fields: Vec<(String, Ty)>, cases: Vec<(String, Vec<(String, Ty)>)> },
    /// `command name:` its fields and its delivery policy.
    Command { fields: Vec<(String, Ty)>, policy: Node },
    /// `unit name symbol s is type`, or `is n other`: a unit, its symbol, and its base type or its factor of another.
    Unit { symbol: String, base: Ty, factor: Option<(i128, String)> },
    /// `type name native because "…"`: a type whose representation the backend gives.
    Native { because: String },
    /// `type name is collection:` or `mapping:` with its constraints: the kind of the runtime that keeps them.
    Collection { kind: String, clauses: Vec<String> },
    /// A trait: its methods' signatures.
    Trait,
}

#[derive(Clone, Debug)]
pub struct Schema {
    pub key: String,
    pub versions: Vec<(u32, Vec<(String, Ty)>)>,
    pub migrations: Vec<(u32, u32, Vec<Node>)>,
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
    /// What it is: `function`, `rule`, `transition`, `handler`, `workflow`, or `method` of an implementation.
    pub kind: String,
    /// A method's `me`: the type it is a method of, and whether it changes it (`mutable function`).
    pub me: Option<(String, bool)>,
    /// `native because "…"`, in place of a body.
    pub native: Option<String>,
    /// The open types its parameters and result name (`item_type`), each with its bounds, the traits a documentation
    /// entry gives it (`item_type: What the items are, as ordered.`): its Rust type parameters.
    pub generics: Vec<(String, Vec<String>)>,
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
    /// The types `main` imports, whose fields it reads and makes: their fields are public.
    pub exported: BTreeSet<String>,
    /// Each schema: its key field, its versions with their fields, and its migrations.
    pub schemas: BTreeMap<String, Schema>,
    /// The functions that may suspend (Y § 3.7): they wait for a capability that may, or call one that does.
    pub suspending: BTreeSet<String>,
    /// Each trait's methods, by the trait.
    pub traits: BTreeMap<String, Vec<FnInfo>>,
    /// Each implementation's methods, by the type and the method; and the traits each type implements.
    pub methods: BTreeMap<String, BTreeMap<String, FnInfo>>,
    pub implements: BTreeMap<String, Vec<String>>,
    /// The units, by their symbols.
    pub unit_symbols: BTreeMap<String, String>,
    /// Each structure that leaves a type open, with its open types in order and the bounds its `x_type is bound` lines
    /// give them.
    pub open: BTreeMap<String, Vec<(String, Vec<String>)>>,
    /// The names the module declares, and the module it is in.
    pub declared: BTreeSet<String>,
}

/// The bounds an open type may have that the module does not declare, and the Rust traits each is.
pub const BOUNDS: &[(&str, &[&str])] = &[
    ("any", &[]),
    ("equatable", &["PartialEq"]),
    ("ordered", &["Ord"]),
    ("hashable", &["std::hash::Hash", "Eq"]),
    ("shown", &["std::fmt::Display"]),
];

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

/// Whether a name used as a type is an open type: named `…_type` (Y § 3.1's convention), and neither declared by the
/// module nor given by the runtime. A declaration does not list type parameters; it leaves the type open, and a use
/// fills it (`pair<item_type=parcel>`).
pub fn is_open_name(name: &str, declared: &BTreeSet<String>) -> bool {
    name.ends_with("_type") && name.len() > 5 && !declared.contains(name) && crate::ty::external(name).is_none()
}

/// The open types a type names, in order, added to `out` once each.
pub fn open_in(t: &Ty, declared: &BTreeSet<String>, out: &mut Vec<String>) {
    match t {
        Ty::Named(n) if is_open_name(n, declared) => {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        Ty::List(e) | Ty::Set(e) | Ty::Array(e, _) | Ty::Opt(e) | Ty::Iter(e) | Ty::Bag(e) | Ty::Queue(e) | Ty::Shared(e) | Ty::Sensitive(e) => open_in(e, declared, out),
        Ty::Res(a, b) | Ty::SortedMap(a, b) => {
            open_in(a, declared, out);
            open_in(b, declared, out);
        }
        Ty::Kind(_, args) => args.iter().for_each(|(_, a)| open_in(a, declared, out)),
        Ty::Coll(_, args) => args.iter().for_each(|a| open_in(a, declared, out)),
        _ => {}
    }
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
        Ty::U32 | Ty::I32 | Ty::U64 | Ty::U16 | Ty::Count | Ty::Int | Ty::Bool | Ty::StrLit | Ty::Decimal(..) | Ty::Integer(..) | Ty::Index(_) | Ty::F64 => true,
        Ty::Named(n) if crate::ty::COPIED.contains(&n.as_str()) => true,
        Ty::Opt(x) => copy_in(m, x, depth + 1),
        Ty::Named(n) => match m.types.get(n).map(|t| &t.def) {
            Some(TypeDef::Struct { fields, .. }) => fields.iter().all(|(_, f)| copy_in(m, f, depth + 1)),
            Some(TypeDef::Enum { cases }) => cases.iter().all(|(_, fs)| fs.iter().all(|(_, f)| copy_in(m, f, depth + 1))),
            Some(TypeDef::Newtype { base, .. }) | Some(TypeDef::Alias(base)) => copy_in(m, base, depth + 1),
            Some(TypeDef::Enumeration { .. }) => true,
            Some(TypeDef::Unit { base, .. }) => copy_in(m, base, depth + 1),
            _ => false,
        },
        _ => false,
    }
}

/// Whether a type has Rust's `Eq`: every type but a picture's, and what holds one.
pub fn has_eq(m: &Module, t: &Ty) -> bool {
    eq_in(m, t, 0)
}

fn eq_in(m: &Module, t: &Ty, depth: usize) -> bool {
    if depth > 8 {
        return true;
    }
    match t {
        Ty::Named(n) if n == "picture" => false,
        Ty::List(x) | Ty::Set(x) | Ty::Opt(x) | Ty::Array(x, _) => eq_in(m, x, depth + 1),
        Ty::Named(n) => match m.types.get(n).map(|t| &t.def) {
            Some(TypeDef::Struct { fields, .. }) => fields.iter().all(|(_, f)| eq_in(m, f, depth + 1)),
            Some(TypeDef::Enum { cases }) => cases.iter().all(|(_, fs)| fs.iter().all(|(_, f)| eq_in(m, f, depth + 1))),
            _ => true,
        },
        _ => true,
    }
}

/// Whether a constant's value is one a Rust constant cannot hold: it calls a method (`bag.of(…)`), or makes a
/// structure that holds a list, text or a number with bounds; such a constant is made by a function.
pub fn needs_fn(m: &Module, v: &Node) -> bool {
    let mut found = false;
    v.walk(&mut |n: &Node| {
        if n.kind != "call" {
            return;
        }
        let c = n.node("callee").unwrap();
        if c.kind == "member" {
            found = true;
        }
        if let Some(TypeDef::Struct { fields, .. }) = c.str("name").filter(|_| c.kind == "name").and_then(|t| m.types.get(t)).map(|t| &t.def) {
            let heavy = |t: &Ty| matches!(m.resolve(t), Ty::List(_) | Ty::Ascii | Ty::Integer(..) | Ty::Bag(_) | Ty::Queue(_) | Ty::Str);
            if fields.iter().any(|(_, t)| heavy(t)) {
                found = true;
            }
        }
    });
    found
}

/// A type passed by value: a number, a truth value, or a type whose values are its cases alone.
pub fn by_value(m: &Module, t: &Ty) -> bool {
    match t {
        Ty::U32 | Ty::I32 | Ty::U64 | Ty::U16 | Ty::Count | Ty::Int | Ty::Bool | Ty::Rune | Ty::Decimal(..) | Ty::Integer(..) | Ty::F64 => true,
        Ty::Named(n) if crate::ty::COPIED.contains(&n.as_str()) => true,
        Ty::Opt(x) => by_value(m, x),
        Ty::Named(n) => matches!(m.types.get(n).map(|t| &t.def), Some(TypeDef::Enum { cases }) if cases.iter().all(|(_, f)| f.is_empty())) || matches!(m.types.get(n).map(|t| &t.def), Some(TypeDef::Enumeration { .. })) || matches!(m.types.get(n).map(|t| &t.def), Some(TypeDef::Unit { base, .. }) if by_value(m, base)),
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
            if matches!(d.kind, "function" | "transition" | "handler" | "workflow") {
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
        let mut m = Module { name, tree, types: BTreeMap::new(), consts: BTreeMap::new(), fns: BTreeMap::new(), facets: vec![], exported: BTreeSet::new(), schemas: BTreeMap::new(), suspending: BTreeSet::new(), traits: BTreeMap::new(), methods: BTreeMap::new(), implements: BTreeMap::new(), unit_symbols: BTreeMap::new(), open: BTreeMap::new(), declared: BTreeSet::new() };
        if let Some(o) = outer {
            m.types = o.types.clone();
            m.consts = o.consts.clone();
            m.traits = o.traits.clone();
            m.methods = o.methods.clone();
            m.implements = o.implements.clone();
            m.unit_symbols = o.unit_symbols.clone();
        }
        for d in tree.nodes("decls") {
            if d.kind == "facet" {
                let members = d.nodes("decls").iter().filter_map(|x| x.str("name").map(str::to_string)).collect();
                m.facets.push((d.str("name").unwrap().to_string(), d.line, d.str("visibility").unwrap_or("private").to_string(), members));
            }
        }
        // every name the module, or the one it is in, declares: a `…_type` among them is no open type
        m.declared = decls.iter().filter_map(|(_, d)| d.str("name").map(str::to_string)).collect();
        if let Some(o) = outer {
            m.declared.extend(o.declared.iter().cloned());
            m.open = o.open.clone();
        }
        let declared_types = m.declared.clone();
        for (facet, d) in &decls {
            let name = d.str("name").unwrap_or("").to_string();
            match d.kind {
                "structure" => {
                    let mut fields = vec![];
                    let mut derived = vec![];
                    let mut derived_values = vec![];
                    let mut invariants = vec![];
                    let mut open: Vec<(String, Vec<String>)> = vec![];
                    for mem in d.nodes("members") {
                        let t = mem.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown);
                        let fname = mem.str("name").unwrap_or("");
                        match mem.kind {
                            // `item_type is ordered`: the structure leaves `item_type` open, and bounds it
                            "field" if is_open_name(fname, &declared_types) => {
                                let bound = match &t { Ty::Named(b) => vec![b.clone()], _ => vec![] };
                                open.push((fname.to_string(), bound.into_iter().filter(|b| b != "any").collect()));
                            }
                            "field" => {
                                let mut found = vec![];
                                open_in(&t, &declared_types, &mut found);
                                for o in found {
                                    if !open.iter().any(|x| x.0 == o) {
                                        open.push((o, vec![]));
                                    }
                                }
                                fields.push((fname.to_string(), t))
                            }
                            "derived" => {
                                derived.push((mem.str("name").unwrap().to_string(), t));
                                derived_values.push((mem.line, mem.node("value").cloned().unwrap_or_else(|| Node::new("literal", mem.line))));
                            }
                            "invariant" => invariants.push((mem.str("name").unwrap().to_string(), mem.line, mem.node("condition").cloned().unwrap())),
                            _ => {}
                        }
                    }
                    if !open.is_empty() {
                        m.open.insert(name.clone(), open);
                    }
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Struct { fields, derived, derived_values, invariants }, line: d.line });
                }
                "type_decl" if d.str("native").is_some() => {
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Native { because: d.str("native").unwrap().to_string() }, line: d.line });
                }
                "type_decl" if !d.nodes("collection").is_empty() || d.node("type").and_then(|t| t.str("name")).is_some_and(|n| n == "collection" || n == "mapping") => {
                    let clauses: Vec<String> = d.nodes("collection").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
                    let shape = d.node("type").and_then(|t| t.str("name")).unwrap_or("collection");
                    let kind = collection_kind(shape, &clauses);
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Collection { kind, clauses }, line: d.line });
                }
                "type_decl" if d.node("type").is_some_and(|t| !t.nodes("args").is_empty()) && d.nodes("cases").is_empty() && d.nodes("where").is_empty() => {
                    let base = Ty::from_node(d.node("type").unwrap());
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Alias(base), line: d.line });
                }
                // a schema, version by version: the latest is the structure; each earlier version is how a record of
                // it is read, migrated by the `migrate` that names it
                "schema" => {
                    let version: u32 = d.str("version").and_then(|v| v.trim_start_matches('v').parse().ok()).unwrap_or(1);
                    let fields: Vec<(String, Ty)> = d.nodes("members").iter().filter(|x| x.kind == "field").map(|f| (f.str("name").unwrap().to_string(), f.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown))).collect();
                    let key = d.nodes("members").iter().find(|x| x.flag("key")).and_then(|x| x.str("name")).unwrap_or("").to_string();
                    let entry = m.schemas.entry(name.clone()).or_insert_with(|| Schema { key: key.clone(), versions: vec![], migrations: vec![] });
                    entry.versions.push((version, fields.clone()));
                    let latest = entry.versions.iter().map(|v| v.0).max().unwrap_or(version);
                    if version == latest {
                        m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Struct { fields, derived: vec![], derived_values: vec![], invariants: vec![] }, line: d.line });
                    }
                }
                "migrate" => {
                    let v = |k: &str| d.str(k).and_then(|v| v.trim_start_matches('v').parse::<u32>().ok()).unwrap_or(0);
                    let body: Vec<Node> = d.nodes("body").into_iter().cloned().collect();
                    m.schemas.entry(name.clone()).or_insert_with(|| Schema { key: String::new(), versions: vec![], migrations: vec![] }).migrations.push((v("from"), v("to"), body));
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
                "enumeration" => {
                    let items = d.nodes("items").iter().map(|i| (i.str("name").unwrap().to_string(), i.node("value").cloned())).collect();
                    let value = d.node("type").map(Ty::from_node);
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Enumeration { items, value }, line: d.line });
                }
                "state" => {
                    let fields = d.nodes("members").iter().filter(|x| x.kind == "field").map(|f| (f.str("name").unwrap().to_string(), f.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown))).collect();
                    let cases = d.nodes("cases").iter().map(|c| (c.str("name").unwrap().to_string(), c.nodes("fields").iter().map(|f| (f.str("name").unwrap().to_string(), f.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown))).collect())).collect();
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::State { fields, cases }, line: d.line });
                }
                "command" => {
                    let fields = d.nodes("members").iter().filter(|x| x.kind == "field").map(|f| (f.str("name").unwrap().to_string(), f.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown))).collect();
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Command { fields, policy: d.node("policy").cloned().unwrap() }, line: d.line });
                }
                "unit" => {
                    let symbol = d.str("symbol").unwrap_or("").to_string();
                    m.unit_symbols.insert(symbol.clone(), name.clone());
                    let factor = d.str("factor").and_then(|f| f.replace('_', "").parse::<i128>().ok()).zip(d.str("of").map(str::to_string));
                    let base = d.node("type").map(Ty::from_node).unwrap_or(Ty::Unknown);
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Unit { symbol, base, factor }, line: d.line });
                }
                "trait" => {
                    m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Trait, line: d.line });
                }
                "constants" | "service" => {
                    let items: Vec<(String, Option<Ty>, Node)> =
                        d.nodes("items").iter().map(|i| (i.str("name").unwrap().to_string(), i.node("type").map(Ty::from_node), i.node("value").cloned().unwrap_or_else(|| Node::new("literal", i.line)))).collect();
                    if used_as_type.contains(&name) && d.kind == "constants" {
                        let cases = items.iter().map(|(n, _, _)| (n.clone(), vec![])).collect();
                        m.types.insert(name.clone(), TypeInfo { name, facet: facet.clone(), def: TypeDef::Enum { cases }, line: d.line });
                    } else {
                        m.consts.insert(name.clone(), ConstGroup { name, facet: facet.clone(), items });
                    }
                }
                _ => {}
            }
        }
        // a structure that constants are made of keeps its text as the host's `&str`: a constant cannot make a string
        let mut in_constants = BTreeSet::new();
        for g in m.consts.values() {
            for (_, _, v) in &g.items {
                v.walk(&mut |n: &Node| {
                    if n.kind == "call" {
                        if let Some(c) = n.node("callee").filter(|c| c.kind == "name").and_then(|c| c.str("name")) {
                            in_constants.insert(c.to_string());
                        }
                    }
                });
            }
        }
        for t in in_constants {
            if let Some(TypeInfo { def: TypeDef::Struct { fields, .. }, .. }) = m.types.get_mut(&t) {
                for f in fields.iter_mut().filter(|f| f.1 == Ty::Str) {
                    f.1 = Ty::StrLit;
                }
            }
        }
        // a unit's base type, for one made of another
        let units: Vec<String> = m.types.iter().filter(|(_, t)| matches!(t.def, TypeDef::Unit { .. })).map(|(n, _)| n.clone()).collect();
        for _ in 0..4 {
            for u in &units {
                if let Some(TypeDef::Unit { factor: Some((_, of)), .. }) = m.types.get(u).map(|t| t.def.clone()) {
                    if let Some(TypeDef::Unit { base, .. }) = m.types.get(&of).map(|t| t.def.clone()) {
                        if let Some(TypeDef::Unit { base: b, .. }) = m.types.get_mut(u).map(|t| &mut t.def) {
                            *b = base;
                        }
                    }
                }
            }
        }
        for (facet, d) in &decls {
            if matches!(d.kind, "function" | "rule" | "transition" | "handler" | "workflow") {
                let f = m.function(d, facet.clone());
                m.fns.insert(f.name.clone(), f);
            }
            if d.kind == "trait" {
                let sigs = d.nodes("decls").iter().filter(|x| x.kind == "function").map(|x| m.function(x, facet.clone())).collect();
                m.traits.insert(d.str("name").unwrap().to_string(), sigs);
            }
            if d.kind == "implementation" {
                let (tr, ty) = (d.str("trait").unwrap().to_string(), d.str("type").unwrap().to_string());
                m.implements.entry(ty.clone()).or_default().push(tr);
                for x in d.nodes("decls").into_iter().filter(|x| x.kind == "function") {
                    let mut f = m.function(x, facet.clone());
                    f.kind = "method".into();
                    f.me = Some((ty.clone(), x.flag("mutable")));
                    m.methods.entry(ty.clone()).or_default().insert(f.name.clone(), f);
                }
            }
        }
        if let Some(o) = outer {
            m.suspending = o.suspending.clone();
        }
        m.suspending.extend(suspending(&decls, outer.map(|o| &o.suspending)));
        m
    }

    /// A callable's `FnInfo`: a lambda's or nested function's, as a function's.
    pub fn function_of(&self, d: &Node, facet: Option<String>) -> FnInfo {
        self.function(d, facet)
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
                // undocumented, a parameter named as a type is one of it (`rule taxed(line)`)
                let ty = doc_type(d, &pname).or_else(|| p.node("type").map(Ty::from_node)).unwrap_or_else(|| if self.types.contains_key(&pname) { Ty::Named(pname.clone()) } else { Ty::Unknown });
                let (mutable, injected) = (p.flag("mutable"), p.flag("injected"));
                let mode = if injected {
                    // a store is lent to change when the function makes a batch of it
                    let batches = { let mut b = false; for x in d.nodes("body") { x.walk(&mut |n: &Node| if n.kind == "member" && n.str("name") == Some("batch") && n.node("object").and_then(|o| o.str("name")) == Some(pname.as_str()) { b = true; }); } b };
                    match &ty {
                        Ty::Cap(c) if c == "forever" || c == "system" || c == "http" || c == "reveal" => Mode::Ref,
                        Ty::Shared(_) | Ty::Submit(_) => Mode::Ref,
                        Ty::Store(_) if !batches => Mode::Ref,
                        _ => Mode::Mut,
                    }
                } else if mutable {
                    Mode::Mut
                } else if by_value(self, &ty) || kept.contains(&pname) || matches!(ty, Ty::StateCase(..)) {
                    Mode::Value
                } else {
                    Mode::Ref
                };
                Param { name: pname, ty, mutable, injected, mode }
            })
            .collect();
        // the open types it names, in order, each bounded by what its documentation entry gives it
        let mut found = vec![];
        for p in &params {
            let p: &Param = p;
            open_in(&p.ty, &self.declared, &mut found);
        }
        if let Some((_, t)) = &result {
            open_in(t, &self.declared, &mut found);
        }
        let generics = found
            .into_iter()
            .map(|o| {
                let bounds = match doc_type(d, &o) { Some(Ty::Named(b)) if b != "any" => vec![b], _ => vec![] };
                (o, bounds)
            })
            .collect();
        FnInfo { name, facet, params, result, fails, rule, line: d.line, kind: d.kind.to_string(), me: None, native: d.str("native").map(str::to_string), generics }
    }

    pub fn rust_type(&self, t: &Ty) -> String {
        match t {
            Ty::U32 => "u32".into(),
            Ty::I32 => "i32".into(),
            Ty::U64 => "u64".into(),
            Ty::U16 => "u16".into(),
            Ty::Count | Ty::Int => "usize".into(),
            Ty::Bool => "bool".into(),
            Ty::Str => "YString".into(),
            Ty::HostStr => "String".into(),
            Ty::StrLit => "&str".into(),
            Ty::Rune => "Rune".into(),
            Ty::List(e) => format!("List<{}>", self.rust_type(e)),
            Ty::Array(e, n) => format!("[{}; {n}]", self.rust_type(e)),
            Ty::Set(e) => format!("Set<{}>", self.rust_type(e)),
            Ty::Opt(e) => format!("Option<{}>", self.rust_type(e)),
            Ty::Iter(e) => format!("impl Iterator<Item = {}>", self.rust_type(e)),
            Ty::Res(a, b) => format!("Result<{}, {}>", self.rust_type(a), self.rust_type(b)),
            // a unit is erased: its value's type (Y § 3.3, "Units are erased before code generation")
            Ty::Named(n) if matches!(self.types.get(n).map(|t| &t.def), Some(TypeDef::Unit { .. })) => match &self.types[n].def {
                TypeDef::Unit { base, .. } => self.rust_type(base),
                _ => unreachable!(),
            },
            Ty::Named(n) if matches!(self.types.get(n).map(|t| &t.def), Some(TypeDef::Trait)) => format!("dyn {}", pascal(n)),
            Ty::Named(n) | Ty::Cap(n) => crate::ty::external(n).map(|(r, _)| r.to_string()).unwrap_or_else(|| pascal(n)),
            Ty::StateCase(st, c) => format!("{}<{}::{}>", pascal(st), st, pascal(c)),
            Ty::Outbox(c) => format!("Outbox<{}>", self.rust_type(c)),
            Ty::Submit(c) => {
                let name = match &**c { Ty::Named(n) => n.clone(), _ => String::new() };
                let (r, e) = self.fns.get(&name).map(|f| (f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unit), f.fails.clone().unwrap_or(Ty::Unit))).unwrap_or((Ty::Unknown, Ty::Unknown));
                format!("Submit<{}, {}, {}>", self.rust_type(c), self.rust_type(&r), self.rust_type(&e))
            }
            Ty::Undelivered(e) => format!("Undelivered<{}>", self.rust_type(e)),
            Ty::Sensitive(t) => format!("Sensitive<{}>", self.rust_type(t)),
            Ty::Kind(..) => match self.resolve(t) {
                Ty::Kind(k, args) => {
                    let kind = match self.types.get(&k).map(|t| &t.def) { Some(TypeDef::Collection { kind, .. }) => kind.clone(), _ => pascal(&k) };
                    // a structure's open types, filled in the order the structure leaves them
                    let a: Vec<String> = match self.open.get(&k) {
                        Some(open) => open.iter().map(|(g, _)| args.iter().find(|a| &a.0 == g).map(|a| self.rust_type(&a.1)).unwrap_or_else(|| "_".into())).collect(),
                        None => args.iter().map(|(_, t)| self.rust_type(t)).collect(),
                    };
                    format!("{kind}<{}>", a.join(", "))
                }
                r => self.rust_type(&r),
            },
            Ty::Decimal(p, s) => format!("Decimal<{}>", crate::ware::decimal_args(*p, *s)),
            Ty::F64 => "f64".into(),
            Ty::Integer(lo, hi) => {
                let arg = |v: i128| if v < 0 { format!("{{ {v} }}") } else { v.to_string() };
                format!("Integer<{}, {}, {}>", crate::ty::range_digits(*lo, *hi), arg(*lo), arg(*hi))
            }
            Ty::Rational(_) => "Rational<Big, UNBOUNDED>".into(),
            Ty::Ascii => "AsciiString".into(),
            Ty::Bag(e) => format!("Bag<{}>", self.rust_type(e)),
            Ty::Queue(e) => format!("Queue<{}>", self.rust_type(e)),
            Ty::SortedMap(k, v) => format!("SortedMap<{}, {}>", self.rust_type(k), self.rust_type(v)),
            Ty::Coll(k, a) => format!("y_collections::{k}<{}>", a.iter().map(|t| self.rust_type(t)).collect::<Vec<_>>().join(", ")),
            Ty::Shared(e) => format!("Shared<{}>", self.rust_type(e)),
            Ty::Store(e) => format!("Store<{}>", self.rust_type(e)),
            Ty::Index(e) => format!("Index<{}>", self.rust_type(e)),
            Ty::Arena(e) => format!("Arena<{}>", self.rust_type(e)),
            Ty::Unit => "()".into(),
            Ty::Unknown => "_".into(),
        }
    }

    /// A type with its aliases resolved.
    pub fn resolve(&self, t: &Ty) -> Ty {
        match t {
            Ty::Named(n) => match self.types.get(n).map(|t| &t.def) {
                Some(TypeDef::Alias(b)) => b.clone(),
                _ => t.clone(),
            },
            // a kind of collection the module declares is the runtime's kind with the same constraints
            Ty::Kind(k, args) => {
                let arg = |n: &str| args.iter().find(|a| a.0 == n).map(|a| Box::new(a.1.clone())).unwrap_or(Box::new(Ty::Unknown));
                match self.types.get(k).map(|t| &t.def) {
                    Some(TypeDef::Collection { kind, .. }) => match kind.as_str() {
                        "List" => Ty::List(arg("element_type")),
                        "Set" => Ty::Set(arg("element_type")),
                        "Bag" => Ty::Bag(arg("element_type")),
                        "Queue" => Ty::Queue(arg("element_type")),
                        "SortedMap" => Ty::SortedMap(arg("key_type"), arg("value_type")),
                        k => match crate::ty::coll_kind(k) {
                            Some((r, ..)) => Ty::Coll(r, ["element_type", "key_type", "value_type"].iter().filter_map(|n| args.iter().find(|a| a.0 == *n).map(|a| a.1.clone())).collect()),
                            None => t.clone(),
                        },
                    },
                    _ => t.clone(),
                }
            }
            _ => t.clone(),
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

/// The runtime's kind for a collection's or a mapping's constraints (Y § 3.13): the standard library's kind with the
/// same constraints, which a backend keeps in one Rust type; an empty name when none has them.
pub fn collection_kind(shape: &str, clauses: &[String]) -> String {
    let has = |c: &str| clauses.iter().any(|x| x == c || x.starts_with(&format!("{c} ")));
    let offers: Vec<String> = clauses.iter().filter_map(|c| c.strip_prefix("offers ")).flat_map(|o| o.split(',').map(|w| w.trim().to_string())).collect();
    let offer = |w: &str| offers.iter().any(|o| o == w);
    let ordered = |by: &str| clauses.iter().any(|c| c == &format!("ordered by {by}") || c.starts_with(&format!("ordered by {by} ")));
    let kind = if shape == "mapping" {
        if has("values many") {
            "MultiMap"
        } else if ordered("key") {
            "SortedMap"
        } else if has("unordered") {
            "UnorderedMap"
        } else {
            "Map"
        }
    } else if has("unique") {
        if ordered("value") {
            "SortedSet"
        } else if has("unordered") {
            "UnorderedSet"
        } else {
            "Set"
        }
    } else if has("unordered") {
        "Bag"
    } else if ordered("value") {
        "SortedList"
    } else if offer("iterate_only") {
        "Enumerable"
    } else if offer("cursor") {
        "LinkedList"
    } else if has("count fixed") {
        "Array"
    } else if offer("back_add") && offer("front_take") && !offer("index") {
        "Queue"
    } else if offer("back") && !offer("front") && !offer("index") {
        "Stack"
    } else if offer("front") && offer("back") && !offer("index") {
        "Deque"
    } else {
        "List"
    };
    kind.to_string()
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
