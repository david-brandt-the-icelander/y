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
    assert_eq!(subject(&j, "demo::Shape")["variants"][2]["fields"][1]["type"], "ref<target type=string>?");
    assert_eq!(subject(&j, "demo::LIMIT")["type"], "u64 as 'usize'");
    assert_eq!(subject(&j, "demo::longest")["generics"][0]["name"], "'a");
    assert!(subject(&j, "demo::elid")["generics"].is_null(), "elidable lifetimes are dropped");
    assert_eq!(subject(&j, "demo::inner::g")["visible"], "path");
    assert_eq!(subject(&j, "demo::Wrapper")["generics"][1]["where"][0], "Default");
    assert_eq!(subject(&j, "demo::af")["result"]["fails"], "any std::error::Error as 'Box'");
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
    // Casts and suffixed literals are bound (`bind`), in the arguments of a standard macro too.
    // A turbofish that states a type is a realization the body names (`collect_vec_u8`, `VecU8`).
    assert!(!b.contains("as _") && !b.contains("::<") && b.contains("(0..3).collect_vec_u8()") && b.contains("VecU8::new()"), "{b}");
    assert!(b.contains("let a_u32 = a;") && b.contains("let x = a_u32 + k1_u8_u32;"), "{b}");
    assert!(b.contains("let x_u32 = x;\n    let k1_u32 = 1;\n    assert_eq!(x_u32, k1_u32);"), "{b}");
    let h = &j["holes"]["demo::body_stuff"];
    assert_eq!(h["locals"]["x"], "u32");
    assert_eq!(h["locals"]["x#2"], "u8");
    assert_eq!(h["locals"]["(m, n)"], "(u8, u8)");
    assert_eq!(h["locals"]["a_u32"], serde_json::json!({"type": "u32", "cast": true}));
    assert_eq!(h["locals"]["k1_u8"], "u8");
    assert_eq!(h["locals"]["k2_5_f32"], "f32");
    assert!(h["hints"].is_null(), "{h}");
    let r = &j["realizations"];
    assert_eq!(r["collect_vec_u8"], serde_json::json!({"template": "<_>::collect", "args": ["list<element type=u8> as 'Vec'"]}));
    assert_eq!(r["VecU8"], serde_json::json!({"template": "std::vec::Vec", "args": ["u8"]}));
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
fn a_lifetime_inside_an_impl_trait_parameter_keeps_its_name() {
    // Rust has no elided lifetimes inside an `impl Trait` parameter (E0658), and one elided inside `Fn(..)` belongs
    // to that bound rather than to the function, so `'a` stays where it is written there.
    let src = r#"
struct W(u32);
fn total<'a>(ws: impl Iterator<Item = &'a W>) -> u32 {
    ws.map(|w| w.0).sum()
}
fn some<'a>(ws: Option<impl Into<&'a W>>) -> bool {
    ws.is_some()
}
fn call<'a>(f: impl Fn(&'a W) -> u32, w: &W) -> u32 {
    0
}
"#;
    let (_, j) = roundtrip(src);
    assert_eq!(subject(&j, "demo::total")["generics"][0]["name"], "'a");
    assert_eq!(subject(&j, "demo::total")["params"][0]["type"]["host"], "impl Iterator<Item = &'a W>");
    assert_eq!(subject(&j, "demo::some")["generics"][0]["name"], "'a");
    assert_eq!(subject(&j, "demo::call")["generics"][0]["name"], "'a");
}

