//! A cast that is the whole initialiser of a typed `let` is carried by the local's entry.
use bare_rust::json::canonical;
use bare_rust::{first_error, normalise_nomut, parse_bare, rustfmt, strip, weave};
use serde_json::{json, Value};

fn schema_errors(j: &Value) -> Vec<String> {
    let schema: Value = serde_json::from_str(include_str!("../schema/y-constraints-0.schema.json")).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    v.iter_errors(j).map(|e| format!("{e} at {}", e.instance_path())).collect()
}

const SRC: &str = r#"
pub fn f(a: u32, b: u32, x: i64, s: &str) -> f64 {
    let y: u8 = (a + b) as u8;
    let z: u64 = x as u64;
    let w: u16 = (x) as u16;
    let q: u64 = 5u32 as u64;
    let n: usize = s.len() as usize;
    let k = a as u64 + (b as u64);
    let r: f64 = -(z as f64);
    (y as f64) + z as f64 + w as f64 + k as f64 + q as f64 + n as f64 + r
}
"#;

#[test]
fn every_cast_is_bound_and_carried_by_its_local() {
    let (b, j) = strip(SRC, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(first_error(&parse_bare(&b)).is_none(), "bare does not parse:\n{b}");
    assert_eq!(schema_errors(&j), Vec::<String>::new());
    // Every cast and suffixed literal is bound (`bind`), then carried by its local. Brackets weave would not put back
    // are dropped (`(x) as u16`), and a value already initialising a `let` keeps that name.
    for line in ["let y = a + b;", "let z = x;", "let w = x;", "let k5_u32 = 5;", "let q = k5_u32;", "let n = s.len();", "let r = -z_f64;"] {
        assert!(b.contains(line), "missing `{line}` in\n{b}");
    }
    assert!(!b.contains(" as "), "{b}");
    let locals = &j["holes"]["demo::f"]["locals"];
    assert_eq!(locals["y"], json!({"type": "u8", "cast": true}));
    assert_eq!(locals["n"], json!({"type": "u64 as 'usize'", "cast": true}));
    assert_eq!(locals["w"], json!({"type": "u16", "cast": true}));
    assert_eq!(locals["k5_u32"], json!("u32"));
    assert!(j["holes"]["demo::f"]["hints"].is_null());

    let w = weave(&b, &j).unwrap();
    let norm = |s: &str| rustfmt(&normalise_nomut(s).unwrap()).unwrap();
    assert_eq!(norm(&w), norm(SRC), "check 2: the woven text is the bound source");
    let (b2, j2) = strip(&w, "demo 0.1.0", "src/lib.rs").unwrap();
    assert_eq!(b2, b, "check 1 (bare)");
    assert_eq!(canonical(&j2), canonical(&j), "check 1 (json)");
}

#[test]
fn a_cast_local_written_by_hand_weaves() {
    // What an agent writes: a `let` that names the cast, with the operand as the body says it.
    let b = "fn g(d, m) {\n    let d = d;\n    let m = m;\n    1.0 - d / m\n}\n";
    let j = json!({
        "format": "y-constraints/0", "host": "rust", "crate": "demo", "module": "crate", "status": "lifted",
        "subjects": [{"kind": "function", "name": "demo::g", "visible": "public",
            "params": [{"name": "d", "type": "u64 as 'usize'"}, {"name": "m", "type": "u64 as 'usize'"}],
            "result": {"type": "f64"}}],
        "holes": {"demo::g": {"locals": {"d": {"type": "f64", "cast": true}, "m": {"type": "f64", "cast": true}}}}
    });
    assert_eq!(schema_errors(&j), Vec::<String>::new());
    let w = weave(b, &j).unwrap();
    assert!(w.contains("let d: f64 = d as f64;") && w.contains("let m: f64 = m as f64;"), "{w}");
}

#[test]
fn a_cast_local_without_an_initialiser_is_refused() {
    let b = "fn g() {\n    let d;\n    d = 1;\n}\n";
    let j = json!({
        "format": "y-constraints/0", "host": "rust", "crate": "demo", "module": "crate", "status": "lifted",
        "subjects": [{"kind": "function", "name": "demo::g", "visible": "public", "params": []}],
        "holes": {"demo::g": {"locals": {"d": {"type": "f64", "cast": true}}}}
    });
    let e = weave(b, &j).unwrap_err();
    assert!(e.contains("no initialiser"), "{e}");
}

#[test]
fn a_cast_local_with_its_type_dropped_keeps_its_cast() {
    let b = "fn g(d) {\n    let x = d;\n    x\n}\n";
    let j = json!({
        "format": "y-constraints/0", "host": "rust", "crate": "demo", "module": "crate", "status": "lifted",
        "subjects": [{"kind": "function", "name": "demo::g", "visible": "public",
            "params": [{"name": "d", "type": "u32"}], "result": {"type": "u64"}}],
        "holes": {"demo::g": {"locals": {"x": {"type": "_", "cast": true}}}}
    });
    let w = weave(b, &j).unwrap();
    assert!(w.contains("let x: _ = d as _;"), "{w}");
}
