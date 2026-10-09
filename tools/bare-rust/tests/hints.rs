//! Error messages that name the fix, and `scaffold`.
use bare_rust::corpus::strip_crate;
use bare_rust::crate_weave::{weave_crate_with, CrateWeave, WeaveOpts};
use bare_rust::json::canonical;
use bare_rust::scaffold::{self, SelfKind, Spec};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A one-file crate called `name`, stripped into `<top>/bare`. Returns (crate dir, bare dir, out dir).
fn krate(name: &str, lib: &str) -> (PathBuf, PathBuf, PathBuf) {
    let top = root().join("target/bare-rust-test").join(name);
    let _ = fs::remove_dir_all(&top);
    let dir = top.join("crate");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")).unwrap();
    fs::write(dir.join("src/lib.rs"), lib).unwrap();
    let bare = top.join("bare");
    let results = strip_crate(&dir, &bare, true).unwrap();
    assert!(results.iter().all(|r| r.outcome.is_ok()), "{:?}", results.iter().map(|r| r.outcome.clone()).collect::<Vec<_>>());
    (dir, bare, top.join("woven"))
}

fn weave(crate_dir: &Path, bare: &Path, out: &Path) -> CrateWeave {
    let o = WeaveOpts { cargo: true, crate_dir: Some(crate_dir.to_path_buf()), target: bare_rust::cargo::target_dir(&root()), drop_holes: false, keep: Default::default() };
    weave_crate_with(bare, out, &o).unwrap()
}

fn read_json(p: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

/// The text between two markers.
fn between<'a>(s: &'a str, a: &str, b: &str) -> &'a str {
    let i = s.find(a).unwrap_or_else(|| panic!("no `{a}` in {s}")) + a.len();
    let j = s[i..].find(b).unwrap_or_else(|| panic!("no `{b}` after `{a}` in {s}")) + i;
    &s[i..j]
}

#[test]
fn a_typed_local_names_the_holes_entry_and_the_fix_works() {
    let src = "pub fn f() -> usize {\n    let v: Vec<u8> = Vec::new();\n    v.len()\n}\n";
    let (dir, bare, out) = krate("hint_local", src);
    let brs = bare.join("src/lib.brs");
    // the mistake: a type on the local, in the bare body
    fs::write(&brs, "fn f() {\n    let mut v: Vec<u8> = Vec::new();\n    v.len()\n}\n").unwrap();
    let cw = weave(&dir, &bare, &out);
    assert!(!cw.ok());
    let d = &cw.weave_diags[0];
    let text = d.show();
    assert!(text.starts_with("src/lib.brs:2:5: a local's type does not go in the bare body"), "{text}");
    assert!(text.contains("\"holes\": { \"hint_local::f\": { \"locals\": { \"v\": \"list<u8> as 'Vec'\" } } }"), "{text}");
    assert!(text.contains("turbofish") && text.contains("without `: Vec<u8>`"), "{text}");
    // in the --json output too
    let j = cw.to_json();
    let e = &j["errors"][0];
    assert_eq!((e["bare"]["file"].as_str(), e["bare"]["line"].as_u64(), e["bare"]["column"].as_u64()), (Some("src/lib.brs"), Some(2), Some(5)));
    assert!(e["message"].as_str().unwrap().contains("\"locals\""), "{e}");
    // the fix the message names: no type in the body, the type in holes
    let holes: Value = serde_json::from_str(&format!("{{{}}}", between(&text, "as  \"holes\": ", "  (merge").trim().trim_start_matches('{').trim_end_matches('}'))).unwrap();
    fs::write(&brs, "fn f() {\n    let v = Vec::new();\n    v.len()\n}\n").unwrap();
    let cj = bare.join("src/lib.constraints.json");
    let mut js = read_json(&cj);
    js["holes"] = holes;
    fs::write(&cj, canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out);
    assert!(cw.ok(), "{:?} {:?}", cw.weave_errors, cw.errors.iter().map(|e| e.mapped.show()).collect::<Vec<_>>());
}

