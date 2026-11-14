//! The examples under the repository's `examples/` start from Y source (`y/src/*.y`). Their bare bodies and constraint
//! files are what the Y compiler would lower that source to; no Y compiler exists yet, so they are written by hand.
//! `weave` with cargo made their Rust (`src/`). This test keeps the four in step without cargo:
//!
//! - weaving the committed bare files gives the committed Rust, apart from the `mut` that weave derives through rustc;
//! - stripping the committed Rust gives the committed bare bodies byte for byte, and the same subjects and holes: what
//!   the Y lowering adds beyond strip's output is only clauses and the file's status;
//! - the constraint files are valid, and their clauses are well formed;
//! - `bare/y-facts.json` matches the Y source and the bare bodies: each named decision is in both, where it says, and
//!   each test covers what its Y source and its Rust comment say;
//! - the paths of each module (`y_paths`, Y § 3.5) are the ones `bare/y-facts.json` lists, every loop that may never
//!   run has its `else`, and the tests together cover every path that needs a test (Y § 3.14).
//!
//! The Y source is read through its tree, from `y-parse` (`tools/y-parse`): for the places the facts give, the regions,
//! the named decisions, the layout and lint rules and each test's `when` here, and in `y_levels`, `y_lint` and
//! `y_paths`.
//!
//! To refresh the Rust after editing the bare files, from `tools/bare-rust`:
//! `cargo run -q -- weave ../../examples/<name>/bare --out ../../examples/.woven --crate ../../examples/<name>`, then
//! copy `.woven/src` over `examples/<name>/src` and delete `.woven`.
mod y_levels;
mod y_lint;
mod y_paths;

use bare_rust::corpus::{crate_trees, strip_sources};
use bare_rust::json::canonical;
use bare_rust::weave::weave_in_crate;
use bare_rust::{normalise_nomut, rustfmt};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
}

/// The lint `type_argument_name` (Y § 3.1) over every Y file of every example: an argument that takes a type is named
/// `…_type`, and one that takes a value is not.
#[test]
fn every_example_names_its_type_arguments_by_the_convention() {
    let mut errors = vec![];
    for name in ["checkers", "hangman", "memory", "nim", "snake", "tic-tac-toe", "warehouse"] {
        let dir = example(name).join("y/src");
        let mut files: Vec<PathBuf> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        for f in files {
            let shown = format!("examples/{name}/y/src/{}", f.file_name().unwrap().to_string_lossy());
            errors.extend(y_lint::type_argument_names(&shown, &read(&f)));
        }
    }
    assert!(errors.is_empty(), "{errors:#?}");
}

/// The bare modules of an example, as paths under `bare/src` and `src` without their extensions.
fn modules(dir: &Path) -> Vec<String> {
    fn go(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                go(base, &p, out);
            } else if p.extension().is_some_and(|x| x == "brs") {
                out.push(p.strip_prefix(base).unwrap().with_extension("").to_string_lossy().to_string());
            }
        }
    }
    let mut out = vec![];
    go(&dir.join("bare/src"), &dir.join("bare/src"), &mut out);
    out.sort();
    out
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The constraint file as strip writes it: no clauses, no module level, no regions, no `injected` mark on a parameter,
/// and the status strip gives.
fn without_lowering(js: &Value) -> Value {
    let mut js = js.clone();
    js["status"] = Value::from("lifted");
    js["subjects"].as_array_mut().unwrap().retain(|s| s["kind"] != "region");
    for s in js["subjects"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("clauses");
        s.as_object_mut().unwrap().remove("level");
        for p in s.get_mut("params").and_then(Value::as_array_mut).into_iter().flatten() {
            p.as_object_mut().unwrap().remove("injected");
        }
    }
    js
}

#[test]
fn hangman_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("hangman", &["hangman", "hangman/tests", "lib", "main"]);
}

#[test]
fn warehouse_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("warehouse", &["lib", "main", "warehouse", "warehouse/tests"]);
}

#[test]
fn nim_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("nim", &["lib", "main", "nim", "nim/tests"]);
}

#[test]
fn snake_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("snake", &["lib", "main", "snake", "snake/tests"]);
}

#[test]
fn tic_tac_toe_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("tic-tac-toe", &["lib", "main", "tic_tac_toe", "tic_tac_toe/tests"]);
}

#[test]
fn memory_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("memory", &["lib", "main", "memory", "memory/tests"]);
}

#[test]
fn checkers_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("checkers", &["checkers", "checkers/tests", "lib", "main"]);
}

