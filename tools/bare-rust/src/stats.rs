//! The counts for E1, computed from a directory of stripped files: `x.brs`, `x.constraints.json`, `refusals.json`.
use crate::life::lit_suffix;
use crate::types::Ty;
use crate::{named_children, parse_bare, text, R};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

type Counts = BTreeMap<String, u64>;

fn bump(c: &mut Counts, k: impl Into<String>) {
    *c.entry(k.into()).or_insert(0) += 1;
}

fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            walk(&p, ext, out);
        } else if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(ext)) {
            out.push(p);
        }
    }
}

/// The outermost form of a mapped type.
fn form(s: &str) -> String {
    match Ty::parse(s) {
        Ok(Ty::Prim(_)) => "prim".into(),
        Ok(Ty::Text) => "text".into(),
        Ok(Ty::Char) => "char".into(),
        Ok(Ty::Name(_)) => "name".into(),
        Ok(Ty::App(..)) => "generic".into(),
        Ok(Ty::Ref(_)) => "ref".into(),
        Ok(Ty::Opt(_)) => "option".into(),
        Ok(Ty::Tuple(_)) => "tuple".into(),
        Ok(Ty::Coll(k, _)) => k,
        Ok(Ty::As(t, w)) => match (&*t, w.as_str()) {
            (_, "usize" | "isize") => w,
            (Ty::Coll(k, _), _) => k.clone(),
            _ => w.rsplit("::").next().unwrap_or("").to_lowercase(),
        },
        Err(_) => "unparsed".into(),
    }
}

struct Acc {
    slots: Counts,
    mapped: Counts,
    host: Counts,
    hints: Counts,
    subjects: Counts,
}

impl Acc {
    fn ty(&mut self, v: &Value) {
        if v.is_null() {
            return;
        }
        if let Some(h) = v.get("host") {
            let f = v["form"].as_str().unwrap_or("unknown");
            let _ = h;
            bump(&mut self.host, f);
        } else if let Some(s) = v.as_str() {
            bump(&mut self.mapped, form(s));
        }
    }

    fn slot(&mut self, o: &Value, prefix: &str) {
        for k in ["type", "access", "hold", "fails", "visible"] {
            if !o[k].is_null() {
                bump(&mut self.slots, format!("{prefix}.{k}"));
            }
        }
        self.ty(&o["type"]);
        self.ty(&o["fails"]);
    }

    fn generics(&mut self, o: &Value) {
        for g in o["generics"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let kind = if g["name"].as_str().is_some_and(|n| n.starts_with('\'')) { "lifetime" } else { "type" };
            bump(&mut self.slots, format!("generics.{kind}"));
            let n = g["bounds"].as_array().map_or(0, Vec::len) + g["where"].as_array().map_or(0, Vec::len);
            *self.slots.entry("bounds (host text)".into()).or_insert(0) += n as u64;
        }
        for w in o["where"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            bump(&mut self.slots, "where");
            *self.slots.entry("bounds (host text)".into()).or_insert(0) += w["bounds"].as_array().map_or(0, Vec::len) as u64;
        }
        if let Some(b) = o["bounds"].as_array() {
            *self.slots.entry("bounds (host text)".into()).or_insert(0) += b.len() as u64;
        }
    }

    fn subject(&mut self, s: &Value) {
        let kind = s["kind"].as_str().unwrap_or("?");
        bump(&mut self.subjects, kind);
        if !s["visible"].is_null() {
            bump(&mut self.slots, "visible");
        }
        self.generics(s);
        match kind {
            "function" => {
                for p in s["params"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                    self.slot(p, "params");
                }
                if !s["result"].is_null() {
                    self.slot(&s["result"], "result");
                }
            }
            "type" => {
                for f in s["fields"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                    self.field(f);
                }
                for v in s["variants"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                    bump(&mut self.slots, "variants");
                    for f in v["fields"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                        self.field(f);
                    }
                }
            }
            "impl" => {
                for k in ["self", "trait"] {
                    if !s[k].is_null() {
                        bump(&mut self.slots, format!("impl.{k}"));
                        self.ty(&s[k]);
                    }
                }
            }
            "const" | "static" | "alias" if !s["type"].is_null() => {
                bump(&mut self.slots, format!("{kind}.type"));
                self.ty(&s["type"]);
            }
            _ => {}
        }
    }

    fn field(&mut self, f: &Value) {
        self.slot(f, "fields");
    }

    fn holes(&mut self, h: &Value, fns: &mut u64) {
        let Some(m) = h.as_object() else { return };
        for (_, v) in m {
            *fns += 1;
            if let Some(l) = v["locals"].as_object() {
                for (_, t) in l {
                    bump(&mut self.slots, "holes.locals");
                    self.ty(t);
                }
            }
            for x in v["hints"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                bump(&mut self.hints, x["kind"].as_str().unwrap_or("?"));
                bump(&mut self.slots, "holes.hints");
                self.ty(&x["type"]);
            }
        }
    }
}