#[test]
fn a_let_mut_is_accepted_in_a_bare_body() {
    let j = json!({"format": "y-constraints/0", "host": "rust", "crate": "c 0.1.0", "module": "src/lib.rs",
        "subjects": [{"kind": "function", "name": "c::f", "params": [], "result": {"type": "u32"}}]});
    assert!(bare_rust::weave("fn f() {\n    let mut n = 0;\n    n\n}\n", &j).is_ok());
}

#[test]
fn a_type_alias_with_a_right_hand_side_names_the_alias_subject() {
    let src = "pub type Table<K> = std::collections::HashMap<K, u32>;\n";
    let (dir, bare, out) = krate("hint_alias", src);
    let brs = bare.join("src/lib.brs");
    let cj = bare.join("src/lib.constraints.json");
    let good = read_json(&cj);
    fs::write(&brs, "pub type Table<K> = std::collections::HashMap<K, u32>;\n").unwrap();
    let cw = weave(&dir, &bare, &out);
    let text = cw.weave_diags[0].show();
    assert!(text.starts_with("src/lib.brs:1:1: a type alias's right-hand side does not go in the bare body; write `type Table;`"), "{text}");
    assert!(text.contains("\"kind\": \"alias\"") && text.contains("\"name\": \"hint_alias::Table\"") && text.contains("map<K, u32> as 'std::collections::HashMap'") && text.contains("\"generics\""), "{text}");
    assert!(cw.to_json()["errors"][0]["message"].as_str().unwrap().contains("type Table;"));
    // the printed entry is the subject strip wrote for the real alias
    let entry: Value = serde_json::from_str(text.rsplit("constraint file: ").next().unwrap()).unwrap();
    assert_eq!(entry["kind"], "alias");
    let mut js = good.clone();
    js["subjects"] = json!([entry]);
    fs::write(&brs, "type Table;\n").unwrap();
    fs::write(&cj, canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out);
    assert!(cw.ok(), "{:?} {:?}", cw.weave_errors, cw.errors.iter().map(|e| e.mapped.show()).collect::<Vec<_>>());
    assert_eq!(read_json(&cj)["subjects"][0], good["subjects"][0]);
}

#[test]
fn a_missing_subject_comes_with_an_entry_that_weaves_once_filled() {
    let src = "pub struct Counter { n: u32 }\nimpl Counter {\n    pub fn get(&self) -> u32 { self.n }\n}\n";
    let (dir, bare, out) = krate("hint_missing", src);
    let brs = bare.join("src/lib.brs");
    let cj = bare.join("src/lib.constraints.json");
    let base = read_json(&cj);
    // a new method, a new function, a new struct and enum, a trait with a method, a const and an alias
    let new = "struct Pair { a, b }\nenum Kind { A, B(_), C { x } }\ntrait Named { fn name(self); }\nconst LIMIT = 3;\ntype Id;\nfn twice(x) { x + x }\nimpl Counter {\n    fn bump(self, by) { self.n += by; }\n    fn take(self) { self.n }\n}\n";
    let mut text = fs::read_to_string(&brs).unwrap();
    text.push('\n');
    text.push_str(new);
    fs::write(&brs, text).unwrap();
    let mut js = base.clone();
    let mut seen = vec![];
    for _ in 0..16 {
        fs::write(&cj, canonical(&js)).unwrap();
        let cw = weave(&dir, &bare, &out);
        if cw.ok() {
            break;
        }
        let d = cw.weave_diags.first().unwrap_or_else(|| panic!("{:?} {:?}", cw.errors.iter().map(|e| e.mapped.show()).collect::<Vec<_>>(), cw.holes));
        let msg = d.show();
        assert!(msg.contains("slot missing: no subject `hint_missing::"), "{msg}");
        assert!(d.bare_line > 0, "{msg}");
        let mut entry: Value = serde_json::from_str(msg.lines().last().unwrap().trim()).unwrap_or_else(|e| panic!("{e}: {msg}"));
        seen.push(entry["name"].as_str().unwrap().to_string());
        let mut s = entry.to_string();
        // the type that goes where each TODO is
        s = s.replace("\"TODO\"", "\"u32\"");
        entry = serde_json::from_str(&s).unwrap();
        // `take` has an owned self, `bump` a mutable one: adjust the way the note says
        if entry["name"] == "hint_missing::Counter::bump" {
            entry["params"][0]["access"] = json!({"rights": "write", "guarantee": "exclusive"});
            entry.as_object_mut().unwrap().remove("result");
        }
        if entry["name"] == "hint_missing::Counter::take" {
            entry["params"][0] = json!({"name": "self", "type": "Counter"});
        }
        js["subjects"].as_array_mut().unwrap().push(entry);
    }
    let cw = weave(&dir, &bare, &out);
    assert!(cw.ok(), "{seen:?} {:?} {:?}", cw.weave_errors, cw.errors.iter().map(|e| e.mapped.show()).collect::<Vec<_>>());
    for n in ["Pair", "Kind", "Named", "Named::name", "LIMIT", "Id", "twice", "Counter::bump", "Counter::take"] {
        assert!(seen.contains(&format!("hint_missing::{n}")), "no entry offered for {n}: {seen:?}");
    }
}

