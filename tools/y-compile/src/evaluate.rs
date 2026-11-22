//! `compile f(…)`, evaluated while the program is built (Y § 3.17), by a build step: the module is lowered once with
//! each `compile` as the call itself, a small crate of that Rust runs every call and prints its value as Rust data, and
//! the module is lowered again with each value written as a constant, from which the value is built once, on first use.
//!
//! The call is the same Rust the program would run, so the value is the one it would return at run time (§ 3.17, "The
//! value is the one the call would return"). A call that fails, or faults, is a compile error that names the call and
//! the failure's case, or the fault.
use crate::model::{Module, TypeDef};
use crate::ty::{pascal, Ty};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use y_parse::Node;

/// One `compile` of a function: where it is, the Rust of its call, the value's type, whether the call may fail, and the
/// name it is bound to.
#[derive(Clone, Debug)]
pub struct Site {
    pub id: String,
    pub function: String,
    pub call: String,
    pub ty: Ty,
    pub fails: bool,
    pub binding: Option<String>,
}

/// The data a value is written as, and how the value is built from it: `&str` and `YString::new(d)` for a string.
pub struct Data {
    pub ty: String,
    /// The Rust that builds the value from its data, which it names `$D`.
    pub build: String,
    /// Whether the data is the value itself, a Rust constant needing nothing built.
    pub plain: bool,
}

/// The data of a structure's fields, or of a case's: a tuple of each field's data, and the Rust that builds the fields
/// from it, `field: value, …`, given the tuple's elements by the names `{prefix}_0`, `{prefix}_1`, ….
fn fields_data(m: &Module, fields: &[(String, Ty)], prefix: &str, depth: usize) -> Option<(String, String)> {
    let mut tys = vec![];
    let mut builds = vec![];
    for (k, (f, t)) in fields.iter().enumerate() {
        let d = data(m, t, depth + 1)?;
        tys.push(format!("{},", d.ty));
        builds.push(format!("{f}: {}", d.build.replace("$D", &format!("{prefix}_{k}"))));
    }
    Some((format!("({})", tys.join(" ")), builds.join(", ")))
}

/// The names that take a tuple of `n` elements apart: `(p_0, p_1,)`.
fn tuple_names(prefix: &str, n: usize) -> String {
    format!("({})", (0..n).map(|k| format!("{prefix}_{k},")).collect::<Vec<_>>().join(" "))
}

/// How a value of a type is written as data and built again, where y-compile can write it: numbers, truth values,
/// strings, a refined type over one, a decimal, a structure of these, a closed type (its payloads of these), and a
/// list, a set or a map of these.
pub fn data(m: &Module, t: &Ty, depth: usize) -> Option<Data> {
    if depth > 8 {
        return None;
    }
    let plain = |ty: &str| Some(Data { ty: ty.to_string(), build: "$D".into(), plain: true });
    match m.resolve(t) {
        Ty::U32 => plain("u32"),
        Ty::I32 => plain("i32"),
        Ty::U64 => plain("u64"),
        Ty::U16 => plain("u16"),
        Ty::Count => plain("usize"),
        Ty::Bool => plain("bool"),
        Ty::F64 => plain("f64"),
        Ty::Str => Some(Data { ty: "&'static str".into(), build: "YString::new($D)".into(), plain: false }),
        Ty::Decimal(p, s) => Some(Data { ty: "&'static str".into(), build: format!("Decimal::<{}>::literal($D)", crate::ware::decimal_args(p, s)), plain: false }),
        Ty::Named(n) => match m.types.get(&n).map(|t| &t.def) {
            Some(TypeDef::Newtype { base, .. }) => {
                let inner = data(m, base, depth + 1)?;
                Some(Data { ty: inner.ty, build: format!("{}::new({}).expect(\"y: a compiled value meets its constraints\")", pascal(&n), inner.build), plain: false })
            }
            Some(TypeDef::Enum { cases }) if cases.iter().all(|c| c.1.is_empty()) => Some(Data { ty: pascal(&n), build: "$D".into(), plain: true }),
            // a structure: the tuple of its fields' data, `(f0, f1,)`
            Some(TypeDef::Struct { fields, .. }) => {
                let p = format!("s{depth}");
                let (ty, build) = fields_data(m, fields, &p, depth)?;
                Some(Data { ty, build: format!("{{ let {} = $D; {} {{ {build} }} }}", tuple_names(&p, fields.len()), pascal(&n)), plain: false })
            }
            // a closed type with payloads: the case's number, then, for each case, its fields' data where it is that
            // case: `(1, None, Some((f0,)))`
            Some(TypeDef::Enum { cases }) => {
                let p = format!("c{depth}");
                let mut tys = vec!["u8,".to_string()];
                let mut arms = vec![];
                for (k, (c, fs)) in cases.iter().enumerate() {
                    let q = format!("{p}_{k}");
                    let (ty, build) = fields_data(m, fs, &q, depth)?;
                    tys.push(format!("Option<{ty}>,"));
                    arms.push(if fs.is_empty() {
                        format!("{k} => {}::{},", pascal(&n), pascal(c))
                    } else {
                        format!("{k} => {{ let {} = {p}_{}.unwrap(); {}::{} {{ {build} }} }}", tuple_names(&q, fs.len()), k + 1, pascal(&n), pascal(c))
                    });
                }
                let names = tuple_names(&p, cases.len() + 1);
                let build = format!("{{ let {names} = $D; match {p}_0 {{ {} _ => unreachable!(\"y: a compiled case\") }} }}", arms.join(" "));
                Some(Data { ty: format!("({})", tys.join(" ")), build, plain: false })
            }
            _ => None,
        },
        Ty::List(e) => {
            let inner = data(m, &e, depth + 1)?;
            let var = format!("d{depth}");
            let each = inner.build.replace("$D", &var);
            Some(Data { ty: format!("&'static [{}]", inner.ty), build: format!("$D.iter().map(|&{var}| {each}).collect::<List<_>>()"), plain: false })
        }
        // a set: its elements, in the order they were added
        Ty::Set(e) => {
            let inner = data(m, &e, depth + 1)?;
            let var = format!("d{depth}");
            let each = inner.build.replace("$D", &var);
            Some(Data { ty: format!("&'static [{}]", inner.ty), build: format!("$D.iter().map(|&{var}| {each}).collect::<y_collections::Set<_>>()"), plain: false })
        }
        // a map: its pairs, in the order their keys were added, or, sorted, in the keys' order
        Ty::Coll(kind @ ("Map" | "SortedMap"), a) if a.len() == 2 => map_data(m, kind, &a[0], &a[1], depth),
        Ty::SortedMap(k, v) => map_data(m, "SortedMap", &k, &v, depth),
        _ => None,
    }
}

