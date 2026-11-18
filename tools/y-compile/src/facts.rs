//! `y-facts.json`: what the Y source says that neither the bare bodies nor the constraint files hold. The decisions and
//! the paths are found as `tools/paths/pathfacts.py` finds them, the paths by the enumerator `tools/bare-rust/tests/
//! y_paths`; the other facts are made from the Y source and the bare bodies, not carried over.
use crate::bare::Files;
use crate::model::{declarations, Module};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::Path;
use y_parse::Node;

#[path = "../../bare-rust/tests/y_paths/mod.rs"]
#[allow(dead_code)]
pub mod y_paths;

pub const ABOUT: &str = "What the Y source says that neither the bare bodies nor the constraint files can hold. Nothing in bare-rust reads this file; it is for the Y toolchain (y explain, y review, coverage), and tools/bare-rust/tests/y_examples.rs checks it against the Y source and the bare bodies.";

fn lines(files: &Files, rel: &str) -> Vec<String> {
    files.iter().find(|(p, _)| p.with_extension("brs").to_string_lossy() == rel.trim_start_matches("bare/")).map(|(_, (b, _))| b.lines().map(str::to_string).collect()).unwrap_or_default()
}

/// The Rust path of a function the module declares.
pub fn rust_path(krate: &str, m: &Module, f: &str) -> String {
    match m.facet_of(f) {
        Some(facet) => format!("{krate}::{}::{facet}::{f}", m.name),
        None => format!("{krate}::{}::{f}", m.name),
    }
}

fn fn_head(line: &str) -> Option<String> {
    let t = line.trim_start();
    let t = t.strip_prefix("pub ").unwrap_or(t);
    let t = t.strip_prefix("async ").unwrap_or(t);
    let t = t.strip_prefix("fn ")?;
    let end = t.find(['(', '<'])?;
    Some(t[..end].to_string())
}