#[test]
fn closure_parameters_are_keyed_by_the_closure_name() {
    // `|_|` has a `_` that tree-sitter-rust leaves anonymous and the bare grammar names: both count it, so the
    // ordinals of the sites after it agree on both sides. A typed parameter of a named closure is keyed by the name.
    let src = r#"
fn f(v: Vec<u8>) -> usize {
    let a = v.iter().filter(|_| true).count();
    let g = |x: u8| x;
    a + g(1) as usize
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("|_| true") && b.contains("|x| x"), "{b}");
    assert_eq!(j["holes"]["demo::f"]["locals"]["g.x"], "u8");
    assert!(j["holes"]["demo::f"]["hints"].is_null(), "{}", j["holes"]);
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
    let e = strip("struct U;\nimpl !Send for U {}", "demo 0.1.0", "src/lib.rs").unwrap_err();
    assert!(e.starts_with("negative-impl:"), "{e}");
    let e = strip("fn f(x: u8", "demo 0.1.0", "src/lib.rs").unwrap_err();
    assert!(e.starts_with("parse-error:"), "{e}");
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
    let src = "fn f(a: u8) -> u8 {\n    let x: u16 = 1;\n    let y: u32 = 2;\n    (0..a).sum::<u8>() + y as u8\n}\nfn g() {}\n";
    let (b, mut j) = strip(src, "demo 0.1.0", "src/lib.rs").unwrap();
    // A renamed local, a hint with no site, a realization the body no longer names, a subject with no item, holes of a
    // function that is gone.
    assert_eq!(j["realizations"]["sum_u8"], serde_json::json!({"template": "<_>::sum", "args": ["u8"]}));
    j["realizations"]["sum_u64"] = serde_json::json!({"template": "<_>::sum", "args": ["u64"]});
    let h = &mut j["holes"]["demo::f"];
    let ty = h["locals"]["x"].take();
    h["locals"].as_object_mut().unwrap().remove("x");
    h["locals"]["z"] = ty;
    h["hints"] = serde_json::json!([{"kind": "targ", "n": 5, "type": "u64"}]);
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
            u("unbound hint", "demo::f", "targ #5"),
            u("unbound subject", "demo::h", ""),
            u("unbound subject", "demo::k", ""),
            u("unbound realization", "demo", "sum_u64"),
        ]
    );
    assert_eq!(w.unbound[0].show(), "unbound local `z` of `demo::f`");
    assert_eq!(w.unbound[2].show(), "unbound subject `demo::h`");
    assert_eq!(w.unbound[4].show(), "unbound realization `sum_u64` of `demo`");
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
fn a_pointer_is_ptr_whose_access_says_whether_it_writes() {
    // `*const T` reads, with no guarantee about other paths; `*mut T` is the same type with that constraint absent.
    // A pointer is no reference: it gives no lifetime for elision, and a field holds it as it holds any value.
    let src = r#"
pub struct Buf {
    pub at: *const u8,
    pub to: *mut u8,
    pub all: Vec<*const u8>,
}
pub fn get(b: &Buf, p: *const u8, n: usize) -> *mut u8 {
    let x: *const u8 = p;
    let y = p as *const u16;
    let z = b as *const Buf as usize + n;
    b.to
}
pub fn first(p: *const u8, s: &[u8]) -> &u8 {
    &s[0]
}
pub fn both(q: *mut *const u8) {}
pub const NULL: *const u8 = std::ptr::null();
"#;
    let (b, j) = roundtrip(src);
    let fields = &subject(&j, "demo::Buf")["fields"];
    assert_eq!(fields[0]["type"], "ptr<target type=u8>");
    assert_eq!(fields[0]["access"], serde_json::json!({"rights": "read"}));
    assert_eq!(fields[1]["type"], "ptr<target type=u8>");
    assert!(fields[1]["access"].is_null());
    assert_eq!(fields[2]["type"], "list<element type=ptr<target type=u8>> as 'Vec'");
    assert_eq!(subject(&j, "demo::get")["result"], serde_json::json!({"type": "ptr<target type=u8>"}));
    assert_eq!(subject(&j, "demo::NULL")["access"], serde_json::json!({"rights": "read"}));
    // Two pointers of different access in one slot have no one access to carry.
    assert_eq!(subject(&j, "demo::both")["params"][0]["type"]["form"], "mixed_refs");
    // A cast to a pointer is a cast local, with its access.
    let locals = &j["holes"]["demo::get"]["locals"];
    assert_eq!(locals["x"], serde_json::json!({"type": "ptr<target type=u8>", "access": {"rights": "read"}}));
    assert_eq!(locals["y"], serde_json::json!({"type": "ptr<target type=u16>", "access": {"rights": "read"}, "cast": true}));
    assert_eq!(locals["b_ptr"], serde_json::json!({"type": "ptr<target type=Buf>", "access": {"rights": "read"}, "cast": true}));
    assert!(b.contains("let b_ptr = b;\n    let b_ptr_usize = b_ptr;\n    let z = b_ptr_usize + n;"), "{b}");
}

#[test]
fn an_impl_for_a_primitive_type_keeps_its_head() {
    let src = "pub trait Unsigned {\n    fn bits(self) -> u32;\n}\nimpl Unsigned for u128 {\n    fn bits(self) -> u32 {\n        128\n    }\n}\nimpl Unsigned for &str {\n    fn bits(self) -> u32 {\n        8\n    }\n}\n";
    let (b, j) = roundtrip(src);
    assert!(b.contains("impl Unsigned for u128 {") && b.contains("impl Unsigned for str {"), "{b}");
    assert_eq!(subject(&j, "demo::u128 as Unsigned::bits")["params"][0]["type"], "u128");
}

