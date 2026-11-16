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

#[test]
fn gated_code_is_checked_in_a_build_that_compiles_it() {
    // Code under a feature, a `--cfg` and a release build's condition loses its `mut`s, which the build that compiles it
    // derives; code under a feature the crate does not declare is in no build, and keeps them.
    let top = root().join("target/bare-rust-test/wc_builds");
    let _ = fs::remove_dir_all(&top);
    let dir = top.join("crate");
    let w = |p: &str, s: &str| {
        fs::create_dir_all(dir.join(p).parent().unwrap()).unwrap();
        fs::write(dir.join(p), s).unwrap();
    };
    let body = |name: &str| format!("pub fn {name}() -> u32 {{\n    let mut n = 0;\n    n += 1;\n    n\n}}\n");
    w(
        "Cargo.toml",
        "[package]\nname = \"wc_builds\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\nx = []\n\n[lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(my_cfg)', 'cfg(feature, values(\"undeclared\"))'] }\n",
    );
    w(
        "src/lib.rs",
        &format!(
            "#[cfg(feature = \"x\")]\npub mod gated;\n#[cfg(feature = \"undeclared\")]\npub mod never;\n\n#[cfg(my_cfg)]\n{}\n#[cfg(not(debug_assertions))]\n{}",
            body("c"),
            body("r")
        ),
    );
    w("src/gated.rs", &body("g"));
    w("src/never.rs", &body("v"));
    let o = bare_rust::check::CheckOpts { work: top.join("work"), target: bare_rust::cargo::target_dir(&root()), tests: false, full: false, ..Default::default() };
    let r = bare_rust::check::check_crate(&dir, &o).unwrap();
    let names = r.reach.names();
    for b in ["default", "feature-x", "cfg-my_cfg", "release"] {
        assert!(names.contains(&b), "{names:?}");
    }
    assert_eq!(r.reach.unreached, ["feature = \"undeclared\""]);
    let bare = |p: &str| fs::read_to_string(top.join("work/wc_builds/bare").join(p)).unwrap();
    assert!(bare("src/gated.brs").contains("let n = 0;") && bare("src/lib.brs").matches("let n = 0;").count() == 2, "{}", bare("src/lib.brs"));
    assert!(bare("src/never.brs").contains("let mut n = 0;"));
    assert!(r.ok(), "{}", serde_json::to_string_pretty(&r.to_json()).unwrap());
    assert_eq!((r.c2.exact, r.c2.files), (3, 3));
    assert_eq!(r.mut_added, 3);
}

