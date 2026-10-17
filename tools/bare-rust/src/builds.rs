//! The builds a crate is checked in. A `#[cfg(..)]` leaves code out of a build, and rustc does not see code it leaves
//! out: it cannot say where that code needs `mut`, nor check what weave made of it. So besides the default build (the
//! host, the default features) the crate is checked in a few others (another target, a feature, a `--cfg`, a release
//! build), chosen so that each piece of gated code is in at least one of them. Code that no usable build compiles keeps
//! its `mut`s in the bare body. A build is usable when the original crate compiles in it on stable Rust.
use crate::{is_comment, named_children, parse_rust, text, R};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use tree_sitter::Node;

/// A cfg predicate (the reference's "configuration predicate").
#[derive(Clone, Debug, PartialEq)]
pub enum Cfg {
    Name(String),
    Pair(String, String),
    All(Vec<Cfg>),
    Any(Vec<Cfg>),
    Not(Box<Cfg>),
    /// Text this parser does not read: never known to hold.
    Unknown(String),
}

/// The options a build sets, as rustc prints them: `unix`, `target_os="linux"`, `feature="std"`.
pub type CfgSet = BTreeSet<String>;

impl Cfg {
    pub fn parse(s: &str) -> Cfg {
        let toks = tokens(s);
        let mut i = 0;
        match pred(&toks, &mut i) {
            Some(c) if i == toks.len() => c,
            _ => Cfg::Unknown(s.trim().to_string()),
        }
    }

    /// Whether the predicate holds in `set`; `None` where an unknown part decides it.
    pub fn eval(&self, set: &CfgSet) -> Option<bool> {
        match self {
            Cfg::Name(n) => Some(set.contains(n)),
            Cfg::Pair(k, v) => Some(set.contains(&format!("{k}=\"{v}\""))),
            Cfg::All(cs) => cs.iter().try_fold(Some(true), |acc, c| match c.eval(set) {
                Some(false) => Err(()),
                Some(true) => Ok(acc),
                None => Ok(None),
            }).unwrap_or(Some(false)),
            Cfg::Any(cs) => cs.iter().try_fold(Some(false), |acc, c| match c.eval(set) {
                Some(true) => Err(()),
                Some(false) => Ok(acc),
                None => Ok(None),
            }).map_or(Some(true), |x| x),
            Cfg::Not(c) => c.eval(set).map(|b| !b),
            Cfg::Unknown(_) => None,
        }
    }

    fn atoms(&self, out: &mut BTreeSet<(String, Option<String>)>) {
        match self {
            Cfg::Name(n) => {
                out.insert((n.clone(), None));
            }
            Cfg::Pair(k, v) => {
                out.insert((k.clone(), Some(v.clone())));
            }
            Cfg::All(cs) | Cfg::Any(cs) => cs.iter().for_each(|c| c.atoms(out)),
            Cfg::Not(c) => c.atoms(out),
            Cfg::Unknown(_) => {}
        }
    }
}

impl std::fmt::Display for Cfg {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let list = |cs: &[Cfg]| cs.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
        match self {
            Cfg::Name(n) => write!(f, "{n}"),
            Cfg::Pair(k, v) => write!(f, "{k} = \"{v}\""),
            Cfg::All(cs) => write!(f, "all({})", list(cs)),
            Cfg::Any(cs) => write!(f, "any({})", list(cs)),
            Cfg::Not(c) => write!(f, "not({c})"),
            Cfg::Unknown(s) => write!(f, "{s}"),
        }
    }
}

fn tokens(s: &str) -> Vec<String> {
    let (mut out, cs): (Vec<String>, Vec<char>) = (vec![], s.chars().collect());
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let mut t = String::from('"');
            i += 1;
            while i < cs.len() && cs[i] != '"' {
                if cs[i] == '\\' && i + 1 < cs.len() {
                    i += 1;
                }
                t.push(cs[i]);
                i += 1;
            }
            out.push(t);
            i += 1;
        } else if c.is_alphanumeric() || c == '_' {
            let st = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            out.push(cs[st..i].iter().collect());
        } else {
            out.push(c.to_string());
            i += 1;
        }
    }
    out
}

