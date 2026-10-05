//! Check 5, the mechanical half (proposal 9): the library file covers every external item the corpus uses, is valid,
//! and its signatures are the real ones, by coercing the real `std` item to the function-pointer type printed from
//! the entry's slots and compiling the lot in a scratch crate.
use crate::cargo;
use crate::externals::{self, Kind};
use crate::json::canonical;
use crate::types::Ty;
use crate::{first_error, parse_rust, R};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA: &str = include_str!("../schema/y-constraints-0.schema.json");

/// The library file of a crate and version: `<libraries>/<crate>-<version>.constraints.json`.
pub fn library_path(libraries: &Path, krate: &str, version: &str) -> PathBuf {
    libraries.join(format!("{krate}-{version}.constraints.json"))
}

// ---------------------------------------------------------------------------------------------------------------
// Validity

/// The words a clause of each kind may start with: a mechanical check of the head, not a parser of language 15.
fn heads(kind: &str) -> &'static [&'static str] {
    match kind {
        "require" => &["require"],
        "ensure" => &["ensure"],
        "on failure" => &["on failure"],
        "fails" => &["fails"],
        "fails only" => &["fails only"],
        "panics" => &["fails panic when"],
        "never panics" => &["never panics"],
        "result must be used" => &["result must be used"],
        "effects" => &[
            "effects none",
            "effects only",
            "no io",
            "no clock",
            "no randomness",
            "no suspension",
            "deterministic",
            "idempotent by",
            "commutes with",
            "depends only on",
            "terminates",
            "reads only",
            "writes only",
            "emits only",
            "sends only",
            "calls only",
        ],
        "law" => &["law"],
        "callback" => &["callback"],
        "invariant" => &["invariant"],
        "access" => &[
            "access",
            "readonly",
            "immutable",
            "frozen after",
            "write once",
            "append only",
            "monotonic",
            "mutates only",
            "interior",
            "unchanged",
            "exclusive",
            "sole",
            "disjoint",
            "shared readonly",
        ],
        "hold" => &["hold"],
        "compat" => &["version", "compatible with", "replaces", "was", "deprecated", "stable", "visible to"],
        _ => &[],
    }
}

fn type_strings(sub: &Value) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut add = |what: String, v: &Value| {
        if let Some(s) = v.as_str() {
            out.push((what, s.to_string()));
        }
    };
    add("type".into(), &sub["type"]);
    for p in sub["params"].as_array().into_iter().flatten() {
        add(format!("param `{}`", p["name"].as_str().unwrap_or("?")), &p["type"]);
    }
    add("result".into(), &sub["result"]["type"]);
    add("result fails".into(), &sub["result"]["fails"]);
    for f in sub["fields"].as_array().into_iter().flatten() {
        add(format!("field `{}`", f["name"].as_str().unwrap_or("?")), &f["type"]);
    }
    out
}

/// Everything that is wrong with a library file's content: the schema, the rules the schema cannot say, and the
/// canonical layout of `text`.
pub fn validate(file: &Value, text: Option<&str>) -> Vec<String> {
    let mut errs = vec![];
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema");
    match jsonschema::validator_for(&schema) {
        Ok(v) => errs.extend(v.iter_errors(file).map(|e| format!("schema: {e} at {}", e.instance_path()))),
        Err(e) => errs.push(format!("schema: {e}")),
    }
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for (i, sub) in file["subjects"].as_array().into_iter().flatten().enumerate() {
        let (kind, name) = (sub["kind"].as_str().unwrap_or(""), sub["name"].as_str().unwrap_or(""));
        let at = format!("subject {i} ({kind} {name})");
        if !seen.insert((kind.to_string(), name.to_string())) {
            errs.push(format!("{at}: duplicate subject"));
        }
        if kind == "macro" && !name.ends_with('!') {
            errs.push(format!("{at}: a macro is named with a trailing `!`"));
        }
        if kind != "macro" && name.ends_with('!') {
            errs.push(format!("{at}: only a macro is named with a trailing `!`"));
        }
        let root = file["module"].as_str().unwrap_or("std");
        if !name.starts_with(&format!("{root}::")) {
            errs.push(format!("{at}: the name is not a canonical path under `{root}`"));
        }
        if kind == "macro" && sub["form"].is_null() {
            errs.push(format!("{at}: a macro has a `form`"));
        }
        for (what, t) in type_strings(sub) {
            if let Err(e) = Ty::parse(&t) {
                errs.push(format!("{at}: {what}: {e}"));
            }
        }
        let mut names = BTreeSet::new();
        for g in sub["generics"].as_array().into_iter().flatten() {
            if !names.insert(g["name"].as_str().unwrap_or("").to_string()) {
                errs.push(format!("{at}: generic `{}` is declared twice", g["name"].as_str().unwrap_or("")));
            }
        }
        for (j, c) in sub["clauses"].as_array().into_iter().flatten().enumerate() {
            let (k, t) = (c["kind"].as_str().unwrap_or(""), c["text"].as_str().unwrap_or(""));
            let hs = heads(k);
            if !hs.is_empty() && !hs.iter().any(|h| t == *h || t.starts_with(&format!("{h} ")) || t.starts_with(&format!("{h}:"))) {
                errs.push(format!("{at}: clause {j}: a `{k}` clause starts with {} , not `{}`", hs.iter().map(|h| format!("`{h}`")).collect::<Vec<_>>().join(" or "), t.split_whitespace().next().unwrap_or("")));
            }
            if t.matches('(').count() != t.matches(')').count() {
                errs.push(format!("{at}: clause {j}: unbalanced parentheses in `{t}`"));
            }
        }
    }
    if let Some(t) = text {
        if t != canonical(file) {
            errs.push("format: the file is not in canonical form (`bare-rust libfmt <file>` rewrites it)".into());
        }
    }
    errs
}