#[test]
fn a_leftover_todo_is_refused_with_its_place() {
    let src = "pub fn g(x: u32) -> u32 { x }\n";
    let (dir, bare, out) = krate("hint_todo", src);
    let cj = bare.join("src/lib.constraints.json");
    let mut js = read_json(&cj);
    js["subjects"][0]["params"][0]["type"] = json!("TODO");
    js["subjects"][0]["result"]["type"] = json!("list<TODO>");
    fs::write(&cj, canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out);
    assert!(!cw.ok());
    let m = &cw.weave_errors[0].1;
    assert!(m.contains("unfilled placeholder") && m.contains("/subjects/0/params/0/type") && m.contains("/subjects/0/result/type") && m.contains("hint_todo::g"), "{m}");
    assert_eq!(cw.iterations, 0, "refused before cargo");
}

// ---- scaffold ------------------------------------------------------------------------------------------------

fn fill(v: &mut Value) {
    match v {
        Value::String(s) if s == "TODO" => *s = "u32".into(),
        Value::Array(a) => a.iter_mut().for_each(fill),
        Value::Object(o) => o.values_mut().for_each(fill),
        _ => {}
    }
}

#[test]
fn scaffold_of_each_kind_weaves_and_checks_once_filled() {
    let src = "pub struct Counter { n: u32 }\nimpl Counter {\n    pub fn get(&self) -> u32 { self.n }\n}\npub trait Describe {\n    fn describe(&self) -> u32;\n}\n";
    let (dir, bare, out) = krate("scaffold_all", src);
    let spec = |kind: &str, path: &str| Spec { kind: kind.into(), path: path.into(), ..Default::default() };
    let specs = vec![
        Spec { params: vec!["a".into(), "b".into()], visible: Some("public".into()), ..spec("fn", "add") },
        Spec { params: vec!["by".into()], self_kind: Some(SelfKind::Mut), result: Some("none".into()), ..spec("fn", "Counter::bump") },
        Spec { params: vec!["key".into()], self_kind: Some(SelfKind::Ref), param_types: vec!["u32".into()], ..spec("fn", "Counter::peek") },
        Spec { params: vec!["x".into()], generics: vec!["T:Clone+Copy".into()], param_types: vec!["T".into()], result: Some("T".into()), ..spec("fn", "ident") },
        Spec { trait_: Some("Describe".into()), methods: vec!["describe".into()], ..spec("impl", "Counter") },
        Spec { ty: Some("u32".into()), ..spec("alias", "Id") },
        Spec { fields: vec!["x".into(), "y".into()], ..spec("struct", "Pair") },
        Spec { tuple: Some(2), ..spec("struct", "Wrap") },
    ];
    for s in &specs {
        scaffold::write(&bare, s).unwrap_or_else(|e| panic!("{}: {e}", s.path));
    }
    // a second scaffold of the same item is refused
    assert!(scaffold::write(&bare, &specs[0]).unwrap_err().contains("already has a subject"));
    // the stub is bare: no TODO in the body; the entries say TODO
    let brs = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    assert!(!brs.contains("TODO") && brs.contains("fn add(a, b) {") && brs.contains("impl Describe for Counter {") && brs.contains("struct Wrap(_, _);"), "{brs}");
    let cj = bare.join("src/lib.constraints.json");
    assert_eq!(fs::read_to_string(&cj).unwrap(), canonical(&read_json(&cj)), "canonical form");
    // unfilled: refused by name
    let cw = weave(&dir, &bare, &out);
    assert!(!cw.ok() && cw.weave_errors[0].1.contains("unfilled placeholder") && cw.weave_errors[0].1.contains("scaffold_all::add"), "{:?}", cw.weave_errors);
    assert!(cw.to_json()["errors"][0]["message"].as_str().unwrap().contains("TODO"));
    // filled: weaves and cargo check passes
    let mut js = read_json(&cj);
    fill(&mut js["subjects"]);
    fs::write(&cj, canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out);
    assert!(cw.ok(), "{:?} {:?}", cw.weave_errors, cw.errors.iter().map(|e| e.mapped.show()).collect::<Vec<_>>());
    let woven = fs::read_to_string(out.join("src/lib.rs")).unwrap();
    assert!(woven.contains("pub fn add(a: u32, b: u32) -> u32") && woven.contains("fn peek(&self, key: u32) -> u32") && woven.contains("fn bump(&mut self, by: u32)"), "{woven}");
    assert!(woven.contains("struct Pair") && woven.contains("struct Wrap(u32, u32);") && woven.contains("type Id = u32;") && woven.contains("impl Describe for Counter"), "{woven}");
}

