//! `bare-rust externals`: every item of `std` that a crate uses, as canonical paths (proposal 7.2, 9 check 5).
//!
//! Two mechanical sources are combined. `rustc -Zunpretty=mir` (with spans and untrimmed paths) spells out every
//! resolved call, function value, constant and local type, so inherent and trait methods are exact. Tree-sitter reads
//! what MIR does not keep: macros, derives, traits implemented or named as bounds, and the types written in the
//! source. Both are normalised to the public `std::` path.
use crate::cargo;
use crate::corpus::{corpus_files, crate_info};
use crate::{named_children, parse_rust, text, R};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use tree_sitter::Node;

/// The roots of the standard library: `core` and `alloc` are folded into `std`.
pub const ROOTS: [&str; 3] = ["std", "core", "alloc"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Function,
    Type,
    Trait,
    Const,
    Macro,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Function, Kind::Type, Kind::Trait, Kind::Const, Kind::Macro];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Function => "function",
            Kind::Type => "type",
            Kind::Trait => "trait",
            Kind::Const => "const",
            Kind::Macro => "macro",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == s)
    }
}

/// One external item, with where it is used.
#[derive(Default, Debug, Clone)]
pub struct Item {
    /// File (relative to the crate) to the source positions of its uses.
    pub sites: BTreeMap<String, BTreeSet<(u32, u32)>>,
    /// `call`, `value` (a function named as a value), `implement`, `bound`, `named`, `derive`, `operator`, `method`.
    pub roles: BTreeSet<String>,
    /// For a trait method: the trait.
    pub trait_path: Option<String>,
    /// For a function: the impls its calls resolve to, as rustc prints them in MIR: `impl Iterator for Chars<'_>` for a
    /// trait method, `impl HashMap` (the inherent impl of the type, its arguments left out) for an inherent one. A
    /// trait method called through a type parameter (a short upper-case name, by a heuristic) resolves to no impl yet,
    /// and says so. A crate's own types are printed by their bare names.
    pub impls: BTreeSet<String>,
    /// For a macro: `bang` or `derive`.
    pub form: Option<String>,
    /// Where the item was seen: `mir`, `syntax`.
    pub via: BTreeSet<String>,
}

impl Item {
    pub fn uses(&self) -> usize {
        self.sites.values().map(BTreeSet::len).sum()
    }
}

#[derive(Default, Debug)]
pub struct Externals {
    pub items: BTreeMap<(Kind, String), Item>,
    /// Paths that rustc printed through `core` or `alloc` because `std` has no public name for them, or that are
    /// compiler desugaring (`?`, `for`, `format_args!`): left out, with their number of uses.
    pub skipped: BTreeMap<String, usize>,
    pub targets: Vec<String>,
}

type Pos = (String, u32, u32);

impl Externals {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, kind: Kind, path: String, role: &str, via: &str, at: &Pos, trait_path: Option<String>, form: Option<&str>) {
        let it = self.items.entry((kind, path)).or_default();
        it.sites.entry(at.0.clone()).or_default().insert((at.1, at.2));
        it.roles.insert(role.to_string());
        it.via.insert(via.to_string());
        if trait_path.is_some() {
            it.trait_path = trait_path;
        }
        if let Some(f) = form {
            it.form = Some(f.to_string());
        }
    }

    fn add_impl(&mut self, path: &str, resolves: Option<String>) {
        if let (Some(r), Some(it)) = (resolves, self.items.get_mut(&(Kind::Function, path.to_string()))) {
            it.impls.insert(r);
        }
    }

    pub fn count(&self, kind: Kind) -> usize {
        self.items.keys().filter(|(k, _)| *k == kind).count()
    }

    pub fn paths(&self) -> impl Iterator<Item = (Kind, &str)> {
        self.items.keys().map(|(k, p)| (*k, p.as_str()))
    }

    /// The committed form.
    pub fn to_json(&self, krate: &str, all_targets: bool) -> Value {
        let mut by: serde_json::Map<String, Value> = serde_json::Map::new();
        for k in Kind::ALL {
            by.insert(k.name().into(), json!(self.count(k)));
        }
        by.insert("total".into(), json!(self.items.len()));
        let mut items = vec![];
        for k in Kind::ALL {
            for ((kk, p), it) in &self.items {
                if *kk != k {
                    continue;
                }
                let mut o = serde_json::Map::new();
                o.insert("kind".into(), json!(k.name()));
                o.insert("path".into(), json!(p));
                if let Some(t) = &it.trait_path {
                    o.insert("trait".into(), json!(t));
                }
                if let Some(f) = &it.form {
                    o.insert("form".into(), json!(f));
                }
                if !it.impls.is_empty() {
                    o.insert("impls".into(), json!(it.impls));
                }
                o.insert("uses".into(), json!(it.uses()));
                let files: serde_json::Map<String, Value> = it.sites.iter().map(|(f, s)| (f.clone(), json!(s.len()))).collect();
                o.insert("files".into(), Value::Object(files));
                o.insert("roles".into(), json!(it.roles));
                o.insert("via".into(), json!(it.via));
                items.push(Value::Object(o));
            }
        }
        json!({
            "format": "y-externals/0",
            "crate": krate,
            "rustc": rustc_version().unwrap_or_default(),
            "all_targets": all_targets,
            "targets": self.targets,
            "counts": Value::Object(by),
            "skipped": self.skipped,
            "items": items,
        })
    }
}

pub fn rustc_version() -> Option<String> {
    let o = Command::new("rustc").arg("--version").output().ok()?;
    String::from_utf8_lossy(&o.stdout).split_whitespace().nth(1).map(str::to_string)
}

// ---------------------------------------------------------------------------------------------------------------
// Paths

const PRIMS: [&str; 17] =
    ["bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64"];

pub fn is_prim(s: &str) -> bool {
    PRIMS.contains(&s)
}

/// Re-export paths that `std` also has under a shorter public name.
const ALIASES: [(&str, &str); 7] = [
    ("std::collections::hash_map::HashMap", "std::collections::HashMap"),
    ("std::collections::hash_set::HashSet", "std::collections::HashSet"),
    ("std::collections::btree_map::BTreeMap", "std::collections::BTreeMap"),
    ("std::collections::btree_set::BTreeSet", "std::collections::BTreeSet"),
    ("std::collections::vec_deque::VecDeque", "std::collections::VecDeque"),
    ("std::collections::binary_heap::BinaryHeap", "std::collections::BinaryHeap"),
    ("std::collections::linked_list::LinkedList", "std::collections::LinkedList"),
];

