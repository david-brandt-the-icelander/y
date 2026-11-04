//! Error messages that name the fix: what `weave` says when a bare body holds something that belongs in the constraint
//! file (a local's type, a type alias's right-hand side), and when a subject is missing (a ready-to-paste entry).
use crate::json::entry;
use crate::weave::{base_path, WeaveError};
use crate::{all_children, collapse, is_comment, is_item_kind, named_children, parse_bare, strip, text};
use serde_json::{json, Value};
use tree_sitter::Node;

/// The placeholder `scaffold` and the missing-subject entries put where the reader must say a type.
pub const TODO: &str = "TODO";

// ---------------------------------------------------------------------------------------------------------------
// Types in the constraint language
// ---------------------------------------------------------------------------------------------------------------

/// A Rust type as a type slot of the constraint file: the type string when `strip` has a form for it, else host text.
pub fn constraint_type(rust_ty: &str, krate: &str, module: &str) -> Value {
    let host = || json!({ "host": rust_ty.trim() });
    if rust_ty.contains('&') {
        return host(); // a reference needs access and hold, which a local's slot has no place for
    }
    match strip(&format!("type __T = {};\n", rust_ty.trim()), krate, module) {
        Ok((_, j)) => match j["subjects"][0]["type"].clone() {
            t if t.is_string() => t,
            _ => host(),
        },
        Err(_) => host(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Placeholders
// ---------------------------------------------------------------------------------------------------------------

fn has_todo(s: &str) -> bool {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|t| t == TODO)
}

fn find_todo(v: &Value, ptr: &str, out: &mut Vec<String>) {
    match v {
        Value::String(s) if has_todo(s) => out.push(ptr.to_string()),
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| find_todo(x, &format!("{ptr}/{i}"), out)),
        Value::Object(m) => {
            for (k, x) in m.iter().filter(|(k, _)| !matches!(k.as_str(), "clauses" | "name")) {
                find_todo(x, &format!("{ptr}/{}", k.replace('~', "~0").replace('/', "~1")), out);
            }
        }
        _ => {}
    }
}

/// Refuse a constraint file that still holds the `TODO` placeholders of `scaffold` or of a missing-subject entry.
pub fn reject_placeholders(json: &Value) -> Result<(), WeaveError> {
    let mut at = vec![];
    find_todo(&json["subjects"], "/subjects", &mut at);
    find_todo(&json["holes"], "/holes", &mut at);
    if at.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = at
        .iter()
        .take(8)
        .map(|p| {
            let sub = p.strip_prefix("/subjects/").and_then(|r| r.split('/').next()).and_then(|i| i.parse::<usize>().ok()).and_then(|i| json["subjects"][i]["name"].as_str());
            match sub {
                Some(n) => format!("{p} (`{n}`)"),
                None => p.clone(),
            }
        })
        .collect();
    let more = if at.len() > 8 { format!(" and {} more", at.len() - 8) } else { String::new() };
    Err(format!("unfilled placeholder: {} still say {TODO}; replace each with the real type: {}{more}", at.len(), named.join(", ")).into())
}

// ---------------------------------------------------------------------------------------------------------------
// A missing subject: the entry to paste
// ---------------------------------------------------------------------------------------------------------------

fn has_tail(body: Node) -> bool {
    let last = named_children(body).into_iter().rfind(|n| !is_comment(*n) && n.kind() != "attribute_item");
    last.is_some_and(|n| !matches!(n.kind(), "expression_statement" | "let_declaration" | "empty_statement" | "use_declaration") && !is_item_kind(n.kind()))
}

fn field_list(body: Node, src: &str) -> Vec<Value> {
    match body.kind() {
        "field_declaration_list" => named_children(body)
            .into_iter()
            .filter(|c| c.kind() == "field_declaration")
            .map(|fd| json!({ "name": text(fd.child_by_field_name("name").unwrap(), src), "type": TODO, "hold": { "kind": "owns" } }))
            .collect(),
        "ordered_field_declaration_list" => all_children(body).into_iter().filter(|c| c.kind() == "_").map(|_| json!({ "type": TODO, "hold": { "kind": "owns" } })).collect(),
        _ => vec![],
    }
}

/// The subject entry for a bare item that has none, in canonical key order, with `TODO` where a type goes. `self_ty` is
/// the type a method's `self` is (`Self` in a trait).
pub fn missing_entry(n: Node, src: &str, name: &str, self_ty: Option<&str>) -> Value {
    let mut o = serde_json::Map::new();
    let kind = match n.kind() {
        "function_item" | "function_signature_item" => "function",
        "struct_item" | "enum_item" | "union_item" => "type",
        "trait_item" => "trait",
        "type_item" => "alias",
        "static_item" => "static",
        _ => "const",
    };
    o.insert("kind".into(), json!(kind));
    o.insert("name".into(), json!(name));
    match n.kind() {
        "function_item" | "function_signature_item" => {
            let params = n.child_by_field_name("parameters").map(all_children).unwrap_or_default();
            let ps: Vec<Value> = params
                .into_iter()
                .filter(|c| matches!(c.kind(), "parameter" | "self_parameter" | "_"))
                .map(|p| {
                    if p.kind() == "self_parameter" {
                        json!({ "name": "self", "type": format!("ref<target type={}>", self_ty.unwrap_or("Self")), "access": { "rights": "read", "guarantee": "readers" } })
                    } else {
                        json!({ "name": collapse(text(p, src)), "type": TODO })
                    }
                })
                .collect();
            o.insert("params".into(), Value::Array(ps));
            if n.child_by_field_name("body").is_none_or(has_tail) {
                o.insert("result".into(), json!({ "type": TODO }));
            }
        }
        "struct_item" => {
            if let Some(b) = n.child_by_field_name("body") {
                o.insert("fields".into(), Value::Array(field_list(b, src)));
            }
        }
        "enum_item" => {
            let vs: Vec<Value> = n
                .child_by_field_name("body")
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .filter(|c| c.kind() == "enum_variant")
                .map(|v| {
                    let mut e = serde_json::Map::new();
                    e.insert("name".into(), json!(text(v.child_by_field_name("name").unwrap(), src)));
                    if let Some(b) = v.child_by_field_name("body") {
                        e.insert("fields".into(), Value::Array(field_list(b, src)));
                    }
                    Value::Object(e)
                })
                .collect();
            o.insert("variants".into(), Value::Array(vs));
        }
        "trait_item" => {}
        _ => {
            o.insert("type".into(), json!(TODO));
        }
    }
    Value::Object(o)
}

/// What follows `slot missing: no subject` in a refusal: the entry, and what to adjust in it.
pub fn paste_note(e: &Value) -> String {
    let mut notes: Vec<&str> = vec![];
    match e["kind"].as_str() {
        Some("function") => {
            if e["params"].as_array().is_some_and(|p| p.iter().any(|x| x["name"] == "self")) {
                notes.push("self is shown as `&self`: for `&mut self` use \"access\": {\"rights\": \"write\", \"guarantee\": \"exclusive\"}, for `self` by value drop \"access\" and use the owned type");
            }
            notes.push("a type that is a reference also needs \"access\"; drop \"result\" if the function returns nothing, add it if it does; add \"generics\" for type parameters");
        }
        Some("type") => notes.push("a field whose type is a reference also needs \"access\"; add \"generics\" for type parameters"),
        _ => {}
    }
    notes.push("add \"visible\": \"public\" (or \"crate\") if the item is not private");
    format!("    add this to \"subjects\" of the constraint file and replace each {TODO} with the type (the weave refuses a leftover {TODO}); {}:\n    {}", notes.join("; "), entry(e))
}

// ---------------------------------------------------------------------------------------------------------------
// A parse error that has a known fix
// ---------------------------------------------------------------------------------------------------------------

/// `src` with comments and the insides of string and char literals blanked, byte for byte.
fn code_only(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for x in &mut out[from..to] {
            if *x != b'\n' {
                *x = b' ';
            }
        }
    };
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let e = src[i..].find('\n').map_or(b.len(), |k| i + k);
                blank(&mut out, i, e);
                i = e;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let mut depth = 1;
                let mut j = i + 2;
                while j < b.len() && depth > 0 {
                    if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut out, i, j);
                i = j;
            }
            b'"' => {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let j = (j + 1).min(b.len());
                blank(&mut out, i + 1, j.saturating_sub(1).max(i + 1));
                i = j;
            }
            b'\'' => {
                // a char literal, not a lifetime: `'x'` or `'\n'`
                let close = if b.get(i + 1) == Some(&b'\\') { src[i + 2..].find('\'').map(|k| i + 2 + k) } else { src[i + 1..].chars().next().and_then(|c| (b.get(i + 1 + c.len_utf8()) == Some(&b'\'')).then_some(i + 1 + c.len_utf8())) };
                match close {
                    Some(c) => {
                        blank(&mut out, i + 1, c);
                        i = c + 1;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

fn is_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Offsets of the keyword `kw` as a token in `code`.
fn keyword_at(code: &str, kw: &str) -> Vec<usize> {
    code.match_indices(kw)
        .map(|(i, _)| i)
        .filter(|&i| !code[..i].chars().next_back().is_some_and(is_id) && !code[i + kw.len()..].chars().next().is_some_and(is_id))
        .collect()
}

fn skip_ws(code: &str, mut i: usize) -> usize {
    while code[i..].chars().next().is_some_and(char::is_whitespace) {
        i += code[i..].chars().next().unwrap().len_utf8();
    }
    i
}

/// The end of the bracketed group that opens at `i`.
fn group_end(code: &str, i: usize) -> Option<usize> {
    let mut depth = 0;
    for (k, c) in code[i..].char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + k + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Where a type that starts at `i` ends: at the first `=` or `;` outside brackets (and at a `{`, `}` or `,` outside).
fn type_end(code: &str, i: usize) -> usize {
    let (mut depth, mut prev) = (0i32, ' ');
    for (k, c) in code[i..].char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' if prev == '-' || prev == '=' => {}
            '>' | ')' | ']' => depth -= 1,
            '=' | ';' | '{' | '}' | ',' if depth <= 0 && !(c == '=' && prev == '>') => return i + k,
            _ => {}
        }
        prev = c;
    }
    code.len()
}

/// A `let` of the bare text: its pattern, and the annotation if it has one.
struct Let {
    /// The pattern without a leading `mut`.
    pat: (usize, usize),
    /// `: T` from the colon to the end of the type.
    annotation: Option<(usize, usize)>,
}

fn parse_let(code: &str, at: usize) -> Option<Let> {
    let mut i = skip_ws(code, at + 3);
    if keyword_at(&code[i..], "mut").first() == Some(&0) {
        i = skip_ws(code, i + 3);
    }
    let start = i;
    let first = code[i..].chars().next()?;
    let end = if matches!(first, '(' | '[') {
        group_end(code, i)?
    } else {
        // `Some(x)`, `Point { x, y }`, `ref x`, `x`
        let mut j = i;
        while code[j..].chars().next().is_some_and(|c| is_id(c) || c == ':' && code[j..].starts_with("::") || c == '&') {
            j += if code[j..].starts_with("::") { 2 } else { code[j..].chars().next().unwrap().len_utf8() };
        }
        if j == i {
            return None;
        }
        let k = skip_ws(code, j);
        match code[k..].chars().next() {
            Some('(') | Some('{') if !code[k..].starts_with("{ ") || code[i..j].chars().next().is_some_and(char::is_uppercase) => group_end(code, k)?,
            _ => j,
        }
    };
    let colon = skip_ws(code, end);
    let annotated = code[colon..].starts_with(':') && !code[colon..].starts_with("::");
    let annotation = annotated.then(|| (colon, type_end(code, colon + 1))).filter(|(c, e)| !code[c + 1..*e].trim().is_empty());
    Some(Let { pat: (start, end), annotation })
}

/// The innermost scope around an offset: the path items inside it take, and the function the offset is in, with the
/// function's range.
type Around = (String, Option<(String, (usize, usize))>);

fn around(n: Node, src: &str, off: usize, path: &str) -> Around {
    let here = (path.to_string(), None);
    for c in named_children(n) {
        if !(c.start_byte() <= off && off <= c.end_byte()) {
            continue;
        }
        let name = |k: &str| c.child_by_field_name(k).map(|x| collapse(text(x, src)));
        match c.kind() {
            "mod_item" | "trait_item" => {
                let (Some(nm), Some(b)) = (name("name"), c.child_by_field_name("body")) else { continue };
                return around(b, src, off, &format!("{path}::{nm}"));
            }
            "impl_item" => {
                let (Some(ty), Some(b)) = (name("type"), c.child_by_field_name("body")) else { continue };
                let p = match name("trait") {
                    Some(t) => format!("{path}::{ty} as {t}"),
                    None => format!("{path}::{ty}"),
                };
                return around(b, src, off, &p);
            }
            "function_item" | "function_signature_item" => {
                let Some(nm) = name("name") else { continue };
                let f = format!("{path}::{nm}");
                let inner = c.child_by_field_name("body").map(|b| around(b, src, off, &f));
                return match inner {
                    Some((p, Some(x))) => (p, Some(x)),
                    _ => (f.clone(), Some((f, (c.start_byte(), c.end_byte())))),
                };
            }
            _ => {
                let r = around(c, src, off, path);
                if r != here {
                    return r;
                }
            }
        }
    }
    here
}

fn lc(src: &str, off: usize) -> (usize, usize) {
    crate::trace::line_col(src, off)
}

/// The fixes for the bare constructs that parse as errors, with the position of each and the text; the first is
/// the one the refusal is placed at. `None` when the error is none of the known ones.
pub fn parse_hint_at(src: &str, json: &Value) -> Option<((usize, usize), String)> {
    let krate = json["crate"].as_str().unwrap_or("crate 0.0.0");
    let module = json["module"].as_str().unwrap_or("src/lib.rs");
    let code = code_only(src);
    let base = base_path(json);
    // Everything the bare body must not say, cut out, so that the rest parses and the scopes can be found.
    let mut cut = src.to_string();
    let mut found: Vec<(usize, Kind)> = vec![];
    enum Kind {
        Local { pat: (usize, usize), ty: (usize, usize) },
        Alias { stmt: (usize, usize), name_end: usize, semi: usize },
    }
    for at in keyword_at(&code, "let") {
        if let Some(Let { pat, annotation: Some(ty), .. }) = parse_let(&code, at) {
            found.push((at, Kind::Local { pat, ty }));
        }
    }
    for at in keyword_at(&code, "type") {
        let i = skip_ws(&code, at + 4);
        let name_end = i + code[i..].find(|c: char| !is_id(c)).unwrap_or(code.len() - i);
        if name_end == i {
            continue;
        }
        let semi = (name_end..code.len()).find(|&k| code.as_bytes()[k] == b';' || code.as_bytes()[k] == b'{');
        let Some(semi) = semi.filter(|&k| code.as_bytes()[k] == b';') else { continue };
        let tail = &code[name_end..semi];
        // generics, bounds and a right-hand side after the name: the part the bare form leaves out
        if !(tail.contains('=') || tail.contains('<') || tail.contains(':') || tail.contains("where")) {
            continue;
        }
        // `pub`, `pub(crate)` before the keyword, on the same line
        let ls = code[..at].rfind('\n').map_or(0, |k| k + 1);
        let before = code[ls..at].trim();
        let start = if before.is_empty() { at } else if before.starts_with("pub") { ls + code[ls..at].find("pub").unwrap() } else { at };
        found.push((at, Kind::Alias { stmt: (start, semi + 1), name_end, semi }));
    }
    if found.is_empty() {
        return None;
    }
    found.sort_by_key(|f| f.0);
    for (_, k) in &found {
        let (a, b) = match k {
            Kind::Local { ty, .. } => *ty,
            Kind::Alias { name_end, semi, .. } => (*name_end, *semi),
        };
        let blank: String = src[a..b].chars().map(|c| if c == '\n' { '\n' } else { ' ' }).collect();
        cut.replace_range(a..b, &blank);
    }
    let tree = parse_bare(&cut);
    let root = tree.root_node();
    let mut lines: Vec<((usize, usize), String)> = vec![];
    for (at, k) in &found {
        let (scope, func) = around(root, &cut, *at, &base);
        let pos = lc(src, if let Kind::Alias { stmt, .. } = k { stmt.0 } else { *at });
        let msg = match k {
            Kind::Local { pat, ty } => {
                let key = collapse(&src[pat.0..pat.1]);
                let ty_text = src[ty.0 + 1..ty.1].trim();
                // `x#2` for the second `let x` of the function
                let n = match &func {
                    Some((_, (a, b))) => keyword_at(&code[*a..(*b).min(*at)], "let").iter().filter(|&&i| parse_let(&code, a + i).is_some_and(|l| collapse(&src[l.pat.0..l.pat.1]) == key)).count() + 1,
                    None => 1,
                };
                let key = if n > 1 { format!("{key}#{n}") } else { key };
                let subject = func.as_ref().map_or("<fn path>".to_string(), |f| f.0.clone());
                let holes = json!({ subject: { "locals": { key.clone(): constraint_type(ty_text, krate, module) } } });
                format!(
                    "a local's type does not go in the bare body; write `let {} ...` without `: {}` and add the type to the constraint file as  \"holes\": {}  (merge it into the \"holes\" entry of the function if there is one)",
                    src[pat.0..pat.1].trim(),
                    ty_text,
                    entry(&holes)
                )
            }
            Kind::Alias { stmt, name_end, .. } => {
                let stmt_text = &src[stmt.0..stmt.1];
                let nm = collapse(&src[*at + 4..*name_end]);
                let mut e = match strip(stmt_text, krate, module) {
                    Ok((_, j)) if j["subjects"][0]["kind"] == "alias" => j["subjects"][0].clone(),
                    _ => {
                        let rhs = stmt_text.split_once('=').map(|x| x.1.trim().trim_end_matches(';').trim().to_string()).unwrap_or_default();
                        json!({ "kind": "alias", "name": format!("{base}::{nm}"), "type": { "host": rhs } })
                    }
                };
                if let Some(Value::String(full)) = e.get("name").cloned() {
                    if let Some(rest) = full.strip_prefix(&format!("{base}::")) {
                        e["name"] = json!(format!("{scope}::{rest}"));
                    }
                }
                let host = e["type"].get("host").is_some();
                format!(
                    "a type alias's right-hand side does not go in the bare body; write `type {nm};` here and add a subject of kind alias to the constraint file: {}{}",
                    entry(&e),
                    if host { "  (the right-hand side is given as host text; write it in the constraint type syntax if the language has a form for it)" } else { "" }
                )
            }
        };
        lines.push((pos, msg));
    }
    let (first, text) = lines.remove(0);
    let mut out = text;
    for ((l, c), t) in lines {
        out.push_str(&format!("\n    also at {l}:{c}: {t}"));
    }
    Some((first, out))
}

/// The text of `parse_hint_at`.
pub fn parse_hint(src: &str, json: &Value) -> Option<String> {
    parse_hint_at(src, json).map(|x| x.1)
}