/// The level check (language 6.22) and the region check (6.23) read what rustc resolved in the woven crate: a method
/// called on a closure's parameter is its type's, a function re-exported under another name is where it is defined,
/// and a transaction called through a local is still the runtime's `atomic`. From the bare bodies' syntax alone, the
/// first is any method of that name, the second is not seen, and the third is no transaction.
#[test]
fn levels_and_regions_read_what_rustc_resolved() {
    let runtime = root().join("../../examples/y-runtime").canonicalize().unwrap();
    let src = "macro_rules! later {\n    ($d:expr) => {\n        crate::rt::start_detached($d, || 2)\n    };\n}\n\npub mod rt {\n    pub use y_runtime::task::detach as start_detached;\n}\n\npub mod waits {\n    pub fn wait(within: std::time::Duration) -> u32 {\n        let _ = later!(within);\n        1\n    }\n}\n\npub mod counts {\n    pub trait Count {\n        fn count(&self) -> u32;\n    }\n    pub struct Tally(pub u32);\n    pub struct Ledger(pub u32);\n    impl Count for Tally {\n        fn count(&self) -> u32 {\n            self.0\n        }\n    }\n    impl Count for Ledger {\n        fn count(&self) -> u32 {\n            let mut n = 0;\n            while n < self.0 {\n                n += 1;\n            }\n            n\n        }\n    }\n}\n\npub mod sums {\n    use crate::counts::{Count, Tally};\n    pub fn total(xs: &[Tally]) -> u32 {\n        xs.iter().map(|t| t.count()).sum()\n    }\n    pub fn shout(xs: &[Tally]) {\n        println!(\"{}\", total(xs));\n    }\n}\n\npub mod ticks {\n    pub fn start(within: std::time::Duration) -> u32 {\n        let _ = crate::rt::start_detached(within, || 1);\n        1\n    }\n}\n\npub mod store {\n    use y_runtime::shared::{atomic, Shared, Step, Tx};\n    fn bump(tx: &mut Tx, count: &Shared<u32>) -> Result<Step<u32>, ()> {\n        Ok(Step::Done(tx.get(count) + 1))\n    }\n    pub fn add(count: &Shared<u32>) -> u32 {\n        let run = atomic;\n        // y: region add is atomic\n        run(\"store::add#add\", None, |tx: &mut Tx| bump(tx, count)).unwrap_or(0)\n    }\n}\n";
    let top = root().join("target/bare-rust-test/wc_levels");
    let _ = fs::remove_dir_all(&top);
    let dir = top.join("crate");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"wc_levels\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ny-runtime = {{ path = \"{}\" }}\n", runtime.display())).unwrap();
    fs::write(dir.join("src/lib.rs"), src).unwrap();
    let bare = top.join("bare");
    let results = strip_crate(&dir, &bare, true).unwrap();
    assert!(results.iter().all(|r| r.outcome.is_ok()), "{:?}", results.iter().map(|r| r.outcome.clone()).collect::<Vec<_>>());
    // `sums` and `ticks` are pinned at `calculations`; `store::add` has an atomic region
    let cj = bare.join("src/lib.constraints.json");
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    for s in js["subjects"].as_array_mut().unwrap() {
        if s["name"] == "wc_levels::sums" || s["name"] == "wc_levels::ticks" || s["name"] == "wc_levels::waits" {
            s["level"] = "calculations".into();
        }
    }
    let clauses: Vec<Value> = ["atomic", "may run again", "on failure unchanged"].iter().map(|t| serde_json::json!({"kind": "region", "text": t, "source": "y/src/lib.y:1"})).collect();
    js["subjects"].as_array_mut().unwrap().push(serde_json::json!({"kind": "region", "name": "wc_levels::store::add#add", "clauses": clauses}));
    fs::write(&cj, serde_json::to_string_pretty(&js).unwrap()).unwrap();
    let cw = weave(&dir, &bare, &top.join("woven"), false);
    assert!(cw.checks_via.as_deref().is_some_and(|v| v.starts_with("rustc (MIR of lib")), "{:?} {:?} {:?} {}", cw.checks_via, cw.weave_errors, cw.errors.iter().map(|e| e.show()).collect::<Vec<_>>(), cw.cargo_stderr);
    // the re-exported `detach` is `y_runtime::task::detach`, which needs `effects`; the closure's `t.count()` is
    // `Tally`'s, a calculation; the region runs in `atomic`, called through `run`; `println!`, which rustc does not
    // place in the bare body, is read from it; and `later!`, the crate's own macro, expands to a call of `detach`
    let mut errs: Vec<&String> = cw.weave_errors.iter().map(|(_, e)| e).collect();
    errs.sort();
    assert!(errs.iter().any(|e| e.contains("a call of `later!`, which needs `effects`, in `wc_levels::waits::wait` needs level `effects`")), "{errs:#?}");
    errs.retain(|e| !e.contains("waits::wait"));
    assert_eq!(errs.len(), 2, "{errs:#?}");
    assert!(errs[0].contains("the macro `println!` (`std::println`) in `wc_levels::sums::shout` needs level `effects`, and module `wc_levels::sums` is at `calculations`"), "{errs:#?}");
    assert!(errs[1].contains("`detach` (`y_runtime::task::detach`) in `wc_levels::ticks::start` needs level `effects`, and module `wc_levels::ticks` is at `calculations`"), "{errs:#?}");
    // the bare bodies' syntax alone sees none of the three
    let brs = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    let syntax = bare_rust::level::check(&[("src/lib.brs".into(), brs.clone(), js.clone())]);
    assert!(syntax.iter().any(|e| e.contains("a call of `.count` (it may be `wc_levels::counts::Ledger::count`") && e.contains("in `wc_levels::sums::total`")), "{syntax:#?}");
    assert!(!syntax.iter().any(|e| e.contains("ticks") || e.contains("waits")), "{syntax:#?}");
    let region = bare_rust::region::check_body(&brs, &js, "wc_levels::store::add#add").unwrap_err();
    assert!(region.contains("runs in no transaction"), "{region}");
    // and with what rustc resolved, the region is kept
    let facts = cw.facts.as_ref().unwrap();
    assert_eq!(bare_rust::region::check_body_with(&brs, &js, "wc_levels::store::add#add", Some(&facts["src/lib.brs"]), None), Ok(()));
}