#[test]
fn a_union_is_a_type_with_fields_and_an_impl_may_be_for_a_pointer() {
    let src = "pub union Word {\n    pub n: u32,\n    b: [u8; 4],\n}\npub trait Pointer {\n    fn is_null(self) -> bool;\n}\nimpl<T> Pointer for *const T {\n    fn is_null(self) -> bool {\n        self.is_null()\n    }\n}\nimpl<T> Pointer for *mut T {\n    fn is_null(self) -> bool {\n        self.is_null()\n    }\n}\n";
    let (b, j) = roundtrip(src);
    assert!(b.contains("union Word {\n    n,\n    b,\n}"), "{b}");
    assert_eq!(subject(&j, "demo::Word")["fields"][0]["type"], "u32");
    // The self type of an impl has no slot for an access: a `*const` there is host text, as a `&mut` is.
    assert_eq!(subject(&j, "demo::T as Pointer")["self"]["host"], "*const T");
    assert_eq!(subject(&j, "demo::T as Pointer#2")["self"], "ptr<target type=T>");
}

#[test]
fn a_generic_parameter_keeps_its_default() {
    let src = "pub struct Error;\npub type Result<T, E = Error> = core::result::Result<T, E>;\npub struct Buf<T = u8, const N: usize = 3>([T; N]);\n";
    let (_, j) = roundtrip(src);
    assert_eq!(subject(&j, "demo::Result")["generics"][1], serde_json::json!({"name": "E", "bounds": [], "default": "Error"}));
    assert_eq!(subject(&j, "demo::Buf")["generics"][0]["default"], "u8");
    assert_eq!(subject(&j, "demo::Buf")["generics"][1]["default"]["host"], "3");
}

#[test]
fn a_mut_the_compiler_cannot_see_stays_in_the_bare_body() {
    // `mut` is derived through rustc, which does not compile what a `#[cfg]` leaves out; there the bare body keeps it.
    let src = "#[cfg(feature = \"simd\")]\npub fn sum(v: &[u8], mut n: u32) -> u32 {\n    let mut t = 0;\n    for x in v {\n        t += *x as u32;\n    }\n    n += t;\n    n\n}\npub fn plain(v: &[u8]) -> usize {\n    let mut t = 0;\n    t += v.len();\n    #[cfg(test)]\n    let mut u = 1;\n    t\n}\n";
    let (b, j) = roundtrip(src);
    assert!(b.contains("fn sum(v, mut n) {\n    let mut t = 0;"), "{b}");
    assert!(b.contains("fn plain(v) {\n    let t = 0;") && b.contains("    let u = 1;"), "{b}");
    assert_eq!(subject(&j, "demo::sum")["params"][1]["name"], "n");
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
    assert!(s.contains("ref<target type=list<element type=u8> as 'std::primitive::slice'>"), "{s}");
    assert!(s.contains("list<element type=u32> where count(value) == 3 as 'std::primitive::array'"), "{s}");
    assert!(s.contains("count(value) == N"), "{s}");
    assert!(!s.contains("\"form\":\"slice\"") && !s.contains("\"form\":\"array\""), "{s}");
}

// ---------- B1: traits, implementations and associated types ----------

#[test]
fn impl_trait_and_dyn_trait_are_existential_types() {
    let src = r#"
use std::fmt::Display;

pub fn show(what: impl Display, k: impl Into<String>) -> usize {
    0
}
pub fn pairs<'a>(s: &'a str) -> impl Iterator<Item = (char, &'a str)> + 'a {
    s.chars().map(|c| (c, s))
}
pub fn kinds(v: &[u8]) -> impl Iterator<Item = (u8, &str)> + Clone {
    v.iter().map(|b| (*b, ""))
}
fn errs() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(())
}
fn each(it: &mut dyn Iterator<Item = u8>, ok: &dyn Fn(&u8) -> bool) -> bool {
    it.all(|x| ok(&x))
}
fn not_a_trait(x: &mut dyn FnMut(&mut u8)) {}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("fn show(what, k)") && !b.contains("impl "), "{b}");
    assert_eq!(subject(&j, "demo::show")["params"][0]["type"], "some Display");
    assert_eq!(subject(&j, "demo::show")["params"][1]["type"], "some Into<string>");
    // a bound's references are shared and unnamed; a named lifetime or a `mut` there stays host text
    assert_eq!(subject(&j, "demo::kinds")["result"]["type"], "some Iterator { Item = (u8, ref<target type=string>) } + Clone");
    assert_eq!(subject(&j, "demo::kinds")["result"]["hold"], serde_json::json!({"kind": "borrows", "from": "v"}));
    assert_eq!(subject(&j, "demo::pairs")["result"]["type"]["form"], "impl_trait");
    assert_eq!(subject(&j, "demo::errs")["result"]["fails"], "any std::error::Error + Send + Sync as 'Box'");
    assert_eq!(subject(&j, "demo::each")["params"][0]["type"], "ref<target type=any Iterator { Item = u8 }>");
    assert_eq!(subject(&j, "demo::each")["params"][0]["access"]["rights"], "write");
    assert_eq!(subject(&j, "demo::each")["params"][1]["type"], "ref<target type=any Fn(ref<target type=u8>) -> bool>");
    assert_eq!(subject(&j, "demo::not_a_trait")["params"][0]["type"]["form"], "dyn");
}

