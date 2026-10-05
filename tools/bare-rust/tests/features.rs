//! The constructs the corpora use, and a few they do not, each through strip and weave.
use bare_rust::json::canonical;
use bare_rust::{first_error, normalise_nomut, parse_bare, rustfmt, strip, weave};
use serde_json::Value;

fn schema_errors(j: &Value) -> Vec<String> {
    let schema: Value = serde_json::from_str(include_str!("../schema/y-constraints-0.schema.json")).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    v.iter_errors(j).map(|e| format!("{e} at {}", e.instance_path())).collect()
}

/// Strip, weave, and check 1 and 2 on one source. Returns the bare body and the constraint file.
fn roundtrip(src: &str) -> (String, Value) {
    let (b, j) = strip(src, "demo 0.1.0", "src/lib.rs").unwrap_or_else(|e| panic!("strip: {e}"));
    assert!(first_error(&parse_bare(&b)).is_none(), "bare does not parse:\n{b}");
    assert_eq!(schema_errors(&j), Vec::<String>::new());
    let w = weave(&b, &j).unwrap_or_else(|e| panic!("weave: {e}\n{b}"));
    let (b2, j2) = strip(&w, "demo 0.1.0", "src/lib.rs").unwrap_or_else(|e| panic!("strip(woven): {e}\n{w}"));
    assert_eq!(b2, b, "check 1 (bare)");
    assert_eq!(canonical(&j2), canonical(&j), "check 1 (json)");
    let norm = |s: &str| rustfmt(&normalise_nomut(s).unwrap()).unwrap();
    assert_eq!(norm(&w), norm(src), "check 2");
    (b, j)
}

fn subject<'a>(j: &'a Value, name: &str) -> &'a Value {
    j["subjects"].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("no subject {name}"))
}

#[test]
fn types_and_items() {
    let src = r#"
use std::collections::HashMap;

#[derive(Debug)]
pub enum Shape {
    Unit,
    Pair(u32, f64),
    Named { w: u32, h: Option<&'static str> },
}

pub(crate) struct Wrapper<'a, T: Clone>
where
    T: Default,
{
    pub name: &'a str,
    items: Vec<T>,
    index: HashMap<String, usize>,
}

pub struct Tup(pub u8, String);

struct Unit;

pub trait Visitor<T>: Send + Sync {
    type Out;
    const N: usize;
    fn visit(&mut self, t: &T) -> Self::Out;
    fn provided(&self) -> u32 {
        1
    }
}

impl<'a, T: Clone + Default> Visitor<T> for Wrapper<'a, T> {
    type Out = u8;
    const N: usize = 2;
    fn visit(&mut self, t: &T) -> Self::Out {
        0
    }
}

pub const LIMIT: usize = 10;
static mut COUNTER: u32 = 0;
pub static NAME: &str = "x";
pub type Alias<T> = Vec<T>;

mod inner {
    pub(crate) fn f() {}
    pub(super) fn g(x: &mut u8) -> &mut u8 {
        x
    }
}

fn lifetimes<'a, 'b>(x: &'a str, y: &'b str) -> &'a str {
    x
}
fn elid<'a>(x: &'a str) -> &'a str {
    x
}
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
unsafe fn uf() {}
async fn af() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
extern "C" {
    fn ext(x: *const u8) -> i32;
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("enum Shape") && b.contains("Pair(_, _)") && b.contains("Named { w, h }"), "{b}");
    assert_eq!(subject(&j, "demo::Shape")["variants"][1]["fields"][1]["type"], "f64");
    assert_eq!(subject(&j, "demo::Shape")["variants"][2]["fields"][1]["type"], "ref<text>?");
    assert_eq!(subject(&j, "demo::LIMIT")["type"], "u64 as 'usize'");
    assert_eq!(subject(&j, "demo::longest")["generics"][0]["name"], "'a");
    assert!(subject(&j, "demo::elid")["generics"].is_null(), "elidable lifetimes are dropped");
    assert_eq!(subject(&j, "demo::inner::g")["visible"], "path");
    assert_eq!(subject(&j, "demo::Wrapper")["generics"][1]["where"][0], "Default");
    assert_eq!(subject(&j, "demo::af")["result"]["fails"]["host"], "Box<dyn std::error::Error>");
}