fn pred(t: &[String], i: &mut usize) -> Option<Cfg> {
    let name = t.get(*i)?.clone();
    if !name.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        return None;
    }
    *i += 1;
    match t.get(*i).map(String::as_str) {
        Some("=") => {
            *i += 1;
            let v = t.get(*i)?.strip_prefix('"')?.to_string();
            *i += 1;
            Some(Cfg::Pair(name, v))
        }
        Some("(") => {
            *i += 1;
            let mut list = vec![];
            while t.get(*i).map(String::as_str) != Some(")") {
                list.push(pred(t, i)?);
                match t.get(*i).map(String::as_str) {
                    Some(",") => *i += 1,
                    Some(")") => {}
                    _ => return None,
                }
            }
            *i += 1;
            match name.as_str() {
                "all" => Some(Cfg::All(list)),
                "any" => Some(Cfg::Any(list)),
                "not" if list.len() == 1 => Some(Cfg::Not(Box::new(list.pop()?))),
                _ => Some(Cfg::Unknown(format!("{name}({})", list.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")))),
            }
        }
        _ => Some(Cfg::Name(name)),
    }
}

/// The predicate of a `#[cfg(..)]` or `#![cfg(..)]` attribute.
fn cfg_of(a: Node, src: &str) -> Option<Cfg> {
    let t = text(a, src).trim();
    let t = t.strip_prefix("#!").or_else(|| t.strip_prefix('#'))?.trim().strip_prefix('[')?.strip_suffix(']')?.trim();
    let rest = t.strip_prefix("cfg")?.trim_start();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    Some(Cfg::parse(inner))
}

/// The cfg attributes on `n` itself: the outer ones before it and its own inner ones.
fn own_conditions(n: Node, src: &str) -> Vec<Cfg> {
    let mut out = vec![];
    let mut p = n.prev_sibling();
    while let Some(a) = p.filter(|a| a.kind() == "attribute_item" || is_comment(*a)) {
        if a.kind() == "attribute_item" {
            out.extend(cfg_of(a, src));
        }
        p = a.prev_sibling();
    }
    out.extend(named_children(n).into_iter().filter(|a| a.kind() == "inner_attribute_item").filter_map(|a| cfg_of(a, src)));
    out
}

/// Every cfg condition on `n` or around it in its file: the build compiles `n` only where all of them hold.
pub fn conditions(n: Node, src: &str) -> Vec<Cfg> {
    let mut out = vec![];
    let mut cur = Some(n);
    while let Some(c) = cur {
        out.extend(own_conditions(c, src));
        cur = c.parent();
    }
    out
}

/// The cfg conditions of each file of a package from the `mod` declarations that lead to it (empty for a crate root
/// and for a file every build compiles).
pub fn file_conditions(sources: &[(PathBuf, String)], trees: &[(PathBuf, Vec<PathBuf>)]) -> BTreeMap<PathBuf, Vec<Cfg>> {
    let roots: BTreeSet<&PathBuf> = trees.iter().map(|(r, _)| r).collect();
    let mut edges: Vec<(PathBuf, PathBuf, Vec<Cfg>)> = vec![];
    for (rel, src) in sources {
        let dir = rel.parent().unwrap_or(Path::new(""));
        let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let base = if roots.contains(rel) || stem == "mod" { dir.to_path_buf() } else { dir.join(stem) };
        let tree = parse_rust(src);
        for m in named_children(tree.root_node()).into_iter().filter(|n| n.kind() == "mod_item" && n.child_by_field_name("body").is_none()) {
            let name = text(m.child_by_field_name("name").unwrap(), src);
            for cand in [base.join(format!("{name}.rs")), base.join(name).join("mod.rs")] {
                if sources.iter().any(|(r, _)| r == &cand) {
                    edges.push((cand, rel.clone(), conditions(m, src)));
                    break;
                }
            }
        }
    }
    let mut out: BTreeMap<PathBuf, Vec<Cfg>> = roots.iter().map(|r| ((*r).clone(), vec![])).collect();
    loop {
        let before = out.len();
        for (child, parent, conds) in &edges {
            if !out.contains_key(child) {
                if let Some(p) = out.get(parent).cloned() {
                    out.insert(child.clone(), p.into_iter().chain(conds.iter().cloned()).collect());
                }
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The conditions of every piece of gated code of a package: a file's own, and those of each item, statement or
/// parameter with a cfg attribute, with the conditions around it.
fn chains(sources: &[(PathBuf, String)], files: &BTreeMap<PathBuf, Vec<Cfg>>) -> Vec<Vec<Cfg>> {
    let mut out: Vec<Vec<Cfg>> = vec![];
    let push = |c: Vec<Cfg>, out: &mut Vec<Vec<Cfg>>| {
        if !c.is_empty() && !out.contains(&c) {
            out.push(c);
        }
    };
    for (rel, src) in sources {
        let file = files.get(rel).cloned().unwrap_or_default();
        push(file.clone(), &mut out);
        let tree = parse_rust(src);
        let mut stack = vec![tree.root_node()];
        while let Some(n) = stack.pop() {
            if !own_conditions(n, src).is_empty() {
                push(file.iter().cloned().chain(conditions(n, src)).collect(), &mut out);
            }
            stack.extend(named_children(n));
        }
    }
    out
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One build of the crate: a target, features, flags.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Build {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_default_features: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub all_features: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_features: Vec<String>,
    /// `--cfg` options: `miri`, `key="value"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cfgs: Vec<String>,
    /// A release build: no `debug_assertions`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub release: bool,
    /// The library only: a target without `std` has no test harness.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lib_only: bool,
    /// The options the crate's code is compiled with (with `test` too, but for `lib_only`): what the builds are chosen
    /// by, on this machine; not written.
    #[serde(skip)]
    pub cfg: Vec<String>,
}

impl Build {
    pub fn default_build() -> Build {
        Build { name: "default".into(), ..Default::default() }
    }

    pub fn is_default(&self) -> bool {
        self.name == "default"
    }

    /// The arguments of `cargo check` in this build (not the default one, which checks all targets).
    pub fn cargo_args(&self) -> Vec<String> {
        let mut a: Vec<String> = if self.lib_only { vec!["--lib".into()] } else { ["--lib", "--bins", "--tests"].map(String::from).to_vec() };
        // Always a target: then the flags are the crate's, not its build scripts', which run on this machine.
        a.extend(["--target".into(), self.target.clone().unwrap_or_else(host)]);
        if self.no_default_features {
            a.push("--no-default-features".into());
        }
        if !self.features.is_empty() {
            a.extend(["--features".into(), self.features.join(",")]);
        }
        if self.all_features {
            a.push("--all-features".into());
        }
        if self.release {
            a.push("--release".into());
        }
        a
    }

    pub fn rustflags(&self) -> Vec<String> {
        let mut f = vec![];
        if !self.target_features.is_empty() {
            f.extend(["-C".into(), format!("target-feature={}", self.target_features.iter().map(|t| format!("+{t}")).collect::<Vec<_>>().join(","))]);
        }
        for c in &self.cfgs {
            f.extend(["--cfg".into(), c.clone()]);
        }
        f
    }

    /// The sets of options the crate's code is compiled with in this build: without `test`, and with it.
    fn sets(&self) -> Vec<CfgSet> {
        let base: CfgSet = self.cfg.iter().cloned().collect();
        let mut v = vec![base.clone()];
        if !self.lib_only {
            let mut t = base;
            t.insert("test".into());
            v.push(t);
        }
        v
    }

    fn reaches(&self, chain: &[Cfg]) -> bool {
        self.sets().iter().any(|s| chain.iter().all(|c| c.eval(s) == Some(true)))
    }
}

/// A build that would compile more gated code, but could not be used.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Skipped {
    pub build: String,
    pub why: String,
}

/// The builds a package is checked in, and the gated code none of them compiles.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reach {
    pub builds: Vec<Build>,
    /// The conditions of gated code that no build compiles: its `mut`s stay in the bare body.
    #[serde(default)]
    pub unreached: Vec<String>,
    #[serde(default)]
    pub skipped: Vec<Skipped>,
}

pub const FILE: &str = "builds.json";

impl Reach {
    /// The default build only, as a weave of files with no `builds.json` has it.
    pub fn default_only() -> Reach {
        Reach { builds: vec![Build::default_build()], ..Default::default() }
    }

    pub fn load(dir: &Path) -> Option<Reach> {
        serde_json::from_str(&std::fs::read_to_string(dir.join(FILE)).ok()?).ok()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    /// Whether some build compiles the code a chain of conditions of the package guards: every chain of it is either
    /// compiled by a build, or listed as `unreached`.
    pub fn reaches(&self, chain: &[Cfg]) -> bool {
        !self.unreached.contains(&show_chain(chain))
    }

    /// Whether the builds say more than a weave without them assumes: more than the default build, or gated code
    /// that none compiles.
    pub fn worth_writing(&self) -> bool {
        self.builds.len() > 1 || !self.unreached.is_empty() || !self.skipped.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.builds.iter().map(|b| b.name.as_str()).collect()
    }
}

/// What strip of one file is told of the builds: the file's own conditions and the builds.
#[derive(Clone, Debug)]
pub struct FileGate {
    pub file: Vec<Cfg>,
    pub reach: std::sync::Arc<Reach>,
}

impl FileGate {
    /// Whether some build compiles `n`: then rustc sees it, and derives its `mut`s.
    pub fn reached(&self, n: Node, src: &str) -> bool {
        let own = conditions(n, src);
        if self.file.is_empty() && own.is_empty() {
            return true;
        }
        let chain: Vec<Cfg> = self.file.iter().cloned().chain(own).collect();
        self.reach.reaches(&chain)
    }
}

// ----- choosing the builds -----

/// The machine's own target.
pub fn host() -> String {
    let out = Command::new("rustc").arg("-vV").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.lines().find_map(|l| l.strip_prefix("host: ")).unwrap_or("x86_64-unknown-linux-gnu").trim().to_string()
}

/// Other targets to try, each one standing for options the host lacks: 32-bit x86 with and without SSE2, 64-bit ARM
/// on Linux and on macOS, Windows, WebAssembly (bare and Emscripten), big endian 64-bit and 32-bit, 32-bit ARM, RISC-V,
/// and a microcontroller with no `std` and no pointer-sized atomics.
const TARGETS: [&str; 12] = [
    "i686-unknown-linux-gnu",
    "i586-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "wasm32-unknown-unknown",
    "wasm32-unknown-emscripten",
    "s390x-unknown-linux-gnu",
    "powerpc-unknown-linux-gnu",
    "armv7-unknown-linux-gnueabihf",
    "riscv64gc-unknown-linux-gnu",
    "thumbv6m-none-eabi",
];

/// Options rustc or cargo set themselves: not given with `--cfg`.
fn builtin(name: &str) -> bool {
    name.starts_with("target_")
        || name.starts_with("sanitizer")
        || matches!(
            name,
            "test" | "debug_assertions" | "unix" | "windows" | "panic" | "feature" | "proc_macro" | "doctest" | "overflow_checks" | "sanitize"
                | "relocation_model" | "fmt_debug" | "ub_checks" | "contract_checks" | "emscripten_wasm_eh"
        )
}

fn rustc_cfg(target: &str, flags: &[String], release: bool) -> R<Vec<String>> {
    let mut c = Command::new("rustc");
    c.args(["--print", "cfg", "--target", target]).args(flags);
    if release {
        c.args(["-C", "debug-assertions=off"]);
    }
    let out = c.output().map_err(|e| format!("rustc --print cfg: {e}"))?;
    if !out.status.success() {
        return Err(format!("rustc --print cfg --target {target}: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect())
}

fn target_features(target: &str) -> BTreeSet<String> {
    let out = Command::new("rustc").args(["--print", "target-features", "--target", target]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    text.lines().filter(|l| l.starts_with("    ")).filter_map(|l| l.split_whitespace().next()).map(str::to_string).collect()
}

fn installed(target: &str) -> bool {
    let out = Command::new("rustc").args(["--print", "sysroot"]).output();
    let root = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    Path::new(&root).join("lib/rustlib").join(target).join("lib").is_dir()
}

/// The package's features (with the optional dependencies that are features) and its default ones, from cargo.
struct Features {
    map: BTreeMap<String, Vec<String>>,
}

impl Features {
    fn of(dir: &Path, name: &str) -> Features {
        let out = Command::new("cargo").args(["metadata", "--no-deps", "--format-version", "1", "--offline"]).current_dir(dir).output();
        let v: serde_json::Value = out.ok().and_then(|o| serde_json::from_slice(&o.stdout).ok()).unwrap_or_default();
        let manifest = dir.join("Cargo.toml").canonicalize().ok();
        let pkg = v["packages"].as_array().into_iter().flatten().find(|p| {
            p["name"] == name && manifest.as_ref().is_none_or(|m| Path::new(p["manifest_path"].as_str().unwrap_or("")).canonicalize().ok().as_ref() == Some(m))
        });
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(p) = pkg {
            for (k, vs) in p["features"].as_object().into_iter().flatten() {
                map.insert(k.clone(), vs.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect());
            }
            // An optional dependency no feature names with `dep:` is a feature of its own.
            let named: BTreeSet<String> = map.values().flatten().filter_map(|x| x.strip_prefix("dep:").map(str::to_string)).collect();
            for d in p["dependencies"].as_array().into_iter().flatten().filter(|d| d["optional"] == true) {
                let n = d["rename"].as_str().or(d["name"].as_str()).unwrap_or("").replace('-', "_");
                if !n.is_empty() && !named.contains(&n) {
                    map.entry(n).or_default();
                }
            }
        }
        Features { map }
    }

    /// The features a build turns on: `asked`, with the default ones unless `no_default`, and what they turn on.
    fn enabled(&self, asked: &[String], no_default: bool, all: bool) -> BTreeSet<String> {
        let mut stack: Vec<String> = if all { self.map.keys().cloned().collect() } else { asked.to_vec() };
        if !no_default && self.map.contains_key("default") {
            stack.push("default".into());
        }
        let mut on = BTreeSet::new();
        while let Some(f) = stack.pop() {
            if !self.map.contains_key(&f) || !on.insert(f.clone()) {
                continue;
            }
            for v in &self.map[&f] {
                if v.starts_with("dep:") {
                    continue;
                }
                match v.split_once('/') {
                    Some((d, _)) if !d.ends_with('?') => stack.push(d.to_string()),
                    Some(_) => {}
                    None => stack.push(v.clone()),
                }
            }
        }
        on
    }
}

/// Choose the builds for a package (its files, as `strip_sources` takes them). The default build first; then, while
/// some gated code is in none of them, the candidate that compiles most of it, if the original crate compiles in it
/// (`verify`, with cargo, in `target`; without `verify` no other build is chosen).
pub fn choose(dir: &Path, sources: &[(PathBuf, String)], trees: &[(PathBuf, Vec<PathBuf>)], target: &Path, verify: bool) -> R<Reach> {
    let files = file_conditions(sources, trees);
    let chains = chains(sources, &files);
    let mut reach = Reach::default_only();
    if chains.is_empty() {
        return Ok(reach);
    }
    let name = crate::corpus::crate_info(dir)?.name;
    let feats = Features::of(dir, &name);
    let host = host();
    let with_features = |mut cfg: Vec<String>, b: &Build| {
        cfg.extend(feats.enabled(&b.features, b.no_default_features, b.all_features).into_iter().map(|f| format!("feature=\"{f}\"")));
        cfg.sort();
        cfg
    };
    let host_cfg = rustc_cfg(&host, &[], false)?;
    let default = &mut reach.builds[0];
    default.cfg = with_features(host_cfg.clone(), default);

    let mut atoms = BTreeSet::new();
    for c in chains.iter().flatten() {
        c.atoms(&mut atoms);
    }
    let on_by_default = feats.enabled(&[], false, false);
    let mut pool: Vec<Build> = vec![];
    let add = |b: Build, pool: &mut Vec<Build>| {
        let b = Build { name: b.name.replace(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '+' | '.')), "-"), ..b };
        if !pool.iter().any(|p| p.name == b.name) {
            pool.push(b);
        }
    };
    for f in feats.map.keys().filter(|f| !on_by_default.contains(*f)) {
        add(Build { name: format!("feature-{f}"), features: vec![f.clone()], ..Default::default() }, &mut pool);
    }
    if !on_by_default.is_empty() {
        add(Build { name: "no-default-features".into(), no_default_features: true, ..Default::default() }, &mut pool);
    }
    if !feats.map.is_empty() {
        add(Build { name: "all-features".into(), all_features: true, ..Default::default() }, &mut pool);
    }
    let targets_matter = atoms.iter().any(|(k, _)| (k.starts_with("target_") && k != "target_feature") || k == "unix" || k == "windows");
    let wanted_tf: BTreeSet<String> = atoms.iter().filter(|(k, _)| k == "target_feature").filter_map(|(_, v)| v.clone()).collect();
    let mut targets: Vec<String> = vec![host.clone()];
    if targets_matter || !wanted_tf.is_empty() {
        targets.extend(TARGETS.iter().filter(|t| **t != host).map(|t| t.to_string()));
    }
    let mut cfgs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &targets {
        if let Ok(c) = rustc_cfg(t, &[], false) {
            cfgs.insert(t.clone(), c);
        }
    }
    for t in targets.iter().filter(|t| **t != host && cfgs.contains_key(*t)) {
        let no_std = cfgs[t].iter().any(|c| c == "target_os=\"none\"");
        add(Build { name: t.clone(), target: Some(t.clone()), no_default_features: no_std, lib_only: no_std, ..Default::default() }, &mut pool);
    }
    for t in targets.iter().filter(|t| cfgs.contains_key(*t)) {
        let has = target_features(t);
        let plus: Vec<String> = wanted_tf.iter().filter(|f| has.contains(*f) && !cfgs[t].contains(&format!("target_feature=\"{f}\""))).cloned().collect();
        if !plus.is_empty() {
            let no_std = cfgs[t].iter().any(|c| c == "target_os=\"none\"");
            let target = (*t != host).then(|| t.clone());
            add(Build { name: format!("{t}+{}", plus.join("+")), target, target_features: plus, no_default_features: no_std, lib_only: no_std, ..Default::default() }, &mut pool);
        }
    }
    for (k, v) in atoms.iter().filter(|(k, _)| !builtin(k)) {
        let c = match v {
            Some(v) => format!("{k}=\"{v}\""),
            None => k.clone(),
        };
        add(Build { name: format!("cfg-{c}"), cfgs: vec![c], ..Default::default() }, &mut pool);
    }
    if atoms.iter().any(|(k, _)| k == "debug_assertions") {
        add(Build { name: "release".into(), release: true, ..Default::default() }, &mut pool);
    }
    // The options of each candidate.
    let mut candidates = vec![];
    for b in pool {
        let t = b.target.clone().unwrap_or_else(|| host.clone());
        let Ok(cfg) = rustc_cfg(&t, &b.rustflags(), b.release) else { continue };
        let cfg = with_features(cfg, &b);
        candidates.push(Build { cfg, ..b });
    }

    let mut uncovered: Vec<&Vec<Cfg>> = chains.iter().filter(|c| !reach.builds[0].reaches(c)).collect();
    let mut tried = vec![false; candidates.len()];
    while !uncovered.is_empty() && verify {
        let gain = |b: &Build| uncovered.iter().filter(|c| b.reaches(c)).count();
        let best = (0..candidates.len()).filter(|i| !tried[*i]).map(|i| (gain(&candidates[i]), i)).filter(|(g, _)| *g > 0).max_by_key(|(g, i)| (*g, std::cmp::Reverse(*i)));
        let Some((_, i)) = best else { break };
        tried[i] = true;
        let b = &candidates[i];
        if let Some(t) = b.target.as_deref().filter(|t| !installed(t)) {
            reach.skipped.push(Skipped { build: b.name.clone(), why: format!("the target is not installed (rustup target add {t})") });
            continue;
        }
        // A crate whose tests do not compile for a target (a dev-dependency without support for it) is checked there
        // without them.
        let lib = Build { lib_only: true, ..b.clone() };
        let ok = match compiles(dir, target, b) {
            Ok(()) => Ok(b.clone()),
            Err(why) if !b.lib_only => compiles(dir, target, &lib).map(|_| lib).map_err(|_| why),
            Err(why) => Err(why),
        };
        match ok {
            Ok(b) => {
                uncovered.retain(|c| !b.reaches(c));
                reach.builds.push(b);
            }
            Err(why) => reach.skipped.push(Skipped { build: b.name.clone(), why }),
        }
    }
    reach.unreached = uncovered.iter().map(|c| show_chain(c)).collect();
    Ok(reach)
}

fn show_chain(c: &[Cfg]) -> String {
    match c {
        [one] => one.to_string(),
        _ => Cfg::All(c.to_vec()).to_string(),
    }
}

/// Whether the original crate compiles in a build, without errors.
fn compiles(dir: &Path, target: &Path, b: &Build) -> Result<(), String> {
    let run = crate::cargo::diagnostics_in(dir, target, "check", b)?;
    if let Some(d) = run.diags.iter().find(|d| d.is_error()) {
        return Err(format!("the original does not compile: {}", d.message));
    }
    if !run.success {
        return Err(format!("cargo check failed: {}", run.stderr.lines().next().unwrap_or("")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(xs: &[&str]) -> CfgSet {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn predicates_parse_and_evaluate() {
        let c = Cfg::parse(r#"all(target_arch = "x86_64", not(feature = "std"), any(unix, windows))"#);
        assert_eq!(c.to_string(), r#"all(target_arch = "x86_64", not(feature = "std"), any(unix, windows))"#);
        assert_eq!(c.eval(&set(&[r#"target_arch="x86_64""#, "unix"])), Some(true));
        assert_eq!(c.eval(&set(&[r#"target_arch="x86_64""#, "unix", r#"feature="std""#])), Some(false));
        assert_eq!(Cfg::parse("not(all(a, b,))").eval(&set(&["a"])), Some(true));
        // What the parser does not read is never known to hold, nor its negation.
        assert!(matches!(Cfg::parse("a ="), Cfg::Unknown(_)));
        assert_eq!(Cfg::parse("any(a, b(c))").eval(&set(&[])), None);
        assert_eq!(Cfg::parse("all(a, b(c))").eval(&set(&[])), Some(false));
    }

    #[test]
    fn conditions_are_collected_from_the_node_outwards() {
        let src = "#![cfg(unix)]\n#[cfg(feature = \"x\")]\nmod m {\n    #[cfg(test)]\n    fn f() { let a = 1; }\n}\n";
        let tree = parse_rust(src);
        let at = src.find("let a").unwrap();
        let n = tree.root_node().descendant_for_byte_range(at, at + 3).unwrap();
        let c: Vec<String> = conditions(n, src).iter().map(|c| c.to_string()).collect();
        assert_eq!(c, ["test", "feature = \"x\"", "unix"]);
    }

    #[test]
    fn features_turn_on_what_they_name() {
        let map: BTreeMap<String, Vec<String>> = [
            ("default", vec!["std"]),
            ("std", vec!["alloc", "dep:memchr", "serde?/std"]),
            ("alloc", vec![]),
            ("serde", vec![]),
            ("simd", vec!["fast/simd"]),
            ("fast", vec![]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
        .collect();
        let f = Features { map };
        assert_eq!(f.enabled(&[], false, false), ["alloc", "default", "std"].map(String::from).into());
        assert_eq!(f.enabled(&["simd".into()], true, false), ["fast", "simd"].map(String::from).into());
    }
}