#[test]
fn lifetime_arguments_are_arguments_of_a_host_type() {
    let src = r#"
use std::fmt;
pub struct Node<'t>(&'t str);
struct Cursor<'a, 'b, T> {
    node: Node<'a>,
    rest: &'b [T],
}
enum Tok<'s> {
    Word(&'s str),
    End,
}
fn first<'a>(n: Node<'a>) -> Vec<Node<'a>> {
    vec![n]
}
fn fill(out: &mut Vec<Node<'_>>, f: &mut fmt::Formatter<'_>) {}
fn pick<'a, 'b>(a: &'a Node<'b>) -> &'a Node<'b> {
    a
}
"#;
    let (_, j) = roundtrip(src);
    assert_eq!(subject(&j, "demo::first")["params"][0]["type"], "Node<'a>");
    assert_eq!(subject(&j, "demo::first")["result"]["type"], "list<element type=Node<'a>> as 'Vec'");
    assert_eq!(subject(&j, "demo::fill")["params"][0]["type"], "ref<target type=list<element type=Node<'_>> as 'Vec'>");
    assert_eq!(subject(&j, "demo::fill")["params"][1]["type"], "ref<target type=fmt::Formatter<'_>>");
    assert_eq!(subject(&j, "demo::pick")["params"][0]["type"], "ref<target type=Node<'b>>");
    // a type with lifetime parameters holds a borrow for each: `hold : borrows for 'a, 'b`
    assert_eq!(subject(&j, "demo::Node")["hold"], serde_json::json!({"kind": "borrows", "for": ["'t"]}));
    assert_eq!(subject(&j, "demo::Cursor")["hold"]["for"], serde_json::json!(["'a", "'b"]));
    assert_eq!(subject(&j, "demo::Tok")["hold"]["for"], serde_json::json!(["'s"]));
    assert_eq!(subject(&j, "demo::Cursor")["fields"][0]["type"], "Node<'a>");
    assert!(j.to_string().find("lifetime_args").is_none(), "no host text is left: {j}");
}

#[test]
fn a_type_declares_what_it_implements_with_its_associated_types() {
    let src = r#"
struct Chars2<'a>(&'a str);
impl<'a> Iterator for Chars2<'a> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        None
    }
}
impl<'a> Clone for Chars2<'a> {
    fn clone(&self) -> Self {
        Chars2(self.0)
    }
}
impl<'a, T: Clone + Default> From<Vec<T>> for Wrap<'a, T> {
    fn from(v: Vec<T>) -> Self {
        Wrap(v, "")
    }
}
struct Wrap<'a, T>(Vec<T>, &'a str);
impl<'b> IntoIterator for &Chars2<'b> {
    type Item = char;
    type IntoIter = Chars2<'b>;
    fn into_iter(self) -> Self::IntoIter {
        Chars2(self.0)
    }
}
trait Shape {
    type Unit;
    fn area(&self) -> Self::Unit;
}
impl<'a> Shape for Chars2<'a> {
    type Unit = Option<&'a str>;
    fn area(&self) -> Self::Unit {
        None
    }
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("impl Iterator for Chars2") && b.contains("fn into_iter(self)") && !b.contains("Self::IntoIter"), "{b}");
    let c = subject(&j, "demo::Chars2");
    assert_eq!(c["implements"][0], serde_json::json!({"trait": "Iterator", "assoc": {"Item": "scalar as 'char'"}, "impl": "demo::Chars2 as Iterator"}));
    assert_eq!(c["implements"][1]["trait"], "Clone");
    assert_eq!(c["implements"].as_array().unwrap().len(), 3, "the impl for `&Chars2` is not the type's own");
    // an associated type of an impl is a type: `Self::IntoIter` in a signature names it
    assert_eq!(subject(&j, "demo::Chars2 as IntoIterator::into_iter")["result"]["type"], "Self::IntoIter");
    assert_eq!(subject(&j, "demo::Chars2 as IntoIterator::IntoIter")["type"], "Chars2<'b>");
    assert_eq!(subject(&j, "demo::Chars2 as IntoIterator")["self"], "ref<target type=Chars2<'b>>");
    assert_eq!(subject(&j, "demo::Chars2 as Shape::area")["result"]["type"], "Self::Unit");
    // the generics of an impl, and its trait with arguments, stay on the impl subject
    let w = subject(&j, "demo::Wrap as From");
    assert_eq!(w["generics"][0]["name"], "'a");
    assert_eq!(w["trait"], "From<list<element type=T> as 'Vec'>");
    assert_eq!(w["self"], "Wrap<'a, T>");
    assert_eq!(subject(&j, "demo::Wrap")["implements"][0]["impl"], "demo::Wrap as From");
}

