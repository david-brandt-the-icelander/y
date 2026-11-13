//! Levels (language 6.22, Y § 3.19): how much of its language a module uses.
//!
//! A module subject may carry a `level`, a ceiling that nothing in the module may rise above. `uses` finds what a bare
//! body uses above `data`, each with the level it needs and the function and line it is at; `check` holds every module
//! that has a level to it, the modules inside it (Rust's `mod`, Y's facets) and the files under it among them. A
//! module's tests are not part of what it carries to a backend, so a `tests` module is not held to its level.
//!
//! What each level adds, as bare Rust shows it:
//! - `calculations`: a function body;
//! - `structured`: a loop (`for`, `while`, `loop`), an assignment, `&mut`, a call of the function it is in;
//! - `shared`: shared state, transactions, arenas and resources (a parameter of type `Shared<…>`, `Shared::new`,
//!   `atomic`, `atomic_when`, `orelse`, `Arena::new`, `Held::new`, and std's `Cell`, `RefCell`, `Mutex`, `RwLock`,
//!   `Rc`, `Arc`);
//! - `effects`: a capability (a parameter of one of the runtime's capability types, `Console`, `Random`, `Clock`,
//!   `Window`, `Http`, `Store`, `Forever`, `System`, …), an `effects only …` clause, `async`, `.await`, the runtime's
//!   tasks and deadlines (`supervised`, `detach`, `Cancellation::held`, `Deadline::after`), and a region that waits
//!   `forever` or is detached.
use crate::region::functions;
use crate::{parse_bare, text};
use serde_json::Value;
use tree_sitter::Node;

/// The levels, lowest first.
pub const LEVELS: &[&str] = &["data", "calculations", "structured", "shared", "effects"];

/// The rank of a level, or `None` for a word that is not one.
pub fn rank(level: &str) -> Option<usize> {
    LEVELS.iter().position(|l| *l == level)
}

/// One construct above `data`: the level it needs, what it is, the function it is in and its line in the bare body.
#[derive(Debug, Clone, PartialEq)]
pub struct Use {
    pub level: usize,
    pub what: String,
    pub function: String,
    pub line: usize,
}

const SHARED: &[&str] = &["Shared::new", "Arena::new", "Held::new", "Cell::new", "RefCell::new", "Mutex::new", "RwLock::new", "Rc::new", "Arc::new"];
const SHARED_CALLS: &[&str] = &["atomic", "atomic_when", "orelse"];
/// The runtime's capabilities (Y § 3.7): a parameter of one of these types is an injected capability.
const CAPABILITIES: &[&str] = &["System", "Console", "Random", "Clock", "Window", "Http", "Forever", "Store", "Deadline", "Detached"];
const EFFECT_CALLS: &[&str] = &["supervised", "detach", "Cancellation::held", "Deadline::after"];

/// What the bare body `brs`, whose constraint file is `js`, uses above `data`.
pub fn uses(brs: &str, js: &Value) -> Vec<Use> {
    let base = crate::weave::base_path(js);
    let tree = parse_bare(brs);
    let mut out = vec![];
    // what the constraint file says of a function: its parameters' types and its bounds on effects
    let subject = |f: &str| js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "function" && s["name"] == f).cloned();
    for (path, node) in functions(tree.root_node(), brs, &base) {
        let name = path.rsplit("::").next().unwrap_or("").to_string();
        let line = node.start_position().row + 1;
        let mut add = |level: &str, what: String, line: usize| out.push(Use { level: rank(level).unwrap(), what, function: path.clone(), line });
        if node.child_by_field_name("body").is_some() {
            add("calculations", "a function body".into(), line);
        }
        if (0..node.child_count()).filter_map(|i| node.child(i)).any(|c| c.kind() == "function_modifiers" && text(c, brs).contains("async")) {
            add("effects", "an `async` function".into(), line);
        }
        if let Some(sub) = subject(&path) {
            for p in sub["params"].as_array().into_iter().flatten() {
                let ty = p["type"].as_str().unwrap_or("");
                let ty = ty.strip_prefix("ref<target_type=").and_then(|t| t.strip_suffix('>')).unwrap_or(ty);
                let head = ty.split('<').next().unwrap_or("");
                let n = p["name"].as_str().unwrap_or("");
                if CAPABILITIES.contains(&head) {
                    add("effects", format!("the capability `{n}: {head}`"), line);
                } else if head == "Shared" {
                    add("shared", format!("the shared state `{n}`"), line);
                }
            }
            for c in sub["clauses"].as_array().into_iter().flatten().filter(|c| c["kind"] == "effects") {
                if let Some(t) = c["text"].as_str().filter(|t| t.starts_with("effects only ") && *t != "effects only none") {
                    add("effects", format!("`{t}`"), line);
                }
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            walk(body, brs, &name, &mut add);
        }
    }
    out
}

