//! strip: Rust source into a bare body and a constraint file (proposal 5).
use crate::life::{self, SK};
use crate::types::{Ty, RESERVED};
use crate::{
    all_children, apply, collapse, first_error, is_comment, is_item_kind, module_path, named_children, parse_bare,
    parse_rust, text, Edit, Namer, R,
};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

type Obj = Map<String, Value>;

fn refuse<T>(category: &str, detail: impl std::fmt::Display) -> R<T> {
    Err(format!("{category}: {detail}"))
}

#[derive(Clone)]
struct Scope {
    path: String,
    generics: Vec<String>,
    self_ty: String,
}

/// A type slot: the mapped type, and what its references say about access and holding.
struct Slot {
    ty: Value,
    access: Option<Value>,
    raw: Option<String>,
    lifetime: Option<String>,
    has_ref: bool,
    shares: bool,
}

struct Cx<'a> {
    generics: &'a [String],
    elide: &'a HashSet<String>,
}

struct Stripper<'a> {
    src: &'a str,
    cuts: Vec<Edit>,
    subjects: Vec<Value>,
    holes: Obj,
    namer: Namer,
}

fn access(mutable: bool) -> Value {
    if mutable {
        json!({"rights": "write", "guarantee": "exclusive"})
    } else {
        json!({"rights": "read", "guarantee": "readers"})
    }
}

fn real_kids(n: Node) -> Vec<Node> {
    named_children(n).into_iter().filter(|c| !is_comment(*c)).collect()
}

fn is_literal(k: &str) -> bool {
    matches!(
        k,
        "string_literal"
            | "integer_literal"
            | "float_literal"
            | "boolean_literal"
            | "char_literal"
            | "negative_literal"
            | "raw_string_literal"
            | "block"
    )
}

impl<'a> Stripper<'a> {
    // ----- types -----

    fn to_ty(&self, n: Node, cx: &Cx, refs: &mut Vec<(bool, Option<String>)>) -> Result<Ty, String> {
        let src = self.src;
        match n.kind() {
            "primitive_type" => match text(n, src) {
                "usize" => Ok(Ty::As(Box::new(Ty::Prim("u64".into())), "usize".into())),
                "isize" => Ok(Ty::As(Box::new(Ty::Prim("i64".into())), "isize".into())),
                "char" => Ok(Ty::As(Box::new(Ty::Char), "char".into())),
                "str" => Err("str".into()),
                p => Ok(Ty::Prim(p.into())),
            },
            "type_identifier" => match text(n, src) {
                "_" => Ok(Ty::Prim("_".into())),
                "String" => Ok(Ty::Text),
                t if RESERVED.contains(&t) => Err("reserved_name".into()),
                t => Ok(Ty::Name(t.into())),
            },
            "scoped_type_identifier" => {
                let t = collapse(text(n, src));
                let first = t.split("::").next().unwrap_or("");
                if first == "Self" || first.starts_with('<') || cx.generics.iter().any(|g| g == first) {
                    Err("assoc".into())
                } else {
                    Ok(Ty::Name(t))
                }
            }
            "reference_type" => {
                let lt = real_kids(n).into_iter().find(|c| c.kind() == "lifetime").map(|l| text(l, src).to_string());
                let lt = lt.filter(|l| l != "'_");
                let mutable = real_kids(n).iter().any(|c| c.kind() == "mutable_specifier");
                let inner = n.child_by_field_name("type").ok_or("other_reference")?;
                refs.push((mutable, lt));
                let t = if inner.kind() == "primitive_type" && text(inner, src) == "str" {
                    Ty::Text
                } else if inner.kind() == "type_identifier" && text(inner, src) == "String" {
                    Ty::As(Box::new(Ty::Text), "String".into())
                } else {
                    self.to_ty(inner, cx, refs)?
                };
                Ok(Ty::Ref(Box::new(t)))
            }
            "generic_type" => {
                let head = n.child_by_field_name("type").ok_or("other_generic")?;
                let args_n = n.child_by_field_name("type_arguments").ok_or("other_generic")?;
                let mut args = vec![];
                for a in real_kids(args_n) {
                    match a.kind() {
                        "lifetime" => return Err("lifetime_args".into()),
                        "type_binding" => return Err("assoc_binding".into()),
                        k if is_literal(k) => return Err("const_args".into()),
                        _ => args.push(self.to_ty(a, cx, refs)?),
                    }
                }
                let path = collapse(text(head, src));
                let last = path.rsplit("::").next().unwrap_or("").to_string();
                let one = |v: &mut Vec<Ty>| Box::new(v.remove(0));
                Ok(match (last.as_str(), args.len()) {
                    ("Option", 1) if path == "Option" => Ty::Opt(one(&mut args)),
                    ("Vec", 1) => Ty::As(Box::new(Ty::Coll("list".into(), args)), path),
                    ("HashSet" | "BTreeSet", 1) => Ty::As(Box::new(Ty::Coll("set".into(), args)), path),
                    ("HashMap" | "BTreeMap", 2) => Ty::As(Box::new(Ty::Coll("map".into(), args)), path),
                    ("Box" | "Rc" | "Arc", 1) => Ty::As(one(&mut args), path),
                    _ => Ty::App(path, args),
                })
            }
            "tuple_type" => {
                let v: Result<Vec<Ty>, String> = real_kids(n).into_iter().map(|c| self.to_ty(c, cx, refs)).collect();
                Ok(Ty::Tuple(v?))
            }
            "unit_type" => Ok(Ty::Tuple(vec![])),
            "array_type" => {
                let el = n.child_by_field_name("element").ok_or("other_array_type")?;
                let el = self.to_ty(el, cx, refs)?;
                let list = Ty::Coll("list".into(), vec![el]);
                Ok(match n.child_by_field_name("length") {
                    None => Ty::As(Box::new(list), "std::primitive::slice".into()),
                    Some(len) => Ty::As(
                        Box::new(Ty::Refined(Box::new(list), format!("size(value) == {}", collapse(text(len, src))))),
                        "std::primitive::array".into(),
                    ),
                })
            }
            "abstract_type" => Err("impl_trait".into()),
            "dynamic_type" => Err("dyn".into()),
            "function_type" => Err("fn_ptr".into()),
            "pointer_type" => Err("pointer".into()),
            "never_type" => Err("never".into()),
            "bounded_type" => Err("bounded".into()),
            "macro_invocation" => Err("macro".into()),
            k => Err(format!("other_{k}")),
        }
    }

