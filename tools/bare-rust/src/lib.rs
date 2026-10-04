//! Bare Rust stage 0: strip Rust into a bare body plus constraint JSON, and weave them back.
use serde_json::{json, Map, Value};
use tree_sitter::{Language, Node, Parser, Tree};

extern "C" {
    fn tree_sitter_bare_rust() -> *const ();
}

type R<T> = Result<T, String>;

pub fn bare_language() -> Language {
    let f = unsafe { tree_sitter_language::LanguageFn::from_raw(tree_sitter_bare_rust) };
    Language::from(f)
}

fn parse_with(lang: Language, src: &str) -> Tree {
    let mut p = Parser::new();
    p.set_language(&lang).expect("language");
    p.parse(src, None).expect("parse")
}

/// Parse with the bare parser; the tree may contain ERROR or MISSING nodes.
pub fn parse_bare(src: &str) -> Tree {
    parse_with(bare_language(), src)
}

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    &src[n.byte_range()]
}

fn named_children<'a>(n: Node<'a>) -> Vec<Node<'a>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

fn apply(src: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|e| e.0);
    let (mut out, mut at) = (String::new(), 0);
    for (s, e, t) in edits {
        out.push_str(&src[at..s]);
        out.push_str(&t);
        at = e;
    }
    out.push_str(&src[at..]);
    out
}

// ---------- strip ----------

fn map_type(n: Node, src: &str) -> R<String> {
    match n.kind() {
        "primitive_type" | "type_identifier" => Ok(text(n, src).to_string()),
        "reference_type" => Ok(format!("ref<{}>", map_type(n.child_by_field_name("type").unwrap(), src)?)),
        "generic_type" => {
            let name = n.child_by_field_name("type").unwrap();
            let args = named_children(n.child_by_field_name("type_arguments").unwrap());
            let args: R<Vec<String>> = args.into_iter().map(|a| map_type(a, src)).collect();
            let args = args?;
            let path = text(name, src);
            let last = path.rsplit("::").next().unwrap();
            let head = match last {
                "Option" if args.len() == 1 => return Ok(format!("{}?", args[0])),
                "Vec" => "list",
                "HashMap" | "BTreeMap" => "map",
                "HashSet" => "set",
                _ => return Err(format!("unsupported type `{}`", text(n, src))),
            };
            Ok(format!("{head}<{}> as '{path}'", args.join(", ")))
        }
        _ => Err(format!("unsupported type `{}`", text(n, src))),
    }
}

fn access_of(ty: Node, src: &str) -> Option<Value> {
    if ty.kind() != "reference_type" {
        return None;
    }
    let mutable = named_children(ty).iter().any(|c| c.kind() == "mutable_specifier");
    let _ = src;
    Some(if mutable {
        json!({"rights": "write", "guarantee": "exclusive"})
    } else {
        json!({"rights": "read", "guarantee": "readers"})
    })
}

fn visibility(n: Node, src: &str, cuts: &mut Vec<(usize, usize, String)>) -> R<Option<&'static str>> {
    let Some(v) = named_children(n).into_iter().find(|c| c.kind() == "visibility_modifier") else {
        return Ok(None);
    };
    let next = v.next_sibling().ok_or("visibility without item")?;
    cuts.push((v.start_byte(), next.start_byte(), String::new()));
    match text(v, src) {
        "pub" => Ok(Some("public")),
        "pub(crate)" => Ok(Some("crate")),
        t => Err(format!("unsupported visibility `{t}`")),
    }
}

fn strip_fn(f: Node, src: &str, name: &str, self_ty: &str, cuts: &mut Vec<(usize, usize, String)>) -> R<Value> {
    let mut sub = Map::new();
    sub.insert("kind".into(), json!("function"));
    sub.insert("name".into(), json!(name));
    if let Some(v) = visibility(f, src, cuts)? {
        sub.insert("visible".into(), json!(v));
    }
    let mut params = vec![];
    let mut refs = vec![]; // names of reference parameters
    for p in named_children(f.child_by_field_name("parameters").unwrap()) {
        match p.kind() {
            "self_parameter" => {
                let selfn = named_children(p).into_iter().find(|c| c.kind() == "self").ok_or("self parameter")?;
                if selfn.start_byte() == p.start_byte() {
                    return Err(format!("{name}: owned self is not supported"));
                }
                if p.child_by_field_name("type").is_some() || p.child_count() > 3 && text(p, src).contains('\'') {
                    return Err(format!("{name}: explicit lifetime on self"));
                }
                cuts.push((p.start_byte(), selfn.start_byte(), String::new()));
                let mutable = named_children(p).iter().any(|c| c.kind() == "mutable_specifier");
                let acc = if mutable {
                    json!({"rights": "write", "guarantee": "exclusive"})
                } else {
                    json!({"rights": "read", "guarantee": "readers"})
                };
                params.push(json!({"name": "self", "type": format!("ref<{self_ty}>"), "access": acc}));
                refs.push("self".to_string());
            }
            "parameter" => {
                let pat = p.child_by_field_name("pattern").unwrap();
                let ty = p.child_by_field_name("type").unwrap();
                let pname = text(pat, src);
                let mut o = Map::new();
                o.insert("name".into(), json!(pname));
                o.insert("type".into(), json!(map_type(ty, src)?));
                if let Some(a) = access_of(ty, src) {
                    o.insert("access".into(), a);
                    refs.push(pname.to_string());
                }
                cuts.push((pat.end_byte(), ty.end_byte(), String::new()));
                params.push(Value::Object(o));
            }
            k => return Err(format!("{name}: unsupported parameter `{k}`")),
        }
    }
    sub.insert("params".into(), Value::Array(params));
    if let Some(rt) = f.child_by_field_name("return_type") {
        let ty = map_type(rt, src)?;
        let mut res = Map::new();
        if ty.contains("ref<") {
            let from = if refs.contains(&"self".to_string()) {
                "self".to_string()
            } else if refs.len() == 1 {
                refs[0].clone()
            } else {
                return Err(format!("{name}: cannot elide the result lifetime"));
            };
            res.insert("hold".into(), json!({"kind": "borrows", "from": from}));
        }
        res.insert("type".into(), json!(ty));
        sub.insert("result".into(), Value::Object(res));
        let before = f.child_by_field_name("parameters").unwrap().end_byte();
        cuts.push((before, rt.end_byte(), String::new()));
    }
    Ok(Value::Object(sub))
}

