//! `bare-rust scaffold`: a bare stub for a new item and the constraint entries that go with it, printed or inserted
//! into the module's `.brs` and `.constraints.json`. Every type the tool cannot know is the placeholder `TODO`, which
//! `weave` refuses until it is replaced.
use crate::hints::TODO;
use crate::json::{canonical, entry};
use crate::weave::base_path;
use crate::R;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelfKind {
    Ref,
    Mut,
    Owned,
}

impl SelfKind {
    fn parse(s: &str) -> R<SelfKind> {
        match s {
            "ref" => Ok(SelfKind::Ref),
            "mut" => Ok(SelfKind::Mut),
            "owned" => Ok(SelfKind::Owned),
            _ => Err(format!("--self takes ref, mut or owned, not `{s}`")),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Spec {
    pub kind: String,
    pub path: String,
    pub params: Vec<String>,
    pub param_types: Vec<String>,
    pub self_kind: Option<SelfKind>,
    /// `T` or `T:Bound+Bound`.
    pub generics: Vec<String>,
    pub result: Option<String>,
    pub trait_: Option<String>,
    pub for_: Option<String>,
    /// `name`, or `name:mut`, `name:owned`, `name:static`.
    pub methods: Vec<String>,
    pub ty: Option<String>,
    pub fields: Vec<String>,
    pub field_types: Vec<String>,
    pub tuple: Option<usize>,
    pub visible: Option<String>,
}

/// What a scaffold makes, with the module it belongs to.
pub struct Scaffold {
    /// The modules of the path, outermost first (empty for the crate root).
    pub module: Vec<String>,
    pub stub: String,
    pub entries: Vec<Value>,
}

/// Split at commas outside `<>`, `()` and `[]`.
fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut prev) = (vec![], String::new(), 0i32, ' ');
    for c in s.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' if prev == '-' => {}
            '>' | ')' | ']' => depth -= 1,
            ',' if depth <= 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
                prev = c;
                continue;
            }
            _ => {}
        }
        cur.push(c);
        prev = c;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn generics_json(g: &[String]) -> R<Option<Value>> {
    if g.is_empty() {
        return Ok(None);
    }
    let mut v = vec![];
    for x in g {
        let (name, bounds) = x.split_once(':').unwrap_or((x, ""));
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("--generics: empty name in `{x}`"));
        }
        let bounds: Vec<&str> = bounds.split('+').map(str::trim).filter(|b| !b.is_empty()).collect();
        v.push(json!({ "name": name, "bounds": bounds }));
    }
    Ok(Some(Value::Array(v)))
}

fn is_type_name(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

fn ident_ok(s: &str) -> R<()> {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_') && !s.chars().next().unwrap().is_ascii_digit() {
        Ok(())
    } else {
        Err(format!("`{s}` is not a name"))
    }
}

/// A function entry: `self` first if `selfk` is given, then the parameters; the types are the given ones or `TODO`.
#[allow(clippy::too_many_arguments)]
fn function(name: &str, selfk: Option<(SelfKind, &str)>, params: &[String], types: &[String], result: Option<&str>, generics: &[String], visible: Option<&str>) -> R<Value> {
    let mut o = serde_json::Map::new();
    o.insert("kind".into(), json!("function"));
    o.insert("name".into(), json!(name));
    if let Some(v) = visible {
        o.insert("visible".into(), json!(v));
    }
    if let Some(g) = generics_json(generics)? {
        o.insert("generics".into(), g);
    }
    let mut ps = vec![];
    if let Some((k, ty)) = selfk {
        ps.push(match k {
            SelfKind::Ref => json!({ "name": "self", "type": format!("ref<{ty}>"), "access": { "rights": "read", "guarantee": "readers" } }),
            SelfKind::Mut => json!({ "name": "self", "type": format!("ref<{ty}>"), "access": { "rights": "write", "guarantee": "exclusive" } }),
            SelfKind::Owned => json!({ "name": "self", "type": ty }),
        });
    }
    for (i, p) in params.iter().enumerate() {
        ident_ok(p)?;
        let t = types.get(i).map(String::as_str).unwrap_or(TODO);
        let mut e = json!({ "name": p, "type": t });
        if t.starts_with("ref<") {
            e["access"] = json!({ "rights": "read", "guarantee": "readers" });
        }
        ps.push(e);
    }
    o.insert("params".into(), Value::Array(ps));
    if result != Some("none") {
        o.insert("result".into(), json!({ "type": result.unwrap_or(TODO) }));
    }
    Ok(Value::Object(o))
}