#[allow(clippy::too_many_arguments)]
pub fn facts(dir: &Path, m: &Module, tree: &Node, test_tree: &Node, main_tree: &Node, files: &Files, krate: &str, name: &str, copies: &[(usize, String, String)]) -> Result<Value, String> {
    let y = format!("y/src/{name}.y");
    let ty = format!("y/src/{name}.test.y");
    let bare = format!("bare/src/{name}.brs");
    let mut f = Map::new();
    f.insert("format".into(), json!("y-facts/0"));
    f.insert("about".into(), json!(ABOUT));
    f.insert(
        "modules".into(),
        json!([
            {"y": y, "module": name, "bare": bare, "rust": format!("src/{name}.rs")},
            {"y": ty, "module": name, "bare": format!("bare/src/{name}/tests.brs"), "rust": format!("src/{name}/tests.rs")},
            {"y": "y/src/main.y", "module": "main", "bare": "bare/src/main.brs", "rust": "src/main.rs"},
        ]),
    );
    let facets: Vec<Value> = m
        .facets
        .iter()
        .map(|(facet, line, vis, members)| {
            json!({"module": name, "facet": facet, "path": format!("{name}.{facet}"), "visible": vis, "y": format!("{y}:{line}"), "rust": format!("{krate}::{name}::{facet}"), "members": members})
        })
        .collect();
    f.insert("facets".into(), Value::Array(facets));
    // the units the lowering erases: of each constant, and of each parameter, whose type has one
    let mut units = vec![];
    let unit_of = |t: &Node| t.str("text").filter(|x| x.contains("unit_type=") || *x == "duration").map(str::to_string);
    for (facet, d) in declarations(tree) {
        if d.kind == "constants" && !m.types.contains_key(d.str("name").unwrap()) {
            for i in d.nodes("items") {
                if let Some(u) = i.node("type").and_then(unit_of) {
                    let path = match &facet {
                        Some(fc) => format!("{krate}::{name}::{fc}::{}::{}", d.str("name").unwrap(), i.str("name").unwrap().to_uppercase()),
                        None => format!("{krate}::{name}::{}::{}", d.str("name").unwrap(), i.str("name").unwrap().to_uppercase()),
                    };
                    units.push(json!({"subject": path, "unit": u, "y": format!("{y}:{}", i.line)}));
                }
            }
        }
        if d.kind == "function" {
            for e in d.node("doc").map(|doc| doc.nodes("entries")).unwrap_or_default() {
                let pname = e.str("name").unwrap_or("");
                if !d.nodes("params").iter().any(|p| p.str("name") == Some(pname)) {
                    continue;
                }
                if let Some(u) = e.node("type").and_then(unit_of) {
                    units.push(json!({"subject": rust_path(krate, m, d.str("name").unwrap()), "param": pname, "unit": u, "y": format!("{y}:{}", d.line)}));
                }
            }
        }
    }
    f.insert("units".into(), Value::Array(units));
    // each function with injected capabilities, in the order of the source; `main` last
    let count = ["", "as the last parameter", "as the last two parameters, in order", "as the last three parameters, in order", "as the last four parameters, in order", "as the last five parameters, in order"];
    let mut caps = vec![];
    for (_, d) in declarations(tree) {
        if d.kind != "function" {
            continue;
        }
        let inj: Vec<String> = d.nodes("params").iter().filter(|p| p.flag("injected")).map(|p| p.str("name").unwrap()).map(|c| format!("{c} as {c}")).collect();
        if !inj.is_empty() {
            caps.push(json!({"subject": rust_path(krate, m, d.str("name").unwrap()), "injected": inj, "lowered": count[inj.len()], "y": format!("{y}:{}", d.line)}));
        }
    }
    for d in main_tree.nodes("decls").into_iter().filter(|d| d.kind == "function") {
        let inj: Vec<String> = d.nodes("params").iter().filter(|p| p.flag("injected")).map(|p| p.str("name").unwrap()).map(|c| format!("{c} as {c}")).collect();
        if !inj.is_empty() {
            caps.push(json!({"subject": format!("{krate}::{}", d.str("name").unwrap()), "injected": inj, "lowered": "System::root(), made by the entry point", "y": format!("y/src/main.y:{}", d.line)}));
        }
    }
    f.insert("capabilities".into(), Value::Array(caps));
    // each claim outside a region, `e forever because "…"`: its Y line, and the bare line its marker is on
    let mut claims = vec![];
    for (yp, t, bp) in [(&y, tree, &bare), (&"y/src/main.y".to_string(), main_tree, &"bare/src/main.brs".to_string())] {
        let bl = lines(files, bp);
        let mut used = BTreeSet::new();
        for n in t.find("forever") {
            let because = n.str("because").unwrap_or("");
            let marker = format!("// y: forever because \"{because}\"");
            let hit = bl.iter().enumerate().find(|(k, l)| l.contains(&marker) && !used.contains(k)).map(|(k, _)| k);
            if let Some(k) = hit {
                used.insert(k);
                claims.push(json!({"claim": "forever", "because": because, "requires": "Forever", "y": format!("{yp}:{}", n.line), "bare": format!("{bp}:{}", k + 1)}));
            }
        }
    }
    f.insert("claims".into(), Value::Array(claims));
    f.insert("suspensions".into(), json!([]));
    // each `compile`: the call, and where the entry point computes its value
    let mut compiled = vec![];
    let ml = lines(files, "bare/src/main.brs");
    for n in main_tree.find("compile") {
        let call = crate::lower::y_text(n.node("value").unwrap());
        let marker = format!("// y: compile {call}");
        if let Some(k) = ml.iter().position(|l| l.contains(&marker)) {
            compiled.push(json!({"call": call, "value": "computed by the entry point when it starts", "y": format!("y/src/main.y:{}", n.line), "bare": format!("bare/src/main.brs:{}", k + 1)}));
        }
    }
    f.insert("compiled".into(), Value::Array(compiled));
    // each copy the lowering made: its Y line, and the bare line its marker is on, in order
    let bl = lines(files, &bare);
    let marked: Vec<usize> = bl.iter().enumerate().filter(|(_, l)| l.contains("// y: a copy")).map(|(k, _)| k + 1).collect();
    let cs: Vec<Value> = copies.iter().zip(marked).map(|((line, what, why), b)| json!({"what": what, "why": why, "y": format!("{y}:{line}"), "bare": format!("{bare}:{b}")})).collect();
    f.insert("copies".into(), Value::Array(cs));
    // the decisions of each function, in the order of the lines that open them
    let mut decisions = Map::new();
    let mut groups: Vec<(String, Vec<Value>)> = vec![];
    for (_, d) in declarations(tree) {
        if d.kind != "function" {
            continue;
        }
        let mut opened: Vec<(usize, Value)> = vec![];
        let branch = |b: &Node| json!({"name": b.str("name").unwrap(), "y": format!("{y}:{}", b.line)});
        d.walk(&mut |n: &Node| match n.kind {
            "if" | "while" | "for" => {
                let mut bs = vec![branch(n)];
                bs.extend(n.nodes("else").into_iter().map(branch));
                opened.push((n.line, json!({"kind": n.kind, "branches": bs})));
            }
            "conditional" | "compare" => {
                let mut bs: Vec<Value> = n.nodes("conditions").into_iter().map(branch).collect();
                bs.extend(n.nodes("else").into_iter().map(branch));
                opened.push((n.line, json!({"kind": n.kind, "label": n.str("label").unwrap(), "y": format!("{y}:{}", n.line), "branches": bs})));
            }
            "region" if !n.nodes("orelse").is_empty() => {
                let mut bs = vec![branch(n)];
                bs.extend(n.nodes("orelse").into_iter().map(branch));
                opened.push((n.nodes("orelse")[0].line, json!({"kind": "orelse", "branches": bs})));
            }
            _ => {}
        });
        opened.sort_by_key(|o| o.0);
        if !opened.is_empty() {
            groups.push((d.str("name").unwrap().to_string(), opened.into_iter().map(|o| o.1).collect()));
        }
    }
    // where the bare body takes each branch: the next `#name` in the function not yet given to a branch
    let bl = lines(files, &bare);
    let mut used: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut current = String::new();
    for (i, l) in bl.iter().enumerate() {
        if let Some(h) = fn_head(l) {
            current = h;
        }
        let Some((_, marker)) = l.split_once("// y:") else { continue };
        for cap in marker.split('#').skip(1) {
            let mark: String = cap.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == '\'').collect();
            let base = mark.split('\'').next().unwrap().to_string();
            for (fname, gs) in groups.iter_mut() {
                if *fname != current {
                    continue;
                }
                'found: for g in gs.iter_mut() {
                    for b in g["branches"].as_array_mut().unwrap() {
                        if b["name"] == base.as_str() && b.get("bare").is_none() && !used.contains(&(mark.clone(), i)) {
                            b["bare"] = json!(format!("{bare}:{}", i + 1));
                            used.insert((mark.clone(), i));
                            break 'found;
                        }
                    }
                }
            }
        }
    }
    for (fname, gs) in groups {
        for g in &gs {
            for b in g["branches"].as_array().unwrap() {
                if b.get("bare").is_none() {
                    return Err(format!("{fname}#{}: no marker in the bare body", b["name"]));
                }
            }
        }
        decisions.insert(format!("{name}.{fname}"), Value::Array(gs));
    }
    f.insert("decisions".into(), Value::Object(decisions));
    let (paths, problems) = y_paths::all_paths(&crate::read(&dir.join(&y))?);
    if !problems.is_empty() {
        return Err(format!("{y}: {problems:?}"));
    }
    let ends = |k: &str| match k {
        "next" | "return" | "continue" | "break" => "returns",
        "fail" => "fails",
        o => o,
    }
    .to_string();
    f.insert("paths".into(), Value::Array(paths.iter().map(|(p, k)| json!({"path": p, "ends": ends(k)})).collect()));
    let mut tests = Map::new();
    for t in test_tree.nodes("decls").into_iter().filter(|d| d.kind == "test") {
        let n = t.str("name").unwrap();
        let covers: Vec<&str> = t.node("covers").and_then(|c| c.str("text")).into_iter().collect();
        tests.insert(n.to_string(), json!({"y": format!("{ty}:{}", t.line), "rust": format!("{krate}::{name}::tests::{n}"), "covers": covers}));
    }
    f.insert("tests".into(), Value::Object(tests));
    Ok(Value::Object(f))
}