/// `libraries/levels.json` against what the library crates of the examples call (`facts::check_library`): no public
/// function of `y-runtime`, `y-collections` or `y-string` calls more than the table gives it. (`y-http` and `y-window`
/// are at `effects`, which nothing is above.) A function that calls more is named, with the call.
#[test]
fn the_table_of_library_levels_holds_for_the_libraries() {
    let target = bare_rust::cargo::target_dir(&root());
    for lib in ["y-runtime", "y-collections", "y-string"] {
        let dir = root().join("../../examples").join(lib);
        assert_eq!(bare_rust::facts::check_library(&dir, &target).unwrap(), Vec::<String>::new(), "{lib}");
    }
    // a `y_string` that writes a file is above its `calculations`
    let top = root().join("target/bare-rust-test/wc_table");
    let _ = fs::remove_dir_all(&top);
    fs::create_dir_all(top.join("src")).unwrap();
    fs::write(top.join("Cargo.toml"), "[package]\nname = \"y-string\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n").unwrap();
    fs::write(top.join("src/lib.rs"), "fn keep(text: &str) {\n    let _ = std::fs::write(\"x\", text);\n}\n\npub fn upper(text: &str) -> String {\n    keep(text);\n    text.to_uppercase()\n}\n").unwrap();
    let errs = bare_rust::facts::check_library(&top, &target).unwrap();
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(errs[0].contains("`y_string::upper` needs `effects`, and libraries/levels.json gives it `calculations`: it calls `y_string::keep`, which needs `effects` (it calls `std::fs::write`)"), "{errs:#?}");
}

/// A `trusted` entry of `libraries/levels.json` vouches for the calls it lists, and only those: a copy of `y-runtime`
/// whose `wait` also reads a file's metadata is above its module's level, and one whose `wait` no longer reads its
/// deadline has an entry that outlived its reason.
#[test]
fn a_trusted_function_is_trusted_only_for_the_calls_it_lists() {
    let target = bare_rust::cargo::target_dir(&root());
    let runtime = root().join("../../examples/y-runtime");
    let copy = |name: &str, from: &str, to: &str| -> PathBuf {
        let dir = root().join("target/bare-rust-test").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::copy(runtime.join("Cargo.toml"), dir.join("Cargo.toml")).unwrap();
        for e in fs::read_dir(runtime.join("src")).unwrap() {
            let p = e.unwrap().path();
            fs::copy(&p, dir.join("src").join(p.file_name().unwrap())).unwrap();
        }
        let shared = fs::read_to_string(dir.join("src/shared.rs")).unwrap();
        assert!(shared.contains(from), "{from}");
        fs::write(dir.join("src/shared.rs"), shared.replacen(from, to, 1)).unwrap();
        dir
    };
    let read = "            let left = d.remaining()?;";
    let more = copy("wc_trust_more", read, &format!("            let _ = std::fs::metadata(\"lock\");\n{read}"));
    let errs = bare_rust::facts::check_library(&more, &target).unwrap();
    assert!(errs.iter().any(|e| e.contains("`y_runtime::shared::atomic` needs `effects`, and libraries/levels.json gives it `shared`") && e.contains("(it calls `std::fs::metadata`)")), "{errs:#?}");
    let stale = copy("wc_trust_stale", read, "            let left = Duration::from_secs(d as *const Deadline as usize as u64 % 2);");
    let errs = bare_rust::facts::check_library(&stale, &target).unwrap();
    assert_eq!(errs, vec!["src/shared.rs: libraries/levels.json trusts `y_runtime::shared::wait` to call `y_runtime::Deadline::remaining`, and it no longer does: remove the entry".to_string()]);
}

