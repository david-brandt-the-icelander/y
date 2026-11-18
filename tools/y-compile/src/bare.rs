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
                if m.types.contains_key(o) {
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
            if callee.kind == "member" {
                let obj = clause_expr(m, callee.node("object").unwrap());
                let method = callee.str("name").unwrap();
                if method == "count" && args.is_empty() {
                    return format!("count({obj})");
                }
                return format!("{obj}.{method}({})", args.join(", "));
            }
            format!("{}({})", clause_expr(m, callee), args.join(", "))
        }
        "binary" => {
            let op = e.str("op").unwrap();
            let r = e.node("right").unwrap();
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
        "unary" => format!("{} {}", e.str("op").unwrap_or("not"), sub("operand")),
        "index" => format!("{}[{}]", sub("object"), sub("index")),
        _ => String::new(),
    }
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

const MUTATING: [&str; 10] = ["set_number", "push", "add", "remove", "set", "insert", "take", "clear", "push_front", "take_front"];

/// The clauses of a function's subject, from its Y declaration.
fn function_clauses(m: &Module, decls: &BTreeMap<String, &Node>, d: &Node, y: &str) -> Vec<Value> {
    let f = &m.fns[d.str("name").unwrap()];
    let mut cs = vec![];
    for w in d.nodes("where") {
        let text = format!("require {} : {}", w.str("name").unwrap(), clause_expr(m, w.node("condition").unwrap()));
        cs.push(json!({"kind": "require", "text": text, "source": format!("{y}:{}", w.line), "status": {"lifted": format!("src/{}.rs: y_runtime::require", m.name)}}));
    }
    for w in d.nodes("ensures") {
        let text = format!("ensure {} : {}", w.str("name").unwrap(), clause_expr(m, w.node("condition").unwrap()));
        cs.push(json!({"kind": "ensure", "text": text, "source": format!("{y}:{}", w.line), "status": {"lifted": format!("src/{}.rs: y_runtime::ensure", m.name)}}));
    }
    let at = format!("{y}:{}", d.line);
    let effects = |t: &str| json!({"kind": "effects", "text": t, "source": at});
    let caps: Vec<&str> = f.params.iter().filter(|p| p.injected).map(|p| p.name.as_str()).filter(|c| !matches!(*c, "forever" | "system")).collect();
    let mutable: Vec<&str> = f.params.iter().filter(|p| p.mutable && !p.injected).map(|p| p.name.as_str()).collect();
    if f.params.iter().any(|p| p.injected) {
        if !caps.is_empty() {
            cs.push(effects(&format!("effects only {}", caps.join(", "))));
        }
        if !caps.contains(&"clock") {
            cs.push(effects("no clock"));
        }
        if !caps.contains(&"random") {
            cs.push(effects("no randomness"));
        }
    } else if mutable.is_empty() {
        cs.push(effects("pure"));
    } else {
        for t in ["no io", "no clock", "no randomness"] {
            cs.push(effects(t));
        }
        let mut changed = BTreeSet::new();
        for p in &mutable {
            changed.extend(mutated(m, decls, &f.name, p, 0));
        }
        if !changed.is_empty() {
            let list: Vec<String> = changed.into_iter().collect();
            cs.push(json!({"kind": "access", "text": format!("mutates only {}", list.join(", ")), "source": at}));
        }
    }
    cs
}

/// The bare files with what the Y lowering adds to a strip.
pub fn lower(m: &Module, tree: &Node, source: &str, stripped: &Files, name: &str) -> Result<Files, String> {
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
        out.push((rel.clone(), (brs.clone(), js)));
    }
    Ok(out)
}