/// Each file of the example weaves to its committed Rust, and the crate's Rust strips back to the bare files. Both are
/// done to the crate as a whole: a realization (a call with explicit type arguments, language § 3.5) belongs to its
/// crate, and is listed in the file of the crate's root (`lib` or `main`), not in the file whose body uses it.
fn weaves_and_strips_back(name: &str, expected: &[&str]) {
    let dir = example(name);
    let mods = modules(&dir);
    assert_eq!(mods, expected);
    let trees = crate_trees(&dir, false).unwrap();
    let crate_of = |m: &str| -> PathBuf {
        let rel = PathBuf::from(format!("src/{m}.rs"));
        trees.iter().find(|(_, files)| files.contains(&rel)).unwrap_or_else(|| panic!("{m}: in no crate of the package")).0.clone()
    };
    let constraints = |m: &str| -> Value { serde_json::from_str(&read(&dir.join(format!("bare/src/{m}.constraints.json")))).unwrap() };
    let mut sources = vec![];
    for m in &mods {
        let brs = read(&dir.join(format!("bare/src/{m}.brs")));
        let cjs = read(&dir.join(format!("bare/src/{m}.constraints.json")));
        let js: Value = serde_json::from_str(&cjs).unwrap();
        assert_eq!(canonical(&js), cjs, "{m}: the constraint file is not in canonical form (bare-rust libfmt)");
        let root = crate_of(m);
        let root_module = root.with_extension("").strip_prefix("src").unwrap().to_string_lossy().into_owned();
        let reals = constraints(&root_module)["realizations"].as_object().cloned().unwrap_or_default();
        let woven = weave_in_crate(&brs, &js, Some(&reals)).unwrap_or_else(|e| panic!("{m}: weave: {}", e.msg)).text;
        let committed = read(&dir.join(format!("src/{m}.rs")));
        let norm = |s: &str| normalise_nomut(s).and_then(|n| rustfmt(&n));
        assert_eq!(norm(&woven).unwrap(), norm(&committed).unwrap(), "{m}: the committed Rust is not the weave of the bare files");
        sources.push((PathBuf::from(format!("src/{m}.rs")), committed));
    }
    let krate = constraints(&mods[0])["crate"].as_str().unwrap().to_string();
    for (rel, r) in strip_sources(&krate, &sources, &trees) {
        let m = rel.with_extension("").strip_prefix("src").unwrap().to_string_lossy().into_owned();
        let (b2, j2) = r.unwrap_or_else(|e| panic!("{m}: strip: {e}"));
        assert_eq!(b2, read(&dir.join(format!("bare/src/{m}.brs"))), "{m}: strip of the Rust differs from the bare body");
        assert_eq!(j2, without_lowering(&constraints(&m)), "{m}: strip of the Rust differs from the constraint file in more than clauses and status");
    }
}

#[test]
fn hangman_constraint_files_are_valid() {
    constraint_files_are_valid("hangman");
    let dir = example("hangman");
    // the clauses are really read: a name the module does not have is refused, and so is a function that is not pure
    let js: Value = serde_json::from_str(&read(&dir.join("bare/src/hangman.constraints.json"))).unwrap();
    for (text, want) in [("require has_words : count(wordz) > 0", "unknown name `wordz`"), ("require has_words : guess(words, words) is Hit", "unknown name `guess`")] {
        let mut bad = js.clone();
        let play = bad["subjects"].as_array_mut().unwrap().iter_mut().find(|s| s["name"] == "hangman_example::hangman::console_play::play").unwrap();
        play["clauses"][0]["text"] = Value::from(text);
        let errors = bare_rust::libcheck::validate(&bad, None);
        assert!(errors.iter().any(|e| e.contains(": clause 0:") && e.contains(want)), "{text}: {errors:#?}");
    }
}

#[test]
fn warehouse_constraint_files_are_valid() {
    constraint_files_are_valid("warehouse");
}

#[test]
fn nim_constraint_files_are_valid() {
    constraint_files_are_valid("nim");
}

#[test]
fn snake_constraint_files_are_valid() {
    constraint_files_are_valid("snake");
}

#[test]
fn tic_tac_toe_constraint_files_are_valid() {
    constraint_files_are_valid("tic-tac-toe");
}

#[test]
fn memory_constraint_files_are_valid() {
    constraint_files_are_valid("memory");
}

#[test]
fn checkers_constraint_files_are_valid() {
    constraint_files_are_valid("checkers");
}

fn constraint_files_are_valid(name: &str) {
    let dir = example(name);
    for m in modules(&dir) {
        let js: Value = serde_json::from_str(&read(&dir.join(format!("bare/src/{m}.constraints.json")))).unwrap();
        // `validate` is written for library files, whose subjects are named by canonical paths under the module and whose
        // impls name their trait and `self` type; a module's file names subjects by crate path, and an impl's trait and
        // `self` are in the bare header when it names them in full. Every other rule applies, the clauses' among them.
        let library_only = |e: &String| {
            ["is not a canonical path under", "an impl names its `trait` and the `self` type", "an impl is named `<type> as <trait>`", "is not an impl of"]
                .iter()
                .any(|m| e.contains(m))
        };
        let errors: Vec<String> = bare_rust::libcheck::validate(&js, None).into_iter().filter(|e| !library_only(e)).collect();
        assert!(errors.is_empty(), "{m}: {errors:#?}");
    }
    // every module with a `level` uses nothing above it (language 6.22)
    let bodies: Vec<(String, String, Value)> = modules(&dir)
        .iter()
        .map(|m| (format!("bare/src/{m}.brs"), read(&dir.join(format!("bare/src/{m}.brs"))), serde_json::from_str(&read(&dir.join(format!("bare/src/{m}.constraints.json")))).unwrap()))
        .collect();
    let errors = bare_rust::level::check(&bodies);
    assert!(errors.is_empty(), "{errors:#?}");
    // and each facet's pinned level is the one its Y source needs, read from the Y alone (`y_levels`)
    let pinned: BTreeMap<String, String> = bodies
        .iter()
        .flat_map(|(_, _, js)| js["subjects"].as_array().unwrap().clone())
        .filter(|s| s["kind"] == "module" && s["level"].is_string())
        .map(|s| (s["name"].as_str().unwrap().to_string(), s["level"].as_str().unwrap().to_string()))
        .collect();
    let y_dir = dir.join("y/src");
    let mut ys: Vec<PathBuf> = fs::read_dir(&y_dir).unwrap().map(|e| e.unwrap().path()).collect();
    ys.sort();
    let mut checked = 0;
    for y in ys.iter().filter(|p| p.extension().is_some_and(|e| e == "y") && !p.to_string_lossy().ends_with(".test.y")) {
        for (facet, (level, why)) in y_levels::facet_levels(&read(y)) {
            let Some((module, pin)) = pinned.iter().find(|(m, _)| m.ends_with(&format!("::{facet}"))) else { continue };
            assert_eq!(pin, y_levels::LEVELS[level], "{module} is pinned at `{pin}`, and its Y source needs `{}` ({why})", y_levels::LEVELS[level]);
            checked += 1;
        }
    }
    assert!(checked > 0 || !pinned.keys().any(|m| m.matches("::").count() > 1), "no facet's level was checked against the Y source");
}

