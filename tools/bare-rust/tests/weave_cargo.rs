//! Weaving into a crate with cargo: `mut` derived through rustc, holes reported, errors mapped back.
use bare_rust::corpus::strip_crate;
use bare_rust::crate_weave::{weave_crate_with, CrateWeave, WeaveOpts};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Make a one-file crate called `name` and strip it into `<dir>/bare`. Returns (crate dir, bare dir, out dir).
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

fn weave(crate_dir: &Path, bare: &Path, out: &Path, drop_holes: bool) -> CrateWeave {
    let o = WeaveOpts {
        cargo: true,
        crate_dir: Some(crate_dir.to_path_buf()),
        target: bare_rust::cargo::target_dir(&root()),
        drop_holes,
        keep: Default::default(),
    };
    weave_crate_with(bare, out, &o).unwrap()
}

#[test]
fn mut_is_derived_through_rustc() {
    let src = "pub fn total(xs: &Vec<u32>) -> u32 {\n    let mut n = 0;\n    for x in xs {\n        n += *x;\n    }\n    let mut v = Vec::new();\n    v.push(n);\n    v[0]\n}\n";
    let (dir, bare, out) = krate("wc_mut", src);
    let cw = weave(&dir, &bare, &out, false);
    assert!(cw.ok(), "{:?}", cw.errors);
    assert_eq!(cw.mut_added(), 2);
    let woven = fs::read_to_string(out.join("src/lib.rs")).unwrap();
    assert!(woven.contains("let mut n = 0;") && woven.contains("let mut v = Vec::new();"), "{woven}");
    assert_eq!(woven, src, "the derived mut gives the original back");
    // The map records both insertions, and is written beside the woven files.
    let map: Value = serde_json::from_str(&fs::read_to_string(out.join(".bare-rust-map.json")).unwrap()).unwrap();
    let muts = map["src/lib.rs"].as_array().unwrap().iter().filter(|s| s["kind"] == "mut").count();
    assert_eq!(muts, 2);
}

#[test]
fn an_undetermined_hole_is_reported_and_not_filled() {
    let src = "pub fn f() -> usize {\n    let v: Vec<u8> = Vec::new();\n    v.len()\n}\n";
    let (dir, bare, out) = krate("wc_hole", src);
    let cw = weave(&dir, &bare, &out, true);
    assert!(!cw.ok());
    assert_eq!(cw.holes.len(), 1, "{:?}", cw.holes);
    let h = &cw.holes[0];
    assert_eq!((h.subject.as_str(), h.kind, h.key.as_deref()), ("wc_hole::f", "local", Some("v")));
    assert_eq!(h.clause, "local v : T");
    assert_eq!((h.file.as_str(), h.line, h.col), ("src/lib.brs", 2, 9));
    assert!(cw.errors.is_empty(), "{:?}", cw.errors);
    // Nothing was filled: the woven text has no type where the file had none.
    let woven = fs::read_to_string(out.join("src/lib.rs")).unwrap();
    assert!(woven.contains("let v = Vec::new();"), "{woven}");
    // With the hole in the file it is determined.
    let cw = weave(&dir, &bare, &out.with_file_name("woven2"), false);
    assert!(cw.ok() && cw.holes.is_empty());
}

