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