#[test]
fn bodies_and_holes() {
    let src = r#"
fn body_stuff(mut a: u8, (mut b, c): (u8, u8)) -> u32 {
    let mut x: u32 = a as u32 + 1u8 as u32;
    let v = (0..3).collect::<Vec<u8>>();
    let w = Vec::<u8>::new();
    let f = |mut p: u8, q| -> u8 { p + q };
    let z = 1_000_u64 + 0xffu64 + 2.5f32 as u64;
    let x: u8 = 1;
    let (mut m, n): (u8, u8) = (1, 2);
    fn nested(n: u32) -> u32 {
        n
    }
    struct Local {
        a: u8,
    }
    assert_eq!(x as u32, 1u32);
    nested(x as u32)
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("as _") && b.contains("collect::<Vec<_>>()") && b.contains("Vec::<_>::new()"), "{b}");
    let h = &j["holes"]["demo::body_stuff"];
    assert_eq!(h["locals"]["x"], "u32");
    assert_eq!(h["locals"]["x#2"], "u8");
    assert_eq!(h["locals"]["(m, n)"], "(u8, u8)");
    assert_eq!(h["hints"].as_array().unwrap().iter().filter(|x| x["kind"] == "literal").count(), 4);
    assert!(j["subjects"].as_array().unwrap().iter().any(|s| s["name"] == "demo::body_stuff::nested"));
    assert!(j["subjects"].as_array().unwrap().iter().any(|s| s["name"] == "demo::body_stuff::Local"));
}

#[test]
fn a_path_type_may_hide_a_lifetime() {
    // `Node` may carry a lifetime the text does not show, so `'a` here is not the only input lifetime and is not
    // elided; `Plain` is a type parameter's neighbour: only `&'a str` is a lifetime, so `'a` can go.
    let src = r#"
struct Node<'t>(&'t str);
fn text<'a>(n: Node, src: &'a str) -> &'a str {
    src
}
fn plain<'a>(n: usize, src: &'a str) -> &'a str {
    src
}
fn typed<'a, T>(n: Vec<T>, src: &'a str) -> &'a str {
    src
}
"#;
    let (_, j) = roundtrip(src);
    assert_eq!(subject(&j, "demo::text")["generics"][0]["name"], "'a");
    assert!(subject(&j, "demo::plain")["generics"].is_null());
    assert!(subject(&j, "demo::typed")["generics"].as_array().is_some_and(|g| g.iter().all(|x| x["name"] != "'a")));
}

#[test]
fn closure_sites_count_underscore_parameters() {
    // `|_|` has a `_` that tree-sitter-rust leaves anonymous and the bare grammar names: both count it, so the
    // ordinals of the hints after it agree on both sides.
    let src = r#"
fn f(v: Vec<u8>) -> usize {
    let a = v.iter().filter(|_| true).count();
    let g = |x: u8| x;
    a + g(1) as usize
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("|_| true") && b.contains("|x| x"), "{b}");
    let hints = j["holes"]["demo::f"]["hints"].as_array().unwrap();
    assert!(hints.iter().any(|h| h["kind"] == "closure_param" && h["n"] == 1), "{hints:?}");
}

#[test]
fn impl_headers_and_duplicates() {
    let src = r#"
struct W<T>(T);
impl<T: Clone> W<T> {
    fn get(&self) -> &T {
        &self.0
    }
}
impl<'b> IntoIterator for &W<&'b str> {
    type Item = char;
    type IntoIter = std::str::Chars<'b>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.chars()
    }
}
impl From<u8> for W<u8> {
    fn from(x: u8) -> Self {
        W(x)
    }
}
#[cfg(unix)]
fn os() -> u8 {
    1
}
#[cfg(not(unix))]
fn os() -> u8 {
    2
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("impl From for W") || b.contains("impl From for W {"), "{b}");
    assert_eq!(subject(&j, "demo::W as From")["trait"], "From<u8>");
    assert_eq!(subject(&j, "demo::W as From")["self"], "W<u8>");
    subject(&j, "demo::os");
    subject(&j, "demo::os#2");
}