    /// The text of a node with the lifetimes that are elided cut out of its references.
    fn host(&self, n: Node, form: String, elide: &HashSet<String>) -> Value {
        fn gather(n: Node, src: &str, elide: &HashSet<String>, out: &mut Vec<Edit>) {
            if n.kind() == "lifetime" {
                if elide.contains(text(n, src)) {
                    if let Some(next) = n.next_sibling() {
                        out.push((n.start_byte(), next.start_byte(), String::new()));
                    }
                }
                return;
            }
            for c in all_children(n) {
                gather(c, src, elide, out);
            }
        }
        let mut edits = vec![];
        gather(n, self.src, elide, &mut edits);
        let base = n.start_byte();
        let edits = edits.into_iter().map(|(s, e, t)| (s - base, e - base, t)).collect();
        let t = apply(&self.src[n.byte_range()], edits);
        json!({"host": collapse(&t), "form": form})
    }

    fn map_type(&self, n: Node, cx: &Cx) -> Slot {
        let mut refs = vec![];
        let fail = |form: String| Slot {
            ty: self.host(n, form, cx.elide),
            access: None,
            raw: None,
            lifetime: None,
            has_ref: false,
            shares: false,
        };
        let ty = match self.to_ty(n, cx, &mut refs) {
            Ok(t) => t,
            Err(f) => return fail(f),
        };
        let first = refs.first().cloned();
        if refs.iter().any(|r| Some(r) != first.as_ref()) {
            return fail("mixed_refs".into());
        }
        let (mutable, raw) = first.clone().unwrap_or((false, None));
        let lifetime = raw.clone().filter(|l| !cx.elide.contains(l));
        Slot {
            ty: json!(ty.show()),
            access: first.map(|_| access(mutable)),
            raw,
            lifetime,
            has_ref: !refs.is_empty(),
            shares: ty.shares(),
        }
    }

    /// A type in a hole: a reference with an access or a lifetime does not fit, and is carried as host text.
    fn loose(&self, n: Node, generics: &[String]) -> Value {
        let none = HashSet::new();
        let s = self.map_type(n, &Cx { generics, elide: &none });
        if s.has_ref && (s.lifetime.is_some() || s.access.as_ref().is_some_and(|a| a["rights"] == "write")) {
            return self.host(n, "ref_hole".into(), &none);
        }
        s.ty
    }

    // ----- slots -----

    fn vis(&mut self, n: Node, sub: &mut Obj) -> R<()> {
        let Some(v) = named_children(n).into_iter().find(|c| c.kind() == "visibility_modifier") else {
            return Ok(());
        };
        self.vis_node(v, sub)
    }

    fn vis_node(&mut self, v: Node, sub: &mut Obj) -> R<()> {
        let next = v.next_sibling().ok_or("parse-error: visibility without item")?;
        self.cuts.push((v.start_byte(), next.start_byte(), String::new()));
        let t: String = text(v, self.src).split_whitespace().collect();
        match t.as_str() {
            "pub" => sub.insert("visible".into(), json!("public")),
            "pub(crate)" | "crate" => sub.insert("visible".into(), json!("crate")),
            "pub(self)" => sub.insert("visible".into(), json!("module")),
            "pub(super)" => {
                sub.insert("visible".into(), json!("path"));
                sub.insert("visible_path".into(), json!("super"))
            }
            s if s.starts_with("pub(in") => {
                sub.insert("visible".into(), json!("path"));
                sub.insert("visible_path".into(), json!(s[6..s.len() - 1].to_string()))
            }
            s => return refuse("visibility", s),
        };
        Ok(())
    }

