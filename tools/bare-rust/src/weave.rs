//! weave: a bare body and its constraint file into Rust. It prints what the file says and infers nothing (proposal 6).
use crate::life::{self, SK};
use crate::types::Ty;
use crate::trace::{line_col, Kind, Origin, Span, SpanMap};
use crate::{all_children, collapse, module_path, named_children, parse_bare, text, Edit, Namer, R};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

#[derive(Clone)]
struct Scope {
    path: String,
    /// The type a method's `self` is: the impl's type, `Self` in a trait; `None` outside both.
    self_ty: Option<String>,
}

impl Scope {
    fn new(path: String) -> Scope {
        Scope { path, self_ty: None }
    }
}

/// A place in a bare body where a concrete type became a hole: a `let`, a cast, a type argument, a numeric literal
/// or a closure's parameter or return type. `filled` says whether the constraint file filled it.
#[derive(Clone, Debug)]
pub struct HoleSite {
    pub subject: String,
    /// `local`, `cast`, `targ`, `literal`, `closure_param` or `closure_return`.
    pub kind: &'static str,
    /// The ordinal among the sites of the same kind in the body (for `local`, the key with its `#k` suffix).
    pub n: usize,
    pub key: Option<String>,
    /// The bare range where the compiler reports an unresolved type for this site.
    pub range: (usize, usize),
    /// The bare range of the whole construct (a `let` with its initialiser, a closure with its body).
    pub full: (usize, usize),
    /// The bare range of the body (or initialiser) the site is in.
    pub body: (usize, usize),
    pub filled: bool,
}

impl HoleSite {
    /// The clause of the constraint file that would fill this hole.
    pub fn clause(&self) -> String {
        match &self.key {
            Some(k) => format!("local {k} : T"),
            None => format!("hint {} #{} : T", self.kind, self.n),
        }
    }
}

/// An entry of the constraint file that binds to nothing in the bare body: stale after a rename or an edit.
/// Weave prints nothing for it and says so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unbound {
    /// `unbound local`, `unbound hint` or `unbound subject`.
    pub what: &'static str,
    /// The subject the entry is under (for `unbound subject`, the subject itself).
    pub subject: String,
    /// The local's key, the hint's `kind #n`, or empty.
    pub name: String,
}

impl Unbound {
    pub fn show(&self) -> String {
        match self.what {
            "unbound subject" => format!("{} `{}`", self.what, self.subject),
            _ => format!("{} `{}` of `{}`", self.what, self.name, self.subject),
        }
    }
}

/// A woven file with its span map and hole sites.
pub struct Woven {
    pub text: String,
    pub map: SpanMap,
    pub sites: Vec<HoleSite>,
    pub bare: String,
    pub unbound: Vec<Unbound>,
}

impl Woven {
    /// "line:col" of a bare offset.
    pub fn bare_at(&self, offset: usize) -> (usize, usize) {
        line_col(&self.bare, offset)
    }
}

struct Weaver<'a> {
    src: &'a str,
    subs: HashMap<String, &'a Value>,
    holes: &'a Value,
    edits: Vec<(Edit, Origin)>,
    cur: Origin,
    sites: Vec<HoleSite>,
    namer: Namer,
    /// Subjects that an item of the body consulted, and the subjects whose `holes` entry a body was scanned for.
    used: HashSet<String>,
    scanned: HashSet<String>,
    unbound: Vec<Unbound>,
    /// Where (a byte of the bare body) the first error came from: the innermost item being woven.
    fail_at: Option<usize>,
}

/// A refusal of `weave`, with where in the bare body it comes from, if it has a place.
#[derive(Clone, Debug)]
pub struct WeaveError {
    /// The message (what `weave` returns as its error).
    pub msg: String,
    /// What to show beside the position, when it differs from `msg`.
    pub short: Option<String>,
    pub at: Option<(usize, usize)>,
}

impl From<String> for WeaveError {
    fn from(msg: String) -> Self {
        WeaveError { msg, short: None, at: None }
    }
}

impl From<&str> for WeaveError {
    fn from(msg: &str) -> Self {
        msg.to_string().into()
    }
}

fn missing<T>(what: impl std::fmt::Display) -> R<T> {
    Err(format!("slot missing: {what}"))
}

fn present(v: &Value) -> Option<&Value> {
    (!v.is_null()).then_some(v)
}