#[test]
fn an_error_at_a_slot_is_reported_against_the_slot() {
    let src = "pub fn g(x: u32) -> u32 {\n    x\n}\n";
    let (dir, bare, out) = krate("wc_slot", src);
    let cj = bare.join("src/lib.constraints.json");
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    js["subjects"][0]["params"][0]["type"] = Value::String("Nope".into());
    fs::write(&cj, bare_rust::json::canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    assert!(!cw.ok());
    let e = cw.errors.iter().find(|e| e.message.contains("`Nope`")).unwrap_or_else(|| panic!("{:?}", cw.errors));
    assert!(e.locus.contains("slot params[0].type of `wc_slot::g`"), "{}", e.locus);
}

#[test]
fn an_error_in_the_body_is_reported_at_the_bare_line() {
    let src = "pub fn g(x: u32) -> u32 {\n    let y = x;\n    y + 1\n}\n";
    let (dir, bare, out) = krate("wc_body", src);
    let b = bare.join("src/lib.brs");
    let text = fs::read_to_string(&b).unwrap().replace("y + 1", "z + 1");
    fs::write(&b, text).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    let e = cw.errors.iter().find(|e| e.code == "E0425").unwrap_or_else(|| panic!("{:?}", cw.errors));
    assert_eq!(e.locus, "bare src/lib.brs:3:5", "woven at {}:{}", e.woven_line, e.woven_col);
}

#[test]
fn a_file_that_does_not_weave_stops_before_cargo() {
    let (dir, bare, out) = krate("wc_refuse", "pub fn g(x: u32) -> u32 { x }\n");
    let cj = bare.join("src/lib.constraints.json");
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    js["subjects"][0]["params"][0]["type"] = Value::Null;
    fs::write(&cj, bare_rust::json::canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    assert!(!cw.ok());
    assert!(cw.weave_errors[0].1.contains("slot missing"), "{:?}", cw.weave_errors);
    assert_eq!(cw.iterations, 0);
}

#[test]
fn mut_on_self_and_on_bindings_is_counted_the_same_on_both_sides() {
    use bare_rust::check::{check_crate, CheckOpts};
    let src = "pub struct S(pub u8);\nimpl S {\n    pub fn f(mut self, mut c: u8) -> u8 {\n        self.0 += c;\n        c += 1;\n        c + self.0\n    }\n}\n";
    let (dir, _bare, _out) = krate("wc_mutself", src);
    let work = root().join("target/bare-rust-test/wc_mutself/check");
    let o = CheckOpts { work, target: bare_rust::cargo::target_dir(&root()), tests: true, full: false, ..Default::default() };
    let r = check_crate(&dir, &o).unwrap();
    assert!(r.c2_ok(), "{:?}", r.c2.mut_differs);
    assert_eq!((r.c2.mut_original, r.c2.mut_derived, r.mut_added), (2, 2, 2));
}

#[test]
fn unbound_entries_are_counted_in_the_weave_and_its_map() {
    let (dir, bare, out) = krate("wc_unbound", "pub fn g(x: u32) -> u32 {\n    let y: u32 = x;\n    y\n}\n");
    let cj = bare.join("src/lib.constraints.json");
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    js["holes"]["wc_unbound::g"]["locals"]["w"] = Value::String("u8".into());
    js["holes"]["wc_unbound::gone"] = serde_json::json!({"locals": {"q": "u8"}});
    fs::write(&cj, bare_rust::json::canonical(&js)).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    assert!(cw.ok(), "stale entries are warnings, not errors");
    assert_eq!(cw.unbound_counts(), (1, 0, 1));
    let map: Value = serde_json::from_str(&fs::read_to_string(out.join(".bare-rust-map.json")).unwrap()).unwrap();
    let u = map["@unbound"].as_array().unwrap();
    assert_eq!(u.len(), 2, "{u:?}");
    assert_eq!((u[0]["what"].as_str(), u[0]["name"].as_str()), (Some("unbound local"), Some("w")));
    assert_eq!((u[1]["what"].as_str(), u[1]["subject"].as_str()), (Some("unbound subject"), Some("wc_unbound::gone")));
}

#[test]
fn report_prints_the_tables_from_check_output() {
    use bare_rust::check::{check_crate, CheckOpts};
    let src = "pub fn f(x: u8) -> u32 {\n    let s = (0..x).map(u32::from).sum::<u32>();\n    let y: u32 = x as u32 + s;\n    y\n}\npub use std::fmt::Debug;\n";
    let (dir, _bare, _out) = krate("wc_report", src);
    let work = root().join("target/bare-rust-test/wc_report/check");
    let o = CheckOpts { work: work.clone(), target: bare_rust::cargo::target_dir(&root()), tests: true, full: false, ..Default::default() };
    let r = check_crate(&dir, &o).unwrap();
    assert!(r.ok());
    let md = bare_rust::report::report(&[&work.join("wc_report")], "provenance line").unwrap();
    assert!(md.starts_with("provenance line\n"), "{md}");
    for row in [
        "| files accepted | 1 | 1 |",
        "| function | 1 | 1 |",
        "| use | 1 | 1 |",
        "| locals | 2 | 2 |",
        "| realizations | 1 | 1 |",
        "| hints | 0 | 0 |",
        "| unbound entries: locals | 0 | 0 |",
        "| wc_report | 1 files, 0 refused | ok, 1 files | ok: 1 of 1 exact, 0 differ only in `mut`, 0 other | ok: cargo check | ok |",
    ] {
        assert!(md.contains(row), "missing `{row}` in\n{md}");
    }
}

#[test]
fn a_crate_directory_with_its_stripped_files_in_bare_counts_one_woven_none_unwoven() {
    // `strip --out <crate>/bare`, then `weave <crate> --out <out>`: the file is woven once, at its own path.
    let (dir, bare, out) = krate("wc_bare_layout", "pub fn f(x: u8) -> u8 {\n    x\n}\n");
    let inside = dir.join("bare");
    fs::create_dir_all(&inside).unwrap();
    fn cp(a: &Path, b: &Path) {
        fs::create_dir_all(b).unwrap();
        for e in fs::read_dir(a).unwrap() {
            let p = e.unwrap().path();
            let t = b.join(p.file_name().unwrap());
            if p.is_dir() { cp(&p, &t) } else { fs::copy(&p, &t).unwrap(); }
        }
    }
    cp(&bare, &inside);
    let o = WeaveOpts { cargo: false, crate_dir: None, target: bare_rust::cargo::target_dir(&root()), drop_holes: false, keep: Default::default() };
    let cw = weave_crate_with(&dir, &out, &o).unwrap();
    assert_eq!((cw.files.len(), cw.unwoven.len()), (1, 0), "{:?}", cw.unwoven);
    assert!(out.join("src/lib.rs").exists() && !out.join("bare/src/lib.brs").exists());
}

const MAPPED_SRC: &str = "pub fn add(a: u8, b: u8) -> u8 {\n    let c = a + b;\n    c\n}\n\npub fn twice(x: u8) -> u8 {\n    add(x, x)\n}\n";

fn bare_lib(bare: &Path) -> (PathBuf, PathBuf) {
    (bare.join("src/lib.brs"), bare.join("src/lib.constraints.json"))
}

#[test]
fn an_error_from_a_constraint_maps_to_its_json_path_and_the_bare_file() {
    let (dir, bare, out) = krate("wc_errmap_json", MAPPED_SRC);
    let (_, cj) = bare_lib(&bare);
    // `twice` takes a type that does not exist.
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    let idx = js["subjects"].as_array().unwrap().iter().position(|s| s["name"] == "wc_errmap_json::twice").unwrap();
    js["subjects"][idx]["params"][0]["type"] = Value::String("Missing".into());
    fs::write(&cj, serde_json::to_string_pretty(&js).unwrap()).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    assert!(!cw.ok());
    let m = crate_errors(&cw);
    let e = m.iter().find(|e| e.code == "E0425").unwrap_or_else(|| panic!("{:?}", m.iter().map(|e| e.show()).collect::<Vec<_>>()));
    assert_eq!(e.origin, "slot");
    assert_eq!(e.json_path.as_deref(), Some(format!("/subjects/{idx}/params/0/type").as_str()));
    assert_eq!(e.bare_file.as_deref(), Some("src/lib.brs"));
    let brs = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    let want = brs.lines().position(|l| l.starts_with("fn twice")).unwrap() + 1;
    assert_eq!(e.bare_line, want, "{}", e.show());
    assert!(e.show().starts_with(&format!("src/lib.brs:{want}:")) && e.show().contains(&format!("[constraints: /subjects/{idx}/params/0/type] error[E0425]")), "{}", e.show());
    let j = e.to_json();
    assert_eq!(j["constraints"]["path"], format!("/subjects/{idx}/params/0/type"));
}

#[test]
fn an_error_in_a_body_maps_to_the_bare_line_and_its_related_slot() {
    let (dir, bare, out) = krate("wc_errmap_body", MAPPED_SRC);
    let (brs, cj) = bare_lib(&bare);
    // A body error: an unknown name in `add`'s body.
    let text = fs::read_to_string(&brs).unwrap().replace("let c = a + b;", "let c = a + bb;");
    fs::write(&brs, &text).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    let m = crate_errors(&cw);
    let e = m.iter().find(|e| e.code == "E0425").unwrap_or_else(|| panic!("{:?}", m.iter().map(|e| e.show()).collect::<Vec<_>>()));
    assert_eq!(e.origin, "body");
    assert_eq!(e.json_path, None);
    let line = text.lines().position(|l| l.contains("a + bb")).unwrap() + 1;
    let col = text.lines().nth(line - 1).unwrap().find("bb").unwrap() + 1;
    assert_eq!((e.bare_line, e.bare_col), (line, col), "{}", e.show());
    // A mismatch between the body and a constraint: `add` now says it returns u16; the error is in the body and the
    // result slot is named as related.
    fs::write(&brs, text.replace("a + bb", "a + b")).unwrap();
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    let idx = js["subjects"].as_array().unwrap().iter().position(|s| s["name"] == "wc_errmap_body::add").unwrap();
    js["subjects"][idx]["result"]["type"] = Value::String("u16".into());
    fs::write(&cj, serde_json::to_string_pretty(&js).unwrap()).unwrap();
    let cw = weave(&dir, &bare, &out, false);
    let m = crate_errors(&cw);
    let e = m.iter().find(|e| e.code == "E0308").unwrap_or_else(|| panic!("{:?}", m.iter().map(|e| e.show()).collect::<Vec<_>>()));
    assert_eq!(e.origin, "body");
    assert!(e.related.iter().any(|r| r.json_path.as_deref() == Some(format!("/subjects/{idx}/result/type").as_str())), "{}", e.show());
}

fn crate_errors(cw: &CrateWeave) -> Vec<bare_rust::errmap::MappedError> {
    bare_rust::errmap::map_crate(cw)
}