#[test]
fn unsupported_is_refused_with_a_reason() {
    let e = strip("union U { a: u8 }", "demo 0.1.0", "src/lib.rs").unwrap_err();
    assert!(e.starts_with("union:"), "{e}");
    let e = strip("fn f(x: u8", "demo 0.1.0", "src/lib.rs").unwrap_err();
    assert!(e.starts_with("parse-error:"), "{e}");
    let e = strip("fn f<const N: usize>() {}", "demo 0.1.0", "src/lib.rs").unwrap_err();
    assert!(e.starts_with("const-generics:"), "{e}");
}

#[test]
fn woven_form_is_not_bare() {
    assert!(parse_bare("pub fn get<'a>(&'a self, key: &Key) -> Option<&'a Entry> { x }").root_node().has_error());
    assert!(parse_bare("fn f(x: u8) {}").root_node().has_error());
    assert!(parse_bare("struct S<T> { a: T }").root_node().has_error());
    assert!(!parse_bare("enum E { A(_), B { x } }").root_node().has_error());
}

#[test]
fn golden_json_validates() {
    let j: Value = serde_json::from_str(include_str!("golden/cache.constraints.json")).unwrap();
    assert_eq!(schema_errors(&j), Vec::<String>::new());
}

#[test]
fn a_comment_inside_an_erased_production_is_refused() {
    // Each of these puts a comment inside a span that strip cuts; a comment elsewhere survives.
    let cases = [
        "struct S { a: Vec</* c */ u8> }",
        "fn f(a: Vec</* c */ u8>) {}",
        "fn f() -> // c4\n u8 { 1 }",
        "fn f() /* c5 */ -> u8 { 1 }",
        "fn f<T /* c6 */: Clone>(t: T) {}",
        "fn f<T>(t: T) -> u8 // c8\nwhere T: Clone { 1 }",
        "fn f<T>(t: T)\nwhere // c8\n T: Clone {}",
        "fn f() { let x /* c */: u8 = 1; }",
        "fn f() { let y = 1 as Option</* c */ u8>; }",
    ];
    for src in cases {
        let e = strip(src, "demo 0.1.0", "src/lib.rs").unwrap_err();
        assert!(e.starts_with("comment inside an erased production: at "), "{src}: {e}");
    }
    // Comments that are not inside a cut are kept in the bare body.
    let (b, _) = strip("// a\nfn f(/* p */ x: u8) -> u8 { /* body */ x }\n", "demo 0.1.0", "src/lib.rs").unwrap();
    for c in ["// a", "/* p */", "/* body */"] {
        assert!(b.contains(c), "{c} lost: {b}");
    }
}

#[test]
fn use_visibility_moves_to_the_file() {
    let src = "pub use a::b;\npub(crate) use c::{d, e};\nuse f::g;\nmod m {\n    pub(super) use super::x as y;\n}\n";
    let (b, j) = roundtrip(src);
    assert!(!b.contains("pub"), "{b}");
    assert!(b.contains("use a::b;") && b.contains("use f::g;"), "{b}");
    let s = subject(&j, "demo::use a::b");
    assert_eq!((s["kind"].as_str(), s["visible"].as_str()), (Some("use"), Some("public")));
    assert_eq!(subject(&j, "demo::use c::{d, e}")["visible"], "crate");
    let m = subject(&j, "demo::m::use super::x as y");
    assert_eq!((m["visible"].as_str(), m["visible_path"].as_str()), (Some("path"), Some("super")));
    assert_eq!(j["subjects"].as_array().unwrap().len(), 3, "a plain `use` is not a subject");
    // The bare grammar has no visibility on `use`; weave prints it from the file, and a missing entry means private.
    assert!(parse_bare("pub use a::b;").root_node().has_error());
    let mut j2 = j.clone();
    j2["subjects"].as_array_mut().unwrap().remove(0);
    let w = weave(&b, &j2).unwrap();
    assert!(w.contains("\nuse a::b;") || w.starts_with("use a::b;"), "{w}");
}