/// Token-level types left inside macro invocations, which the grammar does not see.
fn macro_tokens(n: Node, src: &str) -> u64 {
    let mut count = 0;
    let mut prev_as = false;
    let mut stack = vec![n];
    let mut toks: Vec<Node> = vec![];
    while let Some(x) = stack.pop() {
        if x.child_count() == 0 || matches!(x.kind(), "string_literal" | "char_literal") {
            toks.push(x);
        } else {
            let mut c = x.walk();
            let kids: Vec<Node> = x.children(&mut c).collect();
            stack.extend(kids.into_iter().rev());
        }
    }
    for t in toks {
        let s = text(t, src);
        if matches!(t.kind(), "integer_literal" | "float_literal") && lit_suffix(s).is_some() {
            count += 1;
        }
        if prev_as && matches!(t.kind(), "primitive_type" | "identifier" | "type_identifier") {
            count += 1;
        }
        prev_as = s == "as";
        if s == "::" && t.next_sibling().is_some_and(|x| text(x, src).starts_with('<')) {
            count += 1;
        }
    }
    count
}

#[derive(Default)]
struct BodyCounts {
    macros: Counts,
    modifiers: Counts,
    lint: Counts,
}

fn body_counts(n: Node, src: &str, c: &mut BodyCounts) {
    match n.kind() {
        "macro_definition" => {
            bump(&mut c.macros, "definitions");
            return;
        }
        "macro_invocation" => {
            bump(&mut c.macros, "invocations");
            let name = n.child_by_field_name("macro").map(|m| text(m, src)).unwrap_or("");
            let last = name.rsplit("::").next().unwrap_or("");
            if matches!(last, "assert" | "assert_eq" | "assert_ne" | "debug_assert" | "panic" | "unreachable" | "todo" | "unimplemented") {
                bump(&mut c.lint, format!("{last}!"));
            }
            let typed = n.children(&mut n.walk()).filter(|k| k.kind() == "token_tree").map(|k| macro_tokens(k, src)).sum::<u64>();
            if typed > 0 {
                bump(&mut c.macros, "invocations with typed tokens");
                *c.macros.entry("typed tokens".into()).or_insert(0) += typed;
            }
            return;
        }
        "function_modifiers" => bump(&mut c.modifiers, text(n, src).split_whitespace().collect::<Vec<_>>().join(" ")),
        "call_expression" => {
            if let Some(f) = n.child_by_field_name("function").filter(|f| f.kind() == "field_expression") {
                let m = f.child_by_field_name("field").map(|x| text(x, src)).unwrap_or("");
                if matches!(m, "unwrap" | "expect") {
                    bump(&mut c.lint, format!(".{m}()"));
                }
            }
        }
        _ => {}
    }
    for k in named_children(n) {
        body_counts(k, src, c);
    }
}

fn obj(c: &Counts) -> Value {
    Value::Object(c.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<_, _>>())
}

/// The E1 counts for a directory of stripped files.
pub fn stats(dir: &Path) -> R<Value> {
    let mut cj = vec![];
    walk(dir, ".constraints.json", &mut cj);
    let mut acc = Acc { slots: Counts::new(), mapped: Counts::new(), host: Counts::new(), hints: Counts::new(), subjects: Counts::new() };
    let mut fns_with_holes = 0;
    let mut bc = BodyCounts::default();
    let mut refusals = Counts::new();
    let mut refused_files = vec![];
    let mut accepted = vec![];
    for p in &cj {
        let s = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let v: Value = serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))?;
        accepted.push(p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace(".constraints.json", ".rs"));
        for sub in v["subjects"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            acc.subject(sub);
        }
        acc.holes(&v["holes"], &mut fns_with_holes);
        let b = p.to_string_lossy().replace(".constraints.json", ".brs");
        if let Ok(bare) = fs::read_to_string(&b) {
            let t = parse_bare(&bare);
            body_counts(t.root_node(), &bare, &mut bc);
        }
    }
    let mut rj = vec![];
    walk(dir, "refusals.json", &mut rj);
    for p in rj {
        let s = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let v: Value = serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))?;
        let base = p.parent().and_then(|d| d.strip_prefix(dir).ok()).map(|d| d.to_path_buf()).unwrap_or_default();
        for r in v["refused"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            bump(&mut refusals, r["reason"].as_str().unwrap_or("?"));
            let f = base.join(r["file"].as_str().unwrap_or("?"));
            refused_files.push(json!({"file": f.to_string_lossy(), "reason": r["reason"], "detail": r["detail"]}));
        }
    }
    let mapped: u64 = acc.mapped.values().sum();
    let host: u64 = acc.host.values().sum();
    Ok(json!({
        "files": {
            "accepted": accepted.len(),
            "refused": refused_files.len(),
            "refused_by_reason": obj(&refusals),
            "accepted_files": accepted,
            "refused_files": refused_files,
        },
        "subjects": obj(&acc.subjects),
        "slots_filled": obj(&acc.slots),
        "types": {
            "slots": mapped + host,
            "mapped": mapped,
            "host_text": host,
            "mapped_by_form": obj(&acc.mapped),
            "host_text_by_form": obj(&acc.host),
        },
        "holes": {
            "locals": acc.slots.get("holes.locals").copied().unwrap_or(0),
            "hints": acc.slots.get("holes.hints").copied().unwrap_or(0),
            "hints_by_kind": obj(&acc.hints),
            "functions_with_holes": fns_with_holes,
        },
        "left_in_body": {
            "macros": obj(&bc.macros),
            "function_modifiers": obj(&bc.modifiers),
            "lint": obj(&bc.lint),
        },
    }))
}
