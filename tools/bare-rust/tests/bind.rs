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
    // What is left: an argument a macro runs only sometimes, and a value that releases something at its end where the
    // statement might not run.
    let src = r#"
pub fn f(x: u8, c: bool, m: &std::sync::Mutex<Vec<u8>>) -> bool {
    debug_assert!(x as u32 > 0);
    c && m.lock().unwrap().len() as u8 > x
}
"#;
    let (b, left) = bind_report(src).unwrap();
    let why: Vec<(&str, &str)> = left.iter().map(|l| (l.what, l.why)).collect();
    assert_eq!(
        why,
        vec![
            ("cast", "in a macro argument that runs only sometimes (an assertion's message, `debug_assert!`)"),
            ("cast", "a value that releases something at its end, where the statement might not run"),
        ],
        "{b}"
    );
}

const LEFTOVERS: &str = r#"
use std::cell::RefCell;
thread_local!(static LOG: RefCell<Vec<String>> = RefCell::new(vec![]));
fn note<T: std::fmt::Debug>(s: &str, t: T) -> T { LOG.with(|l| l.borrow_mut().push(format!("{s}={t:?}"))); t }
type Word = u32;
fn double(x: u8) -> u8 { x * 2 }
pub fn f(x: u8, n: u64, a: Option<u8>, items: Vec<u8>) -> String {
    let w = x as Word + 1;
    let g = double as fn(u8) -> u8;
    let d = &x as &dyn std::fmt::Display;
    let mut v = vec![0u8; 4];
    v[note("left", n) as usize % 4] += note("right", 1);
    let mut it = items.into_iter();
    let Some(0u8) = it.next() else { return format!("no zero {}", LOG.with(|l| l.borrow_mut().join(","))) };
    let mut k = 0;
    while let Some(b) = it.nth(note("nth", n) as usize % 2) { k += b as u32; }
    let chained = if let Some(y) = a && y > note("chain", n) as u8 { y } else { 0 };
    format!("{w} {} {d} {v:?} {k} {chained} {}", g(x), LOG.with(|l| l.borrow_mut().join(",")))
}
"#;