#[test]
fn stale_entries_are_reported_as_unbound() {
    use bare_rust::weave::{weave_traced, Unbound};
    let src = "fn f(a: u8) -> u8 {\n    let x: u16 = 1;\n    let y: u32 = 2;\n    (a as u32 + y) as u8\n}\nfn g() {}\n";
    let (b, mut j) = strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    // A renamed local, one hint too many, a duplicate of a hint, a subject with no item, holes of a function that is gone.
    let h = &mut j["holes"]["demo::f"];
    let ty = h["locals"]["x"].take();
    h["locals"].as_object_mut().unwrap().remove("x");
    h["locals"]["z"] = ty;
    h["hints"].as_array_mut().unwrap().push(serde_json::json!({"kind": "cast", "n": 5, "type": "u64"}));
    let dup = h["hints"][0].clone();
    h["hints"].as_array_mut().unwrap().push(dup);
    let g = subject(&j, "demo::g").clone();
    let mut gone = g.clone();
    gone["name"] = "demo::h".into();
    j["subjects"].as_array_mut().unwrap().push(gone);
    j["holes"]["demo::k"] = serde_json::json!({"locals": {"q": "u8"}});
    let w = weave_traced(&b, &j).unwrap();
    let u = |what, subject: &str, name: &str| Unbound { what, subject: subject.into(), name: name.into() };
    assert_eq!(
        w.unbound,
        vec![
            u("unbound local", "demo::f", "z"),
            u("unbound hint", "demo::f", "cast #5"),
            u("unbound hint", "demo::f", "cast #0"),
            u("unbound subject", "demo::h", ""),
            u("unbound subject", "demo::k", ""),
        ]
    );
    assert_eq!(w.unbound[0].show(), "unbound local `z` of `demo::f`");
    assert_eq!(w.unbound[3].show(), "unbound subject `demo::h`");
    // Nothing stale: no warning.
    let (b, j) = strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(weave_traced(&b, &j).unwrap().unbound.is_empty());
}

#[test]
fn mut_self_and_mut_bindings_count_alike() {
    let src = "struct S(u8);\nimpl S {\n    fn f(mut self, mut c: u8) -> u8 { self.0 += c; c += 1; c }\n    fn g(&mut self) {}\n}\n";
    assert_eq!(bare_rust::count_mut(src), 2, "`mut self` and `mut c`, not `&mut self`");
    let n = normalise_nomut(src).unwrap();
    assert_eq!(bare_rust::count_mut(&n), 0, "{n}");
    assert!(n.contains("fn f(self, c: u8)") && n.contains("&mut self"), "{n}");
    let (b, j) = strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(b.contains("fn f(self, c)"), "{b}");
    assert_eq!(subject(&j, "demo::S::g")["params"][0]["access"]["rights"], "write");
}

#[test]
fn slices_and_arrays_are_lists() {
    let src = r#"
const N: usize = 4;

pub fn f(a: &[u8], b: &mut [Vec<u8>], c: [u32; 3], d: &[[u8; N]; 2]) -> [u8; 2] {
    [a[0], c[0] as u8]
}
"#;
    let (_, j) = roundtrip(src);
    let s = j.to_string();
    assert!(s.contains("ref<list<u8> as 'std::primitive::slice'>"), "{s}");
    assert!(s.contains("list<u32> where size(value) == 3 as 'std::primitive::array'"), "{s}");
    assert!(s.contains("size(value) == N"), "{s}");
    assert!(!s.contains("\"form\":\"slice\"") && !s.contains("\"form\":\"array\""), "{s}");
}
