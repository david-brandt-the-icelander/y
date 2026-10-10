//! `bind`: every cast and suffixed literal initialises a typed `let`, in a place that keeps what the program does.
use bare_rust::bind::{bind, bind_report};
use std::path::PathBuf;
use std::process::Command;

const SRC: &str = r#"
pub const P: u64 = 0x100000001b3;
pub fn g(x: u32) -> u32 { x + 1 }
pub fn f(h: u32, a: &str, b: &str, v: &mut Vec<u32>, c: bool) -> f64 {
    let x = (h as u64).wrapping_mul(P);
    let r = a.len() as f64 / b.len() as f64;
    v.push(v.len() as u32);
    let s = g(1) + g(h) as u32 + 2u32;
    let t = c && (h as u8) > 3;
    let w = match h { 0 => 1.0, n if n as u8 > 9 => 2.0, n => n as f64 * 0.5f64 };
    let z: f64 = (0..4).map(|i| i as f64).sum();
    let mut i = 0;
    while i < h as usize { i += 1; }
    let y: u16 = (h) as u16;
    let q = -1i64;
    if x > 1 { r } else if (s as f64) < w { z + y as f64 + q as f64 + t as u8 as f64 } else { (x as u32 as u64) as f64 }
}
"#;

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bare-rust-test/bind").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Compile the source with a `main` that prints `f` over a set of inputs, run it, and return what it printed.
fn run(src: &str, name: &str) -> String {
    let d = scratch(name);
    let main = r#"
fn main() {
    for h in [0u32, 1, 2, 3, 7, 12, 49] {
        for c in [false, true] {
            let mut v = vec![1u32, 2];
            println!("{:?}", (f(h, "abc", "de", &mut v, c), v));
        }
    }
}
"#;
    std::fs::write(d.join("m.rs"), format!("{src}{main}")).unwrap();
    let out = Command::new("rustc").args(["--edition", "2021", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
    assert!(out.status.success(), "{}\n{src}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
}

#[test]
fn the_bound_source_does_what_the_source_does() {
    let (b, left) = bind_report(SRC).unwrap();
    assert!(left.is_empty(), "{left:?}");
    assert_eq!(run(&b, "bound"), run(SRC, "source"));
    // Nothing is left to bind, and a second pass changes nothing.
    assert_eq!(bind(&b).unwrap(), b);
}

#[test]
fn bindings_go_before_the_statement_in_order_or_into_a_new_scope() {
    let b = bind(SRC).unwrap();
    for want in [
        // Before the statement, in the order the values are computed; a call before the cast is bound first.
        "let h_u64: u64 = h as u64;\n    let x = h_u64.wrapping_mul(P);",
        "let len_f64: f64 = a.len() as f64;\n    let len_f64_2: f64 = b.len() as f64;\n    let r = len_f64 / len_f64_2;",
        "let len_u32: u32 = v.len() as u32;\n    v.push(len_u32);",
        "let g_v = g(1);\n    let g_u32: u32 = g(h) as u32;\n    let k2_u32: u32 = 2;\n    let s = g_v + g_u32 + k2_u32;",
        // A new scope where the value would not always be computed, or computed more than once.
        "c && { let h_u8: u8 = h as u8; h_u8 > 3 }",
        "n if { let n_u8: u8 = n as u8; n_u8 > 9 } => 2.0",
        "n => { let n_f64: f64 = n as f64; let k0_5_f64: f64 = 0.5;",
        "map(|i| { let i_f64: f64 = i as f64; i_f64 })",
        "while { let h_usize: usize = h as usize; i < h_usize } {",
        "else { let s_f64: f64 = s as f64; if s_f64 < w {",
        // A value that already initialises a `let` keeps that name.
        "let y: u16 = h as u16;",
        "let q: i64 = -1;",
    ] {
        assert!(b.contains(want), "missing:\n{want}\nin:\n{b}");
    }
}

#[test]
fn sites_that_cannot_be_bound_are_reported() {
    let src = r#"
pub fn f(x: u8, p: &u8, mut v: Vec<u8>) -> usize {
    let a = match x { 1u8 => 0, _ => 1 };
    let q = p as *const u8;
    v.iter_mut().next().map(|e| *e = 1);
    let n = [0u8; 4 as usize].len();
    v.first().copied().unwrap_or(0).max(a as u8) as usize + q as usize + n
}
"#;
    let (b, left) = bind_report(src).unwrap();
    let why: Vec<(&str, &str)> = left.iter().map(|l| (l.what, l.why)).collect();
    assert!(why.contains(&("literal", "in a pattern")), "{why:?}");
    assert!(why.contains(&("cast", "the target is not a primitive type")), "{why:?}");
    assert!(why.contains(&("cast", "in a constant context")), "{why:?}");
    // `v.first()...max(a as u8)`: the receiver chain runs first and is not a plain value.
    assert!(why.iter().any(|w| w.1.starts_with("something before it")), "{why:?}\n{b}");
}

#[test]
fn a_macro_call_that_states_a_type_is_marked_and_weaves_back() {
    let src = "pub fn f(x: u32) {\n    println!(\"{}\", x as u8);\n    let s = format!(\"{}\", 1u8);\n    println!(\"{}\", s);\n}\n";
    let (bare, json) = bare_rust::strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(bare.contains("rust! { println!(\"{}\", x as u8) };"), "{bare}");
    assert!(bare.contains("let s = rust! { format!(\"{}\", 1u8) };"), "{bare}");
    assert!(bare.contains("\n    println!(\"{}\", s);"), "{bare}");
    let woven = bare_rust::weave(&bare, &json).unwrap();
    assert_eq!(woven.trim(), src.trim());
}

#[test]
fn struct_fields_are_bound_in_the_order_they_run_and_shorthand_fields_stay() {
    let src = "pub struct S { a: String, m: String, n: usize }\npub fn f(x: &str, m: String) -> S {\n    S { a: x.to_string(), m, n: x.len() as usize }\n}\n";
    let b = bind(src).unwrap();
    assert!(
        b.contains("let to_string_v = x.to_string();\n    let len_usize: usize = x.len() as usize;\n    S { a: to_string_v, m, n: len_usize }"),
        "{b}"
    );
}
