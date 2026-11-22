//! From the typed Rust to the bare files: `bare-rust`'s strip, then what the Y lowering adds that strip cannot see, as
//! `tools/paths/regen.py` adds it to a strip of hand-written Rust: each `injected` parameter's mark, each region's
//! subject, each module's level, and the clauses, which here are made from the Y source itself.
use crate::model::{declarations, Module};
use crate::ty::{pascal, upper};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use y_parse::Node;

#[path = "../../bare-rust/tests/y_levels/mod.rs"]
#[allow(dead_code)]
mod y_levels;

pub type Files = Vec<(PathBuf, (String, Value))>;

pub const STATUS: &str = "lowered from y/src by y-compile";

/// Strips the typed Rust of the package, as `bare-rust strip --src-only` strips a crate: the library's files and
/// `main`, each crate's realizations in its root's file.
pub fn strip(krate: &str, files: &[(PathBuf, String)]) -> Result<Files, String> {
    let lib: Vec<PathBuf> = files.iter().map(|f| f.0.clone()).filter(|p| p != &PathBuf::from("src/main.rs")).collect();
    let trees = vec![(PathBuf::from("src/lib.rs"), lib), (PathBuf::from("src/main.rs"), vec![PathBuf::from("src/main.rs")])];
    let mut out = vec![];
    for (rel, r) in bare_rust::corpus::strip_sources(krate, files, &trees) {
        let (brs, js) = r.map_err(|e| format!("strip of {}: {e}", rel.display()))?;
        out.push((rel, (brs, js)));
    }
    Ok(out)
}

/// A Y expression as a clause of the constraint language: `check(game, chosen) == result.ok` is `check(game, chosen)
/// is Ok(_)`, `x.count()` is `count(x)`, a case is its Rust name and a constant its Rust constant.
pub fn clause_expr(m: &Module, e: &Node) -> String {
    let sub = |k: &str| clause_expr(m, e.node(k).unwrap());
    match e.kind {
        "name" => e.str("name").unwrap().to_string(),
        "literal" => e.str("text").unwrap_or("").to_string(),
        "member" => {
            let obj = e.node("object").unwrap();
            let name = e.str("name").unwrap();
            if let Some(o) = obj.str("name").filter(|_| obj.kind == "name") {
                if matches!(m.types.get(o).map(|t| &t.def), Some(crate::model::TypeDef::Enum { .. })) {
                    return pascal(name);
                }
                if m.consts.contains_key(o) {
                    return upper(name);
                }
            }
            format!("{}.{name}", clause_expr(m, obj))
        }
        "call" => {
            let callee = e.node("callee").unwrap();
            let args: Vec<String> = e.nodes("args").iter().map(|a| clause_expr(m, a.node("value").unwrap())).collect();
            if callee.kind == "member" && callee.str("name") == Some("matches") {
                let pat = e.nodes("args")[0].node("value").and_then(|v| v.str("value")).unwrap_or("");
                return format!("{} matches /{pat}/", clause_expr(m, callee.node("object").unwrap()));
            }
            if callee.kind == "member" {
                let obj = clause_expr(m, callee.node("object").unwrap());
                let method = callee.str("name").unwrap();
                if method == "count" && args.is_empty() {
                    return format!("count({obj})");
                }
                if method == "at_number" && args.len() == 1 {
                    return format!("{obj}[{} - 1]", args[0]);
                }
                return format!("{obj}.{method}({})", args.join(", "));
            }
            format!("{}({})", clause_expr(m, callee), args.join(", "))
        }
        "binary" => {
            let op = e.str("op").unwrap();
            let r = e.node("right").unwrap();
            if matches!(op, "==" | "!=") && r.kind == "member" && r.node("object").and_then(|o| o.str("name")) == Some("optional") {
                return format!("{} is {}", sub("left"), if op == "==" { "None" } else { "Some(_)" });
            }
            if op == "==" && r.kind == "member" && r.node("object").and_then(|o| o.str("name")) == Some("result") {
                return format!("{} is {}", sub("left"), if r.str("name") == Some("ok") { "Ok(_)" } else { "Err(_)" });
            }
            if op == "==" && r.kind == "member" {
                if let Some(o) = r.node("object").filter(|o| o.kind == "name").and_then(|o| o.str("name")) {
                    if matches!(m.types.get(o).map(|t| &t.def), Some(crate::model::TypeDef::Enum { .. })) {
                        return format!("{} is {}", sub("left"), pascal(r.str("name").unwrap()));
                    }
                }
            }
            format!("{} {op} {}", sub("left"), sub("right"))
        }
        "unary" if e.str("op") == Some("-") => format!("-{}", sub("operand")),
        "unary" => format!("not {}", sub("operand")),
        "index" => format!("{}[{}]", sub("object"), sub("index")),
        _ => String::new(),
    }
}