/// Traits whose methods stand for an operator or an implicit conversion: the call folds into the trait.
const FOLDED: [&str; 31] = [
    "std::ops::Index",
    "std::ops::IndexMut",
    "std::ops::Deref",
    "std::ops::DerefMut",
    "std::ops::Add",
    "std::ops::Sub",
    "std::ops::Mul",
    "std::ops::Div",
    "std::ops::Rem",
    "std::ops::Neg",
    "std::ops::Not",
    "std::ops::BitAnd",
    "std::ops::BitOr",
    "std::ops::BitXor",
    "std::ops::Shl",
    "std::ops::Shr",
    "std::ops::AddAssign",
    "std::ops::SubAssign",
    "std::ops::MulAssign",
    "std::ops::DivAssign",
    "std::ops::RemAssign",
    "std::ops::BitAndAssign",
    "std::ops::BitOrAssign",
    "std::ops::BitXorAssign",
    "std::ops::ShlAssign",
    "std::ops::ShrAssign",
    "std::cmp::PartialEq",
    "std::cmp::PartialOrd",
    "std::ops::RangeBounds",
    "std::ops::Fn",
    "std::ops::FnMut",
];

/// Folded traits keep these methods as functions of their own: they are called by name.
const FOLD_EXCEPT: [&str; 2] = ["std::cmp::PartialOrd::partial_cmp", "std::ops::RangeBounds::contains"];

/// Compiler desugaring and macro internals that print as `std` paths: never a use the author wrote.
const DESUGARING: [&str; 12] = [
    "std::ops::Try",
    "std::ops::FromResidual",
    "std::ops::ControlFlow",
    "std::convert::Infallible",
    "std::fmt::rt::",
    "std::fmt::Arguments",
    "std::rt::",
    "std::panicking::",
    "std::intrinsics::",
    "std::boxed::box_new",
    "std::vec::from_elem",
    "std::marker::Destruct",
];

fn is_desugaring(path: &str) -> bool {
    DESUGARING.iter().any(|d| path == *d || (d.ends_with("::") && path.starts_with(d)) || path.starts_with(&format!("{d}::")))
}

fn alias(path: String) -> String {
    ALIASES.iter().find(|(a, _)| *a == path).map_or(path, |(_, b)| (*b).to_string())
}