/// Strip Rust source into (bare body, constraint JSON).
pub fn strip(src: &str, krate: &str, module: &str) -> R<(String, Value)> {
    let tree = parse_with(tree_sitter_rust::LANGUAGE.into(), src);
    if tree.root_node().has_error() {
        return Err("parse error in Rust source".into());
    }
    let name = krate.split_whitespace().next().unwrap_or(krate);
    let mut cuts = vec![];
    let mut subjects = vec![];
    for item in named_children(tree.root_node()) {
        match item.kind() {
            "struct_item" => {
                let sname = text(item.child_by_field_name("name").unwrap(), src);
                let mut sub = Map::new();
                sub.insert("kind".into(), json!("type"));
                sub.insert("name".into(), json!(format!("{name}::{sname}")));
                if let Some(v) = visibility(item, src, &mut cuts)? {
                    sub.insert("visible".into(), json!(v));
                }
                let body = item.child_by_field_name("body").ok_or("struct without a field list")?;
                if body.kind() != "field_declaration_list" {
                    return Err(format!("{sname}: only named fields are supported"));
                }
                let mut fields = vec![];
                for fd in named_children(body) {
                    if fd.kind() != "field_declaration" {
                        continue;
                    }
                    if visibility(fd, src, &mut cuts)?.is_some() {
                        return Err(format!("{sname}: field visibility is not supported"));
                    }
                    let n = fd.child_by_field_name("name").unwrap();
                    let ty = fd.child_by_field_name("type").unwrap();
                    let t = map_type(ty, src)?;
                    if t.contains("ref<") {
                        return Err(format!("{sname}: reference fields are not supported"));
                    }
                    fields.push(json!({"name": text(n, src), "type": t, "hold": {"kind": "owns"}}));
                    cuts.push((n.end_byte(), ty.end_byte(), String::new()));
                }
                sub.insert("fields".into(), Value::Array(fields));
                subjects.push(Value::Object(sub));
            }
            "impl_item" => {
                if item.child_by_field_name("trait").is_some() || item.child_by_field_name("type_parameters").is_some() {
                    return Err("only inherent impls without generics are supported".into());
                }
                let ty = text(item.child_by_field_name("type").unwrap(), src);
                let body = item.child_by_field_name("body").unwrap();
                for f in named_children(body) {
                    if f.kind() != "function_item" {
                        return Err(format!("unsupported impl item `{}`", f.kind()));
                    }
                    let fname = text(f.child_by_field_name("name").unwrap(), src);
                    subjects.push(strip_fn(f, src, &format!("{name}::{ty}::{fname}"), ty, &mut cuts)?);
                }
            }
            "line_comment" | "block_comment" => {}
            k => return Err(format!("unsupported item `{k}`")),
        }
    }
    let bare = apply(src, cuts);
    if parse_bare(&bare).root_node().has_error() {
        return Err("stripped body does not parse as bare Rust".into());
    }
    let json = json!({
        "format": "y-constraints/0", "host": "rust", "crate": krate, "module": module,
        "status": "lifted", "subjects": subjects
    });
    Ok((bare, json))
}

// ---------- weave ----------

fn rust_ty(t: &str, mutable: bool) -> String {
    if let Some((base, path)) = t.split_once(" as '") {
        let args = &base[base.find('<').unwrap_or(0) + 1..base.len() - 1];
        format!("{}<{args}>", path.trim_end_matches('\''))
    } else if let Some(i) = t.strip_suffix('?') {
        format!("Option<{}>", rust_ty(i, mutable))
    } else if let Some(i) = t.strip_prefix("ref<").and_then(|s| s.strip_suffix('>')) {
        format!("{}{}", if mutable { "&mut " } else { "&" }, rust_ty(i, false))
    } else {
        t.to_string()
    }
}

