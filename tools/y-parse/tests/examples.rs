//! Every Y file of every example under `examples/` parses, into a tree whose lines are the source's.
use std::fs;
use std::path::PathBuf;
use y_parse::{parse_file, Node};

const EXAMPLES: [&str; 7] = ["hangman", "nim", "tic-tac-toe", "snake", "memory", "checkers", "warehouse"];

fn files() -> Vec<PathBuf> {
    let mut out = vec![];
    for e in EXAMPLES {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(e).join("y/src");
        let mut ys: Vec<PathBuf> = fs::read_dir(&dir).unwrap().map(|f| f.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "y")).collect();
        ys.sort();
        assert_eq!(ys.len(), 3, "{}: a module, its tests and `main`", dir.display());
        out.extend(ys);
    }
    out
}

#[test]
fn every_example_file_parses() {
    let mut errors = vec![];
    for f in files() {
        if let Err(e) = parse_file(&f) {
            errors.push(e);
        }
    }
    assert!(errors.is_empty(), "{errors:#?}");
}

/// Each named node starts on a line of the source that has its name, and each `end` is at or after its `line`.
#[test]
fn every_node_is_on_a_line_that_names_it() {
    for f in files() {
        let src = fs::read_to_string(&f).unwrap();
        let lines: Vec<&str> = src.lines().collect();
        let tree = parse_file(&f).unwrap();
        tree.walk(&mut |n: &Node| {
            let at = lines[n.line - 1];
            if let Some(name) = n.str("name").filter(|_| !matches!(n.kind, "doc_entry")) {
                let last = name.rsplit('.').next().unwrap();
                assert!(at.contains(last), "{}:{}: a {} named {name} on `{at}`", f.display(), n.line, n.kind);
            }
            if let Some(end) = n.num("end") {
                assert!(end >= n.line && end <= lines.len(), "{}:{}: a {} that ends at {end}", f.display(), n.line, n.kind);
            }
        });
    }
}

/// Each `main` file is a module named `main` with a function `main`, each test file has tests that end in `then:`, most
/// of them saying the paths they cover (a test of what no path of the module decides covers none), and each module
/// declares its facets.
#[test]
fn each_kind_of_file_has_its_parts() {
    for f in files() {
        let tree = parse_file(&f).unwrap();
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let decls = tree.nodes("decls");
        if name == "main.y" {
            assert_eq!(decls[0].kind, "module");
            assert_eq!(decls[0].str("name"), Some("main"));
            assert!(decls.iter().any(|d| d.kind == "function" && d.str("name") == Some("main")), "{name}");
        } else if name.ends_with(".test.y") {
            let tests = tree.find("test");
            assert!(tests.len() > 10, "{}: {} tests", f.display(), tests.len());
            assert!(tests.iter().all(|t| t.nodes("sections").last().is_some_and(|s| s.str("name") == Some("then"))), "{}", f.display());
            assert!(tests.iter().filter(|t| t.node("covers").is_none()).count() * 10 < tests.len(), "{}", f.display());
        } else {
            assert_eq!(decls[0].kind, "module");
            assert!(decls.iter().filter(|d| d.kind == "facet").count() >= 3, "{}", f.display());
        }
    }
}

/// Every `function` and `rule` head of the examples, found by its text, is a node; and so is every `region`.
#[test]
fn the_tree_has_every_head_the_text_has() {
    for f in files() {
        let src = fs::read_to_string(&f).unwrap();
        let tree = parse_file(&f).unwrap();
        for (kind, word) in [("function", "function "), ("rule", "rule "), ("region", "region ")] {
            let in_text = src.lines().filter(|l| l.trim_start().trim_start_matches("orelse ").starts_with(word)).count();
            assert_eq!(tree.find(kind).len(), in_text, "{}: {kind}", f.display());
        }
    }
}
