//! Check 5, the mechanical half (proposal 9): the library file covers every external item the corpus uses, is valid,
//! and its signatures are the real ones, by coercing the real `std` item to the function-pointer type printed from
//! the entry's slots and compiling the lot in a scratch crate.
use crate::cargo;
use crate::clause::{lint, lint_given, GivenCtx, Scope};
use crate::externals::{self, Kind};
use crate::json::canonical;
use crate::types::{Lower, Ty};
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
pub fn heads(kind: &str) -> &'static [&'static str] {
    match kind {
        "require" => &["require"],
        "ensure" => &["ensure"],
        "on failure" => &["on failure"],
        "fails" => &["fails"],
        "fails only" => &["fails only"],
        "panics" => &["fails panic when", "fails panic"],
        "budget" => &["budget"],
        "layout" => &["layout", "size", "align"],
        "derive" => &["derive"],
        "never panics" => &["never panics except exhausted", "never panics"],
        "result must be used" => &["result must be used", "must be used"],
        "obliges" => &["obliges caller"],
        "effects" => &[
            "effects none",
            "effects only",
            "pure",
            "no device",
            "devices only",
            "no io",
            "no clock",
            "no randomness",
            "no suspension",
            "deterministic",
            "idempotent by",
            "commutes with",
            "depends only on",
            "terminates",
            "evaluable at compile time",
            "reads only",
            "writes only",
            "emits only",
            "sends only",
            "calls only",
        ],
        "law" => &["law"],
        "callback" => &["callback"],
        "protocol" => &["protocol"],
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
        "hold" => &["hold", "lifetime"],
        "implements" => &["implements"],
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
    add("trait".into(), &sub["trait"]);
    add("self".into(), &sub["self"]);
    for (n, t) in sub["assoc"].as_object().into_iter().flatten() {
        add(format!("associated type `{n}`"), t);
    }
    for (i, e) in sub["implements"].as_array().into_iter().flatten().enumerate() {
        add(format!("implements {i}"), &e["trait"]);
        for (n, t) in e["assoc"].as_object().into_iter().flatten() {
            add(format!("implements {i}: associated type `{n}`"), t);
        }
    }
    for p in sub["params"].as_array().into_iter().flatten() {
        add(format!("param `{}`", p["name"].as_str().unwrap_or("?")), &p["type"]);
    }
    add("result".into(), &sub["result"]["type"]);
    add("result fails".into(), &sub["result"]["fails"]);
    for f in sub["fields"].as_array().into_iter().flatten() {
        add(format!("field `{}`", f["name"].as_str().unwrap_or("?")), &f["type"]);
    }
    for v in sub["variants"].as_array().into_iter().flatten() {
        for f in v["fields"].as_array().into_iter().flatten() {
            add(format!("variant `{}`", v["name"].as_str().unwrap_or("?")), &f["type"]);
        }
    }
    out
}

/// The views of a file: (name, subject, host path), the file's own and those of its subjects (whose `of` is the subject).
pub fn views_of(file: &Value) -> Vec<(String, String, String)> {
    let q = |v: &Value, k: &str| v[k].as_str().unwrap_or("").trim_matches('\'').to_string();
    let mut out: Vec<(String, String, String)> = file["views"].as_array().into_iter().flatten().map(|v| (q(v, "name"), q(v, "of"), q(v, "host"))).collect();
    for s in file["subjects"].as_array().into_iter().flatten() {
        out.extend(s["views"].as_array().into_iter().flatten().map(|v| (q(v, "name"), s["name"].as_str().unwrap_or("").to_string(), q(v, "host"))));
    }
    out
}

fn is_host_path(p: &str) -> bool {
    p.contains("::") && ["std::", "core::", "alloc::"].iter().any(|r| p.starts_with(r))
}

/// Everything that is wrong with a library file's content: the schema, the rules the schema cannot say, and the
/// canonical layout of `text`.
pub fn validate(file: &Value, text: Option<&str>) -> Vec<String> {
    validate_with(file, text, &crate::host::builds(None))
}

/// The clause kinds a `given` may qualify: the promises that depend on what an implementation does (language 6.12).
const GIVEN_KINDS: [&str; 5] = ["never panics", "effects", "ensure", "fails", "panics"];

/// A bound's methods the file knows, with those of its supertraits: the functions named `<trait>::m`. `None` when the
/// file has no trait subject by that name, or no method of it, so that a method name cannot be checked.
fn bound_methods(by_name: &BTreeMap<&str, &Value>, file: &Value, bound: &str, depth: usize) -> Option<BTreeSet<String>> {
    let head = bound.split('<').next().unwrap_or("").trim();
    let tr = by_name.get(head).filter(|t| t["kind"] == "trait")?;
    let prefix = format!("{head}::");
    let mut ms: BTreeSet<String> = file["subjects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["kind"] == "function")
        .filter_map(|s| s["name"].as_str()?.strip_prefix(&prefix).filter(|m| !m.contains("::")).map(str::to_string))
        .collect();
    if depth < 8 {
        for sup in tr["bounds"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            ms.extend(bound_methods(by_name, file, sup, depth + 1).unwrap_or_default());
        }
    }
    (!ms.is_empty()).then_some(ms)
}

/// The generic type parameters of a subject, each with the methods of its bounds when every bound is known to the file.
fn given_generics(sub: &Value, by_name: &BTreeMap<&str, &Value>, file: &Value) -> BTreeMap<String, Option<BTreeSet<String>>> {
    let mut out = BTreeMap::new();
    for g in sub["generics"].as_array().into_iter().flatten() {
        let name = g["name"].as_str().unwrap_or("");
        if name.starts_with('\'') {
            continue;
        }
        let strs = |v: &Value| -> Vec<String> { v.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect() };
        let mut bounds: Vec<String> = strs(&g["bounds"]).into_iter().chain(strs(&g["where"])).collect();
        for w in sub["where"].as_array().into_iter().flatten().filter(|w| w["for"].as_str() == Some(name)) {
            bounds.extend(strs(&w["bounds"]));
        }
        bounds.retain(|b| !b.starts_with('\'') && !b.starts_with('?'));
        let mut all = Some(BTreeSet::new());
        for b in &bounds {
            match (bound_methods(by_name, file, b, 0), all.as_mut()) {
                (Some(ms), Some(a)) => a.extend(ms),
                _ => all = None,
            }
        }
        out.insert(name.to_string(), all.filter(|a| !a.is_empty()));
    }
    out
}

/// `validate`, with the build settings a `given build` may name taken from the host profile.
pub fn validate_with(file: &Value, text: Option<&str>, builds: &BTreeSet<String>) -> Vec<String> {
    let mut errs = vec![];
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema");
    match jsonschema::validator_for(&schema) {
        Ok(v) => errs.extend(v.iter_errors(file).map(|e| format!("schema: {e} at {}", e.instance_path()))),
        Err(e) => errs.push(format!("schema: {e}")),
    }
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let failures: BTreeSet<String> = file["failures"].as_object().into_iter().flat_map(|m| m.keys().cloned()).collect();
    // a failure is `"runtime"`, or `{ "class": …, "payload": { field: type } }`, whose fields `fails F { field: e }` fills
    let mut payload: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (f, d) in file["failures"].as_object().into_iter().flatten() {
        for (field, t) in d["payload"].as_object().into_iter().flatten() {
            payload.entry(f.clone()).or_default().insert(field.clone());
            if let Err(e) = t.as_str().map_or(Err("a payload field has a type".into()), Ty::parse) {
                errs.push(format!("failure `{f}`: payload field `{field}`: {e}"));
            }
        }
    }
    // views: pure host functions that name state a value holds, declared for the file or for one subject
    let subject_names: BTreeSet<&str> = file["subjects"].as_array().into_iter().flatten().filter_map(|s| s["name"].as_str()).collect();
    let all_views: Vec<(String, String, String)> = views_of(file);
    let mut seen_views = BTreeSet::new();
    for (name, of, host) in &all_views {
        if !seen_views.insert((name.clone(), of.clone())) {
            errs.push(format!("view `{name}` of `{of}` is declared twice"));
        }
        if !subject_names.contains(of.as_str()) {
            errs.push(format!("view `{name}`: `{of}` is not a subject of the file"));
        }
        if !is_host_path(host) || !host.starts_with(&format!("{of}::")) {
            errs.push(format!("view `{name}`: `{host}` is not a host path under `{of}`"));
        }
    }
    let subject_by_name: BTreeMap<&str, &Value> = file["subjects"].as_array().into_iter().flatten().filter_map(|s| Some((s["name"].as_str()?, s))).collect();
    let mut variants: BTreeSet<String> = file["subjects"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["variants"].as_array().into_iter().flatten().filter_map(|v| v["name"].as_str().map(str::to_string)))
        .collect();
    // language 4: a clause in a module's file (`src/x.rs`) may name a constant of the module, and call a function of the
    // module that is bound `pure` or `effects none` (§ 6.3), by its own name. A library file names other subjects by
    // quoted host path, so its subjects' last segments, which repeat across modules, are not names here.
    let module_file = file["module"].as_str().is_some_and(|m| m.ends_with(".rs"));
    // a module's file is Rust source, where the prelude's variants are always in scope: `x is Ok(_)`, `y is None`
    if module_file {
        variants.extend(["Some", "None", "Ok", "Err"].map(String::from));
    }
    let last = |s: &Value| s["name"].as_str().and_then(|n| n.rsplit("::").next()).map(str::to_string);
    let module_subjects = || file["subjects"].as_array().into_iter().flatten().filter(|_| module_file);
    let file_consts: BTreeSet<String> = module_subjects().filter(|s| s["kind"] == "const").filter_map(last).collect();
    let file_pure: BTreeSet<String> = module_subjects()
        .filter(|s| s["kind"] == "function" && s["clauses"].as_array().into_iter().flatten().any(|c| c["kind"] == "effects" && (c["text"] == "pure" || c["text"] == "effects none")))
        .filter_map(last)
        .collect();
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
        let head = |t: &Value| -> String {
            let s = t.as_str().unwrap_or("");
            s.split(['<', ' ']).next().unwrap_or("").to_string()
        };
        if kind == "impl" {
            if sub["trait"].is_null() || sub["self"].is_null() {
                errs.push(format!("{at}: an impl names its `trait` and the `self` type it is for"));
            }
            if !name.ends_with(&format!(" as {}", head(&sub["trait"]))) {
                errs.push(format!("{at}: an impl is named `<type> as <trait>`, ending in its trait `{}`", head(&sub["trait"])));
            }
        } else {
            if !sub["assoc"].is_null() {
                errs.push(format!("{at}: `assoc` is for an impl"));
            }
            if !sub["self"].is_null() {
                errs.push(format!("{at}: `self` is for an impl"));
            }
        }
        if !sub["implements"].is_null() && kind != "type" {
            errs.push(format!("{at}: `implements` is for a type"));
        }
        for (j, e) in sub["implements"].as_array().into_iter().flatten().enumerate() {
            if let Some(i) = e["impl"].as_str() {
                match subject_by_name.get(i) {
                    Some(t) if t["kind"] == "impl" && head(&t["trait"]) == head(&e["trait"]) => {}
                    Some(_) => errs.push(format!("{at}: implements {j}: `{i}` is not an impl of `{}`", head(&e["trait"]))),
                    None => errs.push(format!("{at}: implements {j}: `{i}` is not a subject of the file")),
                }
            }
        }
        if let Some(i) = sub["impl"].as_str() {
            if kind != "function" || !subject_by_name.get(i).is_some_and(|t| t["kind"] == "impl") {
                errs.push(format!("{at}: `impl` is for a function, and names an impl subject of the file: `{i}`"));
            }
        }
        let mut names = BTreeSet::new();
        for g in sub["generics"].as_array().into_iter().flatten() {
            if !names.insert(g["name"].as_str().unwrap_or("").to_string()) {
                errs.push(format!("{at}: generic `{}` is declared twice", g["name"].as_str().unwrap_or("")));
            }
        }
        let mut scope = Scope { names: variants.clone(), failures: failures.clone(), payload: payload.clone(), ..Scope::default() };
        scope.names.extend(all_views.iter().map(|v| v.0.clone()));
        scope.pure.extend(all_views.iter().map(|v| v.0.clone()));
        scope.names.extend(file_consts.iter().chain(&file_pure).cloned());
        scope.pure.extend(file_pure.iter().cloned());
        if kind == "type" {
            scope.names.insert("Self".into());
            scope.names.insert("other".into());
        }
        for d in sub["derive"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !crate::derive::is_template(d) {
                errs.push(format!("{at}: `{d}` is not a derive template ({})", crate::derive::TEMPLATES.iter().map(|t| t.0).collect::<Vec<_>>().join(", ")));
            }
        }
        if sub["derive"].is_array() && (kind != "type" || sub["fields"].is_null() && sub["variants"].is_null()) {
            errs.push(format!("{at}: `derive` is for a type with `fields` or `variants`"));
        }
        for n in sub["params"].as_array().into_iter().flatten().filter_map(|p| p["name"].as_str()) {
            scope.names.insert(n.to_string());
        }
        for n in sub["generics"].as_array().into_iter().flatten().filter_map(|g| g["name"].as_str()) {
            scope.names.insert(n.trim_start_matches('\'').to_string());
        }
        if matches!(kind, "trait" | "impl") || sub["impl"].is_string() {
            scope.names.insert("Self".into());
        }
        scope.names.extend(sub["assoc"].as_object().into_iter().flatten().map(|(n, _)| n.clone()));
        for l in sub["hold"]["for"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !sub["generics"].as_array().into_iter().flatten().any(|g| g["name"].as_str() == Some(l)) {
                errs.push(format!("{at}: `hold` is for the lifetime `{l}`, which the subject does not declare"));
            }
        }
        // a protocol names the subject's parameters and callbacks, and the methods of a parameter's bounds; a callback's
        // `with` and `returning` names are known to the subject's other clauses (language 6.13)
        let clauses_of = || sub["clauses"].as_array().into_iter().flatten();
        let generics_known = given_generics(sub, &subject_by_name, file);
        for p in sub["params"].as_array().into_iter().flatten() {
            let (n, ty) = (p["name"].as_str().unwrap_or(""), p["type"].as_str().unwrap_or(""));
            scope.events.insert(n.to_string());
            let g = ty.strip_prefix("ref<target type=").and_then(|r| r.strip_suffix('>')).unwrap_or(ty);
            if let Some(ms) = generics_known.get(g) {
                scope.event_methods.insert(n.to_string(), ms.clone());
            }
        }
        for c in clauses_of().filter(|c| c["kind"] == "callback") {
            if let Some(n) = c["text"].as_str().and_then(|t| t.strip_prefix("callback ")).and_then(|t| t.split([' ', ':']).next()) {
                scope.events.insert(n.to_string());
            }
        }
        let before_callbacks = scope.clone();
        let binds: Vec<String> = clauses_of().filter(|c| c["kind"] == "callback").filter_map(|c| c["text"].as_str()).flat_map(|t| crate::clause::callback_binds(t, &before_callbacks)).collect();
        scope.names.extend(binds);
        for (j, c) in sub["clauses"].as_array().into_iter().flatten().enumerate() {
            let (k, t) = (c["kind"].as_str().unwrap_or(""), c["text"].as_str().unwrap_or(""));
            if k == "derive" {
                let d = t.strip_prefix("derive ").unwrap_or("");
                if !crate::derive::is_template(d) || !name.ends_with(&format!("::{d}!")) {
                    errs.push(format!("{at}: clause {j}: `{t}` names a template that is not this derive's"));
                }
            } else if k != "unexpressed" {
                if !c["given"].is_null() {
                    if !GIVEN_KINDS.contains(&k) {
                        errs.push(format!("{at}: clause {j}: a `{k}` clause is not qualified by `given` (only {})", GIVEN_KINDS.join(", ")));
                    }
                    let items: Vec<&str> = c["given"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                    let ctx = GivenCtx { generics: generics_known.clone(), has_self: matches!(kind, "trait" | "impl" | "type") || sub["impl"].is_string() || !sub["trait"].is_null(), trait_method: kind == "function" && (!sub["trait"].is_null() || sub["impl"].is_string()), builds };
                    errs.extend(lint_given(&items, &ctx).into_iter().map(|e| format!("{at}: clause {j}: {e}")));
                }
                if t.split_whitespace().any(|w| w == "given") {
                    errs.push(format!("{at}: clause {j}: `given` is the clause's `given` slot, not part of its text: `{t}`"));
                }
                // language 6.14: `must be used` is for a type or a trait, `result must be used` for a subject with a result;
                // `evaluable at compile time` and `obliges caller` are for a function
                let on_type = matches!(kind, "type" | "trait");
                if k == "result must be used" && on_type != t.starts_with("must be used") {
                    errs.push(format!("{at}: clause {j}: `must be used` is for a type or a trait and `result must be used` for a function or macro: `{t}`"));
                }
                if (t == "evaluable at compile time" || k == "obliges") && kind != "function" {
                    errs.push(format!("{at}: clause {j}: `{}` is for a function: `{t}`", if k == "obliges" { "obliges caller" } else { "evaluable at compile time" }));
                }
                errs.extend(lint(k, t, heads(k), if k == "callback" { &before_callbacks } else { &scope }).into_iter().map(|e| format!("{at}: clause {j}: {e}")));
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
    let t = Ty::parse(s)?;
    t.rust(t.writes(access["rights"].as_str()), lt)
}

/// The same, with each `some I` turned into a generic parameter: a function pointer type cannot hold `impl Trait`.
fn ty_rust_lowered(v: &Value, access: &Value, lt: Option<&str>, lower: &mut Lower) -> Result<String, String> {
    match v.as_str() {
        Some(s) => {
            let t = Ty::parse(s)?;
            t.rust_lowered(t.writes(access["rights"].as_str()), lt, lower)
        }
        None => ty_rust(v, access, lt),
    }
}

/// An impl's self type, its trait, and its trait with the associated types it fixes (a bound; a path cannot hold them).
fn impl_parts(imp: &Value) -> Result<(String, String, String), String> {
    let self_t = ty_rust(&imp["self"], &Value::Null, None)?;
    Ok((self_t, ty_rust(&imp["trait"], &Value::Null, None)?, trait_with_assoc(&imp["trait"], &imp["assoc"], imp)?))
}

/// `std::iter::Iterator<Item = char>`: the trait of an impl or of an `implements` entry, with the associated types
/// it fixes. A borrow in an associated type needs a lifetime a bound can name: the subject's only one.
fn trait_with_assoc(tr: &Value, assoc: &Value, owner: &Value) -> Result<String, String> {
    let t = ty_rust(tr, &Value::Null, None)?;
    let lts: Vec<&str> = owner["generics"].as_array().into_iter().flatten().filter_map(|g| g["name"].as_str()).filter(|n| n.starts_with('\'')).collect();
    let mut parts = vec![];
    for (n, v) in assoc.as_object().into_iter().flatten() {
        let lt = if has_ref(v) {
            match lts.as_slice() {
                [one] => Some(*one),
                _ => return Err("an associated type borrows, and the subject has not exactly one lifetime to name".into()),
            }
        } else {
            None
        };
        parts.push(format!("{n} = {}", ty_rust(v, &Value::Null, lt)?));
    }
    Ok(match (parts.is_empty(), t.strip_suffix('>')) {
        (true, _) => t,
        (false, Some(open)) => format!("{open}, {}>", parts.join(", ")),
        (false, None) => format!("{t}<{}>", parts.join(", ")),
    })
}

fn bounds_of(g: &Value) -> Vec<String> {
    let mut b: Vec<String> = vec![];
    for k in ["bounds", "where"] {
        b.extend(g[k].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string));
    }
    b
}

/// The generic parameters (`T: A + B`, `'a`), lifetimes first, and the where clause of a subject's generics. A generic
/// named `Self` becomes `Self_`.
fn generics_parts(sub: &Value) -> (Vec<String>, String) {
    let mut params = vec![];
    for g in sub["generics"].as_array().into_iter().flatten() {
        let n = g["name"].as_str().unwrap_or("");
        let n = if n == "Self" { "Self_" } else { n };
        let b = bounds_of(g).join(" + ");
        params.push(if b.is_empty() { n.to_string() } else { format!("{n}: {b}") });
    }
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
    let w = if preds.is_empty() { String::new() } else { format!(" where {}", preds.join(", ")) };
    (params, w)
}

fn angle(params: &[String]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

/// `<T: A + B, 'a>` and the where clause, of a subject's generics.
fn generics_of(sub: &Value) -> (String, String) {
    let (p, w) = generics_parts(sub);
    (angle(&p), w)
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

/// The check of a function. `path` is its Rust path when the name is not (a method of an impl).
///
/// A function pointer cannot hold `impl Trait`, so a `some` in a parameter becomes a generic parameter with its bound, and
/// the function is coerced as before. A `some` result is checked another way: the function is called, on values that
/// are never made (`loop {}`), inside a closure whose result must meet the bound.
fn gen_function(sub: &Value, path: Option<String>) -> Gen {
    let go = || -> Result<String, String> {
        let (mut gp, mut w) = generics_parts(sub);
        let path = match path {
            Some(p) => p,
            None => fn_path(sub)?,
        };
        // a trait method can only be called on a `Self` that implements the trait: not something an entry restates
        if let Some(tr) = path.strip_prefix("<Self_ as ").and_then(|p| p.rsplit_once(">::")).map(|(t, _)| t) {
            w = if w.is_empty() { format!(" where Self_: {tr}") } else { format!("{w}, Self_: {tr}") };
        }
        let params = sub["params"].as_array().ok_or("no params")?;
        let (mut binder, mut ps) = (vec![], vec![]);
        let mut lt_of: BTreeMap<String, String> = BTreeMap::new();
        let mut lower = Lower { inner: "'__e".into(), params: vec![] };
        for (i, p) in params.iter().enumerate() {
            let lt = has_ref(&p["type"]).then(|| format!("'p{i}"));
            if let Some(l) = &lt {
                binder.push(l.clone());
                lt_of.insert(p["name"].as_str().unwrap_or("").to_string(), l.clone());
            }
            ps.push(ty_rust_lowered(&p["type"], &p["access"], lt.as_deref(), &mut lower)?);
        }
        let mut ret = String::new();
        let mut some_result: Option<String> = None;
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
            let rt = r["type"].as_str().map(Ty::parse).transpose()?;
            if let Some(b) = rt.as_ref().and_then(|t| t.bounds_rust("'__e")) {
                if !r["fails"].is_null() {
                    return Err("a `some` result that fails".into());
                }
                some_result = Some(b?);
            } else if rt.as_ref().is_some_and(Ty::has_some) || r["fails"].as_str().and_then(|f| Ty::parse(f).ok()).is_some_and(|t| t.has_some()) {
                return Err("a `some` below the top of the result".into());
            }
            let t = ty_rust(&r["type"], &r["access"], lt.as_deref())?;
            ret = if r["fails"].is_null() {
                format!(" -> {t}")
            } else {
                format!(" -> std::result::Result<{t}, {}>", ty_rust(&r["fails"], &Value::Null, lt.as_deref())?)
            };
        }
        let mut helper = String::new();
        // `evaluable at compile time` (language 6.14.1): a `const fn` that only calls the item, with its arguments moved in
        let const_claim = sub["clauses"].as_array().into_iter().flatten().any(|c| c["kind"] == "effects" && c["text"] == "evaluable at compile time");
        let const_fn = if const_claim && some_result.is_none() {
            let mut cg = binder.clone();
            cg.extend(gp.iter().cloned());
            cg.sort_by_key(|p| !p.starts_with('\''));
            let params: Vec<String> = ps.iter().enumerate().map(|(i, t)| format!("a{i}: {t}")).collect();
            let args: Vec<String> = (0..ps.len()).map(|i| format!("a{i}")).collect();
            Some((cg, params.join(", "), format!("{path}({})", args.join(", "))))
        } else {
            None
        };
        let body = if let Some(bound) = some_result {
            // lifetimes are early-bound here: the values are made by `loop {}`, not coerced from a pointer
            gp.splice(0..0, binder.drain(..));
            let args: Vec<String> = (0..ps.len()).map(|i| format!("a{i}")).collect();
            let lets: Vec<String> = ps.iter().enumerate().map(|(i, t)| format!("let a{i}: {t} = loop {{}};")).collect();
            // the bound is in an item of its own, which takes the generics again: a function inside another cannot use them
            let mut hp = gp.clone();
            if bound.contains("'__e") {
                hp.insert(0, "'__e".into());
            }
            hp.push(format!("R: {bound}"));
            hp.push("F: FnOnce() -> R".into());
            let names: Vec<String> = gp.iter().filter(|p| !p.starts_with('\'')).map(|p| p.split(':').next().unwrap_or("").to_string()).collect();
            let turbofish = names.iter().cloned().chain(["_".to_string(), "_".to_string()]).collect::<Vec<_>>().join(", ");
            helper = format!("fn {{id}}_need{}(_: F){w} {{}}\n", angle(&hp));
            format!("{{id}}_need::<{turbofish}>(|| {{ {} {path}({}) }});", lets.join(" "), args.join(", "))
        } else {
            let hr = if binder.is_empty() { String::new() } else { format!("for<{}> ", binder.join(", ")) };
            format!("let _: {hr}fn({}){ret} = {path};", ps.join(", "))
        };
        if !lower.params.is_empty() {
            gp.extend(lower.params.iter().cloned());
            gp.insert(0, "'__e".into());
        }
        gp.sort_by_key(|p| !p.starts_with('\''));
        let mut code = format!("{}fn {{id}}{}(){w} {{ {} }}", word_replace(&helper, "Self", "Self_"), angle(&gp), word_replace(&body, "Self", "Self_"));
        if let Some((mut cg, params, call)) = const_fn {
            cg.extend(lower.params.iter().cloned());
            if !lower.params.is_empty() {
                cg.insert(0, "'__e".into());
            }
            cg.sort_by_key(|p| !p.starts_with('\''));
            code += &word_replace(&format!("\nconst fn {{id}}_const{}({params}){ret}{w} {{ {call} }}", angle(&cg)), "Self", "Self_");
        }
        Ok(code)
    };
    match go() {
        Ok(s) => Gen::Checked(s),
        Err(e) => Gen::Unchecked(e),
    }
}

/// The two items that prove `self_t: tr` under a subject's generics: a function that holds the bound, with the generics
/// and one more parameter for the type, and a function that names it. The bound is in an item of its own because a
/// function inside another cannot use the other's generics.
fn proves(sub: &Value, suffix: &str, self_t: &str, tr: &str) -> String {
    let (mut params, w) = generics_parts(sub);
    let names: Vec<String> = params.iter().filter(|p| !p.starts_with('\'')).map(|p| p.split(':').next().unwrap_or("").to_string()).collect();
    params.push(format!("X: ?Sized + {tr}"));
    let turbofish = names.iter().cloned().chain([self_t.to_string()]).collect::<Vec<_>>().join(", ");
    let (g, _) = generics_of(sub);
    format!("fn {{id}}{suffix}_need{}(){w} {{}}\nfn {{id}}{suffix}{g}(){w} {{ {{id}}{suffix}_need::<{turbofish}>(); }}", angle(&params))
}

/// An impl: its self type implements its trait, with the associated types it fixes.
fn gen_impl(sub: &Value) -> Gen {
    let go = || -> Result<String, String> {
        let (self_t, _, tr) = impl_parts(sub)?;
        Ok(proves(sub, "", &self_t, &tr))
    };
    go().map_or_else(Gen::Unchecked, Gen::Checked)
}

/// What a type declares it implements, each entry checked as an impl is, with the type's own generics.
fn implements_checks(sub: &Value, id_suffix: &str) -> Result<Vec<String>, String> {
    let name = sub["name"].as_str().unwrap_or("");
    let mut args: Vec<&str> = sub["generics"].as_array().into_iter().flatten().filter_map(|g| g["name"].as_str()).collect();
    args.sort_by_key(|p| !p.starts_with('\''));
    let a = if args.is_empty() { String::new() } else { format!("<{}>", args.join(", ")) };
    let mut out = vec![];
    for (i, e) in sub["implements"].as_array().into_iter().flatten().enumerate() {
        let tr = trait_with_assoc(&e["trait"], &e["assoc"], sub)?;
        out.push(proves(sub, &format!("{id_suffix}{i}"), &format!("{name}{a}"), &tr));
    }
    Ok(out)
}

/// The check for one subject, with `{id}` where its function name goes.
pub fn generate(sub: &Value) -> Gen {
    generate_in(sub, &Value::Null)
}

/// The same, for a subject of `file`: a method of an impl is checked through the impl's self type and trait.
pub fn generate_in(sub: &Value, file: &Value) -> Gen {
    if let Some(i) = sub["impl"].as_str() {
        let imp = file["subjects"].as_array().into_iter().flatten().find(|s| s["name"].as_str() == Some(i) && s["kind"] == "impl");
        let Some(imp) = imp else { return Gen::Unchecked(format!("impl `{i}` is not a subject of the file")) };
        let go = || -> Result<(Value, String), String> {
            let (self_t, tr, _) = impl_parts(imp)?;
            let method = sub["name"].as_str().unwrap_or("").rsplit("::").next().unwrap_or("");
            // the impl's generics come first, then the method's
            let mut merged = sub.clone();
            for k in ["generics", "where"] {
                let all: Vec<Value> = imp[k].as_array().into_iter().flatten().chain(sub[k].as_array().into_iter().flatten()).cloned().collect();
                if !all.is_empty() {
                    merged[k] = Value::Array(all);
                }
            }
            Ok((merged, format!("<{self_t} as {tr}>::{method}")))
        };
        return match go() {
            Ok((m, path)) => gen_function(&m, Some(path)),
            Err(e) => Gen::Unchecked(e),
        };
    }
    generate_plain(sub)
}

fn generate_plain(sub: &Value) -> Gen {
    let (kind, name) = (sub["kind"].as_str().unwrap_or(""), sub["name"].as_str().unwrap_or(""));
    match kind {
        "function" => gen_function(sub, None),
        "impl" => gen_impl(sub),
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
            let mut code = format!("fn {{id}}{g}(){w} {{ let _: Option<*const {name}{a}> = None; }}");
            match implements_checks(sub, "_impl") {
                Ok(more) => more.iter().for_each(|m| code += &format!("\n{m}")),
                Err(e) => return Gen::Unchecked(e),
            }
            Gen::Checked(code)
        }
        "trait" => {
            if sub["generics"].as_array().is_none_or(Vec::is_empty) {
                Gen::Checked(format!("fn {{id}}<T: ?Sized + {name}>() {{}}"))
            } else {
                Gen::Resolved(format!("fn {{id}}() {{ use {name} as _; }}"))
            }
        }
        "macro" => match sub["form"].as_str() {
            Some("derive") => {
                // a derive of a trait with supertraits compiles only with those derived beside it
                let tr = name.trim_end_matches('!');
                let with = match tr.rsplit("::").next() {
                    Some("Eq") => "std::cmp::PartialEq, ",
                    Some("Copy") => "std::clone::Clone, ",
                    Some("PartialOrd") => "std::cmp::PartialEq, ",
                    Some("Ord") => "std::cmp::PartialEq, std::cmp::Eq, std::cmp::PartialOrd, ",
                    _ => "",
                };
                Gen::Checked(format!("#[derive({with}{tr})] struct {{id}};"))
            }
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
    /// Views whose host path resolved, and those whose generics the type's subject does not fix.
    pub views_checked: usize,
    pub views_unchecked: Vec<String>,
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
        let (code, resolved) = match generate_in(sub, file) {
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
    // a view's host path must resolve as a method of its type, whose generics the type's subject gives
    for (name, of, host) in views_of(file) {
        let Some(t) = file["subjects"].as_array().into_iter().flatten().find(|s| s["name"].as_str() == Some(&of)) else { continue };
        let (g, w) = generics_of(t);
        let mut args: Vec<&str> = t["generics"].as_array().into_iter().flatten().filter_map(|g| g["name"].as_str()).collect();
        args.sort_by_key(|a| !a.starts_with('\''));
        let m = host.rsplit("::").next().unwrap_or("");
        let code = if t["kind"] == "trait" {
            format!("fn _v{line}<Self_: {of}>() {{ let _ = <Self_ as {of}>::{m}; }}")
        } else {
            format!("fn _v{line}{g}(){w} {{ let _ = <{of}<{}>>::{m}; }}", args.join(", "))
        };
        let label = format!("view {name} of {of}");
        ranges.push((line, line, label, "view".into()));
        src.push_str(&code);
        src.push('\n');
        line += 1;
    }
    let dir = work.join("libcheck");
    fs::create_dir_all(dir.join("src")).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Cargo's fingerprint of a path package does not depend on where it is, so scratch crates of one name share it, and
    // a crate that compiled in one directory would count as fresh in another: each directory gets a name of its own.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&dir.to_string_lossy(), &mut h);
    let manifest = format!("[package]\nname = \"libcheck_scratch_{:x}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n", std::hash::Hasher::finish(&h));
    fs::write(dir.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
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
        if kind == "view" {
            match (bad.get(name), vague.contains(name)) {
                (Some(m), _) => rep.mismatches.push((name.clone(), m.clone())),
                (None, true) => rep.views_unchecked.push(name.clone()),
                _ => rep.views_checked += 1,
            }
            continue;
        }
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
    /// The host profile's path and its macro arms, when the library has one.
    pub host: Option<(PathBuf, usize)>,
    /// Derive clauses the library's `derive` lists expand to.
    pub expanded: usize,
    /// Sampled runtime checks of callbacks, moves, ends, must-use and `yields` against the real std (`runcheck`).
    pub runtime: Option<crate::runcheck::RuntimeReport>,
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
    /// A run of the real std that contradicts a clause fails the check.
    pub fn runtime_ok(&self) -> bool {
        self.runtime.as_ref().is_none_or(|r| r.contradictions() == 0)
    }
    /// Without `strict`, items still missing do not fail the check (the library is incomplete, not wrong).
    pub fn ok(&self, strict: bool) -> bool {
        self.valid_ok() && self.sigs_ok() && self.runtime_ok() && (!strict || self.coverage_ok())
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
                "views": { "resolved": s.views_checked, "unchecked": s.views_unchecked },
                "mismatches": s.mismatches.iter().map(|(n, m)| json!({"subject": n, "error": m})).collect::<Vec<_>>(),
            })),
            "runtime": self.runtime.as_ref().map(|r| r.to_json()),
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
        if let Some((p, arms)) = &self.host {
            println!("  host      {}: {arms} macro arms; {} derive clauses expanded", p.display(), self.expanded);
        }
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
            println!("              views: {} host paths resolved, {} unchecked", s.views_checked, s.views_unchecked.len());
            for (n, m) in &s.mismatches {
                println!("              mismatch {n}: {m}");
            }
        }
        if let Some(r) = &self.runtime {
            r.print();
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
    let hp = crate::host::host_path(libraries, file["host"].as_str().unwrap_or("rust"), &version);
    let profile: Option<(String, Value)> = match fs::read_to_string(&hp) {
        Ok(t) => Some((t.clone(), serde_json::from_str(&t).map_err(|e| format!("{}: {e}", hp.display()))?)),
        Err(_) => None,
    };
    rep.invalid = validate_with(&file, Some(&text), &crate::host::builds(profile.as_ref().map(|p| &p.1)));
    if file["crate"].as_str() != Some(&format!("std {version}")) {
        rep.invalid.push(format!("crate is {:?}, but the toolchain is std {version}", file["crate"]));
    }
    rep.expanded = file["subjects"].as_array().into_iter().flatten().map(|s| crate::derive::expand(s).len()).sum();
    if let Some((t, p)) = &profile {
        let (arms, errs) = crate::host::validate(p, &file, Some(t));
        rep.invalid.extend(errs);
        rep.host = Some((hp, arms));
    }
    rep.sigs = Some(check_signatures(&file, &work.join(&info.name), target)?);
    if std::env::var_os("BARE_RUST_NO_RUNTIME").is_none() {
        rep.runtime = Some(crate::runcheck::run(&file, &work.join(&info.name), target)?);
    }
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
            "params":[{"name":"self","type":"ref<target type=map<key type=K, value type=V, hasher type=S> as 'std::collections::HashMap'>","access":{"rights":"read","guarantee":"readers"}},
                      {"name":"k","type":"ref<target type=Q>","access":{"rights":"read","guarantee":"readers"}}],
            "result":{"type":"ref<target type=V>?","hold":{"kind":"borrows","from":"self"}}}"#,
        );
        let Gen::Checked(c) = generate(&s) else { panic!() };
        assert!(c.contains("for<'p0, 'p1> fn(&'p0 std::collections::HashMap<K, V, S>, &'p1 Q) -> Option<&'p0 V> = std::collections::HashMap::get;"), "{c}");
        assert!(c.starts_with("fn {id}<K: std::cmp::Eq + std::hash::Hash + std::borrow::Borrow<Q>, V, S: std::hash::BuildHasher, Q: ?Sized + std::hash::Hash + std::cmp::Eq>()"), "{c}");
    }

    #[test]
    fn primitives_and_slices_have_paths_rustc_accepts() {
        let chars = sub(r#"{"kind":"function","name":"std::primitive::str::chars","params":[{"name":"self","type":"ref<target type=string>","access":{"rights":"read","guarantee":"readers"}}],"result":{"type":"std::str::Chars","hold":{"kind":"borrows","from":"self"}}}"#);
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
        let no_from = sub(r#"{"kind":"function","name":"std::x::f","params":[{"name":"a","type":"ref<target type=u8>","access":{"rights":"read","guarantee":"readers"}},{"name":"b","type":"ref<target type=u8>","access":{"rights":"read","guarantee":"readers"}}],"result":{"type":"ref<target type=u8>"}}"#);
        assert_eq!(generate(&no_from), Gen::Unchecked("result lifetime is not stated".into()));
        let coll = sub(r#"{"kind":"function","name":"std::x::g","params":[{"name":"a","type":"list<element type=u8>"}]}"#);
        assert!(matches!(generate(&coll), Gen::Unchecked(m) if m.contains("host type name")));
        let derive = sub(r#"{"kind":"macro","name":"std::hash::Hash!","form":"derive"}"#);
        assert_eq!(generate(&derive), Gen::Checked("#[derive(std::hash::Hash)] struct {id};".into()));
        let eq = sub(r#"{"kind":"macro","name":"std::cmp::Eq!","form":"derive"}"#);
        assert_eq!(generate(&eq), Gen::Checked("#[derive(std::cmp::PartialEq, std::cmp::Eq)] struct {id};".into()));
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