/// An expression with a name replaced.
fn renamed(e: &Node, old: &str, new: &str) -> Node {
    let mut e = e.clone();
    fn go(n: &mut Node, old: &str, new: &str) {
        if n.kind == "name" && n.str("name") == Some(old) {
            n.set("name", new);
        }
        for (_, v) in n.fields.iter_mut() {
            match v {
                y_parse::Value::Node(c) => go(c, old, new),
                y_parse::Value::List(vs) => vs.iter_mut().for_each(|v| if let y_parse::Value::Node(c) = v { go(c, old, new) }),
                _ => {}
            }
        }
    }
    go(&mut e, old, new);
    e
}

/// The members of a `mutable` parameter a function changes, itself or through the functions it passes them to.
fn mutated(m: &Module, decls: &BTreeMap<String, &Node>, f: &str, param: &str, depth: usize) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(d) = decls.get(f) else { return out };
    if depth > 6 {
        out.insert(param.to_string());
        return out;
    }
    let text = |n: &Node| crate::lower::y_text(n);
    let rooted = |t: &str| t == param || t.starts_with(&format!("{param}."));
    for b in d.nodes("body") {
        b.walk(&mut |n: &Node| match n.kind {
            "assign" => {
                let t = text(n.node("target").unwrap());
                if rooted(&t) {
                    out.insert(t);
                }
            }
            "call" => {
                let callee = n.node("callee").unwrap();
                if callee.kind == "member" && MUTATING.contains(&callee.str("name").unwrap()) {
                    let t = text(callee.node("object").unwrap());
                    if rooted(&t) {
                        out.insert(t);
                    }
                }
                if let Some(g) = callee.str("name").filter(|_| callee.kind == "name") {
                    if let Some(gf) = m.fns.get(g) {
                        let plain: Vec<_> = gf.params.iter().filter(|p| !p.injected).collect();
                        for (k, a) in n.nodes("args").iter().enumerate() {
                            let v = a.node("value").unwrap();
                            if v.kind != "mutable" {
                                continue;
                            }
                            let t = text(v.node("value").unwrap());
                            if !rooted(&t) {
                                continue;
                            }
                            if let Some(p) = plain.get(k) {
                                for x in mutated(m, decls, g, &p.name, depth + 1) {
                                    out.insert(format!("{t}{}", &x[p.name.len()..]));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        });
    }
    out
}

const MUTATING: [&str; 14] = ["set_number", "push", "add", "remove", "set", "insert", "take", "clear", "push_front", "take_front", "insert_first", "remove_last", "remove_first", "insert_at"];

/// A region's constraints, each as written, as the constraint language's clauses (language 6.23), as
/// `tools/paths/regen.py` makes them: `within d or fail` is `within d`, `waits until name = (p)` is `waits until name :
/// p`, and `atomic` adds what it implies, `may run again` and `on failure unchanged`.
pub fn region_clauses(cs: &[String]) -> Vec<String> {
    let mut out = vec![];
    for c in cs {
        if let Some(rest) = c.strip_prefix("waits until ") {
            if let Some((n, p)) = rest.split_once(" = ") {
                let p = p.strip_prefix('(').and_then(|p| p.strip_suffix(')')).unwrap_or(p);
                out.push(format!("waits until {n} : {p}"));
                continue;
            }
        }
        if c.starts_with("within ") {
            out.push(c.trim_end_matches(" or fail").to_string());
        } else {
            out.push(c.clone());
        }
    }
    if out.iter().any(|c| c == "atomic") {
        out.push("may run again".into());
        out.push("on failure unchanged".into());
    }
    out
}

/// The clauses of a function's subject, from its Y declaration.
fn function_clauses(m: &Module, decls: &BTreeMap<String, &Node>, d: &Node, y: &str) -> Vec<Value> {
    let f = &m.fns[d.str("name").unwrap()];
    let mut cs = vec![];
    for w in d.nodes("where") {
        let text = format!("require {} : {}", w.str("name").unwrap(), clause_expr(m, w.node("condition").unwrap()));
        cs.push(json!({"kind": "require", "text": text, "source": format!("{y}:{}", w.line), "status": {"lifted": format!("src/{}.rs: y_runtime::require", m.name)}}));
    }
    let result = f.result.as_ref().map(|r| r.0.clone()).unwrap_or_default();
    for w in d.nodes("ensures") {
        // of a function that may fail, what it ensures holds of the value it gives
        let text = if f.fails.is_some() {
            format!("ensure {} : result is Ok({result}) implies {}", w.str("name").unwrap(), clause_expr(m, w.node("condition").unwrap()))
        } else {
            format!("ensure {} : {}", w.str("name").unwrap(), clause_expr(m, &renamed(w.node("condition").unwrap(), &result, "result")))
        };
        cs.push(json!({"kind": "ensure", "text": text, "source": format!("{y}:{}", w.line), "status": {"lifted": format!("src/{}.rs: y_runtime::ensure", m.name)}}));
    }
    let at = format!("{y}:{}", d.line);
    // a function lent shared state or a store says nothing of its effects here: its regions' clauses do
    if f.params.iter().any(|p| p.injected && matches!(p.ty, crate::ty::Ty::Shared(_) | crate::ty::Ty::Store(_))) {
        return cs;
    }
    let effects = |t: &str| json!({"kind": "effects", "text": t, "source": at});
    let caps: Vec<&str> = f.params.iter().filter(|p| p.injected).map(|p| p.name.as_str()).filter(|c| !matches!(*c, "forever" | "system")).collect();
    let mutable: Vec<&str> = f.params.iter().filter(|p| p.mutable && !p.injected).map(|p| p.name.as_str()).collect();
    // a module whose capabilities may suspend says of each function that does not that it does not
    let no_suspension = !m.suspending.is_empty() && !m.suspending.contains(&f.name);
    if f.params.iter().any(|p| p.injected) {
        if !caps.is_empty() {
            cs.push(effects(&format!("effects only {}", caps.join(", "))));
        }
        if !caps.iter().any(|c| matches!(*c, "console" | "window" | "site" | "file_system")) && !caps.is_empty() {
            cs.push(effects("no io"));
        }
        if !caps.contains(&"clock") {
            cs.push(effects("no clock"));
        }
        if !caps.contains(&"random") {
            cs.push(effects("no randomness"));
        }
        if no_suspension {
            cs.push(effects("no suspension"));
        }
    } else if mutable.is_empty() {
        cs.push(effects("pure"));
    } else {
        for t in ["no io", "no clock", "no randomness"] {
            cs.push(effects(t));
        }
        if no_suspension {
            cs.push(effects("no suspension"));
        }
    }
    let mut changed = BTreeSet::new();
    for p in &mutable {
        let mut set = mutated(m, decls, &f.name, p, 0);
        // every field of the parameter changed: the parameter
        let ty = f.params.iter().find(|q| q.name == *p).map(|q| q.ty.clone());
        if let Some(crate::ty::Ty::Named(t)) = ty {
            if let Some(crate::model::TypeDef::Struct { fields, .. }) = m.types.get(&t).map(|t| &t.def) {
                if fields.len() > 1 && fields.iter().all(|(n, _)| set.contains(&format!("{p}.{n}"))) {
                    set = BTreeSet::from([p.to_string()]);
                }
            }
        }
        changed.extend(set);
    }
    if !changed.is_empty() {
        let list: Vec<String> = changed.into_iter().collect();
        cs.push(json!({"kind": "access", "text": format!("mutates only {}", list.join(", ")), "source": at}));
    }
    cs
}

/// The invariants of a type: a newtype's `where` clauses, which its constructor checks, and a structure's
/// `invariant`s, checked after each write that can break them.
fn type_clauses(m: &Module, name: &str, info: &crate::model::TypeInfo, y: &str, sites: &[(String, String, String, String)]) -> Vec<Value> {
    let mut cs = vec![];
    match &info.def {
        crate::model::TypeDef::Newtype { base, wheres } => {
            for (w, line, c) in wheres {
                let mut text = clause_expr(m, &renamed(c, "value", "self.0"));
                if *base == crate::ty::Ty::Str {
                    text = text.replace("count(self.0)", "count(scalars(self.0))");
                }
                cs.push(json!({"kind": "invariant", "text": format!("invariant {w} : {text}"), "source": format!("{y}:{line}"), "status": {"lifted": format!("src/{}.rs: {}::new", m.name, pascal(name))}}));
            }
        }
        crate::model::TypeDef::Struct { fields, invariants, .. } => {
            let fnames: Vec<String> = fields.iter().map(|f| f.0.clone()).collect();
            for (w, line, c) in invariants {
                let text = clause_expr(m, &crate::lower::with_fields(c, &fnames, "self"));
                let at: Vec<&(String, String, String, String)> = sites.iter().filter(|s| s.0 == name && &s.1 == w).collect();
                let lifted = match &at[..] {
                    [one] => format!("src/{}.rs: {}, after the one write to {}", m.name, one.2, one.3),
                    _ => format!("src/{}.rs: after each write that can break it", m.name),
                };
                cs.push(json!({"kind": "invariant", "text": format!("invariant {w} : {text}"), "source": format!("{y}:{line}"), "status": {"lifted": lifted}}));
            }
        }
        _ => {}
    }
    cs
}

/// The bare files with what the Y lowering adds to a strip.
#[allow(clippy::too_many_arguments)]
pub fn lower(m: &Module, tree: &Node, source: &str, stripped: &Files, name: &str, sites: &[(String, String, String, String)], regions: &crate::emit::Regions, main_regions: &crate::emit::Regions) -> Result<Files, String> {
    let y = format!("y/src/{name}.y");
    let decls: BTreeMap<String, &Node> = declarations(tree).into_iter().filter(|(_, d)| matches!(d.kind, "function" | "rule")).map(|(_, d)| (d.str("name").unwrap().to_string(), d)).collect();
    let levels = y_levels::facet_levels(source);
    let mut out = vec![];
    for (rel, (brs, js)) in stripped {
        let mut js = js.clone();
        js["status"] = json!({"assumed": STATUS});
        let tests = rel.to_string_lossy().ends_with("tests.rs");
        let main = rel.to_string_lossy().ends_with("main.rs");
        for s in js["subjects"].as_array_mut().unwrap() {
            let sname = s["name"].as_str().unwrap().to_string();
            let last = sname.rsplit("::").next().unwrap().to_string();
            match s["kind"].as_str().unwrap() {
                "module" => {
                    let level = if sname.ends_with(&format!("::{name}")) {
                        Some("effects".to_string())
                    } else if let Some((l, _)) = levels.get(&last) {
                        Some(y_levels::LEVELS[*l].to_string())
                    } else if m.consts.contains_key(&last) {
                        Some("data".to_string())
                    } else {
                        None
                    };
                    if let Some(l) = level {
                        s["level"] = json!(l);
                    }
                }
                "type" if !tests && !main => {
                    let Some((yname, info)) = m.types.iter().find(|(n, _)| pascal(n) == last) else { continue };
                    let cs = type_clauses(m, yname, info, &y, sites);
                    if !cs.is_empty() {
                        s["clauses"] = Value::Array(cs);
                    }
                }
                // a method the lowering made: a newtype's `new`, or a derived value
                "function" if !tests && !main && sname.rsplit("::").nth(1).is_some_and(|t| t.chars().next().is_some_and(char::is_uppercase)) => {
                    let owner = sname.rsplit("::").nth(1).unwrap();
                    let Some((_, info)) = m.types.iter().find(|(n, _)| pascal(n) == owner) else { continue };
                    let line = match &info.def {
                        crate::model::TypeDef::Struct { derived, derived_values, .. } => derived.iter().zip(derived_values).find(|((n, _), _)| *n == last).map(|(_, (l, _))| *l),
                        crate::model::TypeDef::Newtype { .. } if last == "new" => Some(info.line),
                        _ => None,
                    };
                    if let Some(l) = line {
                        s["clauses"] = json!([{"kind": "effects", "text": "pure", "source": format!("{y}:{l}")}]);
                    }
                }
                "function" if !tests && !main => {
                    let Some(d) = decls.get(&last) else { continue };
                    let f = &m.fns[&last];
                    for p in s["params"].as_array_mut().into_iter().flatten() {
                        if f.params.iter().any(|q| q.injected && Some(q.name.as_str()) == p["name"].as_str()) {
                            p["injected"] = json!(true);
                        }
                    }
                    let cs = function_clauses(m, &decls, d, &y);
                    if !cs.is_empty() {
                        s["clauses"] = Value::Array(cs);
                    }
                }
                _ => {}
            }
        }
        // each region of a function: a subject after the function's, with its constraints as clauses (language 6.23)
        if !tests {
            let (regions, y) = if main { (main_regions, "y/src/main.y".to_string()) } else { (regions, y.clone()) };
            let subjects = js["subjects"].as_array_mut().unwrap();
            for (f, r, line, cs) in regions.iter().rev() {
                let Some(k) = subjects.iter().position(|s| s["kind"] == "function" && s["name"].as_str().is_some_and(|n| n.ends_with(&format!("::{f}")))) else { continue };
                let fname = subjects[k]["name"].as_str().unwrap().to_string();
                let clauses: Vec<Value> = region_clauses(cs).into_iter().map(|t| json!({"kind": "region", "text": t, "source": format!("{y}:{line}")})).collect();
                // after the function's subject and the regions of it already placed before this one
                let mut at = k + 1;
                while at < subjects.len() && subjects[at]["kind"] == "region" && subjects[at]["name"].as_str().is_some_and(|n| n.starts_with(&format!("{fname}#"))) {
                    at += 1;
                }
                let _ = at;
                subjects.insert(k + 1, json!({"kind": "region", "name": format!("{fname}#{r}"), "clauses": clauses}));
            }
        }
        out.push((rel.clone(), (brs.clone(), js)));
    }
    Ok(out)
}