#[test]
fn const_fn_and_plain_must_use_are_clauses_not_host_text() {
    let src = r#"
pub struct C {
    v: Vec<u8>,
}
impl C {
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        3
    }
    pub const fn new() -> C {
        C { v: Vec::new() }
    }
    pub const unsafe fn raw(&self) -> u8 {
        0
    }
}
#[must_use = "the message has no slot"]
pub fn kept(a: u32) -> u32 {
    a
}
#[must_use]
pub(crate) fn plain(a: u32) -> u32 {
    a
}
pub const fn g(a: u32) -> u32 {
    a
}
"#;
    let (b, j) = roundtrip(src);
    assert!(!b.contains("const fn") && !b.contains("const unsafe") && b.contains("unsafe fn raw"), "{b}");
    assert!(b.contains("#[inline]") && !b.contains("must_use"), "{b}");
    let says = |n: &str| -> Vec<String> { subject(&j, n)["clauses"].as_array().map_or(vec![], |c| c.iter().map(|c| c["text"].as_str().unwrap().to_string()).collect()) };
    assert_eq!(says("demo::C::len"), ["evaluable at compile time", "result must be used"]);
    assert_eq!(says("demo::C::raw"), ["evaluable at compile time"]);
    assert_eq!(says("demo::plain"), ["result must be used"]);
    assert_eq!(says("demo::g"), ["evaluable at compile time"]);
    assert_eq!(says("demo::kept"), ["result must be used"]);
    let kept = subject(&j, "demo::kept")["clauses"][0]["source"].as_str().unwrap().to_string();
    assert_eq!(kept, "attribute #[must_use = \"the message has no slot\"]", "the message stays in `source`");
    // weave writes them back in place: the attribute before the visibility, `const` after it
    let w = weave(&b, &j).unwrap();
    assert!(w.contains("#[must_use = \"the message has no slot\"] pub fn kept"), "{w}");
    assert!(w.contains("#[must_use] pub(crate) fn plain") && w.contains("pub const fn g") && w.contains("pub const unsafe fn raw"), "{w}");
}

#[test]
fn must_use_on_types_and_traits_round_trips_with_its_message() {
    let src = r#"
#[must_use]
pub struct Plain {
    v: u8,
}
#[must_use = "iterators are lazy"]
pub struct Lazy<'a> {
    v: &'a u8,
}
#[derive(Debug)]
#[must_use = "an enum"]
pub enum E {
    A,
    B(u8),
}
#[must_use = "traits too"]
pub trait T {
    fn f(&self) -> u8;
}
pub struct Not {
    v: u8,
}
"#;
    let (b, j) = roundtrip(src);
    assert!(!b.contains("must_use"), "{b}");
    let clause = |n: &str| subject(&j, n)["clauses"].as_array().map(|c| (c[0]["kind"].as_str().unwrap().to_string(), c[0]["text"].as_str().unwrap().to_string(), c[0]["source"].as_str().unwrap().to_string()));
    assert_eq!(clause("demo::Plain").unwrap(), ("result must be used".into(), "must be used".into(), "attribute #[must_use]".into()));
    assert_eq!(clause("demo::Lazy").unwrap().2, "attribute #[must_use = \"iterators are lazy\"]");
    assert_eq!(clause("demo::E").unwrap().2, "attribute #[must_use = \"an enum\"]");
    assert_eq!(clause("demo::T").unwrap().2, "attribute #[must_use = \"traits too\"]");
    assert!(clause("demo::Not").is_none());
    let w = weave(&b, &j).unwrap();
    assert!(w.contains("#[must_use = \"iterators are lazy\"]") && w.contains("#[must_use = \"traits too\"]"), "{w}");
}