fn map_data(m: &Module, kind: &str, k: &Ty, v: &Ty, depth: usize) -> Option<Data> {
    let (key, value) = (data(m, k, depth + 1)?, data(m, v, depth + 1)?);
    let (kv, vv) = (format!("k{depth}"), format!("v{depth}"));
    let build = format!(
        "{{ let mut built = y_collections::{kind}::new(); for &({kv}, {vv}) in $D {{ built.set({}, {}); }} built }}",
        key.build.replace("$D", &kv),
        value.build.replace("$D", &vv)
    );
    Some(Data { ty: format!("&'static [({}, {})]", key.ty, value.ty), build, plain: false })
}

/// The Rust, in the evaluating crate, that prints a value named `v` of a type as its data.
pub fn printer(m: &Module, t: &Ty, v: &str, depth: usize) -> String {
    match m.resolve(t) {
        Ty::F64 | Ty::Str => format!("format!(\"{{:?}}\", {})", if matches!(m.resolve(t), Ty::Str) { format!("{v}.as_str()") } else { v.to_string() }),
        Ty::Decimal(..) => format!("format!(\"{{:?}}\", {v}.to_string())"),
        Ty::Named(n) => match m.types.get(&n).map(|t| &t.def) {
            Some(TypeDef::Newtype { base, .. }) => printer(m, base, &format!("{v}.as_text()"), depth),
            Some(TypeDef::Struct { fields, .. }) => {
                let parts: Vec<String> = fields.iter().map(|(f, t)| printer(m, t, &format!("{v}.{f}"), depth + 1)).collect();
                format!("format!(\"({})\", {})", "{}, ".repeat(parts.len()), parts.join(", "))
            }
            Some(TypeDef::Enum { cases }) if cases.iter().any(|c| !c.1.is_empty()) => {
                let arms: Vec<String> = cases
                    .iter()
                    .enumerate()
                    .map(|(k, (c, fs))| {
                        let slots: Vec<&str> = (0..cases.len()).map(|j| if j == k { "Some({})" } else { "None" }).collect();
                        let names: Vec<String> = (0..fs.len()).map(|j| format!("p{depth}_{j}")).collect();
                        let pattern = if fs.is_empty() { String::new() } else { format!(" {{ {} }}", fs.iter().zip(&names).map(|((f, _), p)| format!("{f}: {p}")).collect::<Vec<_>>().join(", ")) };
                        let inner: Vec<String> = fs.iter().zip(&names).map(|((_, t), p)| printer(m, t, p, depth + 1)).collect();
                        let payload = format!("format!(\"({})\", {})", "{}, ".repeat(inner.len()), if inner.is_empty() { "\"\"".to_string() } else { inner.join(", ") });
                        let payload = if inner.is_empty() { "String::from(\"()\")".to_string() } else { payload };
                        format!("{}::{}{pattern} => format!(\"({k}, {})\", {payload}),", pascal(&n), pascal(c), slots.join(", "))
                    })
                    .collect();
                format!("match &{v} {{ {} }}", arms.join(" "))
            }
            _ => format!("format!(\"{}::{{:?}}\", {v})", pascal(&n)),
        },
        Ty::List(e) | Ty::Set(e) => {
            let x = format!("x{depth}");
            format!("format!(\"&[{{}}]\", {v}.iter().map(|{x}| {}).collect::<Vec<String>>().join(\", \"))", printer(m, &e, x.as_str(), depth + 1))
        }
        Ty::Coll("Map" | "SortedMap", a) if a.len() == 2 => map_printer(m, &a[0], &a[1], v, depth),
        Ty::SortedMap(k, x) => map_printer(m, &k, &x, v, depth),
        _ => format!("format!(\"{{}}\", {v})"),
    }
}