fn fn_stub(name: &str, has_self: Option<SelfKind>, params: &[String], indent: &str) -> String {
    let mut ps: Vec<&str> = vec![];
    if has_self.is_some() {
        ps.push("self");
    }
    ps.extend(params.iter().map(String::as_str));
    format!("{indent}fn {name}({}) {{\n{indent}    todo!()\n{indent}}}\n", ps.join(", "))
}

/// Build the stub and the entries. `prefix` is the path every subject name starts with (crate and modules).
pub fn build(spec: &Spec, prefix: &str) -> R<Scaffold> {
    let segs: Vec<&str> = spec.path.split("::").filter(|s| !s.is_empty()).collect();
    // `crate::x` and `<crate name>::x` (as the constraint file writes names) both mean the crate root
    let segs: Vec<&str> = if segs.len() > 1 && (segs[0] == "crate" || segs[0] == prefix) { segs[1..].to_vec() } else { segs };
    let (last, rest) = match segs.split_last() {
        Some((l, r)) => (*l, r),
        None if spec.kind == "impl" && spec.for_.is_some() => ("", &segs[..]),
        None => return Err("scaffold: the path is empty".into()),
    };
    segs.iter().try_for_each(|s| ident_ok(s))?;
    let vis = spec.visible.as_deref();
    if let Some(v) = vis {
        if !matches!(v, "public" | "crate" | "module") {
            return Err(format!("--visible takes public, crate or module, not `{v}`"));
        }
    }
    let with_vis = |mut e: Value| {
        if let (Some(v), Some(o)) = (vis, e.as_object_mut()) {
            // after `name`
            let mut n = serde_json::Map::new();
            for (k, x) in std::mem::take(o) {
                let after = k == "name";
                n.insert(k, x);
                if after {
                    n.insert("visible".into(), json!(v));
                }
            }
            *o = n;
        }
        e
    };
    let name_in = |modules: &[&str], tail: &str| {
        let mut p = prefix.to_string();
        for m in modules {
            p += &format!("::{m}");
        }
        format!("{p}::{tail}")
    };
    match spec.kind.as_str() {
        "fn" => {
            let (modules, owner) = match rest.split_last() {
                Some((t, m)) if is_type_name(t) => (m.to_vec(), Some(*t)),
                _ => (rest.to_vec(), None),
            };
            if spec.self_kind.is_some() && owner.is_none() {
                return Err("scaffold fn: a method needs its type in the path: `module::Type::name`".into());
            }
            if let Some(m) = modules.iter().find(|m| is_type_name(m)) {
                return Err(format!("scaffold fn: `{m}` is capitalised, so it reads as a type; only the segment before the name may be one"));
            }
            let sname = match owner {
                Some(t) => name_in(&modules, &format!("{t}::{last}")),
                None => name_in(&modules, last),
            };
            let e = function(&sname, spec.self_kind.zip(owner), &spec.params, &spec.param_types, spec.result.as_deref(), &spec.generics, vis)?;
            let stub = match owner {
                Some(t) => format!("impl {t} {{\n{}}}\n", fn_stub(last, spec.self_kind, &spec.params, "    ")),
                None => fn_stub(last, None, &spec.params, ""),
            };
            Ok(Scaffold { module: modules.iter().map(|s| s.to_string()).collect(), stub, entries: vec![e] })
        }
        "impl" => {
            let ty = spec.for_.clone().unwrap_or_else(|| last.to_string());
            if spec.for_.is_none() && !is_type_name(last) {
                return Err("scaffold impl: the path ends in the type to implement for (`module::Type`), or give --for".into());
            }
            let modules: Vec<&str> = if spec.for_.is_none() { rest.to_vec() } else { segs.clone() };
            let tyc = crate::collapse(&ty);
            let (iname, stub_head, base) = match &spec.trait_ {
                Some(t) => (name_in(&modules, &format!("{tyc} as {}", crate::collapse(t))), format!("impl {t} for {ty}"), name_in(&modules, &format!("{tyc} as {}", crate::collapse(t)))),
                None => (name_in(&modules, &format!("{tyc}::impl")), format!("impl {ty}"), name_in(&modules, &tyc)),
            };
            let mut entries = vec![];
            let mut head = serde_json::Map::new();
            head.insert("kind".into(), json!("impl"));
            head.insert("name".into(), json!(iname));
            if let Some(g) = generics_json(&spec.generics)? {
                head.insert("generics".into(), g);
            }
            entries.push(Value::Object(head));
            let mut stub = format!("{stub_head} {{\n");
            for (i, m) in spec.methods.iter().enumerate() {
                let (mname, kind) = m.split_once(':').unwrap_or((m, "ref"));
                ident_ok(mname)?;
                let sk = match kind {
                    "static" => None,
                    k => Some(SelfKind::parse(k)?),
                };
                if i > 0 {
                    stub.push('\n');
                }
                stub += &fn_stub(mname, sk, &[], "    ");
                entries.push(function(&format!("{base}::{mname}"), sk.map(|k| (k, tyc.as_str())), &[], &[], None, &[], None)?);
            }
            stub += "}\n";
            Ok(Scaffold { module: modules.iter().map(|s| s.to_string()).collect(), stub, entries })
        }
        "alias" => {
            let mut e = json!({ "kind": "alias", "name": name_in(rest, last) });
            if let Some(g) = generics_json(&spec.generics)? {
                e["generics"] = g;
            }
            e["type"] = json!(spec.ty.as_deref().unwrap_or(TODO));
            Ok(Scaffold { module: rest.iter().map(|s| s.to_string()).collect(), stub: format!("type {last};\n"), entries: vec![with_vis(e)] })
        }
        "struct" => {
            let mut e = json!({ "kind": "type", "name": name_in(rest, last) });
            if let Some(g) = generics_json(&spec.generics)? {
                e["generics"] = g;
            }
            let ft = |i: usize| spec.field_types.get(i).map(String::as_str).unwrap_or(TODO).to_string();
            let (stub, fields): (String, Vec<Value>) = if let Some(n) = spec.tuple {
                let fs = (0..n).map(|i| json!({ "type": ft(i), "hold": { "kind": "owns" } })).collect();
                (format!("struct {last}({});\n", vec!["_"; n].join(", ")), fs)
            } else if spec.fields.is_empty() {
                (format!("struct {last};\n", ), vec![])
            } else {
                spec.fields.iter().try_for_each(|f| ident_ok(f))?;
                let fs = spec.fields.iter().enumerate().map(|(i, f)| json!({ "name": f, "type": ft(i), "hold": { "kind": "owns" } })).collect();
                (format!("struct {last} {{\n    {},\n}}\n", spec.fields.join(",\n    ")), fs)
            };
            if !fields.is_empty() {
                e["fields"] = Value::Array(fields);
            }
            Ok(Scaffold { module: rest.iter().map(|s| s.to_string()).collect(), stub, entries: vec![with_vis(e)] })
        }
        k => Err(format!("scaffold: the kind is fn, impl, alias or struct, not `{k}`")),
    }
}

