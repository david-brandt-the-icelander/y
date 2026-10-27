//! The examples under the repository's `examples/` start from Y source (`y/src/*.y`). Their bare bodies and constraint
//! files are what the Y compiler would lower that source to; no Y compiler exists yet, so they are written by hand.
//! `weave` with cargo made their Rust (`src/`). This test keeps the four in step without cargo:
//!
//! - weaving the committed bare files gives the committed Rust, apart from the `mut` that weave derives through rustc;
//! - stripping the committed Rust gives the committed bare bodies byte for byte, and the same subjects and holes: what
//!   the Y lowering adds beyond strip's output is only clauses and the file's status;
//! - the constraint files are valid, and their clauses are well formed;
//! - `bare/y-facts.json` matches the Y source and the bare bodies: each named decision is in both, where it says, and
//!   each test covers what its Y source and its Rust comment say.
//!
//! To refresh the Rust after editing the bare files, from `tools/bare-rust`:
//! `cargo run -q -- weave ../../examples/<name>/bare --out ../../examples/.woven --crate ../../examples/<name>`, then
//! copy `.woven/src` over `examples/<name>/src` and delete `.woven`.
use bare_rust::json::canonical;
use bare_rust::{normalise_nomut, rustfmt, strip, weave};
use serde_json::Value;
use std::collections::BTreeSet;
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
    let dir = example("hangman");
    let mods = modules(&dir);
    assert_eq!(mods, ["hangman", "hangman/tests", "lib", "main"]);
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
    let dir = example("hangman");
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
    // the clauses are really read: a name the module does not have is refused, and so is a function that is not pure
    let js: Value = serde_json::from_str(&read(&dir.join("bare/src/hangman.constraints.json"))).unwrap();
    for (text, want) in [("require has_words : size(wordz) > 0", "unknown name `wordz`"), ("require has_words : guess(words, words) is Hit", "unknown name `guess`")] {
        let mut bad = js.clone();
        let play = bad["subjects"].as_array_mut().unwrap().iter_mut().find(|s| s["name"] == "hangman_example::hangman::play").unwrap();
        play["clauses"][0]["text"] = Value::from(text);
        let errors = bare_rust::libcheck::validate(&bad, None);
        assert!(errors.iter().any(|e| e.contains(": clause 0:") && e.contains(want)), "{text}: {errors:#?}");
    }
}

