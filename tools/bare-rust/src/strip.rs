//! strip: Rust source into a bare body and a constraint file (proposal 5).
use crate::life::{self, SK};
use crate::types::{Bound, Ty, RESERVED};
use crate::{
    all_children, apply, collapse, first_error, is_comment, is_item_kind, module_path, named_children, parse_bare,
    parse_rust, text, Edit, Namer, R,
};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

type Obj = Map<String, Value>;

/// A cast that is the whole initialiser of a typed `let` is carried by the local: `let x: u8 = (a + b) as u8;` becomes
/// `let x = a + b;` with `locals: {"x": {"type": "u8", "cast": true}}`. This returns that cast, and the cuts that
/// remove it. The cast's target must be a primitive written as the local's type, and the brackets around
/// the operand must be exactly those weave puts back.
fn implicit_cast<'a>(node: Node<'a>, src: &str) -> Option<(Node<'a>, Vec<Edit>)> {
    let ty = node.child_by_field_name("type")?;
    let v = node.child_by_field_name("value")?;
    let target = v.child_by_field_name("type").filter(|_| v.kind() == "type_cast_expression")?;
    let operand = v.child_by_field_name("value")?;
    let t = text(ty, src);
    if text(target, src) != t || !crate::bind::carried_type(t) {
        return None;
    }
    if operand.kind() == "parenthesized_expression" {
        let inner = named_children(operand).into_iter().find(|c| !is_comment(*c))?;
        if life::binds_tighter_than_as(inner) || named_children(operand).len() != 1 {
            return None;
        }
        return Some((target, vec![(v.start_byte(), inner.start_byte(), String::new()), (inner.end_byte(), v.end_byte(), String::new())]));
    }
    life::binds_tighter_than_as(operand).then(|| (target, vec![(operand.end_byte(), v.end_byte(), String::new())]))
}

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
    /// A reference inside the bounds of an existential, which the value borrows through.
    bound_ref: bool,
    shares: bool,
}

struct Cx<'a> {
    /// The type parameters in scope. `T::Item` is an associated type whether or not `T` is one, so nothing reads this
    /// yet; a form that must tell a parameter from a path will.
    #[allow(dead_code)]
    generics: &'a [String],
    elide: &'a HashSet<String>,
}

struct Stripper<'a> {
    src: &'a str,
    cuts: Vec<Edit>,
    subjects: Vec<Value>,
    holes: Obj,
    namer: Namer,
    /// (type subject, entry): the impls of a trait whose self type is a type of this file, attached at the end.
    implements: Vec<(String, Value)>,
    /// Realizations of templates (language 3.5), for the whole file: name -> {template, args}, one name for each
    /// template with its arguments (`realized`, keyed by `realization_key`), unique among the file's identifiers.
    paths: crate::paths::Paths,
    realizations: Obj,
    realized: HashMap<String, String>,
    names_used: HashSet<String>,
    /// Names given in advance, by `realization_key` (strip of a crate: one name across the crate).
    forced: HashMap<String, String>,
}

/// What a strip of one file is told from outside it.
#[derive(Default, Clone)]
pub struct StripOpts {
    /// The name of each realization, by `realization_key`, chosen for the whole crate.
    pub realization_names: HashMap<String, String>,
}