// ---------------------------------------------------------------------------------------------------------------
// Coverage

/// The items of an externals list that the library has no subject for.
pub fn missing(items: &[(Kind, String)], file: &Value) -> Vec<(Kind, String)> {
    let have: BTreeSet<(String, String)> = file["subjects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| (s["kind"].as_str().unwrap_or("").to_string(), s["name"].as_str().unwrap_or("").to_string()))
        .collect();
    items
        .iter()
        .filter(|(k, p)| {
            let kinds: &[&str] = if *k == Kind::Type { &["type", "alias"] } else { &[k.name()] };
            !kinds.iter().any(|kk| have.contains(&(kk.to_string(), p.clone())))
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// Signature check: generation

/// What the generator makes of one subject.
#[derive(Debug, PartialEq)]
pub enum Gen {
    /// A function pointer, constant or type that must compile.
    Checked(String),
    /// The path must resolve; nothing more is said about the slots.
    Resolved(String),
    /// The generator cannot handle it, and why.
    Unchecked(String),
}

fn word_replace(s: &str, from: &str, to: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with(from) {
            let before = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
            let end = i + from.len();
            let after = end >= s.len() || !(b[end].is_ascii_alphanumeric() || b[end] == b'_');
            if before && after {
                out.push_str(to);
                i = end;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Host text with the lifetime put on its elided references.
fn host_with(text: &str, lt: Option<&str>) -> String {
    let Some(l) = lt else { return text.to_string() };
    let mut out = String::new();
    let cs: Vec<char> = text.chars().collect();
    for (i, c) in cs.iter().enumerate() {
        out.push(*c);
        if *c == '&' && cs.get(i + 1) != Some(&'\'') && cs.get(i + 1) != Some(&'&') {
            out.push_str(l);
            out.push(' ');
        }
    }
    out
}

fn has_ref(v: &Value) -> bool {
    match v.get("host").and_then(Value::as_str) {
        Some(h) => h.contains('&'),
        None => v.as_str().and_then(|s| Ty::parse(s).ok()).is_some_and(|t| t.has_ref()),
    }
}

fn ty_rust(v: &Value, access: &Value, lt: Option<&str>) -> Result<String, String> {
    if let Some(h) = v.get("host").and_then(Value::as_str) {
        return Ok(host_with(h, lt));
    }
    let s = v.as_str().ok_or("a type slot is missing")?;
    Ty::parse(s)?.rust(access["rights"] == "write", lt)
}

fn bounds_of(g: &Value) -> Vec<String> {
    let mut b: Vec<String> = vec![];
    for k in ["bounds", "where"] {
        b.extend(g[k].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string));
    }
    b
}

/// `<T: A + B, 'a>` and the where clause, of a subject's generics. A generic named `Self` becomes `Self_`.
fn generics_of(sub: &Value) -> (String, String) {
    let mut params = vec![];
    for g in sub["generics"].as_array().into_iter().flatten() {
        let n = g["name"].as_str().unwrap_or("");
        let n = if n == "Self" { "Self_" } else { n };
        let b = bounds_of(g).join(" + ");
        params.push(if b.is_empty() { n.to_string() } else { format!("{n}: {b}") });
    }
    // lifetimes first
    params.sort_by_key(|p| !p.starts_with('\''));
    let preds: Vec<String> = sub["where"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| {
            let b: Vec<&str> = w["bounds"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            format!("{}: {}", w["for"].as_str().unwrap_or("_"), b.join(" + "))
        })
        .collect();
    let g = if params.is_empty() { String::new() } else { format!("<{}>", params.join(", ")) };
    let w = if preds.is_empty() { String::new() } else { format!(" where {}", preds.join(", ")) };
    (g, w)
}

/// The Rust expression for a function subject's path, or why there is none.
fn fn_path(sub: &Value) -> Result<String, String> {
    let name = sub["name"].as_str().unwrap_or("");
    let is_self = sub["generics"].as_array().into_iter().flatten().any(|g| g["name"] == "Self");
    if is_self {
        let (head, method) = name.rsplit_once("::").ok_or("name has no method")?;
        let tr = sub["trait"].as_str().unwrap_or(head);
        return Ok(format!("<Self_ as {tr}>::{method}"));
    }
    if let Some(rest) = name.strip_prefix("std::primitive::") {
        let (prim, m) = rest.split_once("::").ok_or("primitive name")?;
        return match prim {
            "slice" => Ok(format!("<[_]>::{m}")),
            "array" | "tuple" | "pointer" | "reference" => Err(format!("primitive form `{prim}`")),
            _ => Ok(name.to_string()),
        };
    }
    Ok(name.to_string())
}

fn gen_function(sub: &Value) -> Gen {
    let go = || -> Result<String, String> {
        let (g, mut w) = generics_of(sub);
        let path = fn_path(sub)?;
        // a trait method can only be called on a `Self` that implements the trait: not something an entry restates
        if let Some(tr) = path.strip_prefix("<Self_ as ").and_then(|p| p.rsplit_once(">::")).map(|(t, _)| t) {
            w = if w.is_empty() { format!(" where Self_: {tr}") } else { format!("{w}, Self_: {tr}") };
        }
        let params = sub["params"].as_array().ok_or("no params")?;
        let (mut binder, mut ps) = (vec![], vec![]);
        let mut lt_of: BTreeMap<String, String> = BTreeMap::new();
        for (i, p) in params.iter().enumerate() {
            let lt = has_ref(&p["type"]).then(|| format!("'p{i}"));
            if let Some(l) = &lt {
                binder.push(l.clone());
                lt_of.insert(p["name"].as_str().unwrap_or("").to_string(), l.clone());
            }
            ps.push(ty_rust(&p["type"], &p["access"], lt.as_deref())?);
        }
        let mut ret = String::new();
        if !sub["result"].is_null() {
            let r = &sub["result"];
            let lt = if has_ref(&r["type"]) || has_ref(&r["fails"]) {
                let hold = &r["hold"];
                if let Some(f) = hold["from"].as_str() {
                    Some(lt_of.get(f).cloned().ok_or(format!("result borrows from `{f}`, which holds no reference"))?)
                } else if let Some(l) = hold["lifetime"].as_str() {
                    Some(l.to_string())
                } else if lt_of.len() == 1 {
                    lt_of.values().next().cloned()
                } else {
                    return Err("result lifetime is not stated".into());
                }
            } else {
                None
            };
            let t = ty_rust(&r["type"], &r["access"], lt.as_deref())?;
            ret = if r["fails"].is_null() {
                format!(" -> {t}")
            } else {
                format!(" -> std::result::Result<{t}, {}>", ty_rust(&r["fails"], &Value::Null, lt.as_deref())?)
            };
        }
        let hr = if binder.is_empty() { String::new() } else { format!("for<{}> ", binder.join(", ")) };
        let body = format!("let _: {hr}fn({}){ret} = {path};", ps.join(", "));
        Ok(format!("fn {{id}}{g}(){w} {{ {} }}", word_replace(&body, "Self", "Self_")))
    };
    match go() {
        Ok(s) => Gen::Checked(s),
        Err(e) => Gen::Unchecked(e),
    }
}

/// The check for one subject, with `{id}` where its function name goes.
pub fn generate(sub: &Value) -> Gen {
    let (kind, name) = (sub["kind"].as_str().unwrap_or(""), sub["name"].as_str().unwrap_or(""));
    match kind {
        "function" => gen_function(sub),
        "const" | "static" => {
            let go = || -> Result<String, String> {
                let t = ty_rust(&sub["type"], &Value::Null, Some("'static"))?;
                Ok(format!("fn {{id}}() {{ let _: {t} = {}; }}", fn_path(sub)?))
            };
            go().map_or_else(Gen::Unchecked, Gen::Checked)
        }
        "type" | "alias" => {
            let (g, w) = generics_of(sub);
            let args: Vec<String> = sub["generics"].as_array().into_iter().flatten().filter_map(|g| g["name"].as_str()).map(str::to_string).collect();
            let mut args = args;
            args.sort_by_key(|p| !p.starts_with('\''));
            let a = if args.is_empty() { String::new() } else { format!("<{}>", args.join(", ")) };
            Gen::Checked(format!("fn {{id}}{g}(){w} {{ let _: Option<*const {name}{a}> = None; }}"))
        }
        "trait" => {
            if sub["generics"].as_array().is_none_or(Vec::is_empty) {
                Gen::Checked(format!("fn {{id}}<T: ?Sized + {name}>() {{}}"))
            } else {
                Gen::Resolved(format!("fn {{id}}() {{ use {name} as _; }}"))
            }
        }
        "macro" => match sub["form"].as_str() {
            Some("derive") => Gen::Checked(format!("#[derive({})] struct {{id}};", name.trim_end_matches('!'))),
            _ => Gen::Unchecked("macro".into()),
        },
        k => Gen::Unchecked(format!("kind `{k}`")),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Signature check: compile

#[derive(Default, Debug)]
pub struct SigReport {
    pub checked: Vec<String>,
    pub resolved: Vec<String>,
    /// Reason to subjects.
    pub unchecked: BTreeMap<String, Vec<String>>,
    /// Subject and what rustc said.
    pub mismatches: Vec<(String, String)>,
    /// Subjects checked by kind: (checked, mismatched).
    pub by_kind: BTreeMap<String, (usize, usize)>,
}

impl SigReport {
    pub fn unchecked_total(&self) -> usize {
        self.unchecked.values().map(Vec::len).sum()
    }
}

/// Compile the checks of every subject of a library file in a scratch crate under `work`.
pub fn check_signatures(file: &Value, work: &Path, target: &Path) -> R<SigReport> {
    let mut rep = SigReport::default();
    let mut src = String::from("#![allow(warnings, clippy::all)]\n");
    let mut ranges: Vec<(usize, usize, String, String)> = vec![];
    let mut line = 2;
    for (i, sub) in file["subjects"].as_array().into_iter().flatten().enumerate() {
        let (kind, name) = (sub["kind"].as_str().unwrap_or("").to_string(), sub["name"].as_str().unwrap_or("").to_string());
        let id = format!("_c{i}");
        let (code, resolved) = match generate(sub) {
            Gen::Checked(c) => (c, false),
            Gen::Resolved(c) => (c, true),
            Gen::Unchecked(why) => {
                rep.unchecked.entry(why).or_default().push(name);
                continue;
            }
        };
        let code = code.replace("{id}", &id);
        if let Some(e) = first_error(&parse_rust(&code)) {
            rep.mismatches.push((name, format!("the generated item does not parse (at {e}): {code}")));
            continue;
        }
        let n = code.lines().count();
        ranges.push((line, line + n - 1, name.clone(), kind));
        if resolved {
            rep.resolved.push(name.clone());
        }
        src.push_str(&code);
        src.push('\n');
        line += n;
    }
    let dir = work.join("libcheck");
    fs::create_dir_all(dir.join("src")).map_err(|e| format!("{}: {e}", dir.display()))?;
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"libcheck_scratch\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n").map_err(|e| e.to_string())?;
    fs::write(dir.join("src/lib.rs"), &src).map_err(|e| e.to_string())?;
    let run = cargo::diagnostics(&dir, target, "check")?;
    let mut bad: BTreeMap<String, String> = BTreeMap::new();
    let mut vague: BTreeSet<String> = BTreeSet::new();
    let mut stray = vec![];
    for d in run.diags.iter().filter(|d| d.is_error()) {
        let owner = ranges.iter().find(|(a, b, _, _)| d.file == "src/lib.rs" && *a <= d.line && d.line <= *b);
        match owner {
            Some((_, _, name, _)) if matches!(d.code.as_deref(), Some("E0282" | "E0283")) => {
                vague.insert(name.clone());
            }
            Some((_, _, name, _)) => {
                bad.entry(name.clone()).or_insert_with(|| format!("{}: {}", d.code.as_deref().unwrap_or("error"), d.message));
            }
            None => stray.push(format!("{}:{} {}", d.file, d.line, d.message)),
        }
    }
    if !run.success && bad.is_empty() && vague.is_empty() && stray.is_empty() {
        return Err(format!("the scratch crate did not compile: {}", run.stderr));
    }
    for s in stray {
        rep.mismatches.push(("(scratch crate)".into(), s));
    }
    for (_, _, name, kind) in &ranges {
        let e = rep.by_kind.entry(kind.clone()).or_default();
        if let Some(m) = bad.get(name) {
            rep.mismatches.push((name.clone(), m.clone()));
            e.1 += 1;
        } else if vague.contains(name) {
            rep.unchecked.entry("generics not fixed by the signature".into()).or_default().push(name.clone());
            rep.resolved.retain(|r| r != name);
        } else if !rep.resolved.contains(name) {
            rep.checked.push(name.clone());
            e.0 += 1;
        }
    }
    Ok(rep)
}

// ---------------------------------------------------------------------------------------------------------------
// The command

#[derive(Default, Debug)]
pub struct LibReport {
    pub krate: String,
    pub library: Option<PathBuf>,
    pub subjects: usize,
    pub items: usize,
    pub missing: Vec<(Kind, String)>,
    pub invalid: Vec<String>,
    pub sigs: Option<SigReport>,
    /// Why the check could not run, if it could not.
    pub skipped: Option<String>,
}

impl LibReport {
    pub fn covered(&self) -> usize {
        self.items - self.missing.len()
    }
    pub fn coverage_ok(&self) -> bool {
        self.skipped.is_none() && self.missing.is_empty()
    }
    pub fn valid_ok(&self) -> bool {
        self.invalid.is_empty()
    }
    pub fn sigs_ok(&self) -> bool {
        self.sigs.as_ref().is_none_or(|s| s.mismatches.is_empty())
    }
    /// Without `strict`, items still missing do not fail the check (the library is incomplete, not wrong).
    pub fn ok(&self, strict: bool) -> bool {
        self.valid_ok() && self.sigs_ok() && (!strict || self.coverage_ok())
    }

    pub fn to_json(&self) -> Value {
        let s = self.sigs.as_ref();
        json!({
            "crate": self.krate,
            "library": self.library.as_ref().map(|p| p.display().to_string()),
            "subjects": self.subjects,
            "items": self.items,
            "covered": if self.skipped.is_some() { 0 } else { self.covered() },
            "missing": self.missing.iter().map(|(k, p)| format!("{} {p}", k.name())).collect::<Vec<_>>(),
            "invalid": self.invalid,
            "skipped": self.skipped,
            "signatures": s.map(|s| json!({
                "checked": s.checked.len(), "resolved": s.resolved.len(), "unchecked": s.unchecked_total(),
                "unchecked_by_reason": s.unchecked.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<serde_json::Map<_, _>>(),
                "mismatches": s.mismatches.iter().map(|(n, m)| json!({"subject": n, "error": m})).collect::<Vec<_>>(),
            })),
        })
    }

    pub fn print(&self, strict: bool) {
        let st = |ok: bool| if ok { "ok" } else { "FAIL" };
        println!("bare-rust libcheck {}", self.krate);
        if let Some(why) = &self.skipped {
            println!("  library   {why}");
        }
        if let Some(p) = &self.library {
            println!("  library   {}: {} subjects", p.display(), self.subjects);
        }
        let cov = if self.coverage_ok() { "ok" } else if strict { "FAIL" } else { "incomplete" };
        println!("  coverage  {cov}  {} of {} external items have a subject; {} missing", self.covered(), self.items, self.missing.len());
        for k in Kind::ALL {
            let m: Vec<&str> = self.missing.iter().filter(|(kk, _)| *kk == k).map(|(_, p)| p.as_str()).collect();
            if !m.is_empty() {
                println!("              missing {} ({}):", k.name(), m.len());
                for p in m {
                    println!("                {p}");
                }
            }
        }
        println!("  schema    {}  {} problems", st(self.valid_ok()), self.invalid.len());
        for e in &self.invalid {
            println!("              {e}");
        }
        if let Some(s) = &self.sigs {
            println!(
                "  signature {}  {} checked, {} path-resolved only, {} signature unchecked, {} mismatched",
                st(self.sigs_ok()),
                s.checked.len(),
                s.resolved.len(),
                s.unchecked_total(),
                s.mismatches.len()
            );
            for (why, v) in &s.unchecked {
                println!("              unchecked ({why}): {}", v.len());
            }
            for (n, m) in &s.mismatches {
                println!("              mismatch {n}: {m}");
            }
        }
    }
}

/// Check 5, mechanical half, for one crate: its committed externals list against the std library file.
pub fn libcheck(dir: &Path, libraries: &Path, work: &Path, target: &Path) -> R<LibReport> {
    let info = crate::corpus::crate_info(dir)?;
    let mut rep = LibReport { krate: format!("{} {}", info.name, info.version), ..Default::default() };
    let list = libraries.join("externals").join(format!("{}.json", info.name));
    let items: Vec<(Kind, String)> = match fs::read_to_string(&list) {
        Ok(t) => externals::read_list(&serde_json::from_str(&t).map_err(|e| format!("{}: {e}", list.display()))?)?,
        Err(_) => {
            let (_, e) = externals::externals(dir, true)?;
            e.paths().map(|(k, p)| (k, p.to_string())).collect()
        }
    };
    rep.items = items.len();
    let version = externals::rustc_version().ok_or("rustc --version failed")?;
    let path = library_path(libraries, "std", &version);
    let Ok(text) = fs::read_to_string(&path) else {
        rep.missing = items;
        rep.skipped = Some(format!("no library file {}", path.display()));
        return Ok(rep);
    };
    let file: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    rep.library = Some(path);
    rep.subjects = file["subjects"].as_array().map_or(0, Vec::len);
    rep.missing = missing(&items, &file);
    rep.invalid = validate(&file, Some(&text));
    if file["crate"].as_str() != Some(&format!("std {version}")) {
        rep.invalid.push(format!("crate is {:?}, but the toolchain is std {version}", file["crate"]));
    }
    rep.sigs = Some(check_signatures(&file, &work.join(&info.name), target)?);
    Ok(rep)
}

/// Rewrite library files in canonical form.
pub fn format_files(paths: &[PathBuf]) -> R<usize> {
    let mut n = 0;
    for p in paths {
        let t = fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let v: Value = serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?;
        let c = canonical(&v);
        if c != t {
            fs::write(p, c).map_err(|e| format!("{}: {e}", p.display()))?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(j: &str) -> Value {
        serde_json::from_str(j).unwrap()
    }

    #[test]
    fn a_method_becomes_a_function_pointer_with_named_lifetimes() {
        let s = sub(
            r#"{"kind":"function","name":"std::collections::HashMap::get",
            "generics":[{"name":"K","bounds":["std::cmp::Eq","std::hash::Hash","std::borrow::Borrow<Q>"]},{"name":"V","bounds":[]},
                        {"name":"S","bounds":["std::hash::BuildHasher"]},{"name":"Q","bounds":["?Sized","std::hash::Hash","std::cmp::Eq"]}],
            "params":[{"name":"self","type":"ref<map<K, V, S> as 'std::collections::HashMap'>","access":{"rights":"read","guarantee":"readers"}},
                      {"name":"k","type":"ref<Q>","access":{"rights":"read","guarantee":"readers"}}],
            "result":{"type":"ref<V>?","hold":{"kind":"borrows","from":"self"}}}"#,
        );
        let Gen::Checked(c) = generate(&s) else { panic!() };
        assert!(c.contains("for<'p0, 'p1> fn(&'p0 std::collections::HashMap<K, V, S>, &'p1 Q) -> Option<&'p0 V> = std::collections::HashMap::get;"), "{c}");
        assert!(c.starts_with("fn {id}<K: std::cmp::Eq + std::hash::Hash + std::borrow::Borrow<Q>, V, S: std::hash::BuildHasher, Q: ?Sized + std::hash::Hash + std::cmp::Eq>()"), "{c}");
    }

    #[test]
    fn primitives_and_slices_have_paths_rustc_accepts() {
        let chars = sub(r#"{"kind":"function","name":"std::primitive::str::chars","params":[{"name":"self","type":"ref<text>","access":{"rights":"read","guarantee":"readers"}}],"result":{"type":"std::str::Chars","hold":{"kind":"borrows","from":"self"}}}"#);
        let Gen::Checked(c) = generate(&chars) else { panic!() };
        assert!(c.contains("for<'p0> fn(&'p0 str) -> std::str::Chars = std::primitive::str::chars;"), "{c}");
        let iter = sub(r#"{"kind":"function","name":"std::primitive::slice::iter","generics":[{"name":"T","bounds":[]}],"params":[{"name":"self","type":{"host":"&[T]"}}],"result":{"type":{"host":"std::slice::Iter<'_, T>"},"hold":{"kind":"borrows","from":"self"}}}"#);
        let Gen::Checked(c) = generate(&iter) else { panic!() };
        assert!(c.contains("fn(&'p0 [T]) -> std::slice::Iter<'_, T> = <[_]>::iter;"), "{c}");
    }

    #[test]
    fn trait_methods_are_checked_through_a_self_generic() {
        let zip = sub(r#"{"kind":"function","name":"std::iter::Iterator::zip","generics":[{"name":"Self","bounds":["std::iter::Iterator"]},{"name":"U","bounds":["std::iter::IntoIterator"]}],"params":[{"name":"self","type":"Self"},{"name":"other","type":"U"}],"result":{"type":{"host":"std::iter::Zip<Self, U::IntoIter>"}}}"#);
        let Gen::Checked(c) = generate(&zip) else { panic!() };
        assert!(c.contains("fn(Self_, U) -> std::iter::Zip<Self_, U::IntoIter> = <Self_ as std::iter::Iterator>::zip;"), "{c}");
    }

    #[test]
    fn what_the_generator_cannot_handle_is_unchecked_with_a_reason() {
        let bang = sub(r#"{"kind":"macro","name":"std::vec!","form":"bang"}"#);
        assert_eq!(generate(&bang), Gen::Unchecked("macro".into()));
        let no_from = sub(r#"{"kind":"function","name":"std::x::f","params":[{"name":"a","type":"ref<u8>","access":{"rights":"read","guarantee":"readers"}},{"name":"b","type":"ref<u8>","access":{"rights":"read","guarantee":"readers"}}],"result":{"type":"ref<u8>"}}"#);
        assert_eq!(generate(&no_from), Gen::Unchecked("result lifetime is not stated".into()));
        let coll = sub(r#"{"kind":"function","name":"std::x::g","params":[{"name":"a","type":"list<u8>"}]}"#);
        assert!(matches!(generate(&coll), Gen::Unchecked(m) if m.contains("host type name")));
        let derive = sub(r#"{"kind":"macro","name":"std::hash::Hash!","form":"derive"}"#);
        assert_eq!(generate(&derive), Gen::Checked("#[derive(std::hash::Hash)] struct {id};".into()));
    }

    #[test]
    fn clause_heads_and_names_are_checked() {
        let f = sub(
            r#"{"format":"y-constraints/0","host":"rust","crate":"std 1.97.0","module":"std","status":{"assumed":"hand-written"},
            "subjects":[{"kind":"function","name":"std::cmp::max","params":[{"name":"a","type":"u8"}],
              "clauses":[{"kind":"ensure","text":"require x : true","source":"s"},{"kind":"unexpressed","text":"t","source":"s"}]},
              {"kind":"macro","name":"std::vec","form":"bang"}]}"#,
        );
        let e = validate(&f, None).join("\n");
        assert!(e.contains("a `ensure` clause starts with `ensure`"), "{e}");
        assert!(e.contains("schema"), "unexpressed without a reason: {e}");
        assert!(e.contains("trailing `!`"), "{e}");
    }

    #[test]
    fn coverage_reports_the_missing_by_path() {
        let f = sub(r#"{"subjects":[{"kind":"alias","name":"std::fmt::Result"},{"kind":"function","name":"std::cmp::max"}]}"#);
        let items = vec![(Kind::Function, "std::cmp::max".to_string()), (Kind::Type, "std::fmt::Result".into()), (Kind::Trait, "std::cmp::Ord".into())];
        assert_eq!(missing(&items, &f), vec![(Kind::Trait, "std::cmp::Ord".to_string())]);
    }
}