/// Warehouse's regions and levels are enforced, not only accepted: each change below is refused, by validation
/// (language 6.23's rules), by weave (a region with no marker in its function, whose marker says other constraints, or
/// whose code does not keep them) or by the level check (language 6.22).
#[test]
fn warehouse_regions_and_levels_are_enforced() {
    let dir = example("warehouse");
    let brs = read(&dir.join("bare/src/warehouse.brs"));
    let js: Value = serde_json::from_str(&read(&dir.join("bare/src/warehouse.constraints.json"))).unwrap();
    let region = |js: &Value, name: &str| -> usize {
        js["subjects"].as_array().unwrap().iter().position(|s| s["name"] == format!("warehouse_example::warehouse::{name}")).unwrap()
    };
    let woven = |js: &Value| weave_in_crate(&brs, js, Some(&serde_json::Map::new())).err().map(|e| e.msg).unwrap_or_default();
    let validated = |js: &Value| bare_rust::libcheck::validate(js, None).join("\n");
    assert_eq!(woven(&js), "");
    // a region its function does not mark
    let mut renamed = js.clone();
    let i = region(&mut renamed, "packing::pack_orders#count_sale");
    renamed["subjects"][i]["name"] = "warehouse_example::warehouse::packing::pack_orders#count_sales".into();
    assert!(woven(&renamed).contains("has no `// y: region count_sales is …`"), "{}", woven(&renamed));
    // a region whose clauses leave out what its marker states
    let mut fewer = js.clone();
    let i = region(&mut fewer, "receipts::file_receipts#filing");
    fewer["subjects"][i]["clauses"].as_array_mut().unwrap().remove(0);
    assert!(woven(&fewer).contains("the bare body's marker says `uncancellable`, and no clause does"), "{}", woven(&fewer));
    // a snapshot region whose code runs it serializable
    let serializable = brs.replace("Isolation::Snapshot", "Isolation::Serializable");
    let refused = weave_in_crate(&serializable, &js, Some(&serde_json::Map::new())).err().map(|e| e.msg).unwrap_or_default();
    assert!(refused.contains("says `isolation snapshot`, and the code its marker starts does not run it at `y_runtime::shared::Isolation::Snapshot`"), "{refused}");
    // a region whose tasks are supervised, and whose clauses do not say how
    let mut unsupervised = js.clone();
    let i = region(&unsupervised, "packing::pack_day#packing");
    unsupervised["subjects"][i]["clauses"].as_array_mut().unwrap().retain(|c| !c["text"].as_str().unwrap().starts_with("supervise"));
    assert!(woven(&unsupervised).contains("the bare body's marker says `supervise`, and no clause does"), "{}", woven(&unsupervised));
    // a region that waits until a condition, and is not atomic
    let mut waits = js.clone();
    let i = region(&mut waits, "stock::reserve#take_from_shelf");
    waits["subjects"][i]["clauses"].as_array_mut().unwrap().remove(0);
    assert!(validated(&waits).contains("only an atomic region can wait for"), "{}", validated(&waits));
    // a region in a function the file does not have
    let mut lost = js.clone();
    let i = region(&mut lost, "stock::reserve#put_on_backorder");
    lost["subjects"][i]["name"] = "warehouse_example::warehouse::stock::restock#put_on_backorder".into();
    assert!(validated(&lost).contains("which is not a function of the file"), "{}", validated(&lost));
    // `pricing` pinned below what `split` uses
    let mut low = js.clone();
    let i = region(&mut low, "pricing");
    low["subjects"][i]["level"] = "calculations".into();
    let errors = bare_rust::level::check(&[("warehouse.brs".into(), brs.clone(), low)]).join("\n");
    assert!(errors.contains("`&mut` in `warehouse_example::warehouse::pricing::split` needs level `structured`, and module `warehouse_example::warehouse::pricing` is at `calculations`"), "{errors}");
}

#[test]
fn hangman_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("hangman");
}

#[test]
fn warehouse_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("warehouse");
}

#[test]
fn nim_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("nim");
}

#[test]
fn snake_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("snake");
}

#[test]
fn tic_tac_toe_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("tic-tac-toe");
}

#[test]
fn memory_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("memory");
}

#[test]
fn checkers_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("checkers");
}