#[test]
fn every_cast_target_a_compound_left_a_let_else_and_a_let_chain_are_bound() {
    let (b, left) = bind_report(LEFTOVERS).unwrap();
    assert!(left.is_empty(), "{left:?}\n{b}");
    for want in [
        // Any type the cast writes is a cast local's: an alias, a `fn` pointer, a reference.
        "let x_word: Word = x as Word;\n    let w = x_word + 1;",
        "let g: fn(u8) -> u8 = double as fn(u8) -> u8;",
        "let d: &dyn std::fmt::Display = &x as &dyn std::fmt::Display;",
        // The left of `+=` where the right has an effect: a block where the index was.
        "v[{ let note_usize: usize = note(\"left\", n) as usize; note_usize % 4 }] += note(\"right\", 1);",
        // A literal in a `let ... else` pattern is named as in a `match`.
        "let Some(_k0_u8 @ 0u8) = it.next() else",
        // A `while let` value and a part of a let chain: a block in its place.
        "while let Some(b) = { let note_usize_2: usize = note(\"nth\", n) as usize; it.nth(note_usize_2 % 2) } {",
        "if let Some(y) = a && { let note_u8: u8 = note(\"chain\", n) as u8; y > note_u8 } { y } else { 0 };",
    ] {
        assert!(b.contains(want), "missing:\n{want}\nin:\n{b}");
    }
    let (bare, json) = bare_rust::strip(LEFTOVERS, "demo 0.1.0", "src/lib.rs").unwrap();
    let locals = &json["holes"]["demo::f"]["locals"];
    assert_eq!(locals["x_word"], serde_json::json!({"type": "Word", "cast": true}));
    assert_eq!(locals["d"], serde_json::json!({"type": "ref<any std::fmt::Display>", "cast": true}));
    assert_eq!(locals["g"]["type"]["host"], "fn(u8) -> u8");
    assert_eq!(locals["_k0_u8 @ 0"], serde_json::json!({"literal": "u8"}));
    assert!(json["holes"]["demo::f"]["hints"].is_null(), "{json}");
    // A weave of one file derives no `mut` (a crate's weave does, through rustc).
    let woven = bare_rust::weave(&bare, &json).unwrap();
    let norm = |s: &str| bare_rust::rustfmt(&bare_rust::normalise_nomut(s).unwrap()).unwrap();
    assert_eq!(norm(&woven), norm(&b));
    // Let chains are Rust 2024.
    let main = "\nfn main() {\n    for n in [0u64, 1, 5] {\n        for a in [None, Some(0u8), Some(9)] {\n            for items in [vec![], vec![0u8, 3, 4, 5], vec![1u8]] {\n                LOG.with(|l| l.borrow_mut().clear());\n                println!(\"{}\", f(7, n, a, items));\n            }\n        }\n    }\n}\n";
    let run = |s: &str, name: &str| {
        let d = scratch(name);
        std::fs::write(d.join("m.rs"), format!("{s}{main}")).unwrap();
        let out = Command::new("rustc").args(["--edition", "2024", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
        assert!(out.status.success(), "{}\n{s}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
    };
    let (before, after) = (run(LEFTOVERS, "leftovers-source"), run(&b, "leftovers-bound"));
    assert_eq!(after, before);
    assert!(before.contains("right=1,left=") && before.contains("chain="), "{before}");
    assert_eq!(bind(&b).unwrap(), b);
}

const CONSTANTS: &str = r#"
pub const N: u32 = 4;
pub const W: usize = N as usize;
pub const H: usize = N as usize * 2 + 1usize;
pub static S: u64 = (N + 1) as u64;
pub const Z: [u8; 3] = [7u8; 3];
pub struct T;
impl T {
    pub const K: u16 = N as u16 + 1;
}
pub fn f(x: u32) -> usize {
    const L: u8 = N as u8;
    let a = [0u8; N as usize];
    let b = [x; N as usize + 1];
    a.len() + b.len() + L as usize + W + H + S as usize + T::K as usize + Z[0] as usize
}
"#;

#[test]
fn a_cast_in_a_constant_context_is_bound_in_place_or_carried_by_its_item() {
    let (b, left) = bind_report(CONSTANTS).unwrap();
    assert!(left.is_empty(), "{left:?}\n{b}");
    for want in [
        // The whole value of a `const` or `static`: the item is the name.
        "pub const W: usize = N as usize;",
        "pub static S: u64 = (N + 1) as u64;",
        "    const L: u8 = N as u8;",
        // Anywhere else in a constant context, a block in its place.
        "pub const H: usize = { let n_usize: usize = N as usize; let k1_usize: usize = 1; n_usize * 2 + k1_usize };",
        "pub const Z: [u8; 3] = { let k7_u8: u8 = 7; [k7_u8; 3] };",
        "pub const K: u16 = { let n_u16: u16 = N as u16; n_u16 + 1 };",
        "let a = [k0_u8; { let n_usize: usize = N as usize; n_usize }];",
        "let b = [x; { let n_usize_2: usize = N as usize; n_usize_2 + 1 }];",
    ] {
        assert!(b.contains(want), "missing:\n{want}\nin:\n{b}");
    }
    let (bare, json) = bare_rust::strip(CONSTANTS, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(bare.contains("const W = N;") && bare.contains("static S = N + 1;") && bare.contains("const L = N;"), "{bare}");
    let w = json["subjects"].as_array().unwrap().iter().find(|s| s["name"] == "demo::W").unwrap();
    assert_eq!(w["cast"], true);
    assert_eq!(json["holes"]["demo::H"]["locals"]["n_usize"], serde_json::json!({"type": "u64 as 'usize'", "cast": true}));
    let main = "\nfn main() {\n    println!(\"{}\", f(3));\n}\n";
    let run = |s: &str, name: &str| {
        let d = scratch(name);
        std::fs::write(d.join("m.rs"), format!("{s}{main}")).unwrap();
        let out = Command::new("rustc").args(["--edition", "2021", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
        assert!(out.status.success(), "{}\n{s}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
    };
    assert_eq!(run(&b, "constants-bound"), run(CONSTANTS, "constants-source"));
    let woven = bare_rust::weave(&bare, &json).unwrap();
    assert_eq!(bare_rust::rustfmt(&woven).unwrap(), bare_rust::rustfmt(&b).unwrap());
    assert_eq!(bind(&b).unwrap(), b);
}

const PATTERNS: &str = r#"
pub fn f(x: u8, y: &u16, v: &[u8], s: &str) -> u32 {
    let a = match x { 1u8 => 10, 2u8..=5u8 => 20, _ => 0 };
    let b = match (x, y) { (7u8, 9u16) => 1, (_, 3u16) => 2, _ => 0 };
    let c = match v.iter().next().copied() { Some(1u8) => 1, Some(2u8 | 3u8) => 2, Some(n @ 4u8..=6) => n as u32, _ => 0 };
    let d = match s.parse().unwrap_or(0) { 200u8 => 1, _ => 2 };
    let e = if let -1i8 = x as i8 { 1 } else { 0 };
    let g = match v.first() { Some(&9u8) => 1, _ => 0 };
    a + b + c + d + e + g
}
"#;

#[test]
fn a_pattern_literal_loses_a_suffix_its_value_states_or_gets_a_name() {
    let (b, left) = bind_report(PATTERNS).unwrap();
    assert!(left.is_empty(), "{left:?}\n{b}");
    for want in [
        // What it is matched against is declared with that type, through a tuple and a reference.
        "let a = match x { 1 => 10, 2..=5 => 20, _ => 0 };",
        "let b = match (x, y) { (7, 9) => 1, (_, 3) => 2, _ => 0 };",
        "let x_i8: i8 = x as i8;\n    let e = if let -1 = x_i8 { 1 } else { 0 };",
        // A name already over it carries it; every literal under the name takes the suffix.
        "Some(n @ 4u8..=6u8) =>",
        // Otherwise a name that does nothing else, over the literal, its range or its alternatives.
        "Some(_k1_u8 @ 1u8) => 1, Some(_k2_3_u8 @ (2u8 | 3u8)) => 2,",
        // Where the pattern's type decides what `parse` makes, dropping the suffix would change the program.
        "match s.parse().unwrap_or(0) { _k200_u8 @ 200u8 => 1, _ => 2 }",
        "Some(&_k9_u8 @ 9u8) => 1",
    ] {
        assert!(b.contains(want), "missing:\n{want}\nin:\n{b}");
    }
    let (bare, json) = bare_rust::strip(PATTERNS, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(bare.contains("{ _k200_u8 @ 200 => 1, _ => 2 }") && bare.contains("Some(n @ 4..=6)"), "{bare}");
    let locals = &json["holes"]["demo::f"]["locals"];
    assert_eq!(locals["_k2_3_u8 @ (2 | 3)"], serde_json::json!({"literal": "u8"}));
    assert_eq!(locals["n @ 4..=6"], serde_json::json!({"literal": "u8"}));
    assert!(json["holes"]["demo::f"]["hints"].is_null(), "{json}");
    let woven = bare_rust::weave(&bare, &json).unwrap();
    assert_eq!(bare_rust::rustfmt(&woven).unwrap(), bare_rust::rustfmt(&b).unwrap());
    let main = "\nfn main() {\n    for x in [0u8, 1, 3, 7, 255] {\n        for v in [vec![], vec![1u8], vec![3], vec![5], vec![9]] {\n            println!(\"{}\", f(x, &3, &v, \"200\") + f(x, &9, &v, \"7\"));\n        }\n    }\n}\n";
    let run = |s: &str, name: &str| {
        let d = scratch(name);
        std::fs::write(d.join("m.rs"), format!("{s}{main}")).unwrap();
        let out = Command::new("rustc").args(["--edition", "2021", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
        assert!(out.status.success(), "{}\n{s}", String::from_utf8_lossy(&out.stderr));
        assert!(!String::from_utf8_lossy(&out.stderr).contains("warning"), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
    };
    assert_eq!(run(&b, "patterns-bound"), run(PATTERNS, "patterns-source"));
    assert_eq!(bind(&b).unwrap(), b);
}

const RECEIVERS: &str = r#"
use std::cell::RefCell;
thread_local!(static LOG: RefCell<Vec<String>> = RefCell::new(vec![]));
fn note<T: std::fmt::Debug>(s: &str, t: T) -> T { LOG.with(|l| l.borrow_mut().push(format!("{s}={t:?}"))); t }
pub struct T { pub rows: Vec<Vec<u32>>, pub at: usize }
#[derive(Debug, Clone, Copy)]
pub struct W(u32);
impl std::ops::AddAssign<u32> for W { fn add_assign(&mut self, o: u32) { self.0 += o; } }
impl T {
    fn rows_mut(&mut self) -> &mut Vec<Vec<u32>> { note("rows_mut", 0); &mut self.rows }
    pub fn f(&mut self, v: &[u8], a: u32, n: u64) -> String {
        let m = v.iter().max().copied().unwrap_or(note("default", 0)).max(a as u8);
        let i = note("i", 1);
        self.rows[next_row(&self.at)].push(n as u32);
        self.rows_mut()[i].push(note("pushed", n) as u32 + 1);
        update(&mut self.rows[note("borrowed", 1)][0], self.at as u32);
        self.rows[note("left", 0)][0] = note("right", n) as u32;
        self.rows[note("left", 1) as usize][0] = note("right", 7);
        self.rows[0][0] += a as u32;
        // Which side of `+=` runs first depends on the types: here the left (`W`), there the right (`u32`).
        let mut w = [W(0), W(1)];
        w[note("w_left", 1)] += note("w_right", 3) as u32;
        self.rows[note("u_left", 0)][0] += note("u_right", 4) as u32;
        let s: String = self.rows.iter().map(|r| r.len()).sum::<usize>().to_string();
        format!("{m} {s} {:?} {w:?} {}", self.rows, LOG.with(|l| l.borrow_mut().join(",")))
    }
}
fn update(c: &mut u32, n: u32) { *c += note("update", n); }
fn next_row(at: &usize) -> usize { note("next_row", (at + 1) % 2) }
"#;

#[test]
fn a_method_receiver_that_is_a_value_is_bound_whole_and_a_place_keeps_its_place() {
    let (b, left) = bind_report(RECEIVERS).unwrap();
    assert!(left.is_empty(), "{left:?}\n{b}");
    for want in [
        // A value receiver runs first, so it is named first.
        "let unwrap_or_v = v.iter().max().copied().unwrap_or(note(\"default\", 0));\n        let a_u8: u8 = a as u8;\n        let m = unwrap_or_v.max(a_u8);",
        // A place keeps its place; what it computes on the way is named.
        "let next_row_v = next_row(&self.at);\n        let n_u32: u32 = n as u32;\n        self.rows[next_row_v].push(n_u32);",
        "let rows_mut_v = self.rows_mut();\n        let note_u32: u32 = note(\"pushed\", n) as u32;\n        rows_mut_v[i].push(note_u32 + 1);",
        "let note_v = note(\"borrowed\", 1);\n        let at_u32: u32 = self.at as u32;\n        update(&mut self.rows[note_v][0], at_u32);",
        // The right of an assignment runs before its left.
        "let note_u32_2: u32 = note(\"right\", n) as u32;\n        self.rows[note(\"left\", 0)][0] = note_u32_2;",
        "let note_v_2 = note(\"right\", 7);\n        let note_usize: usize = note(\"left\", 1) as usize;\n        self.rows[note_usize][0] = note_v_2;",
        // The other side of a compound assignment has no effect here.
        "let a_u32: u32 = a as u32;\n        self.rows[0][0] += a_u32;",
        // Here it has: the binding goes in the right's place, which runs where the right did.
        "w[note(\"w_left\", 1)] += { let note_u32_3: u32 = note(\"w_right\", 3) as u32; note_u32_3 };",
    ] {
        assert!(b.contains(want), "missing:\n{want}\nin:\n{b}");
    }
    let main = "\nfn main() {\n    let mut t = T { rows: vec![vec![1], vec![2]], at: 0 };\n    println!(\"{}\", t.f(&[3, 9], 300, 5));\n}\n";
    let run = |s: &str, name: &str| {
        let d = scratch(name);
        std::fs::write(d.join("m.rs"), format!("{s}{main}")).unwrap();
        let out = Command::new("rustc").args(["--edition", "2021", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
        assert!(out.status.success(), "{}\n{s}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
    };
    let (before, after) = (run(RECEIVERS, "receivers-source"), run(&b, "receivers-bound"));
    assert_eq!(after, before);
    assert!(before.contains("right=5,left=0") && before.contains("right=7,left=1"), "{before}");
    assert!(before.contains("w_left=1,w_right=3") && before.contains("u_right=4,u_left=0"), "{before}");
    assert_eq!(bind(&b).unwrap(), b);
}

#[test]
fn a_macro_call_that_states_a_type_is_marked_and_weaves_back() {
    // A standard macro's arguments are bound as a call's are; a macro whose syntax the tool does not know is kept as
    // Rust text.
    let src = "macro_rules! twice {\n    ($e:expr) => {\n        $e * 2\n    };\n}\npub fn f(x: u32) -> u8 {\n    println!(\"{}\", x as u8);\n    let s = format!(\"{}\", 1u8);\n    println!(\"{}\", s);\n    twice!(x as u8)\n}\n";
    let (bare, json) = bare_rust::strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(bare.contains("let x_u8 = x;\n    println!(\"{}\", x_u8);"), "{bare}");
    assert!(bare.contains("let k1_u8 = 1;\n    let s = format!(\"{}\", k1_u8);"), "{bare}");
    assert!(bare.contains("\n    rust! { twice!(x as u8) }\n"), "{bare}");
    let woven = bare_rust::weave(&bare, &json).unwrap();
    assert_eq!(woven.trim(), bind(src).unwrap().trim());
}

const MACROS: &str = r#"
pub fn f(x: u32, v: &mut Vec<u64>, c: bool, out: &mut String) -> String {
    use std::fmt::Write;
    let mut log = vec![];
    let mut push = |s: &str, n: u64| { log.push(format!("{s}{n}")); n };
    println!("{}", x as u8);
    let s = format!("{} {} {}", push("a", 1), v.iter().sum::<u64>() as u8, push("b", 2));
    assert_eq!(x as u64 + 0 * push("c", 3), x as u64, "{}", push("never", 4) as u32);
    assert!(c || x > 0, "{:?}", v.iter().map(|n| n + push("never", 5)).collect::<Vec<_>>());
    debug_assert!(push("debug", 6) as u8 >= 6);
    let w = vec![push("d", 7) as u8; x as usize % 3];
    let w2 = vec![x as u8, push("e", 8) as u8];
    writeln!(out, "{x} {n} {m}", m = push("f", 9), n = v.len() as u16).unwrap();
    v.push(x as u64);
    drop(push);
    format!("{s}|{}|{}|{}|{}", w.len(), w2.len(), log.join(","), out)
}
"#;

#[test]
fn macro_arguments_are_bound_in_the_order_they_run_and_only_where_they_run() {
    let (b, left) = bind_report(MACROS).unwrap();
    for want in [
        // In order with what runs before it (a turbofish inside a cast goes with the cast's binding).
        "let push_v = push(\"a\", 1);\n    let sum_u8: u8 = v.iter().sum::<u64>() as u8;",
        "let x_u64: u64 = x as u64;\n    let value_v = x_u64 + 0 * push(\"c\", 3);\n    let x_u64_2: u64 = x as u64;\n    assert_eq!(value_v, x_u64_2,",
        "let w = vec![push_u8; x_usize % 3];",
        "writeln!(out, \"{x} {n} {m}\", m = push_v_2, n = len_u16).unwrap();",
        // An assertion's message runs only when it fails, and `debug_assert!` only in a debug build: a binding
        // before the macro would run it always, and one inside it stays in the macro. Left, and reported.
        "\"{}\", push(\"never\", 4) as u32);",
        "debug_assert!(push(\"debug\", 6) as u8 >= 6);",
    ] {
        assert!(b.contains(want), "missing:\n{want}\nin:\n{b}");
    }
    let why: Vec<(&str, &str)> = left.iter().map(|l| (l.what, l.why)).collect();
    let sometimes = "in a macro argument that runs only sometimes (an assertion's message, `debug_assert!`)";
    assert_eq!(why, vec![("cast", sometimes), ("macro_arg", sometimes), ("cast", sometimes)], "{b}");
    let main = "\nfn main() {\n    for (x, c) in [(0u32, true), (3, false), (7, true)] {\n        let (mut v, mut out) = (vec![1u64, 2], String::new());\n        println!(\"{}\", f(x, &mut v, c, &mut out));\n    }\n}\n";
    let run = |s: &str, name: &str| {
        let d = scratch(name);
        std::fs::write(d.join("m.rs"), format!("{s}{main}")).unwrap();
        let out = Command::new("rustc").args(["--edition", "2021", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
        assert!(out.status.success(), "{}\n{s}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
    };
    assert_eq!(run(&b, "macros-bound"), run(MACROS, "macros-source"));
    assert_eq!(bind(&b).unwrap(), b);
}

#[test]
fn a_json_value_is_bound_in_the_order_it_is_built() {
    let src = "pub fn f(x: u32, v: &[u64]) -> serde_json::Value {\n    serde_json::json!({ \"a\": x as u8, \"b\": [1, v.iter().collect::<Vec<_>>()], \"c\": { \"d\": g() }, \"e\": v.len() as u64 })\n}\n";
    // A path before the macro's name stays.
    let b = bind(src).unwrap();
    assert!(
        b.contains("let x_u8: u8 = x as u8;\n    let collect_v = v.iter().collect::<Vec<_>>();\n    let g_v = g();\n    let len_u64: u64 = v.len() as u64;\n    serde_json::json!({ \"a\": x_u8, \"b\": [1, collect_v], \"c\": { \"d\": g_v }, \"e\": len_u64 })"),
        "{b}"
    );
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

#[test]
fn closures_with_typed_parameters_get_a_name_and_return_types_go_onto_a_local() {
    let src = "pub fn f(v: &mut Vec<String>, n: usize) -> usize {\n    v.retain(|s: &String| s.len() > n);\n    v.iter().map(|s| -> usize { s.len() * 2 }).sum()\n}\n";
    let b = bind(src).unwrap();
    assert!(b.contains("let retain_fn = |s: &String| s.len() > n;\n    v.retain(retain_fn);"), "{b}");
    assert!(b.contains("map(|s| { let out: usize = { s.len() * 2 }; out })"), "{b}");
    let (bare, json) = bare_rust::strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    assert_eq!(json["holes"]["demo::f"]["locals"]["retain_fn.s"], "ref<text as 'String'>", "{bare}");
    assert_eq!(json["holes"]["demo::f"]["locals"]["out"], "u64 as 'usize'");
}

#[test]
fn a_value_that_releases_at_its_end_keeps_its_end_where_it_was() {
    // `Guard` releases at its end (an `impl Drop`); `log` sees whether it is still held. Binding the cast must not
    // end the guard before `log` runs: the guard is named, in a block that ends where the statement did.
    let src = r#"
use std::cell::Cell;
pub struct Guard<'a>(&'a Cell<bool>);
impl Drop for Guard<'_> {
    fn drop(&mut self) { self.0.set(false); }
}
impl Guard<'_> {
    pub fn len(&self) -> usize { 3 }
}
pub fn acquire(c: &Cell<bool>) -> Guard<'_> { c.set(true); Guard(c) }
pub fn try_acquire(c: &Cell<bool>) -> Option<Guard<'_>> { c.set(true); Some(Guard(c)) }
fn log(c: &Cell<bool>, n: u32) -> String { format!("{n} held={}", c.get()) }
pub fn f(c: &Cell<bool>) -> String {
    let a = log(c, acquire(c).len() as u32);
    let b = log(c, try_acquire(c).unwrap().len() as u32 + 1);
    log(c, acquire(c).len() as u32) + &a + &b
}
"#;
    let b = bind(src).unwrap();
    assert!(b.contains("let a = { let acquire_guard = acquire(c); let len_u32: u32 = acquire_guard.len() as u32; log(c, len_u32) };"), "{b}");
    assert!(b.contains("let b = { let try_acquire_guard = try_acquire(c).unwrap();"), "{b}");
    let main = "\nfn main() { let c = Cell::new(false); println!(\"{}\", f(&c)); }\n";
    let run = |s: &str, name: &str| {
        let d = scratch(name);
        std::fs::write(d.join("m.rs"), format!("{s}{main}")).unwrap();
        let out = Command::new("rustc").args(["--edition", "2021", "-o"]).arg(d.join("m")).arg(d.join("m.rs")).output().unwrap();
        assert!(out.status.success(), "{}\n{s}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(Command::new(d.join("m")).output().unwrap().stdout).unwrap()
    };
    let (before, after) = (run(src, "guard-source"), run(&b, "guard-bound"));
    assert_eq!(before, after);
    assert!(before.contains("held=true") && !before.contains("held=false"), "{before}");
}

#[test]
fn a_standard_library_lock_guard_is_kept_to_the_end_of_its_statement() {
    // `Mutex::lock` and `MutexGuard` come from the library file: the guard releases the lock at its end.
    let src = "use std::sync::Mutex;\nfn log(m: &Mutex<Vec<u8>>, n: u32) -> String { format!(\"{n} held={}\", m.try_lock().is_err()) }\npub fn f(m: &Mutex<Vec<u8>>) -> String {\n    log(m, m.lock().unwrap().len() as u32)\n}\n";
    let b = bind(src).unwrap();
    assert!(b.contains("{ let lock_guard = m.lock().unwrap(); let len_u32: u32 = lock_guard.len() as u32; log(m, len_u32) }"), "{b}");
}