    fn bound_list(&self, tb: Node) -> Vec<String> {
        named_children(tb).into_iter().filter(|c| !is_comment(*c)).map(|c| collapse(text(c, self.src))).collect()
    }

    /// Generic parameters, bounds and `where`: returns (generics, where predicates that are not on a parameter,
    /// the names declared).
    fn generics(&mut self, item: Node, cut_from: usize, cut: bool, elide: &HashSet<String>) -> R<(Vec<Value>, Vec<Value>, Vec<String>)> {
        let src = self.src;
        let mut ents: Vec<(String, Vec<String>, Vec<String>)> = vec![];
        if let Some(tp) = item.child_by_field_name("type_parameters") {
            if cut {
                self.cuts.push((cut_from, tp.end_byte(), String::new()));
            }
            for c in named_children(tp) {
                match c.kind() {
                    "type_parameter" | "lifetime_parameter" => {
                        if c.child_by_field_name("default_type").is_some() {
                            return refuse("generic-default", text(c, src));
                        }
                        let name = text(c.child_by_field_name("name").unwrap(), src).to_string();
                        let b = c.child_by_field_name("bounds").map(|b| self.bound_list(b)).unwrap_or_default();
                        ents.push((name, b, vec![]));
                    }
                    "const_parameter" => return refuse("const-generics", text(c, src)),
                    k if is_comment(c) => {
                        let _ = k;
                    }
                    k => return refuse("generic-param", k),
                }
            }
        }
        let mut extra = vec![];
        if let Some(wc) = named_children(item).into_iter().find(|c| c.kind() == "where_clause") {
            if cut {
                let mut p = wc.prev_sibling();
                while p.is_some_and(is_comment) {
                    p = p.and_then(|x| x.prev_sibling());
                }
                let from = p.map(|x| x.end_byte()).unwrap_or(wc.start_byte());
                self.cuts.push((from, wc.end_byte(), String::new()));
            }
            // A predicate on a parameter attaches to it when the clause lists them first and in the order of
            // the parameters; otherwise every predicate stays in written order, so that weave prints it back.
            let mut preds: Vec<(String, Option<usize>, Vec<String>)> = vec![];
            for wp in named_children(wc).into_iter().filter(|c| c.kind() == "where_predicate") {
                let left = wp.child_by_field_name("left").unwrap();
                let l = collapse(text(left, src));
                let bl = wp.child_by_field_name("bounds").map(|b| self.bound_list(b)).unwrap_or_default();
                let simple = matches!(left.kind(), "type_identifier" | "lifetime");
                let at = if simple { ents.iter().position(|e| e.0 == l) } else { None };
                preds.push((l, at, bl));
            }
            let mut last: Option<usize> = None;
            let mut seen_other = false;
            let mut canonical = true;
            for (_, at, _) in &preds {
                match at {
                    Some(i) if !seen_other && last.is_none_or(|l| *i > l) => last = Some(*i),
                    Some(_) => canonical = false,
                    None => seen_other = true,
                }
            }
            for (l, at, bl) in preds {
                match at.filter(|_| canonical) {
                    Some(i) => ents[i].2.extend(bl),
                    None => extra.push(json!({"for": l, "bounds": bl})),
                }
            }
        }
        let names: Vec<String> = ents.iter().map(|e| e.0.clone()).collect();
        let gens = ents
            .into_iter()
            .filter(|e| !elide.contains(&e.0))
            .map(|(name, bounds, wh)| {
                let mut o = Obj::new();
                o.insert("name".into(), json!(name));
                o.insert("bounds".into(), json!(bounds));
                if !wh.is_empty() {
                    o.insert("where".into(), json!(wh));
                }
                Value::Object(o)
            })
            .collect();
        Ok((gens, extra, names))
    }

    fn put_generics(sub: &mut Obj, gens: Vec<Value>, extra: Vec<Value>) {
        if !gens.is_empty() {
            sub.insert("generics".into(), Value::Array(gens));
        }
        if !extra.is_empty() {
            sub.insert("where".into(), Value::Array(extra));
        }
    }

    fn type_slot_json(slot: &Slot, always_hold: bool) -> Obj {
        let mut o = Obj::new();
        o.insert("type".into(), slot.ty.clone());
        if let Some(a) = &slot.access {
            o.insert("access".into(), a.clone());
        }
        if slot.has_ref {
            if always_hold || slot.lifetime.is_some() {
                let mut h = Obj::new();
                h.insert("kind".into(), json!("borrows"));
                if let Some(l) = &slot.lifetime {
                    h.insert("lifetime".into(), json!(l));
                }
                o.insert("hold".into(), Value::Object(h));
            }
        } else if always_hold {
            o.insert("hold".into(), json!({"kind": if slot.shares { "shares" } else { "owns" }}));
        }
        o
    }

