use bare_rust::{parse_bare, strip, weave};
use serde_json::Value;
use std::{fs, process::Command};

fn g(n: &str) -> String {
    fs::read_to_string(format!("{}/tests/golden/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}
fn json() -> Value {
    serde_json::from_str(&g("cache.constraints.json")).unwrap()
}
fn do_strip(rs: &str) -> (String, Value) {
    strip(rs, "eve_cache 0.1.0", "src/lib.rs").unwrap()
}

#[test]
fn e0_weave() {
    assert_eq!(weave(&g("cache.brs"), &json()).unwrap(), g("cache.rs"));
}

#[test]
fn e0_strip() {
    let (b, j) = do_strip(&g("cache.rs"));
    assert_eq!(b, g("cache.brs"));
    assert_eq!(j, json());
}

#[test]
fn round_trips() {
    let w = weave(&g("cache.brs"), &json()).unwrap();
    assert_eq!(do_strip(&w), (g("cache.brs"), json()));
    let (b, j) = do_strip(&g("cache.rs"));
    assert_eq!(weave(&b, &j).unwrap(), g("cache.rs"));
}

#[test]
fn missing_slot_is_named() {
    let mut j = json();
    j["subjects"][1]["params"][1]["type"] = Value::Null;
    let e = weave(&g("cache.brs"), &j).unwrap_err();
    assert!(e.contains("eve_cache::Cache::get") && e.contains("`key`"), "{e}");
}

#[test]
fn woven_rust_compiles() {
    let dir = std::env::temp_dir().join(format!("bare-rust-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let src = format!("#[derive(Hash, PartialEq, Eq)] pub struct Key; pub struct Entry;\n{}", g("cache.rs"));
    let woven = weave(&g("cache.brs"), &json()).unwrap();
    assert_eq!(woven, g("cache.rs"));
    let f = dir.join("lib.rs");
    fs::write(&f, src).unwrap();
    let out = Command::new("rustc")
        .args(["--crate-type", "lib", "--edition", "2021", "--out-dir"])
        .arg(&dir)
        .arg(&f)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&dir);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn bare_parser() {
    assert!(!parse_bare(&g("cache.brs")).root_node().has_error());
    assert!(parse_bare(&g("cache.rs")).root_node().has_error());
}