#[test]
fn scaffold_into_a_module_file_and_through_the_cli() {
    let (dir, bare, out) = krate("scaffold_mod", "pub mod geo;\npub fn top() -> u32 { 1 }\n");
    fs::write(dir.join("src/geo.rs"), "pub fn origin() -> u32 { 0 }\n").unwrap();
    let results = strip_crate(&dir, &bare, true).unwrap();
    assert!(results.iter().all(|r| r.outcome.is_ok()));
    let exe = env!("CARGO_BIN_EXE_bare-rust");
    let run = |args: &[&str]| std::process::Command::new(exe).arg("scaffold").args(args).output().unwrap();
    // printed, not written
    let o = run(&["struct", "geo::Pt", "--fields", "x,y", "--field-types", "u32,u32", "--crate", "scaffold_mod"]);
    let printed = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(o.status.success() && printed.contains("// src/geo.brs") && printed.contains("\"name\": \"scaffold_mod::geo::Pt\"") && !printed.contains("\"TODO\""), "{printed} {}", String::from_utf8_lossy(&o.stderr));
    // written into the module's own files
    let o = run(&["struct", "geo::Pt", "--fields", "x,y", "--field-types", "u32,u32", "--write", bare.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(fs::read_to_string(bare.join("src/geo.brs")).unwrap().contains("struct Pt {"));
    assert!(!fs::read_to_string(bare.join("src/lib.brs")).unwrap().contains("Pt"));
    let js = read_json(&bare.join("src/geo.constraints.json"));
    assert_eq!(js["subjects"][1]["name"], "scaffold_mod::geo::Pt");
    let cw = weave(&dir, &bare, &out);
    assert!(cw.ok(), "{:?} {:?}", cw.weave_errors, cw.errors.iter().map(|e| e.mapped.show()).collect::<Vec<_>>());
    // a bad option or kind is a usage error
    assert!(!run(&["union", "X"]).status.success());
    assert!(!run(&["fn", "x", "--nope", "1"]).status.success());
}