#[test]
fn hangman_facts_match_the_y_source_and_the_bare_bodies() {
    let dir = example("hangman");
    let facts: Value = serde_json::from_str(&read(&dir.join("bare/y-facts.json"))).unwrap();
    // `file:line` to the text of that line
    let at = |place: &str| -> String {
        let (file, line) = place.rsplit_once(':').unwrap();
        let n: usize = line.parse().unwrap();
        read(&dir.join(file)).lines().nth(n - 1).unwrap_or_else(|| panic!("{place}: no such line")).to_string()
    };
    let mut listed = BTreeSet::new();
    for (function, decisions) in facts["decisions"].as_object().unwrap() {
        for d in decisions.as_array().unwrap() {
            // every outcome of a decision is a named branch (Y-proposal § 3.5): `if a = (…):`, `else if b = (…):`,
            // `else c:`, each on a line of its own, and an `if` or a `while` always has its `else`
            let branches = d["branches"].as_array().unwrap();
            assert!(branches.len() >= 2, "{function}: a decision with one named outcome: {branches:?}");
            for (k, b) in branches.iter().enumerate() {
                let name = b["name"].as_str().unwrap();
                let y = at(b["y"].as_str().unwrap());
                let opens = if k == 0 { y.contains(&format!(" {name} = (")) && y.trim_end().ends_with("):") } else { y.contains(&format!("else if {name} = (")) || y.trim() == format!("else {name}:") };
                assert!(opens, "{function}#{name}: not at {}: `{y}`", b["y"]);
                assert!(at(b["bare"].as_str().unwrap()).contains(&format!("#{name}")), "{function}#{name}: not at {}", b["bare"]);
                listed.insert(format!("{}#{name}", function.rsplit('.').next().unwrap()));
            }
            let last = branches.last().unwrap()["y"].as_str().unwrap();
            assert!(at(last).trim().starts_with("else ") && !at(last).contains(" if "), "{function}: the decision ends without an `else`");
        }
    }
    // each copy the lowering made is where the facts say, in the Y source and in the bare body
    for c in facts["copies"].as_array().unwrap() {
        let what = c["what"].as_str().unwrap();
        assert!(at(c["y"].as_str().unwrap()).contains(what), "copy of {what}: not at {}", c["y"]);
        assert!(at(c["bare"].as_str().unwrap()).contains("// y: a copy"), "copy of {what}: not marked at {}", c["bare"]);
    }
    // every named decision of the Y source is listed: `if name = (` and `while name = (`
    let mut in_source = BTreeSet::new();
    for y in ["y/src/hangman.y", "y/src/main.y"] {
        let mut function = String::new();
        for line in read(&dir.join(y)).lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if let Some(i) = words.iter().position(|w| *w == "function") {
                function = words[i + 1].split('(').next().unwrap().to_string();
            }
            for (i, w) in words.iter().enumerate() {
                if (*w == "if" || *w == "while") && words.get(i + 2) == Some(&"=") && words.get(i + 3).is_some_and(|x| x.starts_with('(')) {
                    in_source.insert(format!("{function}#{}", words[i + 1]));
                }
                // `else name:`
                if *w == "else" && words.get(i + 1).is_some_and(|x| x.ends_with(':') && *x != "if") {
                    in_source.insert(format!("{function}#{}", words[i + 1].trim_end_matches(':')));
                }
            }
        }
    }
    assert_eq!(listed, in_source, "y-facts.json and the Y source name different decisions");
    // two blank lines before every top-level declaration after the imports, never more (a lint rule of Y § 3.1)
    for y in ["y/src/hangman.y", "y/src/main.y", "y/src/hangman.test.y"] {
        let lines: Vec<String> = read(&dir.join(y)).lines().map(str::to_string).collect();
        let mut inside = false;
        for (i, l) in lines.iter().enumerate() {
            let rule = l.trim().len() >= 3 && l.trim().chars().all(|c| c == '-');
            let starts = !inside && !l.is_empty() && !l.starts_with(' ') && i > 0 && lines[i - 1].is_empty();
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
    let flow = ["if ", "while ", "for ", "switch ", "break", "continue", "retry", "atomic ", "within ", "arena ", "uncancellable", "supervise"];
    for y in ["y/src/hangman.y", "y/src/main.y", "y/src/hangman.test.y"] {
        let lines: Vec<String> = read(&dir.join(y)).lines().map(str::to_string).collect();
        let mut inside = false;
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim();
            if t.len() >= 3 && t.chars().all(|c| c == '-') {
                inside = !inside;
            }
            let with_block = t.starts_with("with ") && t.contains('=');
            if inside || !(flow.iter().any(|f| t.starts_with(f)) || with_block) || t.ends_with("covers") {
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
    // no `else` goes without a name
    for y in ["y/src/hangman.y", "y/src/main.y", "y/src/hangman.test.y"] {
        assert!(!read(&dir.join(y)).lines().any(|l| l.trim() == "else:"), "{y}: an `else` without a name");
    }
    // each test covers what its Y source declares and its Rust comment repeats
    let rust_tests = read(&dir.join("src/hangman/tests.rs"));
    for (test, t) in facts["tests"].as_object().unwrap() {
        let covers: Vec<&str> = t["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
        let y = at(t["y"].as_str().unwrap());
        assert_eq!(y, format!("test {test}"), "{test}: not at {}", t["y"]);
        let body = rust_tests.split(&format!("fn {test}() {{")).nth(1).unwrap_or_else(|| panic!("{test}: no Rust test"));
        let comment: String = body.lines().skip(1).take_while(|l| l.trim_start().starts_with("//")).map(|l| l.trim_start().trim_start_matches("//").trim()).collect::<Vec<_>>().join(" ");
        let rust_covers: Vec<&str> = comment.trim_start_matches("y: covers").split(',').map(str::trim).collect();
        assert_eq!(rust_covers, covers, "{test}: the Rust comment and y-facts.json cover different outcomes");
        assert!(!covers.iter().any(|c| c.ends_with(".else")), "{test}: an outcome is named, never `.else`");
    }
}
