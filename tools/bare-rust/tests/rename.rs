//! `rename`: new names in the bare bodies and the constraint files together, and nothing else.
use bare_rust::corpus::{strip_crate, weave_crate};
use bare_rust::rename::rename_dir;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const LIB: &str = r#"pub mod util;

pub struct S {
    pub n: u64,
}

pub fn f(h: u32, a: &str, b: &str) -> f64 {
    let x = (h as u64).wrapping_mul(3);
    let s = S { n: x };
    let r = a.len() as f64 / b.len() as f64;
    let port = a.parse::<u16>().unwrap_or(0);
    println!("{}", port);
    r + s.n as f64
}
"#;

const UTIL: &str = r#"pub fn g(a: &str) -> u16 {
    let n = a.len();
    let label = format!("{n}");
    a.parse::<u16>().unwrap_or(label.len() as u16)
}
"#;

/// A crate with two modules, stripped into `<dir>/bare`.
fn stripped(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bare-rust-test/rename").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("src")).unwrap();
    fs::write(d.join("Cargo.toml"), "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n").unwrap();
    fs::write(d.join("src/lib.rs"), LIB).unwrap();
    fs::write(d.join("src/util.rs"), UTIL).unwrap();
    for r in strip_crate(&d, &d.join("bare"), false).unwrap() {
        assert!(r.outcome.is_ok(), "{:?}", r.outcome);
    }
    d.join("bare")
}

fn woven(bare: &Path, to: &str) -> (String, String) {
    let out = bare.parent().unwrap().join(to);
    for r in weave_crate(bare, &out).unwrap() {
        assert!(r.outcome.is_ok(), "{:?}", r.outcome);
    }
    (fs::read_to_string(out.join("src/lib.rs")).unwrap(), fs::read_to_string(out.join("src/util.rs")).unwrap())
}

#[test]
fn locals_and_realizations_are_renamed_in_the_body_and_the_file_together() {
    let bare = stripped("ok");
    let lib = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    assert!(lib.contains("let h_u64 = h;") && lib.contains("a.parse_u16()"), "{lib}");
    let (lib_before, util_before) = woven(&bare, "woven-before");
    let map = json!({
        "demo::f": { "h_u64": "wide", "len_f64": "a_total", "s": "shape" },
        "realizations": { "parse_u16": "parse_port" }
    });
    let done = rename_dir(&bare, &map, true).unwrap();
    let uses: Vec<(&str, &str, usize)> = done.iter().map(|r| (r.from.as_str(), r.to.as_str(), r.uses)).collect();
    assert_eq!(uses, vec![("h_u64", "wide", 2), ("len_f64", "a_total", 2), ("s", "shape", 2), ("parse_u16", "parse_port", 2)]);
    let lib = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    let util = fs::read_to_string(bare.join("src/util.brs")).unwrap();
    assert!(lib.contains("let wide = h;") && lib.contains("let shape = S { n: x };") && lib.contains("a.parse_port()"), "{lib}");
    // A realization is renamed in every module that uses it, and in the crate's table.
    assert!(util.contains("a.parse_port()"), "{util}");
    let js: serde_json::Value = serde_json::from_str(&fs::read_to_string(bare.join("src/lib.constraints.json")).unwrap()).unwrap();
    assert_eq!(js["realizations"]["parse_port"], json!({"template": "<_>::parse", "args": ["u16"]}));
    assert!(js["holes"]["demo::f"]["locals"]["wide"].is_object() && js["holes"]["demo::f"]["locals"]["h_u64"].is_null());
    // The woven crate is the same program: the locals have their new names, and a realization weaves to its template
    // whatever its name.
    let (lib_after, util_after) = woven(&bare, "woven-after");
    assert_eq!(util_after, util_before);
    let back = lib_after.replace("wide", "h_u64").replace("a_total", "len_f64").replace("shape", "s");
    assert_eq!(back, lib_before);
}

#[test]
fn a_rename_that_would_change_the_program_is_refused() {
    let bare = stripped("refused");
    let before = fs::read_to_string(bare.join("src/lib.brs")).unwrap();
    for (map, why) in [
        (json!({"demo::f": {"h_u64": "x"}}), "already a name"),
        (json!({"demo::f": {"h_u64": "S"}}), "already a name"),
        (json!({"demo::f": {"h": "hash"}}), "not bound"),
        (json!({"demo::f": {"nothing": "y"}}), "not a name in the body"),
        (json!({"demo::f": {"h_u64": "1x"}}), "is not a name"),
        (json!({"demo::f": {"h_u64": "wide", "len_f64": "wide"}}), "given twice"),
        (json!({"demo::util::g": {"n": "count"}}), "captures `n`"),
        (json!({"demo::nope": {"a": "b"}}), "no body of `demo::nope`"),
        (json!({"realizations": {"parse_u16": "len"}}), "already a name in the crate"),
        (json!({"realizations": {"parse_u8": "p"}}), "no realization"),
    ] {
        let e = rename_dir(&bare, &map, true).unwrap_err();
        assert!(e.contains(why), "{map}: {e}");
    }
    assert_eq!(fs::read_to_string(bare.join("src/lib.brs")).unwrap(), before, "a refused rename writes nothing");
    // Lenient: what does not apply is skipped, the rest is renamed.
    let (done, skipped) = bare_rust::rename::rename_dir_with(&bare, &json!({"demo::f": {"h_u64": "wide", "gone": "x"}, "demo::nope": {"a": "b"}}), true, true).unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(skipped.len(), 2, "{skipped:?}");
}