fn walk(n: Node, src: &str, me: &str, add: &mut impl FnMut(&str, String, usize)) {
    let line = n.start_position().row + 1;
    match n.kind() {
        // a function inside this one is reported as its own
        "function_item" => return,
        "for_expression" | "while_expression" | "loop_expression" => add("structured", "a loop".into(), line),
        "assignment_expression" | "compound_assignment_expr" => add("structured", "an assignment".into(), line),
        "reference_expression" if text(n, src).starts_with("&mut") => add("structured", "`&mut`".into(), line),
        "await_expression" => add("effects", "`.await`".into(), line),
        "line_comment" => {
            let t = text(n, src);
            if t.starts_with("// y:") && (t.contains(" forever because \"") || t.contains(" detach because \"")) {
                add("effects", "a region that waits forever or is detached".into(), line);
            }
        }
        "call_expression" => {
            if let Some(f) = n.child_by_field_name("function") {
                let callee = text(f, src);
                let last2: Vec<&str> = callee.rsplitn(3, "::").collect();
                let tail = if last2.len() >= 2 { format!("{}::{}", last2[1], last2[0]) } else { callee.to_string() };
                if callee == me {
                    add("structured", format!("a call of `{me}` inside it"), line);
                }
                if SHARED.contains(&tail.as_str()) || (f.kind() == "identifier" && SHARED_CALLS.contains(&callee)) {
                    add("shared", format!("`{callee}`"), line);
                }
                if EFFECT_CALLS.contains(&tail.as_str()) || (f.kind() == "identifier" && EFFECT_CALLS.contains(&callee)) {
                    add("effects", format!("`{callee}`"), line);
                }
            }
        }
        _ => {}
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        walk(ch, src, me, add);
    }
}

/// The modules with a `level` in the constraint files (`(bare file, bare body, constraint file)`), and everything
/// in them that needs more: one message for each, naming the construct, where it is, the level it needs and the
/// module's.
pub fn check(files: &[(String, String, Value)]) -> Vec<String> {
    let pinned: Vec<(String, usize)> = files
        .iter()
        .flat_map(|(_, _, js)| js["subjects"].as_array().into_iter().flatten())
        .filter(|s| s["kind"] == "module")
        .filter_map(|s| Some((s["name"].as_str()?.to_string(), rank(s["level"].as_str()?)?)))
        .collect();
    let mut errs = vec![];
    for (file, brs, js) in files {
        for u in uses(brs, js) {
            if u.function.split("::").any(|seg| seg == "tests") {
                continue;
            }
            for (m, level) in &pinned {
                if u.function.starts_with(&format!("{m}::")) && u.level > *level {
                    errs.push(format!(
                        "{file}:{}: {} in `{}` needs level `{}`, and module `{m}` is at `{}` (language 6.22)",
                        u.line, u.what, u.function, LEVELS[u.level], LEVELS[*level]
                    ));
                }
            }
        }
    }
    errs
}