fn facts_match_the_y_source_and_the_bare_bodies(name: &str) {
    let dir = example(name);
    let facts: Value = serde_json::from_str(&read(&dir.join("bare/y-facts.json"))).unwrap();
    // the example's Y files, from the facts: the source, then the source without its tests
    let all: Vec<String> = facts["modules"].as_array().unwrap().iter().map(|m| m["y"].as_str().unwrap().to_string()).collect();
    let code: Vec<String> = all.iter().filter(|y| !y.ends_with(".test.y")).cloned().collect();
    // a place in a bare body, `file:line`, to the text of that line
    let at = |place: &str| -> String {
        let (file, line) = place.rsplit_once(':').unwrap();
        assert!(file.starts_with("bare/"), "{place}: a place in the Y source is read from its tree");
        let n: usize = line.parse().unwrap();
        read(&dir.join(file)).lines().nth(n - 1).unwrap_or_else(|| panic!("{place}: no such line")).to_string()
    };
    // a place in the Y source, `file:line`, to the nodes of its tree that start on that line; and whether that line is
    // in the function `name`
    let trees: BTreeMap<String, y_parse::Node> = all.iter().map(|y| (y.clone(), y_parse::parse_file(&dir.join(y)).unwrap_or_else(|e| panic!("{e}")))).collect();
    let place = |p: &str| -> (&y_parse::Node, usize) {
        let (file, line) = p.rsplit_once(':').unwrap();
        (trees.get(file).unwrap_or_else(|| panic!("{p}: not a Y file of the example")), line.parse().unwrap())
    };
    let nodes_at = |p: &str| -> Vec<&y_parse::Node> {
        let (tree, line) = place(p);
        let mut out = vec![];
        tree.walk(&mut |n: &y_parse::Node| {
            if n.line == line {
                out.push(n);
            }
        });
        out
    };
    let in_function = |p: &str, name: &str| -> bool {
        let (tree, line) = place(p);
        tree.find("function").iter().any(|f| f.str("name") == Some(name) && f.line <= line && f.num("end").is_some_and(|e| line <= e))
    };
    let mut listed = BTreeSet::new();
    for (function, decisions) in facts["decisions"].as_object().unwrap() {
        for d in decisions.as_array().unwrap() {
            // every outcome of a decision is a named branch (Y-proposal § 3.5): `if a = (…):` and `else b:`, each on a
            // line of its own; a `conditional` or a `compare` names each condition after its label, `condition
            // label.a = p:`, and may end in `else label.c:`; `for a in …:` and `while a = (…):` end in `else c` or
            // `else c:` unless they are sure to run
            let kind = d["kind"].as_str().unwrap();
            let switch = kind == "conditional" || kind == "compare";
            let branches = d["branches"].as_array().unwrap();
            let least = if kind == "for" || kind == "while" { 1 } else { 2 };
            assert!(branches.len() >= least, "{function}: a decision with one named outcome: {branches:?}");
            let in_fn = function.rsplit('.').next().unwrap();
            if switch {
                let (label, y) = (d["label"].as_str().unwrap(), d["y"].as_str().unwrap());
                let head = nodes_at(y).into_iter().any(|n| n.kind == kind && n.str("label") == Some(label));
                assert!(head && in_function(y, in_fn), "{function}: no `{kind} {label}` at {y}");
            }
            // the alternatives of an `orelse`: the regions its `orelse region` lines add to the first
            let alternatives: BTreeSet<(usize, &str)> = trees.values().flat_map(|t| t.find("region")).flat_map(|r| r.nodes("orelse")).map(|a| (a.line, a.str("name").unwrap())).collect();
            for (k, b) in branches.iter().enumerate() {
                let (name, y) = (b["name"].as_str().unwrap(), b["y"].as_str().unwrap());
                let line = y.rsplit_once(':').unwrap().1.parse::<usize>().unwrap();
                let named = |n: &&y_parse::Node, kinds: &[&str]| kinds.contains(&n.kind) && n.str("name") == Some(name);
                let here = nodes_at(y);
                let opens = if kind == "orelse" {
                    // alternatives: `region a is …:`, then `orelse region b is …:`
                    here.iter().any(|n| named(n, &["region"]) && (k > 0) == alternatives.contains(&(line, name)))
                } else if k > 0 {
                    here.iter().any(|n| named(n, if switch { &["else", "condition"] } else { &["else"] }))
                } else if switch {
                    here.iter().any(|n| named(n, &["condition"]))
                } else {
                    here.iter().any(|n| named(n, &[kind]))
                };
                assert!(opens && in_function(y, in_fn), "{function}#{name}: not at {y}");
                assert!(at(b["bare"].as_str().unwrap()).contains(&format!("#{name}")), "{function}#{name}: not at {}", b["bare"]);
                listed.insert(format!("{in_fn}#{name}"));
            }
            let last = branches.last().unwrap()["y"].as_str().unwrap();
            assert!(kind != "if" || nodes_at(last).iter().any(|n| n.kind == "else"), "{function}: the `if` ends without an `else`");
        }
    }
    // each copy the lowering made is where the facts say, in the Y source and in the bare body
    for c in facts["copies"].as_array().unwrap() {
        let what = c["what"].as_str().unwrap();
        let named = nodes_at(c["y"].as_str().unwrap()).into_iter().any(|n| matches!(n.kind, "name" | "member" | "index") && y_paths::expression_text(n) == what);
        assert!(named, "copy of {what}: not at {}", c["y"]);
        assert!(at(c["bare"].as_str().unwrap()).contains("// y: a copy"), "copy of {what}: not marked at {}", c["bare"]);
    }
    // each claim outside a region (`console.read_line() forever because "…"`) is where the facts say: its Y line gives
    // the reason, and so does the `// y:` marker of its bare line; no two claims share a line
    let mut claimed = BTreeSet::new();
    for c in facts["claims"].as_array().map_or(&[][..], |v| v) {
        let because = format!("because \"{}\"", c["because"].as_str().unwrap());
        let (y, bare) = (c["y"].as_str().unwrap(), c["bare"].as_str().unwrap());
        // `e forever because "…"`, or a region's constraint with that reason
        let said = nodes_at(y).into_iter().any(|n| matches!(n.kind, "forever" | "constraint") && n.str("because") == c["because"].as_str());
        assert!(said, "claim {because}: not at {y}");
        assert!(at(bare).split_once("// y:").is_some_and(|(_, m)| m.contains(&because)), "claim {because}: not marked at {bare}");
        assert!(claimed.insert(y.to_string()), "two claims at {y}");
        assert!(!at(bare).contains("// y: region "), "claim {because} at {bare} is a region's: it is a clause of the region subject");
    }
    // each point where the Y source may suspend is where the facts say, and the bare body awaits it there (Y § 3.7)
    for s in facts["suspensions"].as_array().unwrap() {
        let point = s["point"].as_str().unwrap();
        // a call of the function the point names, after a path or not; `f()` is such a call with no arguments
        let (callee, rest) = point.split_once('(').unwrap();
        let calls = nodes_at(s["y"].as_str().unwrap()).into_iter().any(|n| {
            let c = (n.kind == "call").then(|| y_paths::expression_text(n.node("callee").unwrap()));
            c.is_some_and(|c| c == callee || c.ends_with(&format!(".{callee}"))) && (rest != ")" || n.nodes("args").is_empty())
        });
        assert!(calls, "suspension at {point}: not at {}", s["y"]);
        assert!(at(s["bare"].as_str().unwrap()).contains(".await"), "suspension at {point}: not awaited at {}", s["bare"]);
    }
    // a region's constraints, its deadline and its `forever` among them, are its subject's in the constraint files
    // (language 6.23), which weave holds its code to; the facts do not repeat them
    assert!(facts.get("deadlines").is_none(), "a region's deadline is a `within` clause of its region subject");
    // and every region of the Y source has its subject: `region r is …:` in function `f` is `…::f#r`
    let regions: BTreeSet<String> = modules(&dir)
        .iter()
        .flat_map(|m| {
            let js: Value = serde_json::from_str(&read(&dir.join(format!("bare/src/{m}.constraints.json")))).unwrap();
            js["subjects"].as_array().unwrap().iter().filter(|s| s["kind"] == "region").map(|s| s["name"].as_str().unwrap().to_string()).collect::<Vec<_>>()
        })
        .collect();
    // a region that Y statements follow in its block ends where the bare body says, `// y: end region r`, in its function
    let bare_text: Vec<String> = facts["modules"].as_array().unwrap().iter().filter_map(|m| m["bare"].as_str()).map(|b| read(&dir.join(b))).collect();
    let ends_in = |function: &str, r: &str| -> bool {
        bare_text.iter().any(|b| {
            let lines: Vec<&str> = b.lines().collect();
            lines.iter().enumerate().filter(|(_, l)| l.contains(&format!("fn {function}("))).any(|(start, _)| {
                lines[start + 1..]
                    .iter()
                    .take_while(|l| !l.trim_start().trim_start_matches("pub ").trim_start_matches("async ").starts_with("fn "))
                    .any(|l| l.trim() == format!("// y: end region {r}"))
            })
        })
    };
    // the Y source's trees (`tools/y-parse`): each region of a function or a rule, in the block of statements it is in,
    // and the decisions each function names
    let mut in_source = BTreeSet::new();
    for y in &code {
        let tree = y_parse::parse_file(&dir.join(y)).unwrap_or_else(|e| panic!("{e}"));
        for f in tree.find("function").into_iter().chain(tree.find("rule")) {
            let function = f.str("name").unwrap();
            f.walk(&mut |n: &y_parse::Node| {
                // a block of statements: each region in it has its subject, and when statements follow it, the bare
                // body marks its end; of `region a is …:` and `orelse region b is …:`, statements follow only `b`
                let block = n.nodes("body");
                for (k, s) in block.iter().enumerate().filter(|(_, s)| s.kind == "region") {
                    let alternatives: Vec<&y_parse::Node> = std::iter::once(*s).chain(s.nodes("orelse")).collect();
                    for (j, a) in alternatives.iter().enumerate() {
                        let r = a.str("name").unwrap();
                        assert!(regions.iter().any(|x| x.ends_with(&format!("::{function}#{r}"))), "{y}:{}: the region {function}#{r} has no region subject", a.line);
                        let followed = j + 1 == alternatives.len() && k + 1 < block.len();
                        assert!(!followed || ends_in(function, r), "{y}:{}: Y statements follow the region {function}#{r} in its block, and the bare body does not mark its end, `// y: end region {r}`", a.line);
                    }
                }
                // every named decision is listed: `if name = …:`, `while name = …:`, `for name in …:`, `condition
                // label.name = …:`, `else name:` and `else name`; and the alternatives of an `orelse` are its outcomes
                let mut named = |x: &y_parse::Node| {
                    in_source.insert(format!("{function}#{}", x.str("name").unwrap()));
                };
                match n.kind {
                    "if" | "while" | "for" => named(n),
                    "conditional" | "compare" => n.nodes("conditions").into_iter().for_each(&mut named),
                    "region" if !n.nodes("orelse").is_empty() => std::iter::once(n).chain(n.nodes("orelse")).for_each(&mut named),
                    _ => {}
                }
                if matches!(n.kind, "if" | "while" | "for" | "conditional" | "compare") {
                    n.node("else").into_iter().for_each(&mut named);
                }
            });
        }
    }
    assert_eq!(listed, in_source, "y-facts.json and the Y source name different decisions");
    // the layout rules of Y § 3.1, on the trees
    for y in &all {
        let errors = layout(y, &y_parse::parse_file(&dir.join(y)).unwrap_or_else(|e| panic!("{e}")));
        assert!(errors.is_empty(), "{errors:#?}");
    }
    // the paths of the module are the ones the facts list, with how each ends; every loop in every Y file that may never
    // run has its `else`, and none that is sure to run has one (Y § 3.5)
    for y in &all {
        let (_, problems) = y_paths::all_paths(&read(&dir.join(y)));
        assert!(problems.is_empty(), "{y}: {problems:#?}");
    }
    // the module the tests belong to: its paths are the ones they must cover
    let tested = facts["modules"].as_array().unwrap().iter().find(|m| m["y"].as_str().unwrap().ends_with(".test.y")).unwrap()["module"].as_str().unwrap().to_string();
    let tested_y = facts["modules"].as_array().unwrap().iter().find(|m| m["module"] == tested.as_str() && !m["y"].as_str().unwrap().ends_with(".test.y")).unwrap()["y"].as_str().unwrap();
    let (paths, _) = y_paths::all_paths(&read(&dir.join(tested_y)));
    let ends = |kind: &str| match kind {
        "next" | "return" | "continue" | "break" => "returns",
        "fail" => "fails",
        other => other,
    }
    .to_string();
    let found: BTreeMap<String, String> = paths.iter().map(|(p, k)| (p.clone(), ends(k))).collect();
    let in_facts: BTreeMap<String, String> = facts["paths"].as_array().unwrap().iter().map(|p| (p["path"].as_str().unwrap().to_string(), p["ends"].as_str().unwrap().to_string())).collect();
    assert_eq!(in_facts, found, "y-facts.json and the Y source have different paths");
    // a function has fewer than 100 paths, the lint `too_many_paths` (Y § 3.14); only its own paths count
    let mut per_function: BTreeMap<&str, usize> = BTreeMap::new();
    for p in paths.keys() {
        *per_function.entry(p.split(" > ").next().unwrap()).or_default() += 1;
    }
    for (function, n) in &per_function {
        assert!(*n < 100, "{tested_y}: `{function}` has {n} paths, 100 or more (too_many_paths)");
    }
    // the tests cover every path but the ones `fail not_reachable` and `fail not_implemented` end (Y § 3.14)
    let required: BTreeSet<String> = paths.iter().filter(|(_, k)| !y_paths::EXEMPT.contains(&k.as_str())).map(|(p, _)| p.clone()).collect();
    let covered: BTreeSet<String> = facts["tests"].as_object().unwrap().values().flat_map(|t| t["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string())).collect();
    assert_eq!(covered, required, "the paths the tests cover and the paths of the module differ");
    // the lint rules of Y § 3.1, § 3.2 and § 3.5 the examples keep, on the trees; an `else if` ladder and an `else`
    // without a name are refused by the parser
    for y in &all {
        let errors = lints(y, &trees[y], code.contains(y));
        assert!(errors.is_empty(), "{errors:#?}");
    }
    // each test covers what its Y source declares and its Rust comment repeats; a test that covers no path of its own
    // (it checks what the paths do not tell apart, such as which failure a call gives) says nothing of covering
    let test_module = facts["modules"].as_array().unwrap().iter().find(|m| m["y"].as_str().unwrap().ends_with(".test.y")).unwrap();
    let rust_tests = read(&dir.join(test_module["rust"].as_str().unwrap()));
    let test_tree = y_parse::parse_file(&dir.join(test_module["y"].as_str().unwrap())).unwrap_or_else(|e| panic!("{e}"));
    for (test, t) in facts["tests"].as_object().unwrap() {
        let covers: Vec<&str> = t["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
        // the test is at the place the facts give, and covers there what they say
        let declared = nodes_at(t["y"].as_str().unwrap()).into_iter().find(|n| n.kind == "test" && n.str("name") == Some(test.as_str()));
        let declared = declared.unwrap_or_else(|| panic!("{test}: not at {}", t["y"]));
        let in_y: Vec<&str> = declared.node("covers").map(|c| c.str("text").unwrap()).into_iter().collect();
        assert_eq!(in_y, covers, "{test}: the Y source and y-facts.json cover different paths");
        let body = rust_tests.split(&format!("fn {test}() {{")).nth(1).unwrap_or_else(|| panic!("{test}: no Rust test"));
        let comment: String = body.lines().skip(1).take_while(|l| l.trim_start().starts_with("//")).map(|l| l.trim_start().trim_start_matches("//").trim()).collect::<Vec<_>>().join(" ");
        let rust_covers: Vec<&str> = comment.strip_prefix("y: covers").map_or(vec![], |c| c.split(',').map(str::trim).collect());
        assert_eq!(rust_covers, covers, "{test}: the Rust comment and y-facts.json cover different paths");
        assert!(!covers.iter().any(|c| c.ends_with(".else")), "{test}: an outcome is named, never `.else`");
        // a test covers at most one path, and its `when` is the one call of that path's function that takes it (Y § 3.14)
        assert!(covers.len() <= 1, "{test}: covers {} paths, not one", covers.len());
        if let Some(path) = covers.first() {
            let function = path.split(" > ").next().unwrap();
            let when = when_of(&test_tree, test);
            let lines: Vec<usize> = when.iter().map(|w| w.line).collect();
            assert_eq!(when.len(), 1, "{test}: `when` holds {} statements, not the one call, at lines {lines:?}", when.len());
            // a call of the function by its name, and none of a method of that name
            let callees: Vec<&y_parse::Node> = when[0].find("call").into_iter().map(|c| c.node("callee").unwrap()).collect();
            let named = |kind: &str| callees.iter().any(|c| c.kind == kind && c.str("name") == Some(function));
            assert!(named("name") && !named("member"), "{test}: covers a path of `{function}`, but `when`, at line {}, does not call it", when[0].line);
        }
    }
}

/// The layout rules of Y § 3.1 that the examples keep, read from a file's tree (the blank lines and comments it records):
///
/// - two blank lines before every declaration after the imports, at module level and in a facet, never more, when any
///   blank line comes before it; and so before its documentation block, and before a comment at that level;
/// - a blank line before every flow-control statement that is not the first of its block (`if`, `while`, `for`,
///   `conditional`, `region`, `arena`, `inject`, `break`, `continue`, `retry`); comments about the statement may come
///   between;
/// - a blank line before every comment line, unless the line above is a comment too or opens a block; the lines of a
///   documentation block are not comments in this sense.
fn layout(y: &str, tree: &y_parse::Node) -> Vec<String> {
    let mut errors = vec![];
    let decls = tree.nodes("decls");
    let module = decls.iter().find(|d| d.kind == "module").map(|d| d.line);
    // a piece of the file that the first rule judges: what is above it is blank, or the `module` line, or more
    let mut judge = |line: usize, blank: usize| {
        let above = line - blank - 1;
        if blank > 0 && above != 0 && Some(above) != module && blank != 2 {
            errors.push(format!("{y}:{line}: {blank} blank lines before a declaration, not 2"));
        }
    };
    let top: Vec<&y_parse::Node> = decls.iter().flat_map(|d| if d.kind == "facet" { [vec![*d], d.nodes("decls")].concat() } else { vec![*d] }).collect();
    for d in top.iter().filter(|d| d.kind != "module" && d.kind != "import") {
        if let Some(doc) = d.node("doc") {
            judge(doc.line, doc.num("blank").unwrap());
        }
        judge(d.line, d.num("blank").unwrap());
    }
    // a comment at the level of the declarations: at the left edge, or indented once in a facet
    let edge: Vec<(usize, bool)> = decls
        .iter()
        .map(|d| (d.line, d.kind == "facet"))
        .chain(tree.nodes("comments").iter().filter(|c| c.flag("alone") && c.num("indent") == Some(0)).map(|c| (c.line, false)))
        .collect();
    let in_facet = |line: usize| edge.iter().filter(|(l, _)| *l <= line).max_by_key(|(l, _)| *l).is_some_and(|(_, facet)| *facet);
    for c in tree.nodes("comments").into_iter().filter(|c| c.flag("alone")) {
        let indent = c.num("indent").unwrap();
        if indent == 0 || (indent == 4 && in_facet(c.line)) {
            judge(c.line, c.num("blank").unwrap());
        }
    }
    // a flow-control statement that is not the first of its block
    let flow = ["if", "while", "for", "conditional", "region", "arena", "inject", "break", "continue", "retry"];
    tree.walk(&mut |n: &y_parse::Node| {
        for st in n.nodes("body").iter().skip(1) {
            if flow.contains(&st.kind) && st.num("gap") == Some(0) {
                errors.push(format!("{y}:{}: no blank line before the `{}`", st.line, st.kind));
            }
        }
    });
    // a comment, after a line that is not blank, not a comment and opens no block
    for c in tree.nodes("comments").into_iter().filter(|c| c.flag("alone") && c.line > 1) {
        if c.num("blank") == Some(0) && !matches!(c.str("after"), Some("comment" | "head")) {
            errors.push(format!("{y}:{}: no blank line before the comment `-- {}`", c.line, c.str("text").unwrap()));
        }
    }
    errors.sort_by_key(|e| e.split(':').nth(1).and_then(|l| l.parse::<usize>().ok()));
    errors
}

/// The lint rules the examples keep, read from a file's tree:
///
/// - in the documentation of a declaration of a module (`code`), the entries of one kind sit together (a parameter, the
///   result, a capability, a `fails`, or a role of another name), and one blank line separates two kinds (Y § 3.2);
/// - no brackets around a single element: a name, a literal, a field path, an index, a call, a list or a negative number
///   (Y § 3.1);
/// - a name is changed, by `:=` or as a `mutable` argument, only when it was declared `mutable` (Y § 3.1, § 3.6); a
///   migration's body gives each new field its value with `:=` (Y § 3.12), which changes no name.
fn lints(y: &str, tree: &y_parse::Node, code: bool) -> Vec<String> {
    let mut errors = vec![];
    if code {
        tree.walk(&mut |owner: &y_parse::Node| {
            let Some(doc) = owner.node("doc") else { return };
            let params = owner.nodes("params");
            let named = |injected: bool, name: &str| params.iter().any(|p| p.flag("injected") == injected && p.str("name") == Some(name));
            let lines: Vec<&str> = doc.str("text").unwrap().split('\n').collect();
            let blank = |line: usize| lines.get(line - doc.line - 1).is_some_and(|l| l.trim().is_empty());
            let mut previous: Option<(String, usize)> = None;
            for e in doc.nodes("entries") {
                let name = e.str("name").unwrap();
                let kind = if name.starts_with("fails ") {
                    Some("fails".to_string())
                } else if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    Some(if named(false, name) { "parameter".into() } else if owner.str("returns") == Some(name) { "result".into() } else if named(true, name) { "capability".into() } else { format!("role {name}") })
                } else {
                    None
                };
                // prose between two entries, or an entry of no kind, starts again
                if let (Some((before, end)), Some(this)) = (&previous, &kind) {
                    if (end + 1..e.line).all(blank) {
                        let (blanks, want) = (e.line - end - 1, usize::from(before != this));
                        if blanks != want {
                            errors.push(format!("{y}:{}: {blanks} blank lines between a {before} and a {this}, not {want}", e.line));
                        }
                    }
                }
                previous = kind.map(|k| (k, e.num("end").unwrap()));
            }
        });
    }
    // no brackets around a single element
    tree.walk(&mut |n: &y_parse::Node| {
        let single = matches!(n.kind, "name" | "literal" | "member" | "index" | "call" | "list") || (n.kind == "unary" && n.str("op") == Some("-"));
        if n.flag("parens") && single {
            errors.push(format!("{y}:{}: brackets around a single element, a {}", n.line, n.kind));
        }
    });
    // a name changed only when it was declared `mutable`, in each function and test
    for f in tree.find("function").into_iter().chain(tree.find("test")) {
        let mut declared: BTreeSet<String> = f.nodes("params").iter().filter(|p| p.flag("mutable")).map(|p| p.str("name").unwrap().to_string()).collect();
        let root = |e: &y_parse::Node| {
            let mut e = e;
            while e.kind == "member" || e.kind == "index" {
                e = e.node("object").unwrap();
            }
            (e.kind == "name").then(|| e.str("name").unwrap().to_string())
        };
        f.walk(&mut |n: &y_parse::Node| {
            let changed = match n.kind {
                "let" if n.flag("mutable") => {
                    declared.insert(n.str("name").unwrap().to_string());
                    None
                }
                "assign" => root(n.node("target").unwrap()),
                "mutable" => root(n.node("value").unwrap()),
                _ => None,
            };
            if let Some(name) = changed.filter(|c| !declared.contains(c)) {
                errors.push(format!("{y}:{}: `{name}` changes but is not declared `mutable`", n.line));
            }
        });
    }
    errors.sort_by_key(|e| e.split(':').nth(1).and_then(|l| l.parse::<usize>().ok()));
    errors
}

/// The statements of a test's `when` section, from the test file's tree.
fn when_of<'a>(tree: &'a y_parse::Node, test: &str) -> Vec<&'a y_parse::Node> {
    let t = tree.nodes("decls").into_iter().find(|d| d.kind == "test" && d.str("name") == Some(test)).unwrap_or_else(|| panic!("{test}: no such test"));
    let when = t.nodes("sections").into_iter().find(|x| x.str("name") == Some("when")).unwrap_or_else(|| panic!("{test}: no `when`"));
    when.nodes("body")
}

