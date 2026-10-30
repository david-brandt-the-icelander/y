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
//! To refresh the Rust after editing the bare files, from `tools/bare-rust`:
//! `cargo run -q -- weave ../../examples/<name>/bare --out ../../examples/.woven --crate ../../examples/<name>`, then
//! copy `.woven/src` over `examples/<name>/src` and delete `.woven`.
mod y_paths;

use bare_rust::json::canonical;
use bare_rust::{normalise_nomut, rustfmt, strip, weave};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
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

/// The constraint file as strip writes it: no clauses, and the status strip gives.
fn without_lowering(js: &Value) -> Value {
    let mut js = js.clone();
    js["status"] = Value::from("lifted");
    for s in js["subjects"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("clauses");
    }
    js
}

#[test]
fn hangman_bare_files_weave_to_the_committed_rust_and_strip_back() {
    weaves_and_strips_back("hangman", &["hangman", "hangman/tests", "lib", "main"]);
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

fn weaves_and_strips_back(name: &str, expected: &[&str]) {
    let dir = example(name);
    let mods = modules(&dir);
    assert_eq!(mods, expected);
    for m in mods {
        let brs = read(&dir.join(format!("bare/src/{m}.brs")));
        let cjs = read(&dir.join(format!("bare/src/{m}.constraints.json")));
        let js: Value = serde_json::from_str(&cjs).unwrap();
        assert_eq!(canonical(&js), cjs, "{m}: the constraint file is not in canonical form (bare-rust libfmt)");
        let woven = weave(&brs, &js).unwrap_or_else(|e| panic!("{m}: weave: {e}"));
        let committed = read(&dir.join(format!("src/{m}.rs")));
        let norm = |s: &str| normalise_nomut(s).and_then(|n| rustfmt(&n));
        assert_eq!(norm(&woven).unwrap(), norm(&committed).unwrap(), "{m}: the committed Rust is not the weave of the bare files");
        let (b2, j2) = strip(&committed, js["crate"].as_str().unwrap(), js["module"].as_str().unwrap())
            .unwrap_or_else(|e| panic!("{m}: strip: {e}"));
        assert_eq!(b2, brs, "{m}: strip of the Rust differs from the bare body");
        assert_eq!(j2, without_lowering(&js), "{m}: strip of the Rust differs from the constraint file in more than clauses and status");
    }
}

#[test]
fn hangman_constraint_files_are_valid() {
    constraint_files_are_valid("hangman");
    let dir = example("hangman");
    // the clauses are really read: a name the module does not have is refused, and so is a function that is not pure
    let js: Value = serde_json::from_str(&read(&dir.join("bare/src/hangman.constraints.json"))).unwrap();
    for (text, want) in [("require has_words : size(wordz) > 0", "unknown name `wordz`"), ("require has_words : guess(words, words) is Hit", "unknown name `guess`")] {
        let mut bad = js.clone();
        let play = bad["subjects"].as_array_mut().unwrap().iter_mut().find(|s| s["name"] == "hangman_example::hangman::console_play::play").unwrap();
        play["clauses"][0]["text"] = Value::from(text);
        let errors = bare_rust::libcheck::validate(&bad, None);
        assert!(errors.iter().any(|e| e.contains(": clause 0:") && e.contains(want)), "{text}: {errors:#?}");
    }
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
}

#[test]
fn hangman_facts_match_the_y_source_and_the_bare_bodies() {
    facts_match_the_y_source_and_the_bare_bodies("hangman");
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
    // `file:line` to the text of that line
    let at = |place: &str| -> String {
        let (file, line) = place.rsplit_once(':').unwrap();
        let n: usize = line.parse().unwrap();
        read(&dir.join(file)).lines().nth(n - 1).unwrap_or_else(|| panic!("{place}: no such line")).to_string()
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
            if switch {
                let label = d["label"].as_str().unwrap();
                assert!(at(d["y"].as_str().unwrap()).trim_start().starts_with(&format!("{kind} {label}")), "{function}: no `{kind} {label}` at {}", d["y"]);
            }
            for (k, b) in branches.iter().enumerate() {
                let name = b["name"].as_str().unwrap();
                let y = at(b["y"].as_str().unwrap());
                let t = y.trim();
                let named_else = t == format!("else {name}:") || ((kind == "for" || kind == "while") && t == format!("else {name}"));
                let opens = if k > 0 {
                    named_else || (switch && t.starts_with(&format!("condition {name} = ")) && t.ends_with(':'))
                } else if switch {
                    t.starts_with(&format!("condition {name} = ")) && t.ends_with(':')
                } else if kind == "for" {
                    t.starts_with(&format!("for {name} in ")) && t.ends_with(':')
                } else {
                    t.starts_with(&format!("{kind} {name} = ")) && t.ends_with(':')
                };
                assert!(opens, "{function}#{name}: not at {}: `{y}`", b["y"]);
                assert!(at(b["bare"].as_str().unwrap()).contains(&format!("#{name}")), "{function}#{name}: not at {}", b["bare"]);
                listed.insert(format!("{}#{name}", function.rsplit('.').next().unwrap()));
            }
            let last = branches.last().unwrap()["y"].as_str().unwrap();
            assert!(kind != "if" || at(last).trim().starts_with("else "), "{function}: the `if` ends without an `else`");
        }
    }
    // each copy the lowering made is where the facts say, in the Y source and in the bare body
    for c in facts["copies"].as_array().unwrap() {
        let what = c["what"].as_str().unwrap();
        assert!(at(c["y"].as_str().unwrap()).contains(what), "copy of {what}: not at {}", c["y"]);
        assert!(at(c["bare"].as_str().unwrap()).contains("// y: a copy"), "copy of {what}: not marked at {}", c["bare"]);
    }
    // each point where the Y source may suspend is where the facts say, and the bare body awaits it there (Y § 3.7)
    for s in facts["suspensions"].as_array().unwrap() {
        let point = s["point"].as_str().unwrap();
        assert!(at(s["y"].as_str().unwrap()).contains(point), "suspension at {point}: not at {}", s["y"]);
        assert!(at(s["bare"].as_str().unwrap()).contains(".await"), "suspension at {point}: not awaited at {}", s["bare"]);
    }
    // each `within` scope is where the facts say, and the bare body starts its deadline there and passes it to each
    // wait inside the scope (Y § 3.8); a file without one has no `deadlines` fact
    for d in facts["deadlines"].as_array().map_or(&[][..], |v| v) {
        let bound = d["bound"].as_str().unwrap();
        assert!(at(d["y"].as_str().unwrap()).trim() == format!("within {bound} or fail:"), "deadline {bound}: not at {}", d["y"]);
        assert!(at(d["bare"].as_str().unwrap()).contains("Deadline::after("), "deadline {bound}: not started at {}", d["bare"]);
        let (file, line) = d["bare"].as_str().unwrap().rsplit_once(':').unwrap();
        let rest: Vec<String> = read(&dir.join(file)).lines().skip(line.parse().unwrap()).map(str::to_string).collect();
        for wait in d["covers"].as_array().unwrap() {
            let wait = wait.as_str().unwrap();
            assert!(rest.iter().any(|l| l.contains(wait) && l.contains("&deadline")), "deadline {bound}: {wait} is not given it");
        }
    }
    // every named decision of the Y source is listed: `if name = …:`, `while name = …:`, `for name in …:`, `condition
    // label.name = …:`, `else name:` and `else name`
    let mut in_source = BTreeSet::new();
    for y in &code {
        let mut function = String::new();
        for line in read(&dir.join(y)).lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if let Some(i) = words.iter().position(|w| *w == "function") {
                function = words[i + 1].split('(').next().unwrap().to_string();
            }
            for (i, w) in words.iter().enumerate() {
                if (*w == "if" || *w == "while") && words.get(i + 2) == Some(&"=") && line.trim_end().ends_with(':') {
                    in_source.insert(format!("{function}#{}", words[i + 1]));
                }
                if i == 0 && *w == "for" && words.get(2) == Some(&"in") && line.trim_end().ends_with(':') {
                    in_source.insert(format!("{function}#{}", words[1]));
                }
                // `else name`, the loop's that may never run
                if i == 0 && *w == "else" && words.len() == 2 && !words[1].ends_with(':') {
                    in_source.insert(format!("{function}#{}", words[1]));
                }
                if i == 0 && *w == "condition" && words.get(2) == Some(&"=") {
                    in_source.insert(format!("{function}#{}", words[1]));
                }
                // `else name:`
                if *w == "else" && words.get(i + 1).is_some_and(|x| x.ends_with(':') && *x != "if") {
                    in_source.insert(format!("{function}#{}", words[i + 1].trim_end_matches(':')));
                }
            }
        }
    }
    assert_eq!(listed, in_source, "y-facts.json and the Y source name different decisions");
    // two blank lines before every declaration after the imports, at module level and in a facet, never more (a lint
    // rule of Y § 3.1)
    for y in &all {
        let lines: Vec<String> = read(&dir.join(y)).lines().map(str::to_string).collect();
        let mut inside = false;
        let mut in_facet = false;
        for (i, l) in lines.iter().enumerate() {
            let rule = l.trim().len() >= 3 && l.trim().chars().all(|c| c == '-');
            if !l.is_empty() && !l.starts_with(' ') && !rule && !inside {
                in_facet = l.starts_with("public facet ") || l.starts_with("facet ");
            }
            let level = !l.starts_with(' ') || (in_facet && l.starts_with("    ") && !l.starts_with("     "));
            let starts = !inside && !l.is_empty() && level && i > 0 && lines[i - 1].is_empty();
            if rule {
                inside = !inside;
            }
            if starts && !l.starts_with("from ") && !lines[..i].iter().all(|x| x.is_empty() || x.starts_with("module ")) {
                let blanks = lines[..i].iter().rev().take_while(|x| x.is_empty()).count();
                assert_eq!(blanks, 2, "{y}:{}: {blanks} blank lines before a declaration, not 2", i + 1);
            }
        }
    }
    // a blank line before every flow-control statement that is not the first of its block; comments about the
    // statement may come between (a lint rule of Y § 3.1)
    let flow = ["if ", "while ", "for ", "conditional ", "break", "continue", "retry", "atomic ", "within ", "arena ", "uncancellable", "supervise"];
    for y in &all {
        let lines: Vec<String> = read(&dir.join(y)).lines().map(str::to_string).collect();
        let mut inside = false;
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim();
            if t.len() >= 3 && t.chars().all(|c| c == '-') {
                inside = !inside;
            }
            let inject_block = t.starts_with("inject(");
            if inside || !(flow.iter().any(|f| t.starts_with(f)) || inject_block) || t.ends_with("covers") {
                continue;
            }
            let mut k = i;
            while k > 0 && lines[k - 1].trim().starts_with("--") {
                k -= 1;
            }
            let above = lines[k - 1].trim_end();
            assert!(above.is_empty() || above.ends_with(':'), "{y}:{}: no blank line before `{t}`", i + 1);
        }
    }
    // a blank line comes before every comment line, unless the line above is a comment too or opens a block; the
    // lines of a documentation block are not comments in this sense (a lint rule of Y § 3.1)
    for y in &all {
        let lines: Vec<String> = read(&dir.join(y)).lines().map(str::to_string).collect();
        let is_rule = |t: &str| t.len() >= 3 && t.chars().all(|c| c == '-');
        let mut inside = false;
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim();
            if is_rule(t) {
                inside = !inside;
                continue;
            }
            if inside || !t.starts_with("--") || i == 0 {
                continue;
            }
            let above = lines[i - 1].trim();
            let allowed = above.is_empty() || (above.starts_with("--") && !is_rule(above)) || above.ends_with(':');
            assert!(allowed, "{y}:{}: no blank line before the comment `{t}`", i + 1);
        }
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
    // documentation roles of one kind sit together, and one blank line separates the kinds (a lint rule of Y § 3.2)
    for y in &code {
        let source = read(&dir.join(y));
        let lines: Vec<&str> = source.lines().collect();
        let is_rule = |l: &str| l.trim().len() >= 3 && l.trim().chars().all(|c| c == '-');
        let mut i = 0;
        while i < lines.len() {
            if !is_rule(lines[i]) {
                i += 1;
                continue;
            }
            let end = i + 1 + lines[i + 1..].iter().position(|l| is_rule(l)).unwrap();
            let indent = lines[i].len() - lines[i].trim_start().len();
            let header: Vec<&str> = lines[end + 1..].iter().copied().take_while(|l| !l.trim().is_empty()).scan(false, |done, l| {
                if *done { return None; }
                *done = l.trim_end().ends_with(':');
                Some(l)
            }).collect();
            let text = header.join(" ");
            let params: Vec<String> = text.split_once('(').and_then(|(_, r)| r.split_once(')')).map(|(p, _)| p.split(',').map(|x| x.trim().trim_start_matches("mutable ").to_string()).collect()).unwrap_or_default();
            let result = text.split_once(" returns ").map(|(_, r)| r.split_whitespace().next().unwrap().to_string());
            // the capabilities are the parameters marked `injected`
            let caps: Vec<String> = params.iter().filter_map(|p| p.strip_prefix("injected ").map(str::to_string)).collect();
            let kind = |l: &str| -> Option<String> {
                let t = l.trim();
                if t.starts_with("fails ") {
                    return Some("fails".into());
                }
                let name = t.split_once(':')?.0;
                if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    return None;
                }
                Some(if params.iter().any(|p| p == name) { "parameter".into() } else if result.as_deref() == Some(name) { "result".into() } else if caps.iter().any(|c| c == name) { "capability".into() } else { format!("role {name}") })
            };
            let (mut previous, mut blanks): (Option<String>, usize) = (None, 0);
            for (k, l) in lines[i + 1..end].iter().enumerate() {
                let at = i + 2 + k;
                if l.trim().is_empty() {
                    blanks += 1;
                } else if l.len() - l.trim_start().len() > indent {
                    blanks = 0;
                } else if let Some(this) = kind(l) {
                    if let Some(before) = &previous {
                        let want = usize::from(*before != this);
                        assert_eq!(blanks, want, "{y}:{at}: {blanks} blank lines between a {before} and a {this}, not {want}");
                    }
                    (previous, blanks) = (Some(this), 0);
                } else {
                    (previous, blanks) = (None, 0);
                }
            }
            i = end + 1;
        }
    }
    // no parentheses around a single element: a name, a literal, a field path, an index or a call (a lint rule of Y § 3.1)
    for y in &all {
        for (n, line) in read(&dir.join(y)).lines().enumerate() {
            if line.trim_start().starts_with("--") {
                continue;
            }
            let chars: Vec<char> = line.chars().collect();
            for (k, c) in chars.iter().enumerate() {
                let opens_group = *c == '(' && (k == 0 || !(chars[k - 1].is_alphanumeric() || "_)]>\"".contains(chars[k - 1])));
                if !opens_group {
                    continue;
                }
                let (mut depth, mut in_string, mut grouped) = (0, false, false);
                for d in &chars[k..] {
                    match d {
                        '"' => in_string = !in_string,
                        '(' | '[' if !in_string => depth += 1,
                        ')' | ']' if !in_string => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        ' ' | ',' if !in_string && depth == 1 => grouped = true,
                        _ => {}
                    }
                }
                assert!(grouped, "{y}:{}: parentheses around a single element: `{}`", n + 1, line.trim());
            }
        }
    }
    // a name is changed, by `:=` or as a `mutable` argument, only when it was declared `mutable` (Y § 3.1, § 3.6)
    for y in &all {
        let mut declared: BTreeSet<String> = BTreeSet::new();
        for (n, line) in read(&dir.join(y)).lines().enumerate() {
            let t = line.trim();
            if t.starts_with("function ") || t.starts_with("test ") {
                declared.clear();
                if let Some((_, params)) = t.split_once('(') {
                    for p in params.split(')').next().unwrap().split(',') {
                        if let Some(name) = p.trim().trim_start_matches("injected ").strip_prefix("mutable ") {
                            declared.insert(name.to_string());
                        }
                    }
                }
            }
            if let Some((name, _)) = t.strip_prefix("mutable ").and_then(|r| r.split_once(" = ")) {
                declared.insert(name.to_string());
            }
            let mut changed: Vec<String> = Vec::new();
            if let Some((target, _)) = t.split_once(" := ") {
                changed.push(target.split('.').next().unwrap().to_string());
            }
            for (k, _) in t.match_indices("mutable ") {
                if k > 0 && (t[..k].ends_with('(') || t[..k].ends_with(", ")) && !t.starts_with("function ") {
                    changed.push(t[k + 8..].split([',', ')']).next().unwrap().to_string());
                }
            }
            for name in changed {
                assert!(declared.contains(&name), "{y}:{}: `{name}` changes but is not declared `mutable`", n + 1);
            }
        }
    }
    // `if` chooses between two ways; a ladder of `else if`s is a lint error, written as a `conditional` (Y § 3.5)
    for y in &all {
        for (n, line) in read(&dir.join(y)).lines().enumerate() {
            assert!(!line.trim_start().starts_with("else if "), "{y}:{}: an `else if` ladder, not a `conditional`", n + 1);
        }
    }
    // no `else` goes without a name
    for y in &all {
        assert!(!read(&dir.join(y)).lines().any(|l| l.trim() == "else:"), "{y}: an `else` without a name");
    }
    // each test covers what its Y source declares and its Rust comment repeats; a test that covers no path of its own
    // (it checks what the paths do not tell apart, such as which failure a call gives) says nothing of covering
    let test_module = facts["modules"].as_array().unwrap().iter().find(|m| m["y"].as_str().unwrap().ends_with(".test.y")).unwrap();
    let rust_tests = read(&dir.join(test_module["rust"].as_str().unwrap()));
    for (test, t) in facts["tests"].as_object().unwrap() {
        let covers: Vec<&str> = t["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
        let y = at(t["y"].as_str().unwrap());
        let header = if covers.is_empty() { format!("test {test}:") } else { format!("test {test}") };
        assert_eq!(y, header, "{test}: not at {}", t["y"]);
        let body = rust_tests.split(&format!("fn {test}() {{")).nth(1).unwrap_or_else(|| panic!("{test}: no Rust test"));
        let comment: String = body.lines().skip(1).take_while(|l| l.trim_start().starts_with("//")).map(|l| l.trim_start().trim_start_matches("//").trim()).collect::<Vec<_>>().join(" ");
        let rust_covers: Vec<&str> = comment.strip_prefix("y: covers").map_or(vec![], |c| c.split(',').map(str::trim).collect());
        assert_eq!(rust_covers, covers, "{test}: the Rust comment and y-facts.json cover different paths");
        assert!(!covers.iter().any(|c| c.ends_with(".else")), "{test}: an outcome is named, never `.else`");
        // a test covers at most one path, and its `when` is the one call of that path's function that takes it (Y § 3.14)
        assert!(covers.len() <= 1, "{test}: covers {} paths, not one", covers.len());
        if let Some(path) = covers.first() {
            let function = path.split(" > ").next().unwrap();
            let when = when_of(&read(&dir.join(test_module["y"].as_str().unwrap())), test);
            assert_eq!(when.len(), 1, "{test}: `when` holds {} statements, not the one call: {when:?}", when.len());
            let called = when[0].contains(&format!("{function}(")) && !when[0].contains(&format!(".{function}("));
            assert!(called, "{test}: covers a path of `{function}`, but `when` does not call it: `{}`", when[0]);
        }
    }
}

/// The statements of a test's `when` section, each with the lines it continues onto.
fn when_of(source: &str, test: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let start = lines.iter().position(|l| l.starts_with(&format!("test {test}")) && l[5 + test.len()..].trim_start_matches(':').is_empty()).unwrap();
    let begin = start + lines[start..].iter().position(|l| l.trim() == "when:").unwrap_or_else(|| panic!("{test}: no `when`"));
    let body: Vec<&str> = lines[begin + 1..].iter().copied().take_while(|l| l.starts_with("        ") || l.trim().is_empty()).filter(|l| !l.trim().is_empty() && !l.trim().starts_with("--")).collect();
    let indent = body.first().map_or(0, |l| l.len() - l.trim_start().len());
    let mut out: Vec<String> = vec![];
    for l in body {
        if l.len() - l.trim_start().len() == indent {
            out.push(l.trim().to_string());
        } else {
            out.last_mut().unwrap().push_str(&format!(" {}", l.trim()));
        }
    }
    out
}

/// A function of the path rules' own: a loop that may never run, a comparison of numbers on each side, and a `compare`
/// whose `else` covers two orders.
const FIRST_ABOVE: &str = "\
----------------------------------------
values: The values, as list<int32>.

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