fn map_printer(m: &Module, k: &Ty, x: &Ty, v: &str, depth: usize) -> String {
    let (kv, xv) = (format!("k{depth}"), format!("x{depth}"));
    format!(
        "format!(\"&[{{}}]\", {v}.iter().map(|({kv}, {xv})| format!(\"({{}}, {{}})\", {}, {})).collect::<Vec<String>>().join(\", \"))",
        printer(m, k, &kv, depth + 1),
        printer(m, x, &xv, depth + 1)
    )
}

/// Whether an argument of a `compile` is known while the program is built: a literal, a list of them, a constant, or a
/// name another `compile` bound (Y § 3.17).
pub fn known(e: &Node, bound: &[String]) -> bool {
    match e.kind {
        "literal" => true,
        "list" => e.nodes("items").iter().all(|i| known(i, bound)),
        "unary" => known(e.node("operand").unwrap(), bound),
        "member" => e.node("object").is_some_and(|o| o.kind == "name"),
        "name" => bound.iter().any(|b| Some(b.as_str()) == e.str("name")),
        _ => false,
    }
}

/// The evaluating crate's function for a file's `compile`s: each call made in order, each value printed as data, a
/// failure printed with its case.
pub fn evaluator(m: &Module, sites: &[Site]) -> String {
    {
        let mut out = vec!["#[allow(unused_variables, dead_code, clippy::all)]".to_string(), "pub fn y_compile_evaluate() {".to_string()];
        for s in sites {
            let name = s.binding.clone().unwrap_or_else(|| "value".into());
            if s.fails {
                out.push(format!("    let {name} = match {} {{", s.call));
                out.push("        Ok(v) => v,".into());
                out.push(format!("        Err(e) => {{ println!(\"Y-COMPILE\\t{}\\tERR\\t{{:?}}\", e); std::process::exit(1) }}", s.id));
                out.push("    };".into());
            } else {
                out.push(format!("    let {name} = {};", s.call));
            }
            out.push(format!("    println!(\"Y-COMPILE\\t{}\\tOK\\t{{}}\", {});", s.id, printer(m, &s.ty, &format!("(&{name})"), 0)));
        }
        out.push("}".into());
        out.join("\n")
    }
}

/// The typed Rust with every plain `let` mutable: it lacks the `mut`s that weave derives, and the evaluating crate only
/// has to run.
fn with_mut(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(k) = rest.find("let ") {
        let before_ok = k == 0 || !rest.as_bytes()[k - 1].is_ascii_alphanumeric() && rest.as_bytes()[k - 1] != b'_';
        out.push_str(&rest[..k + 4]);
        rest = &rest[k + 4..];
        let ident: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let after = rest[ident.len()..].trim_start();
        if before_ok && !ident.is_empty() && ident != "mut" && ident.chars().next().is_some_and(|c| c.is_lowercase() || c == '_') && (after.starts_with('=') || after.starts_with(':')) {
            out.push_str("mut ");
        }
    }
    out.push_str(rest);
    out
}

fn copy_tree(from: &Path, to: &Path) {
    let _ = std::fs::create_dir_all(to);
    for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_tree(&p, &to.join(e.file_name()));
        } else {
            let _ = std::fs::copy(&p, to.join(e.file_name()));
        }
    }
}