#[test]
fn unsafe_fn_with_a_safety_section_is_an_obligation() {
    let src = r#"
pub struct S {
    v: Vec<u8>,
}
impl S {
    /// Reads an element without a check.
    ///
    /// # Safety
    ///
    /// The index must be
    /// less than the length.
    ///
    /// # Examples
    ///
    /// none
    pub const unsafe fn get_unchecked(&self, i: usize) -> u8 {
        0
    }
    /// No safety section here.
    pub unsafe fn bare(&self) -> u8 {
        0
    }
}
/// # Safety
/// The pointer is valid.
pub async unsafe fn a(p: u8) -> u8 {
    p
}
/// # Safety
/// Call it on Tuesdays.
pub unsafe extern "C" fn c(p: u8) -> u8 {
    p
}
"#;
    let (b, j) = roundtrip(src);
    assert!(b.contains("unsafe fn bare") && !b.contains("unsafe fn get_unchecked") && !b.contains("unsafe fn a") && !b.contains("unsafe extern"), "{b}");
    let ob = |n: &str| subject(&j, n)["clauses"].as_array().into_iter().flatten().find(|c| c["kind"] == "obliges").map(|c| c["text"].as_str().unwrap().to_string());
    assert_eq!(ob("demo::S::get_unchecked").unwrap(), "obliges caller : true because \"The index must be less than the length.\"");
    assert!(ob("demo::S::bare").is_none());
    assert_eq!(ob("demo::a").unwrap(), "obliges caller : true because \"The pointer is valid.\"");
    let w = weave(&b, &j).unwrap();
    assert!(w.contains("pub const unsafe fn get_unchecked") && w.contains("pub async unsafe fn a") && w.contains("pub unsafe extern \"C\" fn c"), "{w}");
    let l = bare_rust::clause::lint("obliges", &ob("demo::a").unwrap(), &["obliges caller"], &bare_rust::clause::Scope::default());
    assert!(l.is_empty(), "{l:?}");
}

#[test]
fn named_lifetime_in_trait_argument() {
    let src = "pub struct Ref<'a> {\n    pub s: &'a str,\n}\n\nimpl<'a> From<&'a str> for Ref<'a> {\n    fn from(s: &'a str) -> Ref<'a> {\n        Ref { s }\n    }\n}\n";
    let (b, j) = roundtrip(src);
    let w = weave(&b, &j).unwrap();
    assert!(w.contains("impl<'a> From<&'a str> for Ref<'a>"), "{w}");
}

#[test]
fn const_generics() {
    let src = "pub struct Buf<T> {\n    items: Vec<T>,\n}\n\nimpl<T: Clone> Buf<T> {\n    pub fn get_array<const K: usize>(&self) -> [T; K] {\n        todo!()\n    }\n}\n\npub struct Grid<const N: usize> {\n    cells: [u8; N],\n}\n\nimpl<const N: usize> Grid<N> {\n    pub fn len(&self) -> usize {\n        N\n    }\n}\n";
    let (b, j) = roundtrip(src);
    assert!(b.contains("get_array"), "{b}");
    let w = weave(&b, &j).unwrap();
    assert!(w.contains("get_array<const K: usize>(&self) -> [T; K]") && w.contains("impl<const N: usize> Grid<N>"), "{w}");
}

#[test]
fn item_level_macro_invocations_stay_as_host_text() {
    let src = "pub trait Peek {\n    fn peek_it(&mut self) -> Option<u8>;\n}\n\nmacro_rules! peek_by_clone {\n    ([$($typarm:tt)*] $type_:ty) => {\n        impl<$($typarm)*> Peek for $type_ {\n            fn peek_it(&mut self) -> Option<u8> {\n                None\n            }\n        }\n    };\n}\n\npeek_by_clone! { ['a] ::std::str::Bytes<'a> }\n\n#[cfg(test)]\npeek_by_clone! { ['a, T] ::std::vec::IntoIter<T> }\n\npub fn f(x: u8) -> u8 {\n    x\n}\n";
    let (b, j) = roundtrip(src);
    assert!(b.contains("peek_by_clone! { ['a] ::std::str::Bytes<'a> }"), "{b}");
    assert!(b.contains("macro_rules! peek_by_clone"), "{b}");
    let names: Vec<_> = j["subjects"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap().to_string()).collect();
    assert!(!names.iter().any(|n| n.contains("peek_by_clone")), "{names:?}");
    let d = std::env::temp_dir().join("bare-rust-macro-items");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("lib.brs"), &b).unwrap();
    std::fs::write(d.join("lib.constraints.json"), canonical(&j)).unwrap();
    let st = bare_rust::stats::stats(&d).unwrap();
    assert_eq!(st["left_in_body"]["macros"]["macro items kept as host text"], 2, "{st}");
}