/// The identity of a realization: its template's full path and its arguments.
pub fn realization_key(template: &str, args: &Value) -> String {
    format!("{template}{args}")
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
                // `Self::Item` and `T::Output` name the associated type that the implements slot or a bound fixes
                let t = collapse(text(n, src));
                if t.starts_with('<') {
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
                        "lifetime" => args.push(Ty::Lifetime(text(a, src).to_string())),
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
            // `(dyn A + B)`: a parenthesised bound list is not a one-tuple
            "tuple_type"
                if real_kids(n).len() == 1
                    && !all_children(n).iter().any(|c| c.kind() == ",")
                    && matches!(real_kids(n)[0].kind(), "bounded_type" | "dynamic_type" | "abstract_type") =>
            {
                self.to_ty(real_kids(n)[0], cx, refs)
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
            "abstract_type" | "dynamic_type" | "bounded_type" => self.exist(n, cx),
            "function_type" => Err("fn_ptr".into()),
            "pointer_type" => Err("pointer".into()),
            "never_type" => Err("never".into()),
            "macro_invocation" => Err("macro".into()),
            k => Err(format!("other_{k}")),
        }
    }

    /// `impl Trait` and `dyn Trait`, with the bounds that follow a `+`: `some` and `any`. The references inside the
    /// bounds are shared and unnamed, which is all the bounds of the type language can say.
    fn exist(&self, n: Node, cx: &Cx) -> Result<Ty, String> {
        fn flat<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
            if n.kind() == "bounded_type" {
                for c in real_kids(n) {
                    flat(c, out);
                }
            } else {
                out.push(n);
            }
        }
        let mut parts = vec![];
        flat(n, &mut parts);
        let (any, form) = match parts[0].kind() {
            "abstract_type" => (false, "impl_trait"),
            "dynamic_type" => (true, "dyn"),
            _ => return Err("bounded".into()),
        };
        let mut inner = vec![];
        let mut bounds = vec![];
        for (i, p) in parts.iter().enumerate() {
            if i == 0 && real_kids(*p).iter().any(|c| matches!(c.kind(), "for_lifetimes" | "type_parameters")) {
                return Err(form.into());
            }
            let t = if i == 0 { p.child_by_field_name("trait").ok_or(form)? } else { *p };
            bounds.push(match t.kind() {
                "lifetime" => Bound::Lifetime(text(t, self.src).to_string()),
                "type_identifier" | "scoped_type_identifier" => {
                    Bound::Iface { path: collapse(text(t, self.src)), args: vec![], assoc: vec![], call: None }
                }
                "generic_type" => {
                    let head = t.child_by_field_name("type").ok_or(form)?;
                    let (mut args, mut assoc) = (vec![], vec![]);
                    for a in real_kids(t.child_by_field_name("type_arguments").ok_or(form)?) {
                        match a.kind() {
                            "lifetime" => args.push(Ty::Lifetime(text(a, self.src).to_string())),
                            "type_binding" => {
                                let name = text(a.child_by_field_name("name").ok_or(form)?, self.src).to_string();
                                assoc.push((name, self.to_ty(a.child_by_field_name("type").ok_or(form)?, cx, &mut inner)?));
                            }
                            k if is_literal(k) => return Err(form.into()),
                            _ => args.push(self.to_ty(a, cx, &mut inner)?),
                        }
                    }
                    Bound::Iface { path: collapse(text(head, self.src)), args, assoc, call: None }
                }
                "function_type" => {
                    let head = t.child_by_field_name("trait").ok_or(form)?;
                    let mut args = vec![];
                    for a in real_kids(t.child_by_field_name("parameters").ok_or(form)?) {
                        args.push(self.to_ty(a, cx, &mut inner)?);
                    }
                    let ret = match t.child_by_field_name("return_type") {
                        Some(r) => Some(Box::new(self.to_ty(r, cx, &mut inner)?)),
                        None => None,
                    };
                    Bound::Iface { path: collapse(text(head, self.src)), args: vec![], assoc: vec![], call: Some((args, ret)) }
                }
                _ => return Err(form.into()),
            });
        }
        if inner.iter().any(|(m, l)| *m || l.is_some()) {
            return Err(form.into());
        }
        Ok(if any { Ty::Any(bounds) } else { Ty::Some(bounds) })
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
            bound_ref: false,
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
            bound_ref: ty.bound_borrows(),
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
                    // `const K: usize` is carried as a generic named `const K` whose one bound is the host-text type `usize`
                    "const_parameter" => {
                        if c.child_by_field_name("value").is_some() {
                            return refuse("generic-default", text(c, src));
                        }
                        let name = text(c.child_by_field_name("name").ok_or("parse-error: const parameter")?, src);
                        let ty = c.child_by_field_name("type").ok_or("parse-error: const parameter")?;
                        ents.push((format!("const {name}"), vec![collapse(text(ty, src))], vec![]));
                    }
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
        let names: Vec<String> = ents.iter().map(|e| e.0.strip_prefix("const ").unwrap_or(&e.0).to_string()).collect();
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

    /// A type with lifetime parameters holds a borrow for each: `hold : borrows for 'a` (language 6.11).
    fn put_hold_for(sub: &mut Obj) {
        let lts: Vec<Value> = sub
            .get("generics")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|g| g["name"].as_str().filter(|n| n.starts_with('\'')).map(|n| json!(n)))
            .collect();
        if !lts.is_empty() {
            sub.insert("hold".into(), json!({"kind": "borrows", "for": lts}));
        }
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
        // A macro call that states a type, which no binding can carry: marked as Rust text (`rust! { ... }`).
        for m in life::typed_macro_calls(body, src) {
            self.cuts.push((m.start_byte(), m.start_byte(), "rust! { ".into()));
            self.cuts.push((m.end_byte(), m.end_byte(), " }".into()));
        }
        let mut locals = Obj::new();
        let mut hints: Vec<Value> = vec![];
        let mut seen: HashMap<String, usize> = HashMap::new();
        // Casts carried by a local leave the body, so the casts after them are numbered without them.
        let (mut carried, mut removed): (HashSet<usize>, usize) = (HashSet::new(), 0);
        // The key of each `let`, so that the parameters of a closure it names are keyed `<let>.<parameter>`.
        let mut let_keys: HashMap<usize, String> = HashMap::new();
        let mut param_seen: HashMap<String, usize> = HashMap::new();
        let mut done_targs: HashSet<usize> = HashSet::new();
        // Type arguments named by a realization leave the body, so the turbofish hints after them are numbered without.
        let mut removed_targs = 0;
        for s in life::sites(body) {
            let n = match s.kind {
                SK::Cast => s.n - removed,
                SK::Targ => s.n - removed_targs,
                _ => s.n,
            };
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
                    let_keys.insert(node.id(), key.clone());
                    if let Some(ty) = node.child_by_field_name("type") {
                        self.cuts.push((pat.end_byte(), ty.end_byte(), String::new()));
                        let t = self.loose(ty, generics);
                        if let Some((target, cuts)) = implicit_cast(node, src) {
                            carried.insert(target.id());
                            self.cuts.extend(cuts);
                            locals.insert(key, json!({"type": t, "cast": true}));
                        } else {
                            locals.insert(key, t);
                        }
                    }
                }
                SK::Cast if carried.contains(&node.id()) => removed += 1,
                SK::Cast => {
                    if text(node, src) != "_" {
                        self.cuts.push((node.start_byte(), node.end_byte(), "_".into()));
                        hints.push(json!({"kind": "cast", "n": n, "type": self.loose(node, generics)}));
                    }
                }
                SK::Targ => {
                    // A turbofish that states a type is a realization of a template, named in the body (language 3).
                    if let Some(ta) = node.parent().filter(|p| p.kind() == "type_arguments") {
                        if done_targs.contains(&ta.id()) {
                            removed_targs += 1;
                            continue;
                        }
                        if let Some((cut, template, base, is_type)) = realization_site(ta, src) {
                            let args: Vec<Node> = named_children(ta).into_iter().filter(|c| !is_comment(*c)).collect();
                            let plain = args.iter().all(|a| !matches!(a.kind(), "lifetime" | "type_binding") && !is_literal(a.kind()));
                            if plain && args.iter().any(|a| text(*a, src) != "_") {
                                done_targs.insert(ta.id());
                                let vals: Vec<Value> =
                                    args.iter().map(|a| if text(*a, src) == "_" { json!("_") } else { self.loose(*a, generics) }).collect();
                                let template = if template.starts_with("<_>::") { template } else { self.paths.full(&template) };
                                let key = realization_key(&template, &json!(vals));
                                let name = match self.realized.get(&key).or_else(|| self.forced.get(&key)) {
                                    Some(n) => n.clone(),
                                    None => realization_name(&base, is_type, &args.iter().map(|a| text(*a, src)).collect::<Vec<_>>(), &mut self.names_used),
                                };
                                if !self.realized.contains_key(&key) {
                                    self.realized.insert(key, name.clone());
                                    self.realizations.insert(name.clone(), json!({"template": template, "args": vals}));
                                }
                                self.cuts.push((cut.0, cut.1, name));
                                removed_targs += 1;
                                continue;
                            }
                        }
                    }
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
                            let t = self.loose(ty, generics);
                            match life::closure_let(node).and_then(|l| let_keys.get(&l.id())) {
                                Some(k) => {
                                    let mut pe = vec![];
                                    life::pat_mut_edits(pat, &mut pe);
                                    let base = pat.start_byte();
                                    let shifted = pe.into_iter().map(|(a, b, t)| (a - base, b - base, t)).collect();
                                    let pk = format!("{k}.{}", collapse(&apply(&src[pat.byte_range()], shifted)));
                                    let c = param_seen.entry(pk.clone()).or_insert(0);
                                    *c += 1;
                                    locals.insert(if *c == 1 { pk } else { format!("{pk}#{c}") }, t);
                                }
                                None => hints.push(json!({"kind": "closure_param", "n": n, "type": t})),
                            }
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

    /// A `#[must_use]` or `#[must_use = "…"]` right before an item is a clause (language 6.14.2): the attribute is cut, and
    /// the clause's `source` keeps its text, message included, so that `weave` can write it back.
    fn must_use_clause(&mut self, n: Node, kind_text: &str) -> Option<Value> {
        let a = n.prev_sibling().filter(|a| a.kind() == "attribute_item")?;
        let t = collapse(text(a, self.src));
        if !crate::weave::is_must_use_attr(&t) {
            return None;
        }
        self.cuts.push((a.start_byte(), n.start_byte(), String::new()));
        Some(json!({"kind": "result must be used", "text": kind_text, "source": format!("attribute {t}")}))
    }

    /// The `# Safety` section of the doc comment above an item, on one line; none where there is no such section.
    fn safety_doc(&self, n: Node) -> Option<String> {
        let mut lines: Vec<String> = vec![];
        let mut at = n.prev_sibling();
        while let Some(p) = at {
            match p.kind() {
                "line_comment" => {
                    let t = text(p, self.src);
                    if let Some(d) = t.strip_prefix("///").filter(|d| !d.starts_with('/')) {
                        lines.push(d.trim().to_string());
                    }
                }
                "attribute_item" | "block_comment" => {}
                _ => break,
            }
            at = p.prev_sibling();
        }
        lines.reverse();
        let start = lines.iter().position(|l| l.starts_with('#') && l.trim_start_matches('#').trim() == "Safety")?;
        let body: Vec<&str> = lines[start + 1..].iter().take_while(|l| !l.starts_with('#')).map(String::as_str).filter(|l| !l.is_empty()).collect();
        let t = body.join(" ").replace('"', "'");
        (!t.is_empty()).then_some(t)
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
        Self::put_hold_for(&mut sub);
        if let Some(c) = self.must_use_clause(n, "must be used") {
            sub.insert("clauses".into(), json!([c]));
        }
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
        Self::put_hold_for(&mut sub);
        if let Some(c) = self.must_use_clause(n, "must be used") {
            sub.insert("clauses".into(), json!([c]));
        }
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
        if let Some(c) = self.must_use_clause(n, "must be used") {
            sub.insert("clauses".into(), json!([c]));
        }
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

    /// The `trait` slot of an impl. A reference with a named lifetime inside a trait argument (`From<&'a str>`) has no
    /// slot of its own here, so the whole trait path stays as host text rather than losing the lifetime.
    fn trait_ty(&mut self, t: Node, cx: &Cx) -> Value {
        let slot = self.map_type(t, cx);
        let lost = slot.has_ref && (slot.lifetime.is_some() || slot.access.as_ref().is_some_and(|a| a["rights"] == "write"));
        if lost {
            self.host(t, "ref_trait".into(), cx.elide)
        } else {
            slot.ty
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
            // a shared, unnamed reference is the type's own `ref`; a `&mut` or a named lifetime has no slot here
            let hosted = slot.has_ref && (slot.lifetime.is_some() || slot.access.as_ref().is_some_and(|a| a["rights"] == "write"));
            let v = if hosted { self.host(ty_n, "ref_self".into(), &none) } else { slot.ty };
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
                let tv = self.trait_ty(*t, &cx);
                sub.insert("trait".into(), tv);
                self.cuts.push((t.start_byte(), t.end_byte(), th.clone()));
            }
        }
        let emit = tr.is_some() || sub.len() > 2;
        if emit {
            self.subjects.push(Value::Object(sub));
        }
        // the type declares what it implements, with the associated types this impl fixes (language 6.11)
        if let (Some((t, (th, complex))), false) = (&tr, ty_n.kind() == "reference_type") {
            let tv = if *complex { self.trait_ty(*t, &cx) } else { json!(th) };
            let mut e = Obj::new();
            e.insert("trait".into(), tv);
            let mut assoc = Obj::new();
            for it in named_children(body).into_iter().filter(|c| c.kind() == "type_item") {
                if let (Some(nm), Some(ty)) = (it.child_by_field_name("name"), it.child_by_field_name("type")) {
                    assoc.insert(text(nm, src).to_string(), self.loose(ty, &g));
                }
            }
            if !assoc.is_empty() {
                e.insert("assoc".into(), Value::Object(assoc));
            }
            e.insert("impl".into(), json!(sname));
            self.implements.push((format!("{}::{ty_head}", sc.path), Value::Object(e)));
        }
        let inner = Scope {
            path: if tr.is_some() { sname } else { format!("{}::{ty_head}", sc.path) },
            generics: g,
            self_ty,
        };
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
        // `const fn` and a plain `#[must_use]` are clauses, not host text (language 6.14.1, 6.14.2); a `#[must_use = "…"]`
        // keeps its message, which has no slot, and stays in the body
        let mut clauses = vec![];
        if let Some(mods) = all_children(f).into_iter().find(|c| c.kind() == "function_modifiers") {
            if let Some(kw) = all_children(mods).into_iter().find(|c| c.kind() == "const") {
                let next = kw.next_sibling().or_else(|| mods.next_sibling()).ok_or("parse-error: const without fn")?;
                self.cuts.push((kw.start_byte(), next.start_byte(), String::new()));
                clauses.push(json!({"kind": "effects", "text": "evaluable at compile time", "source": "const fn"}));
            }
        }
        if let Some(c) = self.must_use_clause(f, "result must be used") {
            clauses.push(c);
        }
        // `unsafe fn` with a `# Safety` section is `obliges caller` (language 6.14.9); without one `unsafe` stays host text
        if let (Some(mods), Some(why)) = (all_children(f).into_iter().find(|c| c.kind() == "function_modifiers"), self.safety_doc(f)) {
            if let Some(kw) = all_children(mods).into_iter().find(|c| c.kind() == "unsafe") {
                let next = kw.next_sibling().or_else(|| mods.next_sibling()).ok_or("parse-error: unsafe without fn")?;
                self.cuts.push((kw.start_byte(), next.start_byte(), String::new()));
                clauses.push(json!({
                    "kind": "obliges",
                    "text": format!("obliges caller : true because \"{why}\""),
                    "source": "doc # Safety",
                }));
            }
        }
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
            if slot.has_ref || slot.bound_ref {
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
        if !clauses.is_empty() {
            sub.insert("clauses".into(), Value::Array(clauses));
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
/// The source is bound first (`crate::bind`), so the bare body and the positions in refusals are those of `bind(src)`.
pub fn strip(src: &str, krate: &str, module: &str) -> R<(String, Value)> {
    strip_with(src, krate, module, &StripOpts::default())
}

/// As `strip`, with what the file is told from outside it (`StripOpts`).
pub fn strip_with(src: &str, krate: &str, module: &str, opts: &StripOpts) -> R<(String, Value)> {
    if let Some(e) = first_error(&parse_rust(src)) {
        return refuse("parse-error", format!("Rust source, at {e}"));
    }
    let bound = crate::bind::bind(src)?;
    let src = bound.as_str();
    let tree = parse_rust(src);
    let name = krate.split_whitespace().next().unwrap_or(krate).replace('-', "_");
    let mut path = vec![name];
    path.extend(module_path(module));
    let mut st = Stripper {
        src,
        cuts: vec![],
        subjects: vec![],
        holes: Obj::new(),
        namer: Namer::default(),
        implements: vec![],
        paths: crate::paths::Paths::of_file(tree.root_node(), src, &module_path(module)),
        realizations: Obj::new(),
        realized: HashMap::new(),
        // A realization's name is unique among the file's identifiers (weave turns each use of it back into the template).
        names_used: crate::bind::idents(src),
        forced: opts.realization_names.clone(),
    };
    st.names_used.extend(opts.realization_names.values().cloned());
    let sc = Scope { path: path.join("::"), generics: vec![], self_ty: "Self".into() };
    st.items_in(tree.root_node(), &sc)?;
    for (ty, entry) in std::mem::take(&mut st.implements) {
        let Some(sub) = st.subjects.iter_mut().find(|s| s["kind"] == "type" && s["name"] == ty.as_str()) else { continue };
        let list = sub.as_object_mut().unwrap().entry("implements").or_insert_with(|| json!([]));
        list.as_array_mut().unwrap().push(entry);
    }
    // A type whose end releases something (language 7.3): its own `impl Drop`, or a field of such a type.
    let rel = crate::bind::Release::of_source(tree.root_node(), src);
    for sub in st.subjects.iter_mut().filter(|s| s["kind"] == "type") {
        let name = sub["name"].as_str().unwrap_or("").rsplit("::").next().unwrap_or("").to_string();
        if rel.types.contains(&name) {
            let (source, observed) = if rel.drops.contains(&name) {
                ("impl Drop", "from its Drop impl")
            } else {
                ("a field of a type that releases at its end", "from its fields")
            };
            let c = json!({"kind": "hold", "text": "lifetime self released at end", "source": source, "status": {"observed": observed}});
            sub.as_object_mut().unwrap().entry("clauses").or_insert_with(|| json!([])).as_array_mut().unwrap().push(c);
        }
    }
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
    if !st.realizations.is_empty() {
        out.insert("realizations".into(), Value::Object(st.realizations));
    }
    Ok((bare, Value::Object(out)))
}

/// The place of a turbofish that a realization's name replaces (the template's last segment and its type arguments; a
/// path before it stays), the template as written, and the base of its name: a method (`s.parse::<u32>` ->
/// `s.parse_u32`, template `<_>::parse`: its type is the receiver's), a function (`std::mem::size_of::<u64>` ->
/// `std::mem::size_of_u64`), or a type in a path (`Vec::<u8>::new` -> `VecU8::new`).
fn realization_site(ta: Node, src: &str) -> Option<((usize, usize), String, String, bool)> {
    let p = ta.parent()?;
    fn last(n: Node) -> Option<Node> {
        if n.kind().starts_with("scoped_") { n.child_by_field_name("name") } else { Some(n) }
    }
    match p.kind() {
        "generic_function" => {
            let f = p.child_by_field_name("function")?;
            match f.kind() {
                "field_expression" => {
                    let field = f.child_by_field_name("field")?;
                    let t = text(field, src);
                    Some(((field.start_byte(), ta.end_byte()), format!("<_>::{t}"), t.to_string(), false))
                }
                "identifier" | "scoped_identifier" => {
                    let name = last(f)?;
                    Some(((name.start_byte(), ta.end_byte()), text(f, src).to_string(), text(name, src).to_string(), false))
                }
                _ => None,
            }
        }
        "generic_type" if p.parent().is_some_and(|s| s.kind() == "scoped_identifier" && s.child_by_field_name("path") == Some(p)) => {
            let t = p.child_by_field_name("type")?;
            let name = last(t)?;
            Some(((name.start_byte(), ta.end_byte()), text(t, src).to_string(), text(name, src).to_string(), true))
        }
        _ => None,
    }
}

/// A mechanical name for a realization: the template's last segment and the words of its arguments (`parse_u32`,
/// `collect_vec_i32`), in upper camel case for a type (`VecU8`), unique in the function.
fn realization_name(base: &str, ty: bool, args: &[&str], used: &mut HashSet<String>) -> String {
    let mut words = vec![];
    for a in args {
        if a.trim() == "()" {
            words.push("unit".to_string());
            continue;
        }
        // The last segment of each path: `serde_json::Map<_, _>` gives `map`.
        let mut cur = String::new();
        let chars: Vec<char> = a.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            if c.is_alphanumeric() || *c == '_' {
                cur.push(*c);
            } else {
                let path = *c == ':' && chars.get(i + 1) == Some(&':');
                if !cur.is_empty() && !path && cur != "_" {
                    words.push(cur.to_lowercase());
                }
                cur.clear();
            }
        }
        if !cur.is_empty() && cur != "_" {
            words.push(cur.to_lowercase());
        }
    }
    let name = if ty {
        std::iter::once(base.to_string()).chain(words.iter().map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        })).collect::<String>()
    } else {
        std::iter::once(base.to_lowercase()).chain(words).collect::<Vec<_>>().join("_")
    };
    let mut n = name.clone();
    let mut i = 2;
    while used.contains(&n) {
        n = format!("{name}{}{i}", if ty { "" } else { "_" });
        i += 1;
    }
    used.insert(n.clone());
    n
}