/// The text of a type slot: a type string, or host text.
fn ty_text(v: &Value, acc: &Value, lifetime: Option<&str>, need_access: bool, what: &str) -> R<String> {
    if let Some(h) = v.get("host").and_then(Value::as_str) {
        return Ok(h.to_string());
    }
    let Some(s) = v.as_str() else { return missing(what) };
    let ty = Ty::parse(s)?;
    if ty.has_ref() && need_access && acc.is_null() {
        return missing(format!("access of {what}"));
    }
    ty.rust(acc["rights"] == "write", lifetime)
}

fn bounds_text(v: &Value) -> String {
    let b: Vec<&str> = v.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    b.join(" + ")
}

fn generics_text(sub: &Value) -> String {
    let Some(g) = sub["generics"].as_array() else { return String::new() };
    if g.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = g
        .iter()
        .map(|e| {
            let b = bounds_text(&e["bounds"]);
            let n = e["name"].as_str().unwrap_or("");
            if b.is_empty() {
                n.to_string()
            } else {
                format!("{n}: {b}")
            }
        })
        .collect();
    format!("<{}>", parts.join(", "))
}

fn where_text(sub: &Value) -> String {
    let mut parts = vec![];
    for e in sub["generics"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let w = bounds_text(&e["where"]);
        if !w.is_empty() {
            parts.push(format!("{}: {w}", e["name"].as_str().unwrap_or("")));
        }
    }
    for e in sub["where"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        parts.push(format!("{}: {}", e["for"].as_str().unwrap_or(""), bounds_text(&e["bounds"])));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" where {}", parts.join(", "))
    }
}

/// A well-formed `#[must_use]` or `#[must_use = "…"]`, whitespace collapsed.
pub fn is_must_use_attr(t: &str) -> bool {
    t == "#[must_use]" || (t.starts_with("#[must_use = \"") && t.ends_with("\"]") && t.len() >= "#[must_use = \"\"]".len())
}

/// The attribute a `result must be used` clause of kind `text` stands for: the one its `source` cites, or the plain one.
fn must_use_text(sub: &Value, text: &str) -> Option<String> {
    let c = sub["clauses"].as_array()?.iter().find(|c| c["kind"] == "result must be used" && c["text"] == text)?;
    let cited = c["source"].as_str().and_then(|s| s.strip_prefix("attribute ")).filter(|a| is_must_use_attr(a));
    Some(format!("{} ", cited.unwrap_or("#[must_use]")))
}

fn vis_text(sub: &Value) -> R<String> {
    Ok(match sub["visible"].as_str() {
        None => String::new(),
        Some("public") => "pub ".into(),
        Some("crate") => "pub(crate) ".into(),
        Some("module") => "pub(self) ".into(),
        Some("path") => match sub["visible_path"].as_str() {
            Some("super") => "pub(super) ".into(),
            Some(p) => format!("pub(in {p}) "),
            None => return missing(format!("{}: visible_path", sub["name"])),
        },
        Some(v) => return Err(format!("{}: unsupported visible `{v}`", sub["name"])),
    })
}

impl<'a> Weaver<'a> {
    fn ins(&mut self, at: usize, t: String) {
        if !t.is_empty() {
            self.edits.push(((at, at, t), self.cur.clone()));
        }
    }

    fn rep(&mut self, s: usize, e: usize, t: String) {
        self.edits.push(((s, e, t), self.cur.clone()));
    }

    /// The slot the following edits print.
    fn at(&mut self, subject: &str, slot: impl Into<String>) {
        self.cur = Origin { kind: Kind::Slot, subject: subject.to_string(), slot: slot.into() };
    }

    fn at_hole(&mut self, subject: &str, slot: impl Into<String>) {
        self.cur = Origin { kind: Kind::Hole, subject: subject.to_string(), slot: slot.into() };
    }