/// `core::x::y` and `alloc::x::y` as `std::x::y`.
fn std_root(segs: &[String]) -> String {
    let mut v: Vec<&str> = segs.iter().map(String::as_str).collect();
    if v.first().is_some_and(|r| *r == "core" || *r == "alloc") {
        v[0] = "std";
    }
    alias(v.join("::"))
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The index after the `>` that closes the `<` at `i`, not counting the `>` of `->`.
pub(crate) fn skip_balanced(s: &str, i: usize) -> usize {
    let b = s.as_bytes();
    let (mut depth, mut j) = (0i32, i);
    while j < b.len() {
        match b[j] {
            b'<' => depth += 1,
            b'>' if j > 0 && b[j - 1] == b'-' => {}
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    b.len()
}

/// The segments of the path that starts at `i`, without generic arguments. A `<impl X>` segment is kept whole.
pub(crate) fn read_path(s: &str, i: usize) -> Vec<String> {
    let b = s.as_bytes();
    let (mut j, mut segs) = (i, vec![]);
    loop {
        let st = j;
        while j < b.len() && is_ident_byte(b[j]) {
            j += 1;
        }
        if j == st {
            break;
        }
        segs.push(s[st..j].to_string());
        if !s[j..].starts_with("::") {
            break;
        }
        let k = j + 2;
        if s[k..].starts_with("<impl ") {
            let e = skip_balanced(s, k);
            segs.push(s[k..e].to_string());
            if s[e..].starts_with("::") {
                j = e + 2;
                continue;
            }
            break;
        } else if s[k..].starts_with('<') {
            let e = skip_balanced(s, k);
            if s[e..].starts_with("::") {
                j = e + 2;
                continue;
            }
            break;
        }
        j = k;
    }
    segs
}

/// The byte offsets where a path rooted in `std`, `core` or `alloc` starts.
fn root_positions(s: &str) -> Vec<usize> {
    let b = s.as_bytes();
    let mut out = vec![];
    for r in ROOTS {
        let pat = format!("{r}::");
        let mut from = 0;
        while let Some(p) = s[from..].find(&pat) {
            let at = from + p;
            if at == 0 || !(is_ident_byte(b[at - 1]) || b[at - 1] == b':') {
                out.push(at);
            }
            from = at + pat.len();
        }
    }
    out.sort_unstable();
    out
}

/// The primitive type an `<impl T>` segment is for: `str`, `usize`, `slice`, `array`, `pointer`.
fn prim_of(impl_seg: &str) -> Option<String> {
    let t = impl_seg.strip_prefix("<impl ")?.strip_suffix('>')?.trim();
    Some(if t.starts_with('[') {
        if t.contains(';') { "array" } else { "slice" }.to_string()
    } else if t.starts_with('*') {
        "pointer".to_string()
    } else if t.starts_with('(') {
        "tuple".to_string()
    } else if t.starts_with('&') {
        "reference".to_string()
    } else if is_prim(t) {
        t.to_string()
    } else {
        return None;
    })
}

/// `core::x::Y<alloc::z::W>` as `std::x::Y<std::z::W>`: every path root that is not a public `std` one.
pub(crate) fn fold_roots(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        let at_start = i == 0 || !(is_ident_byte(b[i - 1]) || b[i - 1] == b':');
        if at_start && (s[i..].starts_with("core::") || s[i..].starts_with("alloc::")) {
            out.push_str("std::");
            i += s[i..].find("::").map_or(0, |p| p + 2);
            continue;
        }
        let ch = s[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// The head of an impl's self type: `std::iter::Filter<I, {closure@src/lib.rs:7:3: 7:9}>` is the impl for `std::iter::Filter`,
/// whatever it was instantiated with. A type that does not start with a path (`&[u8]`, `[T; 3]`, `<I as X>::Y`) is kept,
/// with its closures unnamed.
fn self_head(t: &str) -> String {
    let t = fold_roots(t.trim());
    if !t.starts_with(|c: char| c.is_alphabetic()) {
        // a closure is `{closure@file:l:c: l:c}`: where it is says nothing about the impl
        let mut out = String::new();
        let mut rest = t.as_str();
        while let Some(i) = rest.find("{closure@") {
            out.push_str(&rest[..i]);
            let end = rest[i..].find('}').map_or(rest.len(), |e| i + e + 1);
            out.push_str("{closure}");
            rest = &rest[end..];
        }
        out.push_str(rest);
        return out;
    }
    std_root(&read_path(&t, 0))
}

/// Whether the self type of a trait call is a type parameter of a generic function, which MIR names where a call goes
/// through a bound (`<I as Iterator>::next`). MIR prints a crate's own types by their bare names too, so this is a
/// heuristic: a parameter is `Self` or a short upper-case name (`T`, `I`, `K2`).
fn is_type_param(t: &str) -> bool {
    t == "Self" || (t.len() <= 2 && t.bytes().all(is_ident_byte) && t.chars().next().is_some_and(char::is_uppercase))
}

/// Split at the first ` as ` that is not inside `<>`.
pub(crate) fn split_as(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    let (mut depth, mut j) = (0i32, 0);
    while j < b.len() {
        match b[j] {
            b'<' => depth += 1,
            b'>' if j > 0 && b[j - 1] == b'-' => {}
            b'>' => depth -= 1,
            _ if depth == 0 && s[j..].starts_with(" as ") => return Some((&s[..j], &s[j + 4..])),
            _ => {}
        }
        j += 1;
    }
    None
}

/// A function path as MIR prints it for a call or a function value.
#[derive(Debug, PartialEq)]
pub struct FnPath {
    pub path: String,
    pub trait_path: Option<String>,
    /// `core` or `alloc` was the root: not a public `std` path.
    pub internal: bool,
    /// The impl the call resolves to, when the path says: see `Item::impls`.
    pub resolves: Option<String>,
}

/// Canonical form of the function MIR names, or `None` when it is not an item of the standard library.
pub fn parse_fn_path(s: &str) -> Option<FnPath> {
    if let Some(rest) = s.strip_prefix('<') {
        let end = skip_balanced(s, 0);
        let inner = &s[1..end - 1];
        let after = s[end..].strip_prefix("::")?;
        let method: String = after.chars().take_while(|c| is_ident_byte(*c as u8)).collect();
        if method.is_empty() {
            return None;
        }
        let _ = rest;
        return match split_as(inner) {
            Some((self_ty, tr)) => {
                let segs = read_path(tr.trim(), 0);
                if !segs.first().is_some_and(|r| ROOTS.contains(&r.as_str())) {
                    return None;
                }
                let internal = segs[0] != "std";
                let t = std_root(&segs);
                let (self_ty, tr) = (self_ty.trim(), fold_roots(tr.trim()));
                let resolves = if is_type_param(self_ty) {
                    format!("impl {tr} for a type parameter `{self_ty}`")
                } else {
                    format!("impl {tr} for {}", self_head(self_ty))
                };
                Some(FnPath { path: format!("{t}::{method}"), trait_path: Some(t), internal, resolves: Some(resolves) })
            }
            None => {
                // `<[T]>::len`, `<str>::chars`
                let p = prim_of(&format!("<impl {}>", inner.trim()))?;
                Some(FnPath { path: format!("std::primitive::{p}::{method}"), trait_path: None, internal: false, resolves: Some(format!("impl {}", self_head(inner))) })
            }
        };
    }
    let segs = read_path(s, 0);
    if !segs.first().is_some_and(|r| ROOTS.contains(&r.as_str())) || segs.len() < 2 {
        return None;
    }
    if let Some(i) = segs.iter().position(|x| x.starts_with("<impl ")) {
        let p = prim_of(&segs[i])?;
        let rest = segs[i + 1..].join("::");
        let ty = segs[i].strip_prefix("<impl ").and_then(|t| t.strip_suffix('>')).unwrap_or("").trim();
        return Some(FnPath { path: format!("std::primitive::{p}::{rest}"), trait_path: None, internal: false, resolves: Some(format!("impl {}", self_head(ty))) });
    }
    // `Type::method` is an inherent method: the impl is the type's own
    let resolves = (segs.len() >= 3 && upper(&segs[segs.len() - 2])).then(|| format!("impl {}", std_root(&segs[..segs.len() - 1])));
    Some(FnPath { internal: segs[0] != "std", path: std_root(&segs), trait_path: None, resolves })
}

/// A constant path as MIR prints it: `std::char::MAX` is the associated constant of the primitive.
pub fn parse_const_path(s: &str) -> Option<String> {
    let segs = read_path(s, 0);
    if !segs.first().is_some_and(|r| ROOTS.contains(&r.as_str())) || segs.len() < 2 {
        return None;
    }
    if let Some(i) = segs.iter().position(|x| x.starts_with("<impl ")) {
        return Some(format!("std::primitive::{}::{}", prim_of(&segs[i])?, segs[i + 1..].join("::")));
    }
    if segs.len() == 3 && is_prim(&segs[1]) {
        return Some(format!("std::primitive::{}::{}", segs[1], segs[2]));
    }
    Some(std_root(&segs))
}

fn upper(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

// ---------------------------------------------------------------------------------------------------------------
// MIR

/// `file:line:col: line:col` at the end of a comment: the file and start of the span. `None` for spans outside
/// the crate (macro internals of the standard library) and for `no-location`.
fn span_of(s: &str) -> Option<Pos> {
    let s = s.trim();
    let mut parts = s.split(':');
    let file = parts.next()?;
    let line: u32 = parts.next()?.trim().parse().ok()?;
    let col: u32 = parts.next()?.trim().parse().ok()?;
    if file.starts_with('/') || file.is_empty() {
        return None;
    }
    Some((file.to_string(), line, col))
}

fn comment_span(line: &str) -> Option<Pos> {
    let i = line.rfind(" at ")?;
    span_of(&line[i + 4..])
}

/// The start of an `<impl at file:l:c: l:c>` header: where a derive put its generated code.
fn impl_header_span(line: &str) -> Option<Pos> {
    let i = line.find("<impl at ")?;
    span_of(&line[i + 9..])
}

/// Read what a crate target's MIR says about the standard library.
pub fn read_mir(mir: &str, derives: &HashSet<Pos>, out: &mut Externals) {
    let (mut skip_fn, mut pending): (bool, Option<Pos>) = (false, None);
    for line in mir.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            skip_fn = impl_header_span(line).is_some_and(|p| derives.contains(&p));
            pending = None;
            continue;
        }
        if skip_fn {
            continue;
        }
        let t = line.trim();
        if t == "// mir::ConstOperand" {
            pending = None;
        } else if let Some(sp) = t.strip_prefix("// + span: ") {
            pending = span_of(sp);
        } else if let Some(c) = t.strip_prefix("// + const_: ") {
            let Some(at) = pending.take() else { continue };
            let Some(i) = c.find("val: Value(") else { continue };
            let v = &c[i + 11..];
            let Some(v) = v.strip_suffix(") }") else { continue };
            if let Some(f) = parse_fn_path(v) {
                record_fn(out, f, &at);
            }
        } else if let Some(i) = t.find("= const ") {
            let rest = &t[i + 8..];
            let path = rest.split(';').next().unwrap_or("");
            if ROOTS.iter().any(|r| path.starts_with(&format!("{r}::"))) && !path.contains(' ') {
                if let (Some(c), Some(at)) = (parse_const_path(path), comment_span(t)) {
                    if !is_desugaring(&c) {
                        out.add(Kind::Const, c, "named", "mir", &at, None, None);
                    }
                }
            }
        } else if t.starts_with("let ") {
            let Some(at) = comment_span(t) else { continue };
            let Some(colon) = t.find(": ") else { continue };
            let Some(end) = t.rfind("; //") else { continue };
            if end <= colon {
                continue;
            }
            read_type_text(&t[colon + 2..end], &at, out);
        }
    }
}

fn record_fn(out: &mut Externals, f: FnPath, at: &Pos) {
    if f.internal || is_desugaring(&f.path) || f.trait_path.as_deref().is_some_and(is_desugaring) {
        *out.skipped.entry(f.path).or_insert(0) += 1;
        return;
    }
    match f.trait_path {
        Some(t) if FOLDED.contains(&t.as_str()) && !FOLD_EXCEPT.contains(&f.path.as_str()) => {
            out.add(Kind::Trait, t, "operator", "mir", at, None, None);
        }
        Some(t) => {
            out.add(Kind::Trait, t.clone(), "method", "mir", at, None, None);
            out.add(Kind::Function, f.path.clone(), "call", "mir", at, Some(t), None);
            out.add_impl(&f.path, f.resolves);
        }
        None => {
            out.add(Kind::Function, f.path.clone(), "call", "mir", at, None, None);
            out.add_impl(&f.path, f.resolves);
        }
    }
}

/// The types and traits named in the text of a local's type.
fn read_type_text(ty: &str, at: &Pos, out: &mut Externals) {
    for st in root_positions(ty) {
        let segs = read_path(ty, st);
        if segs.len() < 2 || segs.iter().any(|s| s.starts_with("<impl ")) {
            continue;
        }
        let before = &ty[..st];
        let is_trait = before.ends_with(" as ") || before.ends_with("dyn ") || before.ends_with("impl ");
        let mut segs = segs;
        // `Option::<T>::Some` names the variant: the type is the one before it
        if segs.len() >= 3 && upper(&segs[segs.len() - 1]) && upper(&segs[segs.len() - 2]) && !is_trait {
            segs.pop();
        }
        if !upper(segs.last().map_or("", String::as_str)) {
            continue;
        }
        let p = std_root(&segs);
        if is_desugaring(&p) || segs[0] != "std" {
            continue;
        }
        let kind = if is_trait { Kind::Trait } else { Kind::Type };
        out.add(kind, p, "inferred", "mir", at, None, None);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Syntax

const PRELUDE: [(&str, &str); 28] = [
    ("Option", "std::option::Option"),
    ("Result", "std::result::Result"),
    ("Vec", "std::vec::Vec"),
    ("String", "std::string::String"),
    ("Box", "std::boxed::Box"),
    ("Clone", "std::clone::Clone"),
    ("Copy", "std::marker::Copy"),
    ("Send", "std::marker::Send"),
    ("Sized", "std::marker::Sized"),
    ("Sync", "std::marker::Sync"),
    ("Unpin", "std::marker::Unpin"),
    ("Drop", "std::ops::Drop"),
    ("Fn", "std::ops::Fn"),
    ("FnMut", "std::ops::FnMut"),
    ("FnOnce", "std::ops::FnOnce"),
    ("AsMut", "std::convert::AsMut"),
    ("AsRef", "std::convert::AsRef"),
    ("From", "std::convert::From"),
    ("Into", "std::convert::Into"),
    ("DoubleEndedIterator", "std::iter::DoubleEndedIterator"),
    ("ExactSizeIterator", "std::iter::ExactSizeIterator"),
    ("Extend", "std::iter::Extend"),
    ("IntoIterator", "std::iter::IntoIterator"),
    ("Iterator", "std::iter::Iterator"),
    ("Default", "std::default::Default"),
    ("ToOwned", "std::borrow::ToOwned"),
    ("ToString", "std::string::ToString"),
    ("FromIterator", "std::iter::FromIterator"),
];

const PRELUDE_CMP: [(&str, &str); 6] = [
    ("Eq", "std::cmp::Eq"),
    ("Ord", "std::cmp::Ord"),
    ("PartialEq", "std::cmp::PartialEq"),
    ("PartialOrd", "std::cmp::PartialOrd"),
    ("TryFrom", "std::convert::TryFrom"),
    ("TryInto", "std::convert::TryInto"),
];

const DERIVES: [(&str, &str); 9] = [
    ("Clone", "std::clone::Clone"),
    ("Copy", "std::marker::Copy"),
    ("Debug", "std::fmt::Debug"),
    ("Default", "std::default::Default"),
    ("Eq", "std::cmp::Eq"),
    ("Hash", "std::hash::Hash"),
    ("Ord", "std::cmp::Ord"),
    ("PartialEq", "std::cmp::PartialEq"),
    ("PartialOrd", "std::cmp::PartialOrd"),
];

const MACROS: [&str; 41] = [
    "print",
    "println",
    "eprint",
    "eprintln",
    "format",
    "format_args",
    "write",
    "writeln",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "unreachable",
    "unimplemented",
    "todo",
    "vec",
    "matches",
    "dbg",
    "env",
    "option_env",
    "concat",
    "stringify",
    "include",
    "include_str",
    "include_bytes",
    "line",
    "column",
    "file",
    "module_path",
    "cfg",
    "compile_error",
    "thread_local",
    "is_x86_feature_detected",
    "const_format_args",
    "try",
    "debug_assert_matches",
    "assert_matches",
    "addr_of",
];

struct Syn<'a> {
    src: &'a str,
    file: String,
    uses: HashMap<String, Vec<String>>,
    local_macros: HashSet<String>,
    all_targets: bool,
    derives: &'a mut HashSet<Pos>,
}

fn path_segs(t: &str) -> Vec<String> {
    t.split("::").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn collect_use(n: Node, src: &str, prefix: &[String], map: &mut HashMap<String, Vec<String>>) {
    let full = |segs: Vec<String>| prefix.iter().cloned().chain(segs).collect::<Vec<_>>();
    match n.kind() {
        "identifier" => {
            let name = text(n, src).to_string();
            map.insert(name.clone(), full(vec![name]));
        }
        "self" => {
            if let Some(l) = prefix.last() {
                map.insert(l.clone(), prefix.to_vec());
            }
        }
        "scoped_identifier" => {
            let segs = path_segs(text(n, src));
            if let Some(l) = segs.last().cloned() {
                map.insert(l, full(segs));
            }
        }
        "use_as_clause" => {
            let (Some(p), Some(a)) = (n.child_by_field_name("path"), n.child_by_field_name("alias")) else { return };
            let alias = text(a, src).to_string();
            let segs = if p.kind() == "self" { vec![] } else { path_segs(text(p, src)) };
            if alias != "_" {
                map.insert(alias, full(segs));
            }
        }
        "scoped_use_list" => {
            let Some(p) = n.child_by_field_name("path") else { return };
            let np: Vec<String> = full(path_segs(text(p, src)));
            if let Some(l) = n.child_by_field_name("list") {
                for c in named_children(l) {
                    collect_use(c, src, &np, map);
                }
            }
        }
        "use_list" => {
            for c in named_children(n) {
                collect_use(c, src, prefix, map);
            }
        }
        _ => {}
    }
}

impl Syn<'_> {
    fn pos(&self, n: Node) -> Pos {
        let p = n.start_position();
        (self.file.clone(), p.row as u32 + 1, p.column as u32 + 1)
    }

    /// The canonical `std` path of a path written in the source, if it names the standard library.
    fn resolve(&self, segs: &[String]) -> Option<String> {
        let head = segs.first()?;
        let full: Vec<String> = if ROOTS.contains(&head.as_str()) {
            segs.to_vec()
        } else if let Some(u) = self.uses.get(head) {
            u.iter().cloned().chain(segs[1..].iter().cloned()).collect()
        } else if segs.len() == 1 {
            let p = PRELUDE.iter().chain(PRELUDE_CMP.iter()).find(|(n, _)| n == head)?.1;
            path_segs(p)
        } else {
            return None;
        };
        if !full.first().is_some_and(|r| ROOTS.contains(&r.as_str())) {
            return None;
        }
        Some(std_root(&full))
    }

    fn macro_path(&self, segs: &[String]) -> Option<String> {
        if segs.len() == 1 && self.local_macros.contains(&segs[0]) {
            return None;
        }
        if segs.len() == 1 && !self.uses.contains_key(&segs[0]) {
            return MACROS.contains(&segs[0].as_str()).then(|| format!("std::{}", segs[0]));
        }
        let p = self.resolve(segs)?;
        Some(p)
    }

    fn walk(&mut self, n: Node, out: &mut Externals) {
        let mut pending: Vec<String> = vec![];
        let mut c = n.walk();
        for ch in n.children(&mut c) {
            if ch.kind() == "attribute_item" {
                let t = text(ch, self.src).replace(' ', "");
                if t.starts_with("#[derive(") {
                    self.derive(ch, out);
                }
                pending.push(t);
                continue;
            }
            let test_only = pending.iter().any(|a| a == "#[cfg(test)]" || a == "#[test]" || a.starts_with("#[bench"));
            pending.clear();
            if test_only && !self.all_targets {
                continue;
            }
            if crate::is_comment(ch) {
                continue;
            }
            self.node(ch, out);
        }
    }

    fn derive(&mut self, attr: Node, out: &mut Externals) {
        let mut stack = vec![attr];
        let mut tree = None;
        while let Some(x) = stack.pop() {
            if x.kind() == "token_tree" {
                tree = Some(x);
                break;
            }
            let mut c = x.walk();
            stack.extend(x.children(&mut c));
        }
        let Some(tree) = tree else { return };
        let mut c = tree.walk();
        let (mut segs, mut start): (Vec<String>, Option<Node>) = (vec![], None);
        let mut done: Vec<(Vec<String>, Node)> = vec![];
        for ch in tree.children(&mut c) {
            match ch.kind() {
                "identifier" => {
                    start.get_or_insert(ch);
                    segs.push(text(ch, self.src).to_string());
                }
                "," | ")" => {
                    if let Some(s) = start.take() {
                        done.push((std::mem::take(&mut segs), s));
                    }
                }
                _ => {}
            }
        }
        for (segs, st) in done {
            let at = self.pos(st);
            self.derives.insert(at.clone());
            let path = if segs.len() == 1 && !self.uses.contains_key(&segs[0]) {
                DERIVES.iter().find(|(n, _)| *n == segs[0]).map(|(_, p)| (*p).to_string())
            } else {
                self.resolve(&segs)
            };
            if let Some(p) = path {
                out.add(Kind::Macro, format!("{p}!"), "derive", "syntax", &at, None, Some("derive"));
                out.add(Kind::Trait, p, "derive", "syntax", &at, None, None);
            }
        }
    }

    /// Whether a type node stands where a trait is named.
    fn is_trait_position(&self, node: Node) -> bool {
        let mut top = node;
        while let Some(p) = top.parent() {
            if p.kind() == "generic_type" && p.child_by_field_name("type").is_some_and(|t| t.id() == top.id()) {
                top = p;
            } else {
                break;
            }
        }
        let Some(p) = top.parent() else { return false };
        match p.kind() {
            "trait_bounds" => true,
            "impl_item" => p.child_by_field_name("trait").is_some_and(|t| t.id() == top.id()),
            "abstract_type" | "dynamic_type" => p.child_by_field_name("trait").is_some_and(|t| t.id() == top.id()),
            _ => false,
        }
    }

    fn token_macros(&mut self, tree: Node, out: &mut Externals) {
        let kids: Vec<Node> = {
            let mut c = tree.walk();
            tree.children(&mut c).collect()
        };
        for (i, k) in kids.iter().enumerate() {
            match k.kind() {
                "identifier" if kids.get(i + 1).is_some_and(|x| x.kind() == "!") => {
                    let segs = vec![text(*k, self.src).to_string()];
                    if let Some(p) = self.macro_path(&segs) {
                        let at = self.pos(*k);
                        out.add(Kind::Macro, format!("{p}!"), "call", "syntax", &at, None, Some("bang"));
                    }
                }
                "token_tree" => self.token_macros(*k, out),
                "macro_invocation" => self.node(*k, out),
                _ => {}
            }
        }
    }

    fn node(&mut self, n: Node, out: &mut Externals) {
        match n.kind() {
            "use_declaration" | "extern_crate_declaration" => {}
            "macro_invocation" => {
                if let Some(m) = n.child_by_field_name("macro") {
                    let segs = path_segs(text(m, self.src));
                    if let Some(p) = self.macro_path(&segs) {
                        let at = self.pos(n);
                        out.add(Kind::Macro, format!("{p}!"), "call", "syntax", &at, None, Some("bang"));
                    }
                }
                let mut c = n.walk();
                for ch in n.children(&mut c) {
                    if ch.kind() == "token_tree" {
                        self.token_macros(ch, out);
                    }
                }
            }
            "type_identifier" | "scoped_type_identifier" => {
                if self.is_definition_name(n) {
                    return;
                }
                let segs = path_segs(text(n, self.src));
                if let Some(p) = self.resolve(&segs) {
                    if upper(p.rsplit("::").next().unwrap_or("")) {
                        let (kind, role) = if self.is_trait_position(n) { (Kind::Trait, "bound") } else { (Kind::Type, "named") };
                        let at = self.pos(n);
                        out.add(kind, p, role, "syntax", &at, None, None);
                    }
                }
            }
            "scoped_identifier" => {
                if let Some(p) = n.child_by_field_name("path") {
                    match p.kind() {
                        "identifier" | "scoped_identifier" => {
                            let segs = path_segs(text(p, self.src));
                            if segs.last().is_some_and(|l| upper(l)) {
                                if let Some(r) = self.resolve(&segs) {
                                    let at = self.pos(n);
                                    out.add(Kind::Type, r, "named", "syntax", &at, None, None);
                                }
                            }
                        }
                        _ => self.node(p, out),
                    }
                }
            }
            "impl_item" => {
                self.imp(n, out);
                self.walk(n, out);
            }
            _ => self.walk(n, out),
        }
    }

    fn is_definition_name(&self, n: Node) -> bool {
        n.parent().is_some_and(|p| {
            matches!(p.kind(), "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" | "associated_type")
                && p.child_by_field_name("name").is_some_and(|x| x.id() == n.id())
        })
    }

    /// `impl Trait for T`: the trait, and each method the impl defines.
    fn imp(&mut self, n: Node, out: &mut Externals) {
        let Some(tr) = n.child_by_field_name("trait") else { return };
        let head = if tr.kind() == "generic_type" { tr.child_by_field_name("type").unwrap_or(tr) } else { tr };
        let segs = path_segs(text(head, self.src));
        let Some(tp) = self.resolve(&segs) else { return };
        let at = self.pos(tr);
        out.add(Kind::Trait, tp.clone(), "implement", "syntax", &at, None, None);
        let Some(body) = n.child_by_field_name("body") else { return };
        for f in named_children(body) {
            if f.kind() == "function_item" {
                if let Some(name) = f.child_by_field_name("name") {
                    let at = self.pos(name);
                    let path = format!("{tp}::{}", text(name, self.src));
                    out.add(Kind::Function, path, "implement", "syntax", &at, Some(tp.clone()), None);
                }
            }
        }
    }
}

/// Read the syntax of the corpus files: macros, derives, traits and the types written in the source.
/// Returns the positions of derive attributes, to tell the code they generate in MIR.
pub fn read_syntax(dir: &Path, files: &[String], all_targets: bool, out: &mut Externals) -> R<HashSet<Pos>> {
    let mut derives = HashSet::new();
    for rel in files {
        let src = fs::read_to_string(dir.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
        let tree = parse_rust(&src);
        let root = tree.root_node();
        let mut uses = HashMap::new();
        let mut local_macros = HashSet::new();
        let mut stack = vec![root];
        while let Some(x) = stack.pop() {
            match x.kind() {
                "use_declaration" => {
                    if let Some(a) = x.child_by_field_name("argument") {
                        collect_use(a, &src, &[], &mut uses);
                    }
                }
                "macro_definition" => {
                    if let Some(nm) = x.child_by_field_name("name") {
                        local_macros.insert(text(nm, &src).to_string());
                    }
                }
                _ => {}
            }
            let mut c = x.walk();
            stack.extend(x.children(&mut c));
        }
        // local `use` of a std path shadows nothing: a name bound to a non-std path is not std
        let mut syn = Syn { src: &src, file: rel.clone(), uses, local_macros, all_targets, derives: &mut derives };
        syn.walk(root, out);
    }
    Ok(derives)
}

// ---------------------------------------------------------------------------------------------------------------
// Driver

pub(crate) struct Target {
    pub(crate) kind: &'static str,
    pub(crate) name: String,
}

pub(crate) fn targets(dir: &Path, all_targets: bool) -> R<Vec<Target>> {
    let o = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !o.status.success() {
        return Err(format!("cargo metadata: {}", String::from_utf8_lossy(&o.stderr)));
    }
    let v: Value = serde_json::from_slice(&o.stdout).map_err(|e| e.to_string())?;
    let root = fs::canonicalize(dir).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for p in v["packages"].as_array().into_iter().flatten() {
        let mf = p["manifest_path"].as_str().unwrap_or("");
        if fs::canonicalize(mf).ok().and_then(|m| m.parent().map(Path::to_path_buf)) != Some(root.clone()) {
            continue;
        }
        for t in p["targets"].as_array().into_iter().flatten() {
            let kinds: Vec<&str> = t["kind"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let name = t["name"].as_str().unwrap_or("").to_string();
            if kinds.iter().any(|k| matches!(*k, "lib" | "rlib" | "proc-macro")) {
                out.push(Target { kind: "lib", name });
            } else if kinds.contains(&"bin") {
                out.push(Target { kind: "bin", name });
            } else if kinds.contains(&"test") && all_targets {
                out.push(Target { kind: "test", name });
            }
        }
    }
    Ok(out)
}

/// The MIR of one target, from `cargo rustc`.
pub(crate) fn mir_of(dir: &Path, target_dir: &Path, t: &Target, unit_tests: bool) -> R<String> {
    mir_in(dir, target_dir, t, unit_tests, None)
}

/// As `mir_of`, in a build of the crate (`crate::builds`) other than the default one: its target, features and profile,
/// and its `--cfg` and target features as the flags of every crate, in a target directory of its own.
pub(crate) fn mir_in(dir: &Path, target_dir: &Path, t: &Target, unit_tests: bool, build: Option<&crate::builds::Build>) -> R<String> {
    let build = build.filter(|b| !b.is_default());
    let release = build.is_some_and(|b| b.release);
    let mut args = vec!["rustc".to_string(), if release { "--release".into() } else { "--profile=check".to_string() }];
    if let Some(b) = build {
        // the build's target and features; which targets of the crate are compiled is chosen here
        args.extend(b.cargo_args().into_iter().filter(|a| !matches!(a.as_str(), "--lib" | "--bins" | "--tests" | "--release")));
    }
    match t.kind {
        "lib" => args.push("--lib".into()),
        "bin" => args.extend(["--bin".into(), t.name.clone()]),
        _ => args.extend(["--test".into(), t.name.clone()]),
    }
    args.extend(["--".into(), "-Zunpretty=mir".into(), "-Ztrim-diagnostic-paths=no".into(), "-Zmir-include-spans=yes".into(), "-Zmir-opt-level=0".into()]);
    if unit_tests && t.kind != "test" {
        args.push("--test".into());
    }
    let mut c = Command::new("cargo");
    c.args(&args).current_dir(dir).env("CARGO_TARGET_DIR", target_dir).env("CARGO_TERM_COLOR", "never").env("RUSTC_BOOTSTRAP", "1");
    if let Some(b) = build {
        c.env("CARGO_TARGET_DIR", target_dir.join("builds").join(format!("{}-mir", b.name)));
        c.env_remove("RUSTFLAGS").env("CARGO_ENCODED_RUSTFLAGS", b.rustflags().join("\x1f"));
    }
    // The MIR is printed on stdout; a stale check build would print nothing.
    c.env("CARGO_INCREMENTAL", "0");
    let o = c.output().map_err(|e| format!("cargo rustc: {e}"))?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        return Err(format!("cargo rustc ({} {}): {}", t.kind, t.name, err.lines().filter(|l| l.starts_with("error")).collect::<Vec<_>>().join("; ")));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Every external item the crate uses. `all_targets` adds the unit tests of the library and binaries and the
/// integration tests, as `cargo check --all-targets` does (benches and examples are not corpus files).
pub fn externals(dir: &Path, all_targets: bool) -> R<(String, Externals)> {
    let info = crate_info(dir)?;
    let krate = format!("{} {}", info.name, info.version);
    let files: Vec<String> = corpus_files(dir, all_targets)?.into_iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
    let mut out = Externals::default();
    let derives = read_syntax(dir, &files, all_targets, &mut out)?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let target_dir = cargo::target_dir(&cwd);
    let mut mir_out = Externals::default();
    for t in targets(dir, all_targets)? {
        let mir = mir_of(dir, &target_dir, &t, all_targets && t.kind != "test")?;
        out.targets.push(format!("{} {}{}", t.kind, t.name, if all_targets && t.kind != "test" { " (with unit tests)" } else { "" }));
        read_mir(&mir, &derives, &mut mir_out);
    }
    merge(&mut out, mir_out);
    Ok((krate, out))
}

/// Combine the MIR's items into the syntax's: sites are a set, so a site seen by both is counted once; a type that
/// MIR infers is added only when the source never names it; a name that MIR proves is a trait is not a type.
fn merge(out: &mut Externals, mir: Externals) {
    let traits: HashSet<String> = out.items.keys().chain(mir.items.keys()).filter(|(k, _)| *k == Kind::Trait).map(|(_, p)| p.clone()).collect();
    let syntax_types: HashSet<String> = out.items.keys().filter(|(k, _)| *k == Kind::Type).map(|(_, p)| p.clone()).collect();
    // syntax types that are traits
    let moved: Vec<(Kind, String)> = out.items.keys().filter(|(k, p)| *k == Kind::Type && traits.contains(p)).cloned().collect();
    for key in moved {
        if let Some(it) = out.items.remove(&key) {
            let e = out.items.entry((Kind::Trait, key.1)).or_default();
            absorb(e, it);
        }
    }
    for ((kind, path), it) in mir.items {
        if kind == Kind::Type && (syntax_types.contains(&path) || traits.contains(&path)) {
            continue;
        }
        let e = out.items.entry((kind, path)).or_default();
        absorb(e, it);
    }
    for (k, v) in mir.skipped {
        *out.skipped.entry(k).or_insert(0) += v;
    }
}

fn absorb(e: &mut Item, it: Item) {
    for (f, s) in it.sites {
        e.sites.entry(f).or_default().extend(s);
    }
    e.roles.extend(it.roles);
    e.via.extend(it.via);
    if it.trait_path.is_some() {
        e.trait_path = it.trait_path;
    }
    if it.form.is_some() {
        e.form = it.form;
    }
    e.impls.extend(it.impls);
}

/// The summary of an externals list.
pub fn summary(krate: &str, e: &Externals) -> String {
    let kinds: Vec<String> = Kind::ALL.iter().map(|k| format!("{} {}", e.count(*k), k.name())).collect();
    format!("{krate}: {} external items ({})", e.items.len(), kinds.join(", "))
}

/// Read a committed externals file back: (kind, path) of each item.
pub fn read_list(j: &Value) -> R<Vec<(Kind, String)>> {
    let mut out = vec![];
    for it in j["items"].as_array().ok_or("externals: no items")? {
        let k = it["kind"].as_str().and_then(Kind::parse).ok_or("externals: bad kind")?;
        out.push((k, it["path"].as_str().ok_or("externals: no path")?.to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherent_method_drops_generics() {
        let f = parse_fn_path("std::collections::HashMap::<Elem, usize>::insert").unwrap();
        assert_eq!(f.path, "std::collections::HashMap::insert");
        assert_eq!(f.trait_path, None);
    }

    #[test]
    fn a_call_names_the_impl_it_resolves_to() {
        let at = |p: &str| parse_fn_path(p).unwrap().resolves.unwrap();
        // a trait method: the impl is the trait with its arguments for the head of the self type
        assert_eq!(at("<core::str::iter::Chars<'_> as core::iter::Iterator>::next"), "impl std::iter::Iterator for std::str::iter::Chars");
        assert_eq!(at("<std::vec::Vec<usize> as std::ops::Index<usize>>::index"), "impl std::ops::Index<usize> for std::vec::Vec");
        assert_eq!(at("<u64 as std::convert::From<u8>>::from"), "impl std::convert::From<u8> for u64");
        assert_eq!(at("<&[u8] as std::iter::IntoIterator>::into_iter"), "impl std::iter::IntoIterator for &[u8]");
        // a closure's position says nothing about the impl
        assert_eq!(at("<<&Iter1 as std::iter::IntoIterator>::IntoIter as std::iter::Iterator>::next"), "impl std::iter::Iterator for <&Iter1 as std::iter::IntoIterator>::IntoIter");
        // through a type parameter there is no impl yet
        assert_eq!(at("<I as std::iter::Iterator>::next"), "impl std::iter::Iterator for a type parameter `I`");
        // an inherent method: the type's own impl; a primitive's
        assert_eq!(at("std::collections::HashMap::<Elem, usize>::insert"), "impl std::collections::HashMap");
        assert_eq!(at("core::str::<impl str>::len"), "impl str");
        assert_eq!(at("<str>::chars"), "impl str");
        // a free function resolves to none
        assert_eq!(parse_fn_path("std::cmp::max::<u8>").unwrap().resolves, None);
        assert_eq!(self_head("std::iter::Filter<I, {closure@src/lib.rs:7:3: 7:9}>"), "std::iter::Filter");
        assert_eq!(self_head("(u8, {closure@src/lib.rs:7:3: 7:9})"), "(u8, {closure})");
    }

    #[test]
    fn trait_method_names_the_trait() {
        let f = parse_fn_path("<std::str::Chars<'_> as std::iter::Iterator>::count").unwrap();
        assert_eq!(f.path, "std::iter::Iterator::count");
        assert_eq!(f.trait_path.as_deref(), Some("std::iter::Iterator"));
        let g = parse_fn_path("<<&Iter2 as std::iter::IntoIterator>::IntoIter as std::iter::Iterator>::enumerate").unwrap();
        assert_eq!(g.path, "std::iter::Iterator::enumerate");
        let h = parse_fn_path("<usize as std::convert::From<u8>>::from").unwrap();
        assert_eq!(h.path, "std::convert::From::from");
    }

    #[test]
    fn primitive_methods_live_under_std_primitive() {
        assert_eq!(parse_fn_path("core::str::<impl str>::chars").unwrap().path, "std::primitive::str::chars");
        assert_eq!(parse_fn_path("core::slice::<impl [bool]>::iter").unwrap().path, "std::primitive::slice::iter");
        assert_eq!(parse_fn_path("std::char::methods::<impl char>::is_whitespace").unwrap().path, "std::primitive::char::is_whitespace");
        assert_eq!(parse_fn_path("core::num::<impl usize>::saturating_sub").unwrap().path, "std::primitive::usize::saturating_sub");
        assert_eq!(parse_const_path("std::char::MAX").unwrap(), "std::primitive::char::MAX");
        assert_eq!(parse_const_path("std::f64::consts::PI").unwrap(), "std::f64::consts::PI");
    }

    #[test]
    fn core_and_alloc_fold_into_std() {
        let f = parse_fn_path("core::mem::swap::<u8>").unwrap();
        assert_eq!(f.path, "std::mem::swap");
        assert!(f.internal);
        assert_eq!(std_root(&["alloc".into(), "vec".into(), "Vec".into()]), "std::vec::Vec");
        assert_eq!(std_root(&["std".into(), "collections".into(), "hash_map".into(), "HashMap".into()]), "std::collections::HashMap");
    }

    #[test]
    fn local_items_are_not_external() {
        assert!(parse_fn_path("flat_index").is_none());
        assert!(parse_fn_path("<ValueType as MyTrait>::go").is_none());
        assert!(parse_fn_path("GrowingHashmapChar::<ValueType>::lookup").is_none());
    }

    #[test]
    fn mir_sites_are_user_spans_only() {
        let mir = "fn f(_1: &str) -> usize {\n    bb0: {\n        _2 = core::str::<impl str>::len(copy _1) -> [return: bb1, unwind continue]; // scope 0 at src/lib.rs:3:5: 3:12\n                                         // mir::ConstOperand\n                                         // + span: src/lib.rs:3:7: 3:10\n                                         // + const_: Const { ty: fn(&str) -> usize {core::str::<impl str>::len}, val: Value(core::str::<impl str>::len) }\n        _3 = std::vec::from_elem::<u8>(const 0_u8, move _4) -> [return: bb2, unwind continue]; // scope 0 at /rustc/abc/library/alloc/src/macros.rs:47:9: 47:42\n                                         // mir::ConstOperand\n                                         // + span: /rustc/abc/library/alloc/src/macros.rs:47:9: 47:42\n                                         // + const_: Const { ty: fn(u8, usize) -> std::vec::Vec<u8> {std::vec::from_elem::<u8>}, val: Value(std::vec::from_elem::<u8>) }\n    }\n}\n";
        let mut out = Externals::default();
        read_mir(mir, &HashSet::new(), &mut out);
        let paths: Vec<_> = out.paths().collect();
        assert_eq!(paths, vec![(Kind::Function, "std::primitive::str::len")]);
        assert_eq!(out.items[&(Kind::Function, "std::primitive::str::len".to_string())].uses(), 1);
    }

    #[test]
    fn operators_fold_into_their_trait() {
        let mir = "fn f() {\n    bb0: {\n        _2 = <std::vec::Vec<usize> as std::ops::Index<usize>>::index(copy _1, const 0) -> [return: bb1, unwind continue]; // scope 0 at src/lib.rs:3:5: 3:12\n                                         // mir::ConstOperand\n                                         // + span: src/lib.rs:3:5: 3:12\n                                         // + const_: Const { ty: x, val: Value(<std::vec::Vec<usize> as std::ops::Index<usize>>::index) }\n    }\n}\n";
        let mut out = Externals::default();
        read_mir(mir, &HashSet::new(), &mut out);
        assert_eq!(out.paths().collect::<Vec<_>>(), vec![(Kind::Trait, "std::ops::Index")]);
    }

    fn syn(src: &str, all: bool) -> Externals {
        let dir = std::env::temp_dir().join(format!("bare-rust-ext-{}-{}", std::process::id(), src.len()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("lib.rs"), src).unwrap();
        let mut out = Externals::default();
        read_syntax(&dir, &["lib.rs".to_string()], all, &mut out).unwrap();
        let _ = fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn syntax_finds_macros_derives_traits_and_types() {
        let e = syn(
            "use std::collections::HashMap;\nuse std::hash::{Hasher, BuildHasherDefault};\n#[derive(Clone, Debug, std::hash::Hash)]\nstruct A(HashMap<u8, u8>);\nimpl Hasher for A { fn finish(&self) -> u64 { 0 } fn write(&mut self, b: &[u8]) { println!(\"{}\", b.len()); assert_eq!(1, 1); } }\nfn f<T: Iterator + Default>(_t: T) -> Option<BuildHasherDefault<A>> { let _ = std::cmp::Ordering::Less; vec![1u8]; None }\n",
            true,
        );
        let has = |k: Kind, p: &str| e.items.contains_key(&(k, p.to_string()));
        assert!(has(Kind::Macro, "std::println!") && has(Kind::Macro, "std::assert_eq!") && has(Kind::Macro, "std::vec!"));
        assert!(has(Kind::Macro, "std::fmt::Debug!") && has(Kind::Macro, "std::hash::Hash!") && has(Kind::Macro, "std::clone::Clone!"));
        assert!(has(Kind::Trait, "std::hash::Hasher") && has(Kind::Trait, "std::iter::Iterator") && has(Kind::Trait, "std::default::Default"));
        assert!(has(Kind::Function, "std::hash::Hasher::finish") && has(Kind::Function, "std::hash::Hasher::write"));
        assert!(has(Kind::Type, "std::collections::HashMap") && has(Kind::Type, "std::hash::BuildHasherDefault"));
        assert!(has(Kind::Type, "std::option::Option") && has(Kind::Type, "std::cmp::Ordering"));
        assert!(!e.items.keys().any(|(_, p)| p == "A" || p.ends_with("::A")), "local types are not external: {:?}", e.items.keys());
    }

    #[test]
    fn test_code_is_left_out_unless_all_targets() {
        let src = "#[cfg(test)]\nmod t { use std::collections::BTreeMap; fn f(_m: BTreeMap<u8, u8>) { assert!(true); } }\nfn g() -> String { String::new() }\n";
        let without = syn(src, false);
        assert!(!without.items.keys().any(|(_, p)| p.contains("BTreeMap") || p.contains("assert")));
        assert!(without.items.contains_key(&(Kind::Type, "std::string::String".to_string())));
        let with = syn(src, true);
        assert!(with.items.contains_key(&(Kind::Type, "std::collections::BTreeMap".to_string())));
    }
}