/// The `.brs` of a module below `dir` (the bare folder or the crate): `src/lib.brs`, `src/a.brs` or `src/a/mod.brs`,
/// or the same without `src/`, looked for under `dir/bare` too.
pub fn module_file(dir: &Path, module: &[String]) -> R<PathBuf> {
    let rels: Vec<String> = if module.is_empty() {
        // `lib.brs` beside Cargo.toml too, for a crate whose `[lib] path` is `lib.rs` (fnv)
        vec!["src/lib.brs".into(), "src/main.brs".into(), "lib.brs".into(), "main.brs".into()]
    } else {
        let m = module.join("/");
        vec![format!("src/{m}.brs"), format!("src/{m}/mod.brs"), format!("{m}.brs"), format!("{m}/mod.brs")]
    };
    let mut tried = vec![];
    for root in [dir.to_path_buf(), dir.join("bare")] {
        for r in &rels {
            let p = root.join(r);
            if p.is_file() {
                return Ok(p);
            }
            tried.push(p.display().to_string());
        }
    }
    Err(format!("no bare file for module `{}`; looked for {}", if module.is_empty() { "crate".to_string() } else { module.join("::") }, tried.join(", ")))
}

/// Append the stub to the module's `.brs` and the entries to its `.constraints.json` (rewritten in canonical form).
/// Returns the two files.
pub fn write(dir: &Path, spec: &Spec) -> R<(PathBuf, PathBuf)> {
    // The module comes from the path; build once to learn it.
    // The crate's own name counts as `crate::` (constraint files name items `<crate>::…`).
    let krate = crate::corpus::crate_info(dir).map(|i| i.name.replace('-', "_")).unwrap_or_else(|_| "crate".into());
    let probe = build(spec, &krate)?;
    let brs = module_file(dir, &probe.module)?;
    let cj = brs.with_extension("constraints.json");
    let text = fs::read_to_string(&cj).map_err(|e| format!("{}: {e}", cj.display()))?;
    let mut js: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", cj.display()))?;
    // Names start with the file's own base path; the path's modules are the file's.
    let sc = build(&Spec { path: strip_modules(spec, &probe.module), ..spec.clone() }, &base_path(&js))?;
    let subjects = js["subjects"].as_array_mut().ok_or_else(|| format!("{}: no subjects", cj.display()))?;
    for e in &sc.entries {
        if subjects.iter().any(|s| s["name"] == e["name"]) {
            return Err(format!("{} already has a subject `{}`", cj.display(), e["name"].as_str().unwrap_or("")));
        }
    }
    subjects.extend(sc.entries.clone());
    let mut bare = fs::read_to_string(&brs).map_err(|e| format!("{}: {e}", brs.display()))?;
    if !bare.is_empty() && !bare.ends_with('\n') {
        bare.push('\n');
    }
    if !bare.trim().is_empty() {
        bare.push('\n');
    }
    bare += &sc.stub;
    fs::write(&brs, bare).map_err(|e| format!("{}: {e}", brs.display()))?;
    fs::write(&cj, canonical(&js)).map_err(|e| format!("{}: {e}", cj.display()))?;
    Ok((brs, cj))
}