/// The level check reads the MIR of the crate's tests and of every build whose check passes: a function under a
/// feature that only another build turns on, and that calls the runtime's `detach` re-exported under another name, is
/// above its module's level; without that build, no MIR has it, and its syntax does not show the call for what it is.
#[test]
fn gated_and_test_code_is_read_from_rustc() {
    let runtime = root().join("../../examples/y-runtime").canonicalize().unwrap();
    let src = "pub mod rt {\n    pub use y_runtime::task::detach as start_detached;\n}\n\npub mod ticks {\n    #[cfg(feature = \"extra\")]\n    pub fn start(within: std::time::Duration) -> u32 {\n        let _ = crate::rt::start_detached(within, || 1);\n        1\n    }\n\n    pub fn one() -> u32 {\n        1\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn one() {\n        assert_eq!(crate::ticks::one(), 1);\n    }\n}\n";
    let top = root().join("target/bare-rust-test/wc_gated");
    let _ = fs::remove_dir_all(&top);
    let dir = top.join("crate");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"wc_gated\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\nextra = []\n\n[dependencies]\ny-runtime = {{ path = \"{}\" }}\n", runtime.display())).unwrap();
    fs::write(dir.join("src/lib.rs"), src).unwrap();
    let bare = top.join("bare");
    let results = strip_crate(&dir, &bare, true).unwrap();
    assert!(results.iter().all(|r| r.outcome.is_ok()), "{:?}", results.iter().map(|r| r.outcome.clone()).collect::<Vec<_>>());
    let cj = bare.join("src/lib.constraints.json");
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    for s in js["subjects"].as_array_mut().unwrap() {
        if s["name"] == "wc_gated::ticks" {
            s["level"] = "calculations".into();
        }
    }
    fs::write(&cj, serde_json::to_string_pretty(&js).unwrap()).unwrap();
    // strip found the build that turns the feature on; in the default build alone, `start` is compiled by nothing: its
    // syntax is read, and passes
    assert!(fs::read_to_string(bare.join("builds.json")).unwrap().contains("feature-extra"));
    fs::write(bare.join("builds.json"), r#"{"builds": [{"name": "default"}]}"#).unwrap();
    let cw = weave(&dir, &bare, &top.join("woven"), false);
    let via = cw.checks_via.clone().unwrap_or_default();
    assert!(via.starts_with("rustc (MIR of lib, lib tests") && !via.contains("build feature-extra"), "{via}");
    assert!(cw.weave_errors.is_empty(), "{:#?}", cw.weave_errors);
    // the test's call is one rustc resolved
    let facts = &cw.facts.as_ref().unwrap()["src/lib.brs"];
    let test_line = fs::read_to_string(bare.join("src/lib.brs")).unwrap().lines().position(|l| l.contains("crate::ticks::one()")).unwrap() + 1;
    assert!(facts.iter().any(|f| f.line == test_line && f.callee.path == "wc_gated::ticks::one"), "{facts:#?}");
    // with the build that turns the feature on, its MIR has `start`, and the call of `detach`
    fs::write(bare.join("builds.json"), r#"{"builds": [{"name": "default"}, {"name": "feature-extra", "features": ["extra"]}]}"#).unwrap();
    let cw = weave(&dir, &bare, &top.join("woven2"), false);
    let via = cw.checks_via.clone().unwrap_or_default();
    assert!(via.contains("build feature-extra: lib, lib tests"), "{via}");
    let errs: Vec<&String> = cw.weave_errors.iter().map(|(_, e)| e).collect();
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(errs[0].contains("`detach` (`y_runtime::task::detach`) in `wc_gated::ticks::start` needs level `effects`, and module `wc_gated::ticks` is at `calculations`"), "{errs:#?}");
}

/// A call through a function pointer or a `dyn Fn` may be any function value or closure of the crate that takes its
/// arguments, as rustc reads them: `ops`, pinned at `calculations`, calls what it is given, and the crate gives it
/// `double` (a calculation), `shout` (which prints) and a closure that loops. From the syntax alone, `f(x)` names no
/// function.
#[test]
fn calls_through_pointers_and_dyn_fn_are_resolved() {
    let src = "pub mod ops {\n    pub fn apply(f: fn(u32) -> u32, x: u32) -> u32 {\n        f(x)\n    }\n\n    pub fn apply_dyn(f: &dyn Fn(u32) -> u32, x: u32) -> u32 {\n        f(x)\n    }\n\n    pub fn named(f: fn(&str) -> usize) -> usize {\n        f(\"x\")\n    }\n}\n\npub mod calm {\n    pub fn double(x: u32) -> u32 {\n        x * 2\n    }\n\n    pub fn length(s: &str) -> usize {\n        s.len()\n    }\n}\n\npub mod loud {\n    pub fn shout(x: u32) -> u32 {\n        println!(\"{x}\");\n        x\n    }\n\n    pub fn run() -> u32 {\n        let slow = |x: u32| {\n            let mut n = x;\n            while n > 0 {\n                n -= 1;\n            }\n            n\n        };\n        let _ = crate::ops::named(crate::calm::length);\n        crate::ops::apply(crate::calm::double, 1) + crate::ops::apply_dyn(&shout, 2) + crate::ops::apply_dyn(&slow, 3)\n    }\n}\n";
    let (dir, bare, out) = krate("wc_indirect", src);
    let cj = bare.join("src/lib.constraints.json");
    let mut js: Value = serde_json::from_str(&fs::read_to_string(&cj).unwrap()).unwrap();
    for s in js["subjects"].as_array_mut().unwrap() {
        if s["name"] == "wc_indirect::ops" {
            s["level"] = "calculations".into();
        }
    }
    fs::write(&cj, serde_json::to_string_pretty(&js).unwrap()).unwrap();
    let brs = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    assert_eq!(bare_rust::level::check(&[("src/lib.brs".into(), brs, js.clone())]), Vec::<String>::new());
    let cw = weave(&dir, &bare, &out, false);
    assert!(cw.checks_via.as_deref().is_some_and(|v| v.starts_with("rustc")), "{:?}", cw.checks_via);
    let mut errs: Vec<&String> = cw.weave_errors.iter().map(|(_, e)| e).collect();
    errs.sort();
    // `apply` and `apply_dyn` take a `u32`, as `double`, `shout` and `slow` do: they may print; `named` takes a `&str`,
    // as only `length` does
    assert_eq!(errs.len(), 2, "{errs:#?}");
    for (e, f) in errs.iter().zip(["apply", "apply_dyn"]) {
        assert!(e.contains(&format!("a call of a function pointer or an `Fn` bound (it may be `wc_indirect::loud::shout`, which needs the most of the 3 it may be), which needs `effects`, in `wc_indirect::ops::{f}`")), "{errs:#?}");
    }
}