    // ----- holes -----

    fn skeleton(&self, t: Node) -> String {
        let src = self.src;
        if t.kind() == "generic_type" {
            if let (Some(h), Some(a)) = (t.child_by_field_name("type"), t.child_by_field_name("type_arguments")) {
                let args: Vec<Node> = named_children(a).into_iter().filter(|c| !is_comment(*c)).collect();
                let plain = args.iter().all(|c| !matches!(c.kind(), "lifetime" | "type_binding") && !is_literal(c.kind()));
                if plain && !args.is_empty() {
                    return format!("{}<{}>", text(h, src), vec!["_"; args.len()].join(", "));
                }
            }
        }
        "_".to_string()
    }

    fn body_holes(&mut self, body: Node, subject: &str, generics: &[String]) {
        let src = self.src;
        let mut locals = Obj::new();
        let mut hints: Vec<Value> = vec![];
        let mut seen: HashMap<String, usize> = HashMap::new();
        for s in life::sites(body) {
            let n = s.n;
            let node = s.node;
            match s.kind {
                SK::Let => {
                    life::mut_edits(node, &mut self.cuts);
                    let Some(pat) = node.child_by_field_name("pattern") else { continue };
                    let mut pe = vec![];
                    life::pat_mut_edits(pat, &mut pe);
                    let base = pat.start_byte();
                    let shifted = pe.into_iter().map(|(a, b, t)| (a - base, b - base, t)).collect();
                    let key = collapse(&apply(&src[pat.byte_range()], shifted));
                    let c = seen.entry(key.clone()).or_insert(0);
                    *c += 1;
                    let key = if *c == 1 { key } else { format!("{key}#{c}") };
                    if let Some(ty) = node.child_by_field_name("type") {
                        self.cuts.push((pat.end_byte(), ty.end_byte(), String::new()));
                        locals.insert(key, self.loose(ty, generics));
                    }
                }
                SK::Cast => {
                    if text(node, src) != "_" {
                        self.cuts.push((node.start_byte(), node.end_byte(), "_".into()));
                        hints.push(json!({"kind": "cast", "n": n, "type": self.loose(node, generics)}));
                    }
                }
                SK::Targ => {
                    let sk = self.skeleton(node);
                    if sk != text(node, src) {
                        self.cuts.push((node.start_byte(), node.end_byte(), sk));
                        hints.push(json!({"kind": "targ", "n": n, "type": self.loose(node, generics)}));
                    }
                }
                SK::Lit => {
                    if let Some((digits, sep, suffix)) = life::lit_suffix(text(node, src)) {
                        self.cuts.push((node.start_byte() + digits.len(), node.end_byte(), String::new()));
                        let ty = match suffix {
                            "usize" | "isize" => format!("{} as '{suffix}'", if suffix == "usize" { "u64" } else { "i64" }),
                            s => s.to_string(),
                        };
                        let mut h = json!({"kind": "literal", "n": n, "type": ty});
                        if sep {
                            h["sep"] = json!("_");
                        }
                        hints.push(h);
                    }
                }
                SK::ClParam => {
                    if node.kind() == "parameter" {
                        life::mut_edits(node, &mut self.cuts);
                        if let (Some(pat), Some(ty)) = (node.child_by_field_name("pattern"), node.child_by_field_name("type")) {
                            self.cuts.push((pat.end_byte(), ty.end_byte(), String::new()));
                            hints.push(json!({"kind": "closure_param", "n": n, "type": self.loose(ty, generics)}));
                        }
                    } else {
                        life::pat_mut_edits(node, &mut self.cuts);
                    }
                }
                SK::ClRet => {
                    if let (Some(rt), Some(ps)) = (node.child_by_field_name("return_type"), node.child_by_field_name("parameters")) {
                        self.cuts.push((ps.end_byte(), rt.end_byte(), String::new()));
                        hints.push(json!({"kind": "closure_return", "n": n, "type": self.loose(rt, generics)}));
                    }
                }
            }
        }
        if !locals.is_empty() || !hints.is_empty() {
            let mut h = Obj::new();
            if !locals.is_empty() {
                h.insert("locals".into(), Value::Object(locals));
            }
            if !hints.is_empty() {
                h.insert("hints".into(), Value::Array(hints));
            }
            self.holes.insert(subject.to_string(), Value::Object(h));
        }
    }