/// A function of the path rules' own: a loop that may never run, a comparison of numbers on each side, and a `compare`
/// whose `else` covers two orders.
const FIRST_ABOVE: &str = "\
----------------------------------------
values: The values, as list<element_type=int32>.

limit: The limit, as int32.

found: The value found, as int32.
----------------------------------------
function first_above(values, limit) returns found:
    for value in values:
        if above = (value > limit):
            return found = value
        else not_above:
            continue
    else no_values

    compare sign = limit with 0:
        condition sign.negative = less:
            fail not_reachable
        else sign.rest:
            return found = limit
";

#[test]
fn paths_name_the_outcomes_hidden_in_a_comparison_and_a_loop() {
    let (paths, problems) = y_paths::all_paths(FIRST_ABOVE);
    assert!(problems.is_empty(), "{problems:#?}");
    let id = |p: &str| format!("first_above > {p}");
    // a pass that returns is named for how often the loop ran; `above` is the only order on its branch, so not split;
    // `else sign.rest` stands for two orders, and names each; `not_above` is split by the order of value and limit
    for p in ["value > above", "value'many > above", "no_values > sign.rest'eq", "no_values > sign.rest'gt", "value > not_above'lt > sign.rest'gt", "value'many > not_above'eq > sign.rest'eq"] {
        assert_eq!(paths.get(&id(p)).map(String::as_str), Some("return"), "{p}: {paths:#?}");
    }
    // a path that ends in `fail not_reachable` needs no test
    assert_eq!(paths.get(&id("no_values > sign.negative")).map(String::as_str), Some("not_reachable"));
    assert_eq!(paths.len(), 17, "{paths:#?}");
    assert_eq!(paths.values().filter(|k| y_paths::EXEMPT.contains(&k.as_str())).count(), 5);
}

#[test]
fn a_loop_that_may_never_run_needs_its_else_and_one_sure_to_run_has_none() {
    let (_, problems) = y_paths::all_paths(&FIRST_ABOVE.replace("    else no_values\n", ""));
    assert_eq!(problems["first_above"], ["`for value in values` may never run, and has no named else"]);
    let sure = "function counted():\n    for attempt in [1, 2, 3]:\n        log(attempt)\n    else none\n";
    let (paths, problems) = y_paths::all_paths(sure);
    assert_eq!(problems["counted"], ["`for attempt in [1, 2, 3]` runs for sure, and needs no else"]);
    // three elements: the loop runs more than once, and only so
    assert_eq!(paths.keys().collect::<Vec<_>>(), ["counted > attempt'many"]);
}