/// Runs the evaluating crate: the package's typed Rust, its `main` replaced by one that evaluates each `compile`.
/// Gives each site's value as Rust data, by its id, or the compile error.
pub fn run(dir: &Path, files: &[(PathBuf, String)], main_eval: &str, module_has_sites: bool, lib_crate: &str, module: &str, sites: &[Site], failures: &BTreeMap<String, String>) -> Result<BTreeMap<String, String>, String> {
    let root = std::env::temp_dir().join("y-compile-eval");
    let work = root.join(lib_crate);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(work.join("src")).map_err(|e| format!("{}: {e}", work.display()))?;
    // the manifest, its paths made absolute so the crate builds where it is
    let toml = crate::read(&dir.join("Cargo.toml"))?;
    let mut parent = dir.canonicalize().map_err(|e| e.to_string())?.parent().map(Path::to_path_buf).unwrap_or_default();
    // a test program names the runtime crates as an example does, by where its weave sits, beside the examples
    if !parent.join("y-runtime").is_dir() {
        parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").canonicalize().map_err(|e| e.to_string())?;
    }
    let toml = toml.replace("path = \"../", &format!("path = \"{}/", parent.display()));
    crate::write(&work.join("Cargo.toml"), &toml)?;
    // a program's manifest names the runtime crates beside the examples
    if !toml.contains("[workspace]") {
        return Err(format!("{}: the manifest has no [workspace]", dir.display()));
    }
    for (p, s) in files {
        let text = if p == Path::new("src/main.rs") {
            // the entry point is the evaluator: the module's `compile`s, then `main`'s
            let mut t = s.replacen("fn main(", "#[allow(dead_code)]\nfn y_entry_point(", 1);
            if t.contains("y_window::run(") {
                t = t.replacen("y_window::run(", "let _ = (", 1);
            }
            let call = if module_has_sites { format!("    {lib_crate}::{module}::y_compile_evaluate();\n") } else { String::new() };
            format!("#![allow(unused_mut, unused_variables, dead_code, unused_imports)]\n{t}\n{main_eval}\n\nfn main() {{\n{call}    y_compile_evaluate();\n}}\n")
        } else if p == Path::new("src/lib.rs") {
            format!("#![allow(unused_mut, unused_variables, dead_code, unused_imports)]\n{s}")
        } else {
            s.clone()
        };
        crate::write(&work.join(p), &with_mut(&text))?;
    }
    if dir.join("native").is_dir() {
        copy_tree(&dir.join("native"), &work.join("native"));
    }
    let out = std::process::Command::new("cargo")
        .args(["run", "-q"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .current_dir(&work)
        .output()
        .map_err(|e| format!("compile: cargo: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut values = BTreeMap::new();
    for l in stdout.lines() {
        let parts: Vec<&str> = l.splitn(4, '\t').collect();
        match parts[..] {
            ["Y-COMPILE", id, "OK", v] => {
                values.insert(id.to_string(), v.to_string());
            }
            ["Y-COMPILE", id, "ERR", e] => {
                let site = sites.iter().find(|s| s.id == id);
                let what = site.map(|s| s.call.clone()).unwrap_or_default();
                let case = e.split(|c: char| !c.is_alphanumeric()).next().unwrap_or(e);
                let role = failures.get(&format!("{id}#{case}")).map(|r| format!(": {r}")).unwrap_or_default();
                return Err(format!("{id}: `compile` of `{what}` fails with {e}{role} (Y § 3.17: a failure is a build error)"));
            }
            _ => {}
        }
    }
    if !out.status.success() && stderr.contains("could not compile") {
        let errors: Vec<&str> = stderr.lines().filter(|l| l.starts_with("error") || l.trim_start().starts_with("-->")).take(12).collect();
        return Err(format!("y-compile's build step could not build the crate that evaluates the `compile`s (a fault of y-compile):\n{}", errors.join("\n")));
    }
    if !out.status.success() {
        let ls: Vec<&str> = stderr.lines().collect();
        let fault = match ls.iter().position(|l| l.contains("panicked")) {
            Some(k) => ls[k + 1..].iter().take_while(|l| !l.starts_with("note:") && !l.starts_with("stack backtrace")).cloned().collect::<Vec<_>>().join(" "),
            None => ls.iter().rev().take(12).cloned().collect::<Vec<_>>().join("\n"),
        };
        let pending = sites.iter().find(|s| !values.contains_key(&s.id)).map(|s| s.id.clone()).unwrap_or_default();
        return Err(format!("{pending}: `compile` faulted while the program was built (Y § 3.17: a fault is a build error): {fault}"));
    }
    Ok(values)
}