    // ----- items -----

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
            "union_item" => refuse("union", "unions are not supported"),
            "foreign_mod_item" => match n.child_by_field_name("body") {
                Some(b) => self.items_in(b, sc),
                None => Ok(()),
            },
            "trait_item" => self.trait_item(n, sc),
            "impl_item" => self.imp(n, sc),
            "const_item" | "static_item" => self.konst(n, sc),
            "type_item" | "associated_type" => self.alias(n, sc),
            "mod_item" => self.module(n, sc),
            "use_declaration" => self.use_decl(n, sc),
            _ => Ok(()),
        }
    }

    /// A `use` is a subject only to carry its visibility: named `<scope>::use <path>`.
    fn use_decl(&mut self, n: Node, sc: &Scope) -> R<()> {
        let arg = n.child_by_field_name("argument").ok_or("parse-error: use without a path")?;
        let sname = self.namer.unique(format!("{}::use {}", sc.path, collapse(text(arg, self.src))));
        let mut sub = Obj::new();
        self.vis(n, &mut sub)?;
        if !sub.is_empty() {
            let mut o = Obj::new();
            o.insert("kind".into(), json!("use"));
            o.insert("name".into(), json!(sname));
            o.extend(sub);
            self.subjects.push(Value::Object(o));
        }
        Ok(())
    }

    /// Items declared inside a function body are subjects too, named under the function.
    fn nested(&mut self, body: Node, sc: &Scope, name: &str) -> R<()> {
        fn find<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
            for c in named_children(n) {
                if is_item_kind(c.kind()) {
                    out.push(c);
                } else if c.kind() != "token_tree" {
                    find(c, out);
                }
            }
        }
        let mut items = vec![];
        find(body, &mut items);
        let inner = Scope { path: name.to_string(), generics: sc.generics.clone(), self_ty: sc.self_ty.clone() };
        for it in items {
            self.item(it, &inner)?;
        }
        Ok(())
    }

    fn module(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = text(n.child_by_field_name("name").unwrap(), self.src);
        let sname = self.namer.unique(format!("{}::{name}", sc.path));
        let mut sub = Obj::new();
        self.vis(n, &mut sub)?;
        if !sub.is_empty() {
            let mut o = Obj::new();
            o.insert("kind".into(), json!("module"));
            o.insert("name".into(), json!(sname));
            o.extend(sub);
            self.subjects.push(Value::Object(o));
        }
        if let Some(b) = n.child_by_field_name("body") {
            let inner = Scope { path: format!("{}::{name}", sc.path), ..sc.clone() };
            self.items_in(b, &inner)?;
        }
        Ok(())
    }

    fn fields(&mut self, body: Node, cx: &Cx) -> R<Vec<Value>> {
        let src = self.src;
        let mut out = vec![];
        match body.kind() {
            "field_declaration_list" => {
                for fd in named_children(body).into_iter().filter(|c| c.kind() == "field_declaration") {
                    let mut o = Obj::new();
                    let name = fd.child_by_field_name("name").unwrap();
                    o.insert("name".into(), json!(text(name, src)));
                    self.vis(fd, &mut o)?;
                    let ty = fd.child_by_field_name("type").unwrap();
                    let slot = self.map_type(ty, cx);
                    self.cuts.push((name.end_byte(), ty.end_byte(), String::new()));
                    o.extend(Self::type_slot_json(&slot, true));
                    out.push(Value::Object(o));
                }
            }
            "ordered_field_declaration_list" => {
                let mut vis: Option<Node> = None;
                for c in named_children(body) {
                    match c.kind() {
                        "attribute_item" | "line_comment" | "block_comment" => {}
                        "visibility_modifier" => vis = Some(c),
                        _ => {
                            let mut o = Obj::new();
                            if let Some(v) = vis.take() {
                                self.vis_node(v, &mut o)?;
                            }
                            let slot = self.map_type(c, cx);
                            self.cuts.push((c.start_byte(), c.end_byte(), "_".into()));
                            o.extend(Self::type_slot_json(&slot, true));
                            out.push(Value::Object(o));
                        }
                    }
                }
            }
            k => return refuse("fields", k),
        }
        Ok(out)
    }

    fn strukt(&mut self, n: Node, sc: &Scope) -> R<()> {
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, self.src)));
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!("type"));
        sub.insert("name".into(), json!(sname));
        self.vis(n, &mut sub)?;
        let none = HashSet::new();
        let (gens, extra, names) = self.generics(n, name.end_byte(), true, &none)?;
        Self::put_generics(&mut sub, gens, extra);
        if let Some(body) = n.child_by_field_name("body") {
            let mut g = sc.generics.clone();
            g.extend(names);
            let fields = self.fields(body, &Cx { generics: &g, elide: &none })?;
            sub.insert("fields".into(), Value::Array(fields));
        }
        self.subjects.push(Value::Object(sub));
        Ok(())
    }

    fn enm(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, src)));
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!("type"));
        sub.insert("name".into(), json!(sname));
        self.vis(n, &mut sub)?;
        let none = HashSet::new();
        let (gens, extra, names) = self.generics(n, name.end_byte(), true, &none)?;
        Self::put_generics(&mut sub, gens, extra);
        let mut g = sc.generics.clone();
        g.extend(names);
        let mut variants = vec![];
        for v in named_children(n.child_by_field_name("body").unwrap()).into_iter().filter(|c| c.kind() == "enum_variant") {
            let mut o = Obj::new();
            o.insert("name".into(), json!(text(v.child_by_field_name("name").unwrap(), src)));
            if let Some(b) = v.child_by_field_name("body") {
                let f = self.fields(b, &Cx { generics: &g, elide: &none })?;
                o.insert("fields".into(), Value::Array(f));
            }
            variants.push(Value::Object(o));
        }
        sub.insert("variants".into(), Value::Array(variants));
        self.subjects.push(Value::Object(sub));
        Ok(())
    }

    fn konst(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, src)));
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!(if n.kind() == "const_item" { "const" } else { "static" }));
        sub.insert("name".into(), json!(sname));
        self.vis(n, &mut sub)?;
        let none = HashSet::new();
        let ty = n.child_by_field_name("type").ok_or("parse-error: const without type")?;
        let slot = self.map_type(ty, &Cx { generics: &sc.generics, elide: &none });
        self.cuts.push((name.end_byte(), ty.end_byte(), String::new()));
        sub.extend(Self::type_slot_json(&slot, false));
        self.subjects.push(Value::Object(sub));
        if let Some(v) = n.child_by_field_name("value") {
            self.body_holes(v, &sname, &sc.generics);
            self.nested(v, sc, &sname)?;
        }
        Ok(())
    }

    fn alias(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, src)));
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!("alias"));
        sub.insert("name".into(), json!(sname));
        self.vis(n, &mut sub)?;
        let none = HashSet::new();
        let (gens, extra, names) = self.generics(n, name.end_byte(), false, &none)?;
        Self::put_generics(&mut sub, gens, extra);
        if let Some(b) = n.child_by_field_name("bounds") {
            sub.insert("bounds".into(), json!(self.bound_list(b)));
        }
        if let Some(ty) = n.child_by_field_name("type") {
            let mut g = sc.generics.clone();
            g.extend(names);
            let slot = self.map_type(ty, &Cx { generics: &g, elide: &none });
            sub.extend(Self::type_slot_json(&slot, false));
        }
        let semi = all_children(n).into_iter().rev().find(|c| c.kind() == ";").ok_or("parse-error: type without `;`")?;
        self.cuts.push((name.end_byte(), semi.start_byte(), String::new()));
        self.subjects.push(Value::Object(sub));
        Ok(())
    }

    fn trait_item(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name = n.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name, src)));
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!("trait"));
        sub.insert("name".into(), json!(sname));
        self.vis(n, &mut sub)?;
        let none = HashSet::new();
        let (gens, extra, names) = self.generics(n, name.end_byte(), true, &none)?;
        Self::put_generics(&mut sub, gens, extra);
        if let Some(b) = n.child_by_field_name("bounds") {
            sub.insert("bounds".into(), json!(self.bound_list(b)));
            let from = b.prev_sibling().map(|p| p.end_byte()).unwrap_or(b.start_byte());
            self.cuts.push((from, b.end_byte(), String::new()));
        }
        self.subjects.push(Value::Object(sub));
        let mut g = sc.generics.clone();
        g.extend(names);
        let inner = Scope { path: sname, generics: g, self_ty: "Self".into() };
        self.items_in(n.child_by_field_name("body").unwrap(), &inner)
    }

    /// The head name of an `impl` type or trait, and whether the written type has more than the name.
    fn head(&self, n: Node) -> R<(String, bool)> {
        let src = self.src;
        match n.kind() {
            "type_identifier" | "scoped_type_identifier" => Ok((collapse(text(n, src)), false)),
            "generic_type" => Ok((collapse(text(n.child_by_field_name("type").unwrap(), src)), true)),
            "reference_type" => {
                let (h, _) = self.head(n.child_by_field_name("type").ok_or("parse-error: reference")?)?;
                Ok((h, true))
            }
            k => refuse("impl-type", k),
        }
    }

    fn imp(&mut self, n: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        if all_children(n).iter().any(|c| c.kind() == "!") {
            return refuse("negative-impl", "negative impls are not supported");
        }
        let ty_n = n.child_by_field_name("type").unwrap();
        let body = n.child_by_field_name("body").ok_or("impl-type: impl without a body")?;
        let (ty_head, ty_complex) = self.head(ty_n)?;
        let tr = match n.child_by_field_name("trait") {
            Some(t) => Some((t, self.head(t)?)),
            None => None,
        };
        let sname = match &tr {
            Some((_, (th, _))) => self.namer.unique(format!("{}::{ty_head} as {th}", sc.path)),
            None => self.namer.unique(format!("{}::{ty_head}::impl", sc.path)),
        };
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!("impl"));
        sub.insert("name".into(), json!(sname));
        let kw = all_children(n).into_iter().find(|c| c.kind() == "impl").ok_or("parse-error: impl")?;
        let none = HashSet::new();
        let (gens, extra, names) = self.generics(n, kw.end_byte(), true, &none)?;
        Self::put_generics(&mut sub, gens, extra);
        let mut g = sc.generics.clone();
        g.extend(names);
        let cx = Cx { generics: &g, elide: &none };
        let mut self_ty = ty_head.clone();
        if ty_complex {
            let slot = self.map_type(ty_n, &cx);
            let v = if slot.has_ref { self.host(ty_n, "ref_self".into(), &none) } else { slot.ty };
            if let Some(s) = v.as_str() {
                self_ty = s.to_string();
            } else {
                self_ty = "Self".into();
            }
            sub.insert("self".into(), v);
            self.cuts.push((ty_n.start_byte(), ty_n.end_byte(), ty_head.clone()));
        }
        if let Some((t, (th, complex))) = &tr {
            if *complex {
                sub.insert("trait".into(), self.map_type(*t, &cx).ty);
                self.cuts.push((t.start_byte(), t.end_byte(), th.clone()));
            }
        }
        let emit = tr.is_some() || sub.len() > 2;
        if emit {
            self.subjects.push(Value::Object(sub));
        }
        let inner = Scope {
            path: if tr.is_some() { sname } else { format!("{}::{ty_head}", sc.path) },
            generics: g,
            self_ty,
        };
        let _ = src;
        self.items_in(body, &inner)
    }

    fn func(&mut self, f: Node, sc: &Scope) -> R<()> {
        let src = self.src;
        let name_n = f.child_by_field_name("name").unwrap();
        let sname = self.namer.unique(format!("{}::{}", sc.path, text(name_n, src)));
        let life = life::analyse(f, src);
        let mut sub = Obj::new();
        sub.insert("kind".into(), json!("function"));
        sub.insert("name".into(), json!(sname));
        self.vis(f, &mut sub)?;
        let (gens, extra, names) = self.generics(f, name_n.end_byte(), true, &life.elide)?;
        Self::put_generics(&mut sub, gens, extra);
        let mut g = sc.generics.clone();
        g.extend(names);
        let cx = Cx { generics: &g, elide: &life.elide };
        let params_n = f.child_by_field_name("parameters").unwrap();
        let mut params = vec![];
        let mut positions: Vec<(String, Vec<Option<String>>)> = vec![];
        let mut self_ref = false;
        for p in named_children(params_n) {
            match p.kind() {
                "attribute_item" | "line_comment" | "block_comment" => {}
                "self_parameter" => {
                    let selfn = named_children(p).into_iter().find(|c| c.kind() == "self").ok_or("parse-error: self")?;
                    let t = text(p, src);
                    let mut o = Obj::new();
                    o.insert("name".into(), json!("self"));
                    if t.starts_with('&') {
                        let mutable = named_children(p).iter().any(|c| c.kind() == "mutable_specifier");
                        let lt = named_children(p).into_iter().find(|c| c.kind() == "lifetime").map(|l| text(l, src).to_string());
                        let lt = lt.filter(|l| l != "'_");
                        o.insert("type".into(), json!(format!("ref<{}>", sc.self_ty)));
                        o.insert("access".into(), access(mutable));
                        if let Some(l) = lt.as_ref().filter(|l| !life.elide.contains(*l)) {
                            o.insert("hold".into(), json!({"kind": "borrows", "lifetime": l}));
                        }
                        positions.push(("self".into(), vec![lt]));
                        self_ref = true;
                    } else {
                        o.insert("type".into(), json!(sc.self_ty));
                        positions.push(("self".into(), vec![]));
                    }
                    self.cuts.push((p.start_byte(), selfn.start_byte(), String::new()));
                    params.push(Value::Object(o));
                }
                "parameter" => {
                    let pat = p.child_by_field_name("pattern").unwrap();
                    let ty = p.child_by_field_name("type").unwrap();
                    if pat.kind() == "self" {
                        return refuse("typed-self", text(p, src));
                    }
                    life::mut_edits(p, &mut self.cuts);
                    let mut pe = vec![];
                    life::pat_mut_edits(pat, &mut pe);
                    let base = pat.start_byte();
                    let pname = collapse(&apply(&src[pat.byte_range()], pe.into_iter().map(|(a, b, t)| (a - base, b - base, t)).collect()));
                    let slot = self.map_type(ty, &cx);
                    self.cuts.push((pat.end_byte(), ty.end_byte(), String::new()));
                    let mut o = Obj::new();
                    o.insert("name".into(), json!(pname));
                    o.extend(Self::type_slot_json(&slot, false));
                    positions.push((pname, life::positions(ty, src)));
                    params.push(Value::Object(o));
                }
                k => return refuse("parameter", k),
            }
        }
        sub.insert("params".into(), Value::Array(params));
        if let Some(rt) = f.child_by_field_name("return_type") {
            let mut res = Obj::new();
            let result_generic = (rt.kind() == "generic_type")
                .then(|| {
                    let head = collapse(text(rt.child_by_field_name("type")?, src));
                    let args: Vec<Node> = named_children(rt.child_by_field_name("type_arguments")?)
                        .into_iter()
                        .filter(|c| !is_comment(*c))
                        .collect();
                    (head == "Result" && args.len() == 2 && args.iter().all(|a| a.kind() != "lifetime")).then_some(args)
                })
                .flatten();
            let slot = match result_generic {
                Some(args) => {
                    let ok = self.map_type(args[0], &cx);
                    let err = self.map_type(args[1], &cx);
                    if err.has_ref {
                        self.map_type(rt, &cx)
                    } else {
                        res.insert("fails".into(), err.ty);
                        ok
                    }
                }
                None => self.map_type(rt, &cx),
            };
            res.extend(Self::type_slot_json(&slot, false));
            // A shared result reference is the default, as in the proposal's example.
            if res.get("access").is_some_and(|a| a["rights"] == "read") {
                res.remove("access");
            }
            if slot.has_ref {
                let e = if self_ref {
                    Some("self".to_string())
                } else {
                    let with: Vec<&(String, Vec<Option<String>>)> = positions.iter().filter(|(_, m)| !m.is_empty()).collect();
                    (with.len() == 1).then(|| with[0].0.clone())
                };
                let from = match &slot.raw {
                    Some(l) if l == "'static" => None,
                    Some(l) => positions.iter().find(|(_, m)| m.contains(&Some(l.clone()))).map(|(n, _)| n.clone()),
                    None => e,
                };
                let mut h = Obj::new();
                h.insert("kind".into(), json!("borrows"));
                if let Some(fr) = from {
                    h.insert("from".into(), json!(fr));
                }
                if let Some(l) = &slot.lifetime {
                    h.insert("lifetime".into(), json!(l));
                }
                res.insert("hold".into(), Value::Object(h));
            }
            sub.insert("result".into(), Value::Object(res));
            self.cuts.push((params_n.end_byte(), rt.end_byte(), String::new()));
        }
        self.subjects.push(Value::Object(sub));
        if let Some(body) = f.child_by_field_name("body") {
            self.body_holes(body, &sname, &g);
            self.nested(body, &Scope { generics: g.clone(), ..sc.clone() }, &sname)?;
        }
        Ok(())
    }
}