/// The level a module of the files needs, the highest of the uses under it (`data` if none), with the first use that
/// needs it and its file. For proposing a level to pin (language 6.22, harvested).
pub fn needed(files: &[(String, String, Value)], module: &str) -> (&'static str, Option<(String, Use)>) {
    let mut top: Option<(String, Use)> = None;
    for (file, brs, js) in files {
        for u in uses(brs, js) {
            if u.function.starts_with(&format!("{module}::")) && !u.function.split("::").any(|seg| seg == "tests") && top.as_ref().is_none_or(|(_, t)| u.level > t.level) {
                top = Some((file.clone(), u));
            }
        }
    }
    (LEVELS[top.as_ref().map_or(0, |(_, u)| u.level)], top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(brs: &str, subjects: Value) -> (String, String, Value) {
        ("src/m.brs".into(), brs.into(), json!({"crate": "c 0.0.0", "module": "src/m.rs", "subjects": subjects}))
    }

    fn pure(name: &str) -> Value {
        json!({"kind": "function", "name": name, "params": [], "clauses": [{"kind": "effects", "text": "pure", "source": "y/src/m.y:1"}]})
    }

    #[test]
    fn each_construct_needs_its_level() {
        let brs = "mod sums {\n    fn total(xs) {\n        let t = 0;\n        for x in xs {\n            t += x;\n        }\n        t\n    }\n    fn twice(x) {\n        x * 2\n    }\n}\n";
        let f = file(brs, json!([pure("c::m::sums::total"), pure("c::m::sums::twice")]));
        let levels: Vec<(String, &str)> = uses(&f.1, &f.2).into_iter().map(|u| (u.what, LEVELS[u.level])).collect();
        assert!(levels.contains(&("a loop".into(), "structured")));
        assert!(levels.contains(&("an assignment".into(), "structured")));
        assert_eq!(needed(&[f.clone()], "c::m::sums").0, "structured");
        // pinned at `calculations`, the loop and the assignment fail; at `structured`, nothing does
        let at = |level: &str| {
            let mut g = f.clone();
            g.2["subjects"].as_array_mut().unwrap().push(json!({"kind": "module", "name": "c::m::sums", "level": level}));
            check(&[g])
        };
        let errs = at("calculations");
        assert_eq!(errs.len(), 2, "{errs:#?}");
        assert!(errs[0].contains("a loop in `c::m::sums::total` needs level `structured`, and module `c::m::sums` is at `calculations`"));
        assert!(at("structured").is_empty());
        assert_eq!(at("data").len(), 4);
    }

    #[test]
    fn shared_state_and_effects() {
        let brs = "fn count(sold) {\n    atomic(|tx| tx.get(sold))\n}\nfn show(console) {\n    console.write_line(\"hi\")\n}\nasync fn wait(w) {\n    w.next().await\n}\n";
        let f = file(brs, json!([pure("c::m::count"), {"kind": "function", "name": "c::m::show", "params": [{"name": "console", "type": "ref<target_type=Console>"}]}, pure("c::m::wait")]));
        let found: Vec<(String, &str)> = uses(&f.1, &f.2).into_iter().filter(|u| u.level >= 3).map(|u| (u.function, LEVELS[u.level])).collect();
        assert!(found.contains(&("c::m::count".into(), "shared")));
        // `show` is given the console; `wait` is async and awaits
        assert!(found.contains(&("c::m::show".into(), "effects")));
        assert_eq!(found.iter().filter(|(f, l)| f == "c::m::wait" && *l == "effects").count(), 2);
    }

    #[test]
    fn a_modules_tests_are_not_held_to_its_level() {
        let brs = "mod tests {\n    fn t() {\n        loop {}\n    }\n}\n";
        let mut f = file(brs, json!([pure("c::m::tests::t")]));
        f.2["subjects"].as_array_mut().unwrap().push(json!({"kind": "module", "name": "c::m", "level": "calculations"}));
        assert!(check(&[f]).is_empty());
    }
}