#[test]
fn a_type_that_releases_at_its_end_says_so() {
    // Its own `impl Drop`, or a field of such a type (language 7.3, `lifetime self released at end`).
    let src = r#"
use std::cell::Cell;
pub struct Guard<'a>(&'a Cell<bool>);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
pub struct Holder<'a> {
    g: Guard<'a>,
    n: u8,
}
pub struct Plain {
    n: u8,
}
"#;
    let (_, j) = roundtrip(src);
    let says = |n: &str| subject(&j, n)["clauses"].as_array().into_iter().flatten().any(|c| c["text"] == "lifetime self released at end");
    assert!(says("demo::Guard") && says("demo::Holder") && !says("demo::Plain"), "{}", j["subjects"]);
}

#[test]
fn a_type_of_the_crate_shadows_a_standard_one_of_the_same_name() {
    // `Ref` here is the crate's own, not `std::cell::Ref`, so a field of it releases nothing.
    // A variant named `Ref` is not a field of type `Ref` either.
    let src = "pub struct Ref {\n    n: u8,\n}\npub struct Holder {\n    r: Ref,\n}\npub enum Kind {\n    File,\n    Mut,\n}\n";
    let (_, j) = roundtrip(src);
    assert!(subject(&j, "demo::Holder")["clauses"].is_null(), "{}", j["subjects"]);
    assert!(subject(&j, "demo::Kind")["clauses"].is_null(), "{}", j["subjects"]);
}

#[test]
fn a_turbofish_is_a_realization_the_body_names() {
    // A method (`parse::<u16>`), a function by path (`std::mem::size_of::<u64>`), a type in a path (`Vec::<u8>`); the
    // same template and arguments twice are one realization; a turbofish of only `_` states nothing and stays.
    let src = r#"
pub fn f(s: &str, v: Vec<i32>) -> Result<usize, std::num::ParseIntError> {
    let port: u16 = s.parse::<u16>()?;
    let n = std::mem::size_of::<u64>();
    let m = size_of::<u64>();
    let mut bytes = Vec::<u8>::new();
    bytes.push(1);
    let evens = v.iter().filter(|x| *x % 2 == 0).collect::<Vec<_>>();
    let again = v.iter().collect::<Vec<_>>();
    let r = Ok::<_, ()>(1).unwrap_or(0) + Ok::<_, _>(2).unwrap_or(0);
    Ok(port as usize + n + m + bytes.len() + evens.len() + again.len() + r as usize)
}
"#;
    let (b, j) = roundtrip(src);
    // The name takes the place of the template's last segment; a path before it stays. `size_of` (the prelude's) and
    // `std::mem::size_of` are one template.
    for want in ["s.parse_u16()?", "let n = std::mem::size_of_u64();", "let m = size_of_u64();", "VecU8::new()", ".collect_vec();", "ok_unit(1)", "Ok::<_, _>(2)"] {
        assert!(b.contains(want), "missing `{want}` in\n{b}");
    }
    // One table for the file (a crate root's file holds its crate's), with the template's full path; a method's
    // template is `<_>::name`, its type the receiver's.
    assert!(j["holes"]["demo::f"]["realizations"].is_null());
    let r = &j["realizations"];
    assert_eq!(r["parse_u16"], serde_json::json!({"template": "<_>::parse", "args": ["u16"]}));
    assert_eq!(r["size_of_u64"], serde_json::json!({"template": "std::mem::size_of", "args": ["u64"]}));
    assert_eq!(r["VecU8"], serde_json::json!({"template": "std::vec::Vec", "args": ["u8"]}));
    assert_eq!(r["collect_vec"], serde_json::json!({"template": "<_>::collect", "args": ["list<element type=_> as 'Vec'"]}));
    assert_eq!(r["ok_unit"], serde_json::json!({"template": "std::result::Result::Ok", "args": ["_", "()"]}));
    assert_eq!(r.as_object().unwrap().len(), 5, "{r}");
}

