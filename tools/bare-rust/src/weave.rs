//! weave: a bare body and its constraint file into Rust. It prints what the file says and infers nothing (proposal 6).
use crate::life::{self, SK};
use crate::types::Ty;
use crate::{all_children, apply, collapse, module_path, named_children, parse_bare, text, Edit, Namer, R};
use serde_json::Value;
use std::collections::HashMap;
use tree_sitter::Node;

#[derive(Clone)]
struct Scope {
    path: String,
}

struct Weaver<'a> {
    src: &'a str,
    subs: HashMap<String, &'a Value>,
    holes: &'a Value,
    edits: Vec<Edit>,
    namer: Namer,
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
            self.edits.push((at, at, t));
        }
    }

    fn find(&self, name: &str) -> R<&'a Value> {
        match self.subs.get(name) {
            Some(s) => Ok(*s),
            None => missing(format!("no subject `{name}`")),
        }
    }

    fn items_in(&mut self, container: Node, sc: &Scope) -> R<()> {
        for c in named_children(container) {
            self.item(c, sc)?;
        }
        Ok(())
    }

    fn item(&mut self, n: Node, sc: &Scope) -> R<()> {
        match n.kind() {
            "function_item" | "function_signature_item" => self.func(n, sc),
            "struct_item" => self.strukt(n, sc),
            "enum_item" => self.enm(n, sc),
            "trait_item" => self.trait_item(n, sc),
            "impl_item" => self.imp(n, sc),
            "const_item" | "static_item" => self.konst(n, sc),
            "type_item" => self.alias(n, sc),
            "mod_item" => self.module(n, sc),
            "foreign_mod_item" => match n.child_by_field_name("body") {
                Some(b) => self.items_in(b, sc),
                None => Ok(()),
            },
            _ => Ok(()),
        }
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
        let inner = Scope { path: name.to_string() };
        for it in items {
            self.item(it, &inner)?;
        }
        Ok(())
    }

    fn module(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = text(n.child_by_field_name("name").unwrap(), self.src);
        let sname = self.namer.unique(format!("{}::{name}", sc.path));
        if let Some(sub) = self.subs.get(&sname) {
            let v = vis_text(sub)?;
            self.ins(n.start_byte(), v);
        }
        if let Some(b) = n.child_by_field_name("body") {
            self.items_in(b, &Scope { path: format!("{}::{name}", sc.path) })?;
        }
        Ok(())
    }

    fn fields(&mut self, body: Node, sub: &Value, fields: &Value, owner: &str) -> R<()> {
        let src = self.src;
        let list = fields.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let _ = sub;
        match body.kind() {
            "field_declaration_list" => {
                let fds: Vec<Node> = named_children(body).into_iter().filter(|c| c.kind() == "field_declaration").collect();
                for (i, fd) in fds.iter().enumerate() {
                    let fname = text(fd.child_by_field_name("name").unwrap(), src);
                    let Some(f) = list.get(i).filter(|f| f["name"] == fname) else {
                        return missing(format!("{owner} field `{fname}`"));
                    };
                    let v = vis_text(f)?;
                    self.ins(fd.start_byte(), v);
                    let t = ty_text(&f["type"], &f["access"], f["hold"]["lifetime"].as_str(), true, &format!("{owner} field `{fname}`"))?;
                    self.ins(fd.end_byte(), format!(": {t}"));
                }
            }
            "ordered_field_declaration_list" => {
                let holes: Vec<Node> = all_children(body).into_iter().filter(|c| c.kind() == "_").collect();
                for (i, h) in holes.iter().enumerate() {
                    let Some(f) = list.get(i) else { return missing(format!("{owner} field {i}")) };
                    let v = vis_text(f)?;
                    let t = ty_text(&f["type"], &f["access"], f["hold"]["lifetime"].as_str(), true, &format!("{owner} field {i}"))?;
                    self.edits.push((h.start_byte(), h.end_byte(), format!("{v}{t}")));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn strukt(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(&sname)?;
        self.ins(n.start_byte(), vis_text(sub)?);
        self.ins(name.end_byte(), generics_text(sub));
        let body = n.child_by_field_name("body");
        let w = where_text(sub);
        match body {
            Some(b) if b.kind() == "ordered_field_declaration_list" => self.ins(b.end_byte(), w),
            _ => self.ins(name.end_byte(), w),
        }
        if let Some(b) = body {
            self.fields(b, sub, &sub["fields"], &sname)?;
        }
        Ok(())
    }

    fn enm(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, src)));
        let sub = self.find(&sname)?;
        self.ins(n.start_byte(), vis_text(sub)?);
        self.ins(name.end_byte(), generics_text(sub));
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
                self.fields(b, sub, &vf["fields"], &format!("{sname}::{vname}"))?;
            }
        }
        Ok(())
    }

    fn konst(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(&sname)?;
        self.ins(n.start_byte(), vis_text(sub)?);
        let lt = sub["hold"]["lifetime"].as_str();
        let t = ty_text(&sub["type"], &sub["access"], lt, true, &format!("{sname} type"))?;
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
        let Some(sub) = self.subs.get(&sname).copied() else { return Ok(()) };
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
        self.ins(name.end_byte(), t);
        Ok(())
    }

    fn trait_item(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let sub = self.find(&sname)?;
        self.ins(n.start_byte(), vis_text(sub)?);
        let mut t = generics_text(sub);
        let b = bounds_text(&sub["bounds"]);
        if !b.is_empty() {
            t += &format!(": {b}");
        }
        t += &where_text(sub);
        self.ins(name.end_byte(), t);
        self.items_in(n.child_by_field_name("body").unwrap(), &Scope { path: sname })
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
        if let Some(sub) = self.subs.get(&sname).copied() {
            let kw = all_children(n).into_iter().find(|c| c.kind() == "impl").ok_or("parse-error: impl")?;
            self.ins(kw.end_byte(), generics_text(sub));
            if let Some(v) = present(&sub["self"]) {
                let t = ty_text(v, &Value::Null, None, false, &format!("{sname} self"))?;
                self.edits.push((ty_n.start_byte(), ty_n.end_byte(), t));
            }
            if let (Some(t), Some(v)) = (tr, present(&sub["trait"])) {
                let tt = ty_text(v, &Value::Null, None, false, &format!("{sname} trait"))?;
                self.edits.push((t.start_byte(), t.end_byte(), tt));
            }
            self.ins(ty_n.end_byte(), where_text(sub));
        }
        let path = if tr.is_some() { sname } else { format!("{}::{ty_head}", sc.path) };
        match n.child_by_field_name("body") {
            Some(b) => self.items_in(b, &Scope { path }),
            None => Ok(()),
        }
    }

    fn func(&mut self, f: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name_n = f.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name_n, src)));
        let sub = self.find(&sname)?;
        self.ins(f.start_byte(), vis_text(sub)?);
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
                self.edits.push((p.start_byte(), p.end_byte(), s + "self"));
                self_ref = true;
            } else {
                let t = ty_text(&slot["type"], &slot["access"], lt, true, &format!("{sname} type of `{pname}`"))?;
                if present(&slot["access"]).is_some() {
                    ref_params.push(pname.clone());
                }
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
            self.ins(params_n.end_byte(), format!(" -> {t}"));
        }
        self.ins(params_n.end_byte(), where_text(sub));
        if let Some(body) = f.child_by_field_name("body") {
            self.body_holes(body, &sname)?;
            self.nested(body, &sname)?;
        }
        Ok(())
    }

    fn body_holes(&mut self, body: Node, subject: &str) -> R<()> {
        let src = self.src;
        let h = &self.holes[subject];
        if h.is_null() {
            return Ok(());
        }
        let mut hints: HashMap<(String, usize), &Value> = HashMap::new();
        for x in h["hints"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            hints.insert((x["kind"].as_str().unwrap_or("").to_string(), x["n"].as_u64().unwrap_or(0) as usize), x);
        }
        let hold = |x: &Value| ty_text(&x["type"], &Value::Null, None, false, &format!("{subject} hint"));
        let mut seen: HashMap<String, usize> = HashMap::new();
        for s in life::sites(body) {
            let node = s.node;
            let hint = |k: &str| hints.get(&(k.to_string(), s.n)).copied();
            match s.kind {
                SK::Let => {
                    let Some(pat) = node.child_by_field_name("pattern") else { continue };
                    let key = collapse(text(pat, src));
                    let c = seen.entry(key.clone()).or_insert(0);
                    *c += 1;
                    let key = if *c == 1 { key } else { format!("{key}#{c}") };
                    if let Some(ty) = present(&h["locals"][&key]) {
                        let t = ty_text(ty, &Value::Null, None, false, &format!("{subject} local `{key}`"))?;
                        self.ins(pat.end_byte(), format!(": {t}"));
                    }
                }
                SK::Cast | SK::Targ => {
                    if let Some(x) = hint(if s.kind == SK::Cast { "cast" } else { "targ" }) {
                        let t = hold(x)?;
                        self.edits.push((node.start_byte(), node.end_byte(), t));
                    }
                }
                SK::Lit => {
                    if let Some(x) = hint("literal") {
                        let t = hold(x)?;
                        let sep = x["sep"].as_str().unwrap_or("");
                        self.ins(node.end_byte(), format!("{sep}{t}"));
                    }
                }
                SK::ClParam => {
                    if let Some(x) = hint("closure_param") {
                        let t = hold(x)?;
                        self.ins(node.end_byte(), format!(": {t}"));
                    }
                }
                SK::ClRet => {
                    if let (Some(x), Some(ps)) = (hint("closure_return"), node.child_by_field_name("parameters")) {
                        let t = hold(x)?;
                        self.ins(ps.end_byte(), format!(" -> {t}"));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Weave a bare body and its constraint JSON into Rust.
pub fn weave(brs: &str, json: &Value) -> R<String> {
    let tree = parse_bare(brs);
    if let Some(e) = crate::first_error(&tree) {
        return Err(format!("parse-error: bare body, at {e}"));
    }
    let krate = json["crate"].as_str().ok_or("slot missing: crate")?;
    let name = krate.split_whitespace().next().unwrap_or("").replace('-', "_");
    let subjects = json["subjects"].as_array().ok_or("slot missing: subjects")?;
    let subs: HashMap<String, &Value> = subjects.iter().filter_map(|s| Some((s["name"].as_str()?.to_string(), s))).collect();
    let module = json["module"].as_str().unwrap_or("");
    let mut path = vec![name];
    path.extend(module_path(module));
    let mut w = Weaver { src: brs, subs, holes: &json["holes"], edits: vec![], namer: Namer::default() };
    w.items_in(tree.root_node(), &Scope { path: path.join("::") })?;
    Ok(apply(brs, w.edits))
}