use tree_sitter::Node;

/// The byte ranges of every comment below a node, in source order.
fn collect_comments(n: Node, out: &mut Vec<(usize, usize)>) {
    if is_comment(n) {
        out.push((n.start_byte(), n.end_byte()));
        return;
    }
    for c in all_children(n) {
        collect_comments(c, out);
    }
}

/// Strip Rust source into (bare body, constraint JSON). `krate` is "name version"; `module` the file's path.
/// A refusal is an `Err` whose text starts with its category and a colon.
pub fn strip(src: &str, krate: &str, module: &str) -> R<(String, Value)> {
    let tree = parse_rust(src);
    if let Some(e) = first_error(&tree) {
        return refuse("parse-error", format!("Rust source, at {e}"));
    }
    let name = krate.split_whitespace().next().unwrap_or(krate).replace('-', "_");
    let mut path = vec![name];
    path.extend(module_path(module));
    let mut st = Stripper {
        src,
        cuts: vec![],
        subjects: vec![],
        holes: Obj::new(),
        namer: Namer::default(),
    };
    let sc = Scope { path: path.join("::"), generics: vec![], self_ty: "Self".into() };
    st.items_in(tree.root_node(), &sc)?;
    // A comment inside a span that strip cuts would be dropped: refuse the file (proposal 2.6, 2.7).
    let mut comments = vec![];
    collect_comments(tree.root_node(), &mut comments);
    for &(s, e, _) in st.cuts.iter().filter(|c| c.0 < c.1) {
        let at = comments.partition_point(|c| c.1 <= s);
        if comments.get(at).is_some_and(|c| c.0 < e) {
            let (l, c) = crate::trace::line_col(src, comments[at].0);
            return refuse("comment inside an erased production", format!("at {l}:{c}"));
        }
    }
    let bare = apply(src, std::mem::take(&mut st.cuts));
    if let Some(e) = first_error(&parse_bare(&bare)) {
        return refuse("bare-parse", format!("stripped body does not parse as bare Rust, at {e}"));
    }
    let mut out = Obj::new();
    out.insert("format".into(), json!("y-constraints/0"));
    out.insert("host".into(), json!("rust"));
    out.insert("crate".into(), json!(krate));
    out.insert("module".into(), json!(module));
    out.insert("status".into(), json!("lifted"));
    out.insert("subjects".into(), Value::Array(st.subjects));
    if !st.holes.is_empty() {
        out.insert("holes".into(), Value::Object(st.holes));
    }
    Ok((bare, Value::Object(out)))
}