#[test]
fn a_file_whose_mod_has_a_cfg_keeps_its_muts() {
    // `#[cfg(feature = "x")] mod gated;`: rustc may never see `gated.rs`, nor what is under it, so its `mut`s stay.
    let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bare-rust-test/cfg-gated-file");
    let _ = std::fs::remove_dir_all(&d);
    let w = |p: &str, s: &str| {
        std::fs::create_dir_all(d.join(p).parent().unwrap()).unwrap();
        std::fs::write(d.join(p), s).unwrap();
    };
    let body = "pub fn f() -> u32 {\n    let mut n = 0;\n    n += 1;\n    n\n}\n";
    w("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    w("src/lib.rs", "pub mod open;\n#[cfg(feature = \"x\")]\npub mod gated;\n");
    w("src/open.rs", body);
    w("src/gated.rs", &format!("pub mod inner;\n{body}"));
    w("src/gated/inner.rs", body);
    let out = d.join("bare");
    for r in bare_rust::corpus::strip_crate(&d, &out, false).unwrap() {
        assert!(r.outcome.is_ok(), "{:?}", r.outcome);
    }
    let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap();
    assert!(read("src/open.brs").contains("let n = 0;"));
    assert!(read("src/gated.brs").contains("let mut n = 0;"));
    assert!(read("src/gated/inner.brs").contains("let mut n = 0;"));
}

#[test]
fn a_crate_has_one_table_of_realizations_in_its_root_file() {
    // Two modules realize `parse` with `u16`, one of them with a local already called `parse_u16`: one name across the
    // crate, kept clear of every identifier, and one entry, in `src/lib.constraints.json`.
    let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bare-rust-test/crate-realizations");
    let _ = std::fs::remove_dir_all(&d);
    let w = |p: &str, s: &str| {
        std::fs::create_dir_all(d.join(p).parent().unwrap()).unwrap();
        std::fs::write(d.join(p), s).unwrap();
    };
    w("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    w("src/lib.rs", "pub mod a;\npub mod b;\n");
    w("src/a.rs", "use std::mem;\npub fn f(s: &str) -> usize {\n    s.parse::<u16>().unwrap() as usize + mem::size_of::<u8>()\n}\n");
    w("src/b.rs", "pub fn g(s: &str) -> u16 {\n    let parse_u16 = 1;\n    s.parse::<u16>().unwrap() + parse_u16 + std::mem::size_of::<u8>() as u16\n}\n");
    let out = d.join("bare");
    for r in bare_rust::corpus::strip_crate(&d, &out, false).unwrap() {
        assert!(r.outcome.is_ok(), "{:?}", r.outcome);
    }
    let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap();
    let js = |p: &str| serde_json::from_str::<Value>(&read(p)).unwrap();
    assert_eq!(
        js("src/lib.constraints.json")["realizations"],
        serde_json::json!({
            "parse_u16_2": {"template": "<_>::parse", "args": ["u16"]},
            "size_of_u8": {"template": "std::mem::size_of", "args": ["u8"]}
        })
    );
    assert!(js("src/a.constraints.json")["realizations"].is_null() && js("src/b.constraints.json")["realizations"].is_null());
    assert!(read("src/a.brs").contains("s.parse_u16_2().unwrap()") && read("src/a.brs").contains("mem::size_of_u8()"), "{}", read("src/a.brs"));
    assert!(read("src/b.brs").contains("s.parse_u16_2().unwrap() + parse_u16"), "{}", read("src/b.brs"));
    // The crate weaves back: each module's uses are the template with its arguments.
    let woven = d.join("woven");
    for r in bare_rust::corpus::weave_crate(&out, &woven).unwrap() {
        assert!(r.outcome.is_ok(), "{:?}", r.outcome);
    }
    let b = std::fs::read_to_string(woven.join("src/b.rs")).unwrap();
    assert!(b.contains("s.parse::<u16>().unwrap() + parse_u16"), "{b}");
}

#[test]
fn a_const_fn_written_in_the_body_and_as_a_clause_weaves_with_one_const() {
    let (b, j) = strip("pub const fn f(x: u8) -> u8 {\n    x\n}\n", "demo 0.1.0", "src/lib.rs").unwrap();
    assert!(b.starts_with("fn f(x)"), "{b}");
    let w = weave(&b.replacen("fn f", "const fn f", 1), &j).unwrap();
    assert!(w.contains("pub const fn f(x: u8) -> u8") && !w.contains("const const"), "{w}");
}