    fn lookup(&mut self, name: &str) -> Option<&'a Value> {
        let s = self.subs.get(name).copied();
        if s.is_some() {
            self.used.insert(name.to_string());
        }
        s
    }

    /// The subject of an item. A missing one is refused with a ready-to-paste entry for it (see `hints`).
    fn find(&mut self, n: Node, sc: &Scope, name: &str) -> R<&'a Value> {
        match self.lookup(name) {
            Some(s) => Ok(s),
            None => {
                let entry = crate::hints::missing_entry(n, self.src, name, sc.self_ty.as_deref());
                missing(format!("no subject `{name}`\n{}", crate::hints::paste_note(&entry)))
            }
        }
    }

    fn items_in(&mut self, container: Node, sc: &Scope) -> R<()> {
        for c in named_children(container) {
            self.item(c, sc)?;
        }
        Ok(())
    }

    fn item(&mut self, n: Node, sc: &Scope) -> R<()> {
        let r = self.item_inner(n, sc);
        if r.is_err() && self.fail_at.is_none() {
            self.fail_at = Some(n.start_byte());
        }
        r
    }

    fn item_inner(&mut self, n: Node, sc: &Scope) -> R<()> {
        match n.kind() {
            "function_item" | "function_signature_item" => self.func(n, sc),
            "struct_item" => self.strukt(n, sc),
            "enum_item" => self.enm(n, sc),
            "trait_item" => self.trait_item(n, sc),
            "impl_item" => self.imp(n, sc),
            "const_item" | "static_item" => self.konst(n, sc),
            "type_item" => self.alias(n, sc),
            "mod_item" => self.module(n, sc),
            "use_declaration" => self.use_decl(n, sc),
            "foreign_mod_item" => match n.child_by_field_name("body") {
                Some(b) => self.items_in(b, sc),
                None => Ok(()),
            },
            _ => Ok(()),
        }
    }

    fn use_decl(&mut self, n: Node, sc: &Scope) -> R<()> {
        let arg = n.child_by_field_name("argument").ok_or("parse-error: use without a path")?;
        let sname = self.namer.unique(format!("{}::use {}", sc.path, collapse(text(arg, self.src))));
        if let Some(sub) = self.lookup(&sname) {
            let v = vis_text(sub)?;
            self.at(&sname, "visible");
            self.ins(n.start_byte(), v);
        }
        Ok(())
    }

    fn nested(&mut self, body: Node, name: &str) -> R<()> {
        fn find<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
            for c in named_children(n) {
                if crate::is_item_kind(c.kind()) {
                    out.push(c);
                } else if c.kind() != "token_tree" {
                    find(c, out);
                }
            }
        }
        let mut items = vec![];
        find(body, &mut items);
        let inner = Scope::new(name.to_string());
        for it in items {
            self.item(it, &inner)?;
        }
        Ok(())
    }

    fn module(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = text(n.child_by_field_name("name").unwrap(), self.src);
        let sname = self.namer.unique(format!("{}::{name}", sc.path));
        if let Some(sub) = self.lookup(&sname) {
            let v = vis_text(sub)?;
            self.at(&sname, "visible");
            self.ins(n.start_byte(), v);
        }
        if let Some(b) = n.child_by_field_name("body") {
            self.items_in(b, &Scope::new(format!("{}::{name}", sc.path)))?;
        }
        Ok(())
    }

    fn fields(&mut self, body: Node, fields: &Value, owner: &str, subject: &str, prefix: &str) -> R<()> {
        let src = self.src;
        let list = fields.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        match body.kind() {
            "field_declaration_list" => {
                let fds: Vec<Node> = named_children(body).into_iter().filter(|c| c.kind() == "field_declaration").collect();
                for (i, fd) in fds.iter().enumerate() {
                    let fname = text(fd.child_by_field_name("name").unwrap(), src);
                    let Some(f) = list.get(i).filter(|f| f["name"] == fname) else {
                        return missing(format!("{owner} field `{fname}`"));
                    };
                    let v = vis_text(f)?;
                    self.at(subject, format!("{prefix}fields[{i}].visible"));
                    self.ins(fd.start_byte(), v);
                    let t = ty_text(&f["type"], &f["access"], f["hold"]["lifetime"].as_str(), true, &format!("{owner} field `{fname}`"))?;
                    self.at(subject, format!("{prefix}fields[{i}].type"));
                    self.ins(fd.end_byte(), format!(": {t}"));
                }
            }
            "ordered_field_declaration_list" => {
                let holes: Vec<Node> = all_children(body).into_iter().filter(|c| c.kind() == "_").collect();
                for (i, h) in holes.iter().enumerate() {
                    let Some(f) = list.get(i) else { return missing(format!("{owner} field {i}")) };
                    let v = vis_text(f)?;
                    let t = ty_text(&f["type"], &f["access"], f["hold"]["lifetime"].as_str(), true, &format!("{owner} field {i}"))?;
                    self.at(subject, format!("{prefix}fields[{i}].type"));
                    self.rep(h.start_byte(), h.end_byte(), format!("{v}{t}"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn strukt(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(n, sc, &sname)?;
        self.at(&sname, "clauses");
        if let Some(a) = must_use_text(sub, "must be used") {
            self.ins(n.start_byte(), a);
        }
        self.at(&sname, "visible");
        self.ins(n.start_byte(), vis_text(sub)?);
        self.at(&sname, "generics");
        self.ins(name.end_byte(), generics_text(sub));
        let body = n.child_by_field_name("body");
        let w = where_text(sub);
        self.at(&sname, "where");
        match body {
            Some(b) if b.kind() == "ordered_field_declaration_list" => self.ins(b.end_byte(), w),
            _ => self.ins(name.end_byte(), w),
        }
        if let Some(b) = body {
            self.fields(b, &sub["fields"], &sname, &sname, "")?;
        }
        Ok(())
    }

    fn enm(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, src)));
        let sub = self.find(n, sc, &sname)?;
        self.at(&sname, "clauses");
        if let Some(a) = must_use_text(sub, "must be used") {
            self.ins(n.start_byte(), a);
        }
        self.at(&sname, "visible");
        self.ins(n.start_byte(), vis_text(sub)?);
        self.at(&sname, "generics");
        self.ins(name.end_byte(), generics_text(sub));
        self.at(&sname, "where");
        self.ins(name.end_byte(), where_text(sub));
        let list = sub["variants"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let vs: Vec<Node> = named_children(n.child_by_field_name("body").unwrap())
            .into_iter()
            .filter(|c| c.kind() == "enum_variant")
            .collect();
        for (i, v) in vs.iter().enumerate() {
            let vname = text(v.child_by_field_name("name").unwrap(), src);
            let Some(vf) = list.get(i).filter(|f| f["name"] == vname) else {
                return missing(format!("{sname} variant `{vname}`"));
            };
            if let Some(b) = v.child_by_field_name("body") {
                self.fields(b, &vf["fields"], &format!("{sname}::{vname}"), &sname, &format!("variants[{i}]."))?;
            }
        }
        Ok(())
    }

    fn konst(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(n, sc, &sname)?;
        self.at(&sname, "visible");
        self.ins(n.start_byte(), vis_text(sub)?);
        let lt = sub["hold"]["lifetime"].as_str();
        let t = ty_text(&sub["type"], &sub["access"], lt, true, &format!("{sname} type"))?;
        self.at(&sname, "type");
        self.ins(name.end_byte(), format!(": {t}"));
        if let Some(v) = n.child_by_field_name("value") {
            self.body_holes(v, &sname)?;
            self.nested(v, &sname)?;
        }
        Ok(())
    }

    fn alias(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(n, sc, &sname)?;
        self.at(&sname, "visible");
        self.ins(n.start_byte(), vis_text(sub)?);
        let mut t = generics_text(sub);
        let b = bounds_text(&sub["bounds"]);
        if !b.is_empty() {
            t += &format!(": {b}");
        }
        if let Some(ty) = present(&sub["type"]) {
            let lt = sub["hold"]["lifetime"].as_str();
            t += &format!(" = {}", ty_text(ty, &sub["access"], lt, true, &format!("{sname} type"))?);
        }
        t += &where_text(sub);
        self.at(&sname, "type");
        self.ins(name.end_byte(), t);
        Ok(())
    }

    fn trait_item(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(n, sc, &sname)?;
        self.at(&sname, "clauses");
        if let Some(a) = must_use_text(sub, "must be used") {
            self.ins(n.start_byte(), a);
        }
        self.at(&sname, "visible");
        self.ins(n.start_byte(), vis_text(sub)?);
        let mut t = generics_text(sub);
        let b = bounds_text(&sub["bounds"]);
        if !b.is_empty() {
            t += &format!(": {b}");
        }
        t += &where_text(sub);
        self.at(&sname, "generics");
        self.ins(name.end_byte(), t);
        self.items_in(n.child_by_field_name("body").unwrap(), &Scope { path: sname, self_ty: Some("Self".into()) })
    }

    fn imp(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let ty_n = n.child_by_field_name("type").unwrap();
        let ty_head = collapse(text(ty_n, src));
        let tr = n.child_by_field_name("trait");
        let sname = match tr {
            Some(t) => self.namer.unique(format!("{}::{ty_head} as {}", sc.path, collapse(text(t, src)))),
            None => self.namer.unique(format!("{}::{ty_head}::impl", sc.path)),
        };
        if let Some(sub) = self.lookup(&sname) {
            let kw = all_children(n).into_iter().find(|c| c.kind() == "impl").ok_or("parse-error: impl")?;
            self.at(&sname, "generics");
            self.ins(kw.end_byte(), generics_text(sub));
            if let Some(v) = present(&sub["self"]) {
                let t = ty_text(v, &Value::Null, None, false, &format!("{sname} self"))?;
                self.at(&sname, "self");
                self.rep(ty_n.start_byte(), ty_n.end_byte(), t);
            }
            if let (Some(t), Some(v)) = (tr, present(&sub["trait"])) {
                let tt = ty_text(v, &Value::Null, None, false, &format!("{sname} trait"))?;
                self.at(&sname, "trait");
                self.rep(t.start_byte(), t.end_byte(), tt);
            }
            self.at(&sname, "where");
            self.ins(ty_n.end_byte(), where_text(sub));
        }
        let path = if tr.is_some() { sname } else { format!("{}::{ty_head}", sc.path) };
        match n.child_by_field_name("body") {
            Some(b) => self.items_in(b, &Scope { path, self_ty: Some(ty_head.clone()) }),
            None => Ok(()),
        }
    }

    fn func(&mut self, f: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name_n = f.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name_n, src)));
        let sub = self.find(f, sc, &sname)?;
        // a clause that stands for host text: `#[must_use]` before the item, `const` after its visibility (language 6.14)
        let says = |kind: &str, t: &str| sub["clauses"].as_array().into_iter().flatten().any(|c| c["kind"] == kind && c["text"] == t);
        self.at(&sname, "clauses");
        if let Some(a) = must_use_text(sub, "result must be used") {
            self.ins(f.start_byte(), a);
        }
        self.at(&sname, "visible");
        self.ins(f.start_byte(), vis_text(sub)?);
        self.at(&sname, "clauses");
        if says("effects", "evaluable at compile time") {
            self.ins(f.start_byte(), "const ".into());
        }
        if sub["clauses"].as_array().into_iter().flatten().any(|c| c["kind"] == "obliges") {
            // `unsafe` follows `async` and comes before `extern`
            let mods = all_children(f).into_iter().find(|c| c.kind() == "function_modifiers");
            match mods {
                Some(m) => match all_children(m).into_iter().find(|c| c.kind() == "extern_modifier") {
                    Some(e) => self.ins(e.start_byte(), "unsafe ".into()),
                    None => self.ins(m.end_byte(), " unsafe".into()),
                },
                None => self.ins(f.start_byte(), "unsafe ".into()),
            }
        }
        self.at(&sname, "generics");
        self.ins(name_n.end_byte(), generics_text(sub));
        let params_n = f.child_by_field_name("parameters").unwrap();
        let slots = sub["params"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let ps: Vec<Node> = all_children(params_n)
            .into_iter()
            .filter(|c| matches!(c.kind(), "parameter" | "self_parameter" | "_"))
            .collect();
        if slots.len() != ps.len() {
            return Err(format!("{sname}: the constraint file has {} parameters, the body has {}", slots.len(), ps.len()));
        }
        let (mut self_ref, mut ref_params) = (false, vec![]);
        for (i, p) in ps.iter().enumerate() {
            let pname = if p.kind() == "self_parameter" { "self".to_string() } else { collapse(text(*p, src)) };
            let Some(slot) = slots.get(i).filter(|q| q["name"].as_str().is_some_and(|n| n == pname)) else {
                return missing(format!("{sname} parameter `{pname}`"));
            };
            let lt = slot["hold"]["lifetime"].as_str();
            if p.kind() == "self_parameter" {
                if present(&slot["access"]).is_none() {
                    continue; // an owned self
                }
                let mut s = String::from("&");
                if let Some(l) = lt {
                    s += &format!("{l} ");
                }
                if slot["access"]["rights"] == "write" {
                    s += "mut ";
                }
                self.at(&sname, format!("params[{i}].access"));
                self.rep(p.start_byte(), p.end_byte(), s + "self");
                self_ref = true;
            } else {
                let t = ty_text(&slot["type"], &slot["access"], lt, true, &format!("{sname} type of `{pname}`"))?;
                if present(&slot["access"]).is_some() {
                    ref_params.push(pname.clone());
                }
                self.at(&sname, format!("params[{i}].type"));
                self.ins(p.end_byte(), format!(": {t}"));
            }
        }
        if let Some(res) = present(&sub["result"]) {
            let lt = res["hold"]["lifetime"].as_str();
            let mut t = ty_text(&res["type"], &res["access"], lt, false, &format!("{sname} result type"))?;
            if let Some(e) = present(&res["fails"]) {
                t = format!("Result<{t}, {}>", ty_text(e, &Value::Null, None, false, &format!("{sname} fails"))?);
            }
            if let (None, Some(from)) = (lt, res["hold"]["from"].as_str()) {
                let ok = if self_ref { from == "self" } else { ref_params.len() <= 1 && ref_params.iter().all(|p| p == from) };
                if !ok {
                    return Err(format!("{sname}: result hold from `{from}` needs an explicit lifetime"));
                }
            }
            self.at(&sname, "result.type");
            self.ins(params_n.end_byte(), format!(" -> {t}"));
        }
        self.at(&sname, "where");
        self.ins(params_n.end_byte(), where_text(sub));
        if let Some(body) = f.child_by_field_name("body") {
            self.body_holes(body, &sname)?;
            self.nested(body, &sname)?;
        }
        Ok(())
    }

    fn body_holes(&mut self, body: Node, subject: &str) -> R<()> {
        let src = self.src;
        self.unmark(body);
        self.scanned.insert(subject.to_string());
        let h = &self.holes[subject];
        // Hints bind by kind and ordinal. An entry that no site takes (or a second one for the same site) is unbound.
        let hints: Vec<(&str, usize, &Value)> = h["hints"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|x| (x["kind"].as_str().unwrap_or(""), x["n"].as_u64().unwrap_or(0) as usize, x))
            .collect();
        let mut taken = vec![false; hints.len()];
        let mut keys_taken: HashSet<String> = HashSet::new();
        let hold = |x: &Value| ty_text(&x["type"], &Value::Null, None, false, &format!("{subject} hint"));
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut casts_after: Vec<(usize, String, Origin)> = vec![];
        let mut let_keys: HashMap<usize, String> = HashMap::new();
        let mut param_seen: HashMap<String, usize> = HashMap::new();
        for s in life::sites(body) {
            let node = s.node;
            let mut hint = |k: &str| {
                let i = (0..hints.len()).find(|i| !taken[*i] && hints[*i].0 == k && hints[*i].1 == s.n)?;
                taken[i] = true;
                Some(hints[i].2)
            };
            let mut site = HoleSite {
                subject: subject.to_string(),
                kind: "",
                n: s.n,
                key: None,
                range: (node.start_byte(), node.end_byte()),
                full: (node.start_byte(), node.end_byte()),
                body: (body.start_byte(), body.end_byte()),
                filled: false,
            };
            match s.kind {
                SK::Let => {
                    let Some(pat) = node.child_by_field_name("pattern") else { continue };
                    let key = collapse(text(pat, src));
                    let c = seen.entry(key.clone()).or_insert(0);
                    *c += 1;
                    let key = if *c == 1 { key } else { format!("{key}#{c}") };
                    let_keys.insert(node.id(), key.clone());
                    site.kind = "local";
                    site.range = (pat.start_byte(), pat.end_byte());
                    site.key = Some(key.clone());
                    if let Some(ty) = present(&h["locals"][&key]) {
                        keys_taken.insert(key.clone());
                        // `{"type": T, "cast": true}`: the initialiser is cast to the local's type.
                        let cast = ty.get("cast").and_then(Value::as_bool) == Some(true);
                        let ty = if cast { &ty["type"] } else { ty };
                        let t = ty_text(ty, &Value::Null, None, false, &format!("{subject} local `{key}`"))?;
                        // A cast local whose type was dropped (check 4) keeps its cast: `let x: _ = e as _;` fills no hole.
                        if cast && t == "_" {
                            self.at(subject, format!("locals[{key}]"));
                        } else {
                            self.at_hole(subject, format!("locals[{key}]"));
                        }
                        self.ins(pat.end_byte(), format!(": {t}"));
                        if cast {
                            let Some(v) = node.child_by_field_name("value") else {
                                return Err(format!(
                                    "weave: {subject} local `{key}` is a cast, but its `let` has no initialiser to cast"
                                ));
                            };
                            // The closing text goes in after every other edit at the end of the initialiser.
                            if life::binds_tighter_than_as(v) {
                                casts_after.push((v.end_byte(), format!(" as {t}"), self.cur.clone()));
                            } else {
                                self.ins(v.start_byte(), "(".into());
                                casts_after.push((v.end_byte(), format!(") as {t}"), self.cur.clone()));
                            }
                        }
                        site.filled = true;
                    }
                }
                SK::Cast | SK::Targ => {
                    let k = if s.kind == SK::Cast { "cast" } else { "targ" };
                    site.kind = k;
                    if let Some(x) = hint(k) {
                        let t = hold(x)?;
                        self.at_hole(subject, format!("hints[{k} #{}]", s.n));
                        self.rep(node.start_byte(), node.end_byte(), t);
                        site.filled = true;
                    }
                }
                SK::Lit => {
                    site.kind = "literal";
                    if let Some(x) = hint("literal") {
                        let t = hold(x)?;
                        let sep = x["sep"].as_str().unwrap_or("");
                        self.at_hole(subject, format!("hints[literal #{}]", s.n));
                        self.ins(node.end_byte(), format!("{sep}{t}"));
                        site.filled = true;
                    }
                }
                SK::ClParam => {
                    site.kind = "closure_param";
                    // A parameter of a closure that a `let` names is keyed `<let>.<parameter>` in `locals`.
                    let keyed = life::closure_let(node).and_then(|l| let_keys.get(&l.id())).map(|k| {
                        let pat = if node.kind() == "parameter" { node.child_by_field_name("pattern").unwrap_or(node) } else { node };
                        let pk = format!("{k}.{}", collapse(text(pat, src)));
                        let c = param_seen.entry(pk.clone()).or_insert(0);
                        *c += 1;
                        if *c == 1 { pk } else { format!("{pk}#{c}") }
                    });
                    if let Some(k) = &keyed {
                        // A `locals` entry, present or not, and identified as one (check 4 drops and restores it so).
                        site.kind = "local";
                        site.key = Some(k.clone());
                    }
                    if let Some(ty) = keyed.as_ref().and_then(|k| present(&h["locals"][k])) {
                        let k = keyed.clone().unwrap();
                        keys_taken.insert(k.clone());
                        let t = ty_text(ty, &Value::Null, None, false, &format!("{subject} local `{k}`"))?;
                        self.at_hole(subject, format!("locals[{k}]"));
                        self.ins(node.end_byte(), format!(": {t}"));
                        site.filled = true;
                    } else if let Some(x) = hint("closure_param") {
                        let t = hold(x)?;
                        self.at_hole(subject, format!("hints[closure_param #{}]", s.n));
                        self.ins(node.end_byte(), format!(": {t}"));
                        site.filled = true;
                    }
                }
                SK::ClRet => {
                    site.kind = "closure_return";
                    let ps = node.child_by_field_name("parameters");
                    if let Some(p) = ps {
                        site.range = (p.start_byte(), p.end_byte());
                    }
                    if let (Some(x), Some(ps)) = (hint("closure_return"), ps) {
                        let t = hold(x)?;
                        self.at_hole(subject, format!("hints[closure_return #{}]", s.n));
                        self.ins(ps.end_byte(), format!(" -> {t}"));
                        site.filled = true;
                    }
                }
            }
            self.sites.push(site);
        }
        for (at, t, origin) in casts_after {
            self.edits.push(((at, at, t), origin));
        }
        for k in h["locals"].as_object().into_iter().flatten().map(|(k, _)| k).filter(|k| !keys_taken.contains(*k)) {
            self.unbound.push(Unbound { what: "unbound local", subject: subject.to_string(), name: k.clone() });
        }
        for (i, (k, n, _)) in hints.iter().enumerate().filter(|(i, _)| !taken[*i]) {
            let _ = i;
            self.unbound.push(Unbound { what: "unbound hint", subject: subject.to_string(), name: format!("{k} #{n}") });
        }
        Ok(())
    }
}

impl Weaver<'_> {
    /// `rust! { <text> }` in a body weaves to the text.
    fn unmark(&mut self, n: Node) {
        if crate::is_item_kind(n.kind()) {
            return;
        }
        if let Some(t) = life::rust_marker(n, self.src) {
            let t = t.to_string();
            self.rep(n.start_byte(), n.end_byte(), t);
            return;
        }
        for c in named_children(n) {
            self.unmark(c);
        }
    }
}