/// The path of a spec without the modules the file already stands for.
fn strip_modules(spec: &Spec, module: &[String]) -> String {
    let segs: Vec<&str> = spec.path.split("::").filter(|s| !s.is_empty() && *s != "crate").collect();
    let n = if spec.kind == "impl" && spec.for_.is_some() { module.len().min(segs.len()) } else { module.len() };
    segs[n.min(segs.len())..].join("::")
}

fn flag(args: &[String], f: &str) -> Option<String> {
    args.iter().position(|a| a == f).and_then(|i| args.get(i + 1).cloned())
}

fn list(args: &[String], f: &str) -> Vec<String> {
    flag(args, f).map(|v| split_top(&v)).unwrap_or_default()
}

pub fn run(args: &[String]) -> R<()> {
    let (Some(kind), Some(path)) = (args.first(), args.get(1)) else { return Err(USAGE.into()) };
    const VALUED: [&str; 17] = ["--params", "--self", "--generics", "--trait", "--for", "--methods", "--type", "--fields", "--tuple", "--param-types", "--result", "--field-types", "--visible", "--crate", "--write", "--dir", "--vis"];
    for a in args[2..].iter().filter(|a| a.starts_with("--")) {
        if !VALUED.contains(&a.as_str()) {
            return Err(format!("scaffold: unknown option {a}\n{USAGE}"));
        }
    }
    let spec = Spec {
        kind: kind.clone(),
        path: path.clone(),
        params: list(args, "--params"),
        param_types: list(args, "--param-types"),
        self_kind: flag(args, "--self").map(|s| SelfKind::parse(&s)).transpose()?,
        generics: list(args, "--generics"),
        result: flag(args, "--result"),
        trait_: flag(args, "--trait"),
        for_: flag(args, "--for"),
        methods: list(args, "--methods"),
        ty: flag(args, "--type"),
        fields: list(args, "--fields"),
        field_types: list(args, "--field-types"),
        tuple: flag(args, "--tuple").map(|n| n.parse().map_err(|_| "--tuple takes a number".to_string())).transpose()?,
        visible: flag(args, "--visible"),
    };
    if let Some(dir) = flag(args, "--write") {
        let (b, c) = write(Path::new(&dir), &spec)?;
        println!("scaffold: stub appended to {}, entries to {}; replace each {TODO} in the constraint file", b.display(), c.display());
        return Ok(());
    }
    let krate = flag(args, "--crate")
        .or_else(|| crate::corpus::crate_info(Path::new(".")).ok().map(|i| i.name))
        .unwrap_or_else(|| "crate".into())
        .replace('-', "_");
    let sc = build(&spec, &krate)?;
    let file = if sc.module.is_empty() { "src/lib".to_string() } else { format!("src/{}", sc.module.join("/")) };
    println!("// {file}.brs\n{}\n// {file}.constraints.json, appended to \"subjects\" (replace each {TODO}; the weave refuses it)", sc.stub);
    for e in &sc.entries {
        println!("{}", entry(e));
    }
    Ok(())
}

pub const USAGE: &str = "usage: bare-rust scaffold <fn|impl|alias|struct> <path> [options] [--write <dir>]
  fn     scaffold fn [mod::][Type::]name [--params a,b] [--self ref|mut|owned] [--generics T,U:Bound+Bound]
                 [--param-types t1,t2] [--result T|none]
  impl   scaffold impl [mod::]Type [--trait Path] [--for Type] [--methods m1,m2:mut,m3:owned,m4:static] [--generics T]
  alias  scaffold alias [mod::]Name [--type T] [--generics T]
  struct scaffold struct [mod::]Name [--fields a,b | --tuple N] [--field-types t1,t2] [--generics T]
  common [--visible public|crate|module] [--crate <name>]";