fn is_write(p: &Value) -> bool {
    p["access"]["rights"] == "write"
}

fn find<'a>(subs: &'a [Value], name: &str) -> R<&'a Value> {
    subs.iter()
        .find(|s| s["name"] == name)
        .ok_or_else(|| format!("slot missing: no subject `{name}`"))
}

fn vis(sub: &Value) -> R<Option<&'static str>> {
    match sub["visible"].as_str() {
        None => Ok(None),
        Some("public") => Ok(Some("pub ")),
        Some("crate") => Ok(Some("pub(crate) ")),
        Some(v) => Err(format!("{}: unsupported visible `{v}`", sub["name"])),
    }
}

/// Weave a bare body and its constraint JSON into Rust.
pub fn weave(brs: &str, json: &Value) -> R<String> {
    let tree = parse_bare(brs);
    if tree.root_node().has_error() {
        return Err("parse error in bare body".into());
    }
    let name = json["crate"].as_str().ok_or("slot missing: crate")?.split_whitespace().next().unwrap_or("");
    let subs = json["subjects"].as_array().ok_or("slot missing: subjects")?;
    let mut ed = vec![];
    for item in named_children(tree.root_node()) {
        match item.kind() {
            "struct_item" => {
                let sname = text(item.child_by_field_name("name").unwrap(), brs);
                let sub = find(subs, &format!("{name}::{sname}"))?;
                if let Some(v) = vis(sub)? {
                    ed.push((item.start_byte(), item.start_byte(), v.to_string()));
                }
                for fd in named_children(item.child_by_field_name("body").unwrap()) {
                    if fd.kind() != "field_declaration" {
                        continue;
                    }
                    let fname = text(fd.child_by_field_name("name").unwrap(), brs);
                    let field = sub["fields"]
                        .as_array()
                        .and_then(|a| a.iter().find(|f| f["name"] == fname))
                        .ok_or_else(|| format!("slot missing: {name}::{sname} field `{fname}`"))?;
                    let t = field["type"].as_str().ok_or_else(|| format!("slot missing: type of field `{fname}`"))?;
                    ed.push((fd.end_byte(), fd.end_byte(), format!(": {}", rust_ty(t, false))));
                }
            }
            "impl_item" => {
                let ty = text(item.child_by_field_name("type").unwrap(), brs);
                for f in named_children(item.child_by_field_name("body").unwrap()) {
                    if f.kind() == "function_item" {
                        weave_fn(f, brs, name, ty, subs, &mut ed)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(apply(brs, ed))
}

fn weave_fn(f: Node, brs: &str, krate: &str, ty: &str, subs: &[Value], ed: &mut Vec<(usize, usize, String)>) -> R<()> {
    let fname = text(f.child_by_field_name("name").unwrap(), brs);
    let sname = format!("{krate}::{ty}::{fname}");
    let sub = find(subs, &sname)?;
    if let Some(v) = vis(sub)? {
        ed.push((f.start_byte(), f.start_byte(), v.to_string()));
    }
    let params = sub["params"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    let mut refs = vec![];
    for p in named_children(f.child_by_field_name("parameters").unwrap()) {
        let pname = if p.kind() == "self_parameter" { "self" } else { text(p, brs) };
        let slot = params
            .iter()
            .find(|q| q["name"] == pname)
            .ok_or_else(|| format!("slot missing: {sname} parameter `{pname}`"))?;
        if p.kind() == "self_parameter" {
            if slot["access"].is_null() {
                return Err(format!("slot missing: {sname} access of self"));
            }
            let s = if is_write(slot) { "&mut self" } else { "&self" };
            ed.push((p.start_byte(), p.end_byte(), s.to_string()));
            refs.push("self");
        } else {
            let t = slot["type"].as_str().ok_or_else(|| format!("slot missing: {sname} type of `{pname}`"))?;
            if t.contains("ref<") {
                if slot["access"].is_null() {
                    return Err(format!("slot missing: {sname} access of `{pname}`"));
                }
                refs.push(pname);
            }
            ed.push((p.end_byte(), p.end_byte(), format!(": {}", rust_ty(t, is_write(slot)))));
        }
    }
    if let Some(res) = sub.get("result") {
        let t = res["type"].as_str().ok_or_else(|| format!("slot missing: {sname} result type"))?;
        if t.contains("ref<") {
            let from = res["hold"]["from"].as_str().ok_or_else(|| format!("slot missing: {sname} result hold"))?;
            let elided = (from == "self" && refs.contains(&"self")) || (refs.len() == 1 && refs[0] == from);
            if !elided {
                return Err(format!("{sname}: result hold from `{from}` needs an explicit lifetime"));
            }
        }
        let b = f.child_by_field_name("body").unwrap().start_byte();
        ed.push((b, b, format!("-> {} ", rust_ty(t, false))));
    }
    Ok(())
}