/// Apply the edits in order of position, recording where each landed in the woven text.
fn apply_traced(src: &str, mut edits: Vec<(Edit, Origin)>) -> (String, SpanMap) {
    edits.sort_by_key(|e| e.0 .0);
    let (mut out, mut at, mut map) = (String::new(), 0, SpanMap::default());
    for ((s, e, t), origin) in edits {
        if s < at {
            continue; // nested inside an earlier cut
        }
        out.push_str(&src[at..s]);
        let start = out.len();
        out.push_str(&t);
        map.spans.push(Span { start, end: out.len(), bare_start: s, bare_end: e, origin });
        at = e;
    }
    out.push_str(&src[at..]);
    (out, map)
}

/// The path every subject of a file starts with: the crate and the modules of the file.
pub fn base_path(json: &Value) -> String {
    let name = json["crate"].as_str().and_then(|k| k.split_whitespace().next()).unwrap_or("crate").replace('-', "_");
    let mut path = vec![name];
    path.extend(module_path(json["module"].as_str().unwrap_or("")));
    path.join("::")
}

/// Weave a bare body and its constraint JSON into Rust, with the span map and the hole sites.
pub fn weave_traced(brs: &str, json: &Value) -> R<Woven> {
    weave_at(brs, json).map_err(|e| e.msg)
}

/// As `weave_traced`, and an error says where in the bare body it comes from.
pub fn weave_at(brs: &str, json: &Value) -> Result<Woven, WeaveError> {
    let tree = parse_bare(brs);
    if let Some(e) = crate::first_error(&tree) {
        let at = e.split_once(':').and_then(|(l, c)| Some((l.parse().ok()?, c.parse().ok()?)));
        return Err(match crate::hints::parse_hint_at(brs, json) {
            Some((hat, h)) => WeaveError { msg: format!("parse-error: bare body, at {}:{}: {h}", hat.0, hat.1), short: Some(h), at: Some(hat) },
            None => WeaveError { msg: format!("parse-error: bare body, at {e}"), short: Some("parse error in the bare body".into()), at },
        });
    }
    crate::hints::reject_placeholders(json)?;
    weave_checked(brs, json, tree).map_err(|(e, off)| WeaveError { at: off.map(|o| line_col(brs, o)), msg: e, short: None })
}

fn weave_checked(brs: &str, json: &Value, tree: tree_sitter::Tree) -> Result<Woven, (String, Option<usize>)> {
    let krate = json["crate"].as_str().ok_or_else(|| ("slot missing: crate".to_string(), None))?;
    let subjects = json["subjects"].as_array().ok_or_else(|| ("slot missing: subjects".to_string(), None))?;
    let subs: HashMap<String, &Value> = subjects.iter().filter_map(|s| Some((s["name"].as_str()?.to_string(), s))).collect();
    let _ = krate;
    let cur = Origin { kind: Kind::Slot, subject: String::new(), slot: String::new() };
    let mut w = Weaver {
        src: brs,
        subs,
        holes: &json["holes"],
        edits: vec![],
        cur,
        sites: vec![],
        namer: Namer::default(),
        used: HashSet::new(),
        scanned: HashSet::new(),
        unbound: vec![],
        fail_at: None,
    };
    if let Err(e) = w.items_in(tree.root_node(), &Scope::new(base_path(json))) {
        return Err((e, w.fail_at));
    }
    // Entries of the file that nothing in the body took: a subject with no item, a `holes` entry with no body.
    let mut unbound = std::mem::take(&mut w.unbound);
    let mut stale: Vec<&str> = subjects.iter().filter_map(|s| s["name"].as_str()).filter(|n| !w.used.contains(*n)).collect();
    for k in json["holes"].as_object().into_iter().flatten().map(|(k, _)| k.as_str()) {
        if !w.scanned.contains(k) && !stale.contains(&k) {
            stale.push(k);
        }
    }
    unbound.extend(stale.into_iter().map(|n| Unbound { what: "unbound subject", subject: n.to_string(), name: String::new() }));
    let (text, map) = apply_traced(brs, w.edits);
    Ok(Woven { text, map, sites: w.sites, bare: brs.to_string(), unbound })
}

/// Weave a bare body and its constraint JSON into Rust.
pub fn weave(brs: &str, json: &Value) -> R<String> {
    weave_traced(brs, json).map(|w| w.text)
}
