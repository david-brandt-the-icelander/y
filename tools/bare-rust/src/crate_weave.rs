//! Weave a crate and run cargo on it: derive `mut` through rustc, report undetermined holes, and map every remaining
//! error back to the bare file or the constraint slot (proposal 2.3, 2.4 and 6).
use crate::cargo::{self, Diag};
use crate::corpus::{corpus_files, find_brs};
use crate::trace::Kind;
use crate::weave::{weave_in_crate, HoleSite, WeaveError, Woven};
use crate::R;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const MUT_CODES: [&str; 3] = ["E0384", "E0594", "E0596"];
const INFER_CODES: [&str; 4] = ["E0282", "E0283", "E0284", "E0790"];
const MARKER: &str = ".bare-rust-woven";

#[derive(Clone)]
pub struct WeaveOpts {
    /// Run cargo (`--no-cargo` clears it).
    pub cargo: bool,
    /// The crate the stripped files came from, for the files that are not corpus files. Defaults to the input directory.
    pub crate_dir: Option<PathBuf>,
    /// Cargo's target directory.
    pub target: PathBuf,
    /// Weave with every `locals` and `hints` entry dropped (check 4).
    pub drop_holes: bool,
    /// With `drop_holes`: the holes (by `hole_id`) to keep all the same.
    pub keep: BTreeSet<String>,
}

pub struct WovenFile {
    /// The woven `.rs` file, relative to the crate.
    pub rel: PathBuf,
    pub woven: Woven,
    pub json: Value,
    pub mut_added: usize,
    /// In a weave with holes dropped: the holes the file had (see `hole_id`).
    pub dropped: BTreeSet<String>,
}

impl WovenFile {
    pub fn bare_name(&self) -> String {
        self.rel.with_extension("brs").display().to_string()
    }
}

/// An undetermined hole: where it is in the bare body and the clause that would fill it.
#[derive(Clone, Debug)]
pub struct HoleReport {
    pub file: String,
    pub subject: String,
    pub kind: &'static str,
    pub n: usize,
    pub key: Option<String>,
    pub line: usize,
    pub col: usize,
    pub clause: String,
    pub code: String,
    pub message: String,
    /// `uninferred`: rustc could not infer the type. `rejected`: it inferred one that the program does not accept
    /// (only known in a weave with holes dropped, where the file once filled the hole).
    pub how: &'static str,
}

impl HoleReport {
    pub fn show(&self) -> String {
        let what = match &self.key {
            Some(k) if self.kind == "realization" => format!("realization `{k}`"),
            Some(k) => format!("local `{k}`"),
            None => format!("{} #{}", self.kind, self.n),
        };
        let path = match &self.key {
            Some(k) if self.kind == "realization" => format!("realizations[\"{k}\"].args, in the crate root's file"),
            Some(k) => format!("holes[\"{}\"].locals[\"{k}\"]", self.subject),
            None => format!("holes[\"{}\"].hints += {{kind: {}, n: {}, type: T}}", self.subject, self.kind, self.n),
        };
        format!(
            "hole undetermined: {what} of `{}`\n    at {}:{}:{}  [{} {}{}]\n    fill with: {}   ({path})",
            self.subject,
            self.file,
            self.line,
            self.col,
            self.code,
            self.message,
            if self.how == "rejected" { "; the type rustc chose is rejected" } else { "" },
            self.clause
        )
    }
}

/// A compiler error that is not an undetermined hole, with where it comes from.
#[derive(Clone, Debug)]
pub struct ErrReport {
    pub code: String,
    pub message: String,
    pub woven_file: String,
    pub woven_line: usize,
    pub woven_col: usize,
    /// "slot params[1].type of `a::f`" or "bare file.brs:12:5" or "a file of the crate that is not woven".
    pub locus: String,
    /// The offset in the bare file (0 when the file is not woven).
    pub bare_start: usize,
    pub rendered: String,
    /// The same error with its position in the bare file and the JSON pointer of the slot (see `errmap`).
    pub mapped: crate::errmap::MappedError,
}

impl ErrReport {
    pub fn show(&self) -> String {
        format!(
            "error[{}]: {}\n    at {}  (woven {}:{}:{})",
            self.code, self.message, self.locus, self.woven_file, self.woven_line, self.woven_col
        )
    }
}

#[derive(Default)]
pub struct CrateWeave {
    pub files: Vec<WovenFile>,
    pub weave_errors: Vec<(PathBuf, String)>,
    /// The same refusals mapped to the bare file, with the hint that names the fix (see `hints`).
    pub weave_diags: Vec<crate::errmap::MappedError>,
    /// Corpus files with no bare file (strip refused them): copied as they are.
    pub unwoven: Vec<PathBuf>,
    pub iterations: usize,
    pub holes: Vec<HoleReport>,
    pub errors: Vec<ErrReport>,
    /// Errors in a function that has an undetermined hole, which restoring no single hole removes: taken to follow
    /// from it (set by check 4).
    pub follow_on: Vec<ErrReport>,
    pub warnings: Vec<Diag>,
    pub cargo: bool,
    pub cargo_stderr: String,
    pub out: PathBuf,
    /// The crate's realizations as woven (with check 4, the arguments dropped), from the files that list them.
    pub realizations: serde_json::Map<String, Value>,
}

impl CrateWeave {
    pub fn ok(&self) -> bool {
        self.weave_errors.is_empty() && self.holes.is_empty() && self.errors.is_empty() && self.cargo_stderr.is_empty()
    }

    /// Entries of the constraint files that bind to nothing in their bare body, with the bare file.
    pub fn unbound(&self) -> Vec<(String, &crate::weave::Unbound)> {
        self.files.iter().flat_map(|f| f.woven.unbound.iter().map(|u| (f.bare_name(), u))).collect()
    }

    /// The counts of `unbound()`: (locals, hints, subjects).
    pub fn unbound_counts(&self) -> (usize, usize, usize) {
        let n = |w: &str| self.unbound().iter().filter(|(_, u)| u.what == w).count();
        (n("unbound local"), n("unbound hint"), n("unbound subject"))
    }

    /// The result as JSON, for `weave --json`: counts, and every error and undetermined hole mapped to the bare file and
    /// the constraint pointer (see `errmap`).
    pub fn to_json(&self) -> Value {
        let (ul, uh, us) = self.unbound_counts();
        serde_json::json!({
            "ok": self.ok(),
            "files_woven": self.files.len(),
            "files_not_woven": self.unwoven.len(),
            "weave_errors": self.weave_errors.iter().map(|(f, e)| serde_json::json!({"file": f.display().to_string(), "message": e})).collect::<Vec<_>>(),
            "unbound": {"locals": ul, "hints": uh, "subjects": us},
            "mut_inserted": self.mut_added(),
            "errors": crate::errmap::map_crate(self).iter().map(|e| e.to_json()).collect::<Vec<_>>(),
        })
    }

    pub fn mut_added(&self) -> usize {
        self.files.iter().map(|f| f.mut_added).sum()
    }

    pub fn print(&self) {
        for d in &self.weave_diags {
            eprintln!("{}", d.show());
        }
        for h in &self.holes {
            eprintln!("{}", h.show());
        }
        for e in self.errors.iter().chain(&self.follow_on) {
            eprintln!("{}", e.mapped.show());
        }
        for (f, u) in self.unbound() {
            eprintln!("warning: {f}: {}", u.show());
        }
        if !self.cargo_stderr.is_empty() {
            eprintln!("{}", self.cargo_stderr);
        }
        let (ul, uh, us) = self.unbound_counts();
        println!(
            "weave: {} files woven, {} not woven, {} weave errors; {} unbound entries (locals {ul}, hints {uh}, subjects {us}){}",
            self.files.len(),
            self.unwoven.len(),
            self.weave_errors.len(),
            ul + uh + us,
            if self.cargo {
                format!(
                    "; cargo check after {} runs: {} mut inserted, {} holes undetermined, {} other errors, {} warnings",
                    self.iterations,
                    self.mut_added(),
                    self.holes.len(),
                    self.errors.len(),
                    self.warnings.len()
                )
            } else {
                String::new()
            }
        );
    }
}

fn skip(rel: &Path, woven: &BTreeSet<PathBuf>) -> bool {
    let first = rel.components().next().and_then(|c| c.as_os_str().to_str()).unwrap_or("");
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
    (first.starts_with("target") || matches!(first, ".git" | "node_modules"))
        || rel.starts_with("corpora/src") // linked in, not copied: see `link_corpora`
        || name == MARKER
        || (rel.parent().is_none_or(|p| p.as_os_str().is_empty()) && name == "refusals.json")
        || woven.contains(rel)
        || rel.to_str().is_some_and(|r| {
            // The stripped files of a corpus file: x.brs and x.constraints.json beside x.rs.
            let stem = r.strip_suffix(".brs").or_else(|| r.strip_suffix(".constraints.json"));
            let under_bare = stem.and_then(|st| st.strip_prefix("bare/"));
            stem.is_some_and(|st| woven.contains(Path::new(&format!("{st}.rs"))))
                || under_bare.is_some_and(|st| woven.contains(Path::new(&format!("{st}.rs"))))
        })
        || rel == Path::new("bare/refusals.json")
}

fn copy_tree(src: &Path, base: &Path, dst: &Path, woven: &BTreeSet<PathBuf>) -> R<()> {
    let out = fs::canonicalize(dst).unwrap_or_else(|_| dst.to_path_buf());
    let rd = fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        let rel = p.strip_prefix(base).unwrap_or(&p).to_path_buf();
        if skip(&rel, woven) || fs::canonicalize(&p).is_ok_and(|c| c.starts_with(&out)) {
            continue; // not copied: an excluded file, or the output directory itself, if it is inside the crate
        }
        let to = dst.join(&rel);
        if p.is_dir() {
            fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
            copy_tree(&p, base, dst, woven)?;
        } else {
            fs::copy(&p, &to).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    Ok(())
}

/// A crate whose tests read `corpora/src` (the tool's own) gets it in the woven copy, as a link to the original, so
/// that its clippy and tests run there.
fn link_corpora(crate_dir: &Path, out: &Path) -> R<()> {
    let from = crate_dir.join("corpora/src");
    if !from.is_dir() {
        return Ok(());
    }
    let from = fs::canonicalize(&from).map_err(|e| format!("{}: {e}", from.display()))?;
    let to = out.join("corpora/src");
    fs::create_dir_all(out.join("corpora")).map_err(|e| format!("{}: {e}", to.display()))?;
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&from, &to);
    #[cfg(not(unix))]
    let linked = fs::create_dir_all(&to).map_err(|e| e.to_string()).and_then(|_| copy_tree(&from, &from, &to, &BTreeSet::new()));
    linked.map_err(|e| format!("{}: {e}", to.display()))
}

/// A crate that is a member of a workspace finds the workspace's other files beside it (`../data/x`, the workspace's
/// `clippy.toml`): the woven copy gets them too, as links beside it, all but the workspace's own manifest, which would
/// make the copy a member.
fn link_workspace(crate_dir: &Path, out: &Path) -> R<()> {
    let (Some(ws), Some(beside)) = (crate_dir.parent(), out.parent()) else { return Ok(()) };
    let is_ws = fs::read_to_string(ws.join("Cargo.toml")).is_ok_and(|t| t.lines().any(|l| l.trim() == "[workspace]"));
    if !is_ws {
        return Ok(());
    }
    let own = fs::canonicalize(crate_dir).unwrap_or(crate_dir.to_path_buf());
    for e in fs::read_dir(ws).map_err(|e| format!("{}: {e}", ws.display()))?.filter_map(|e| e.ok()) {
        let name = e.file_name();
        let skip = matches!(name.to_str(), Some("Cargo.toml" | "Cargo.lock" | "target" | ".git"));
        let to = beside.join(&name);
        if skip || fs::canonicalize(e.path()).is_ok_and(|p| p == own) || to.exists() || fs::symlink_metadata(&to).is_ok() {
            continue;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(e.path().canonicalize().unwrap_or(e.path()), &to).map_err(|err| format!("{}: {err}", to.display()))?;
    }
    Ok(())
}

fn prepare_out(out: &Path, dirs: &[&Path]) -> R<()> {
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or(p.to_path_buf());
    if out.exists() {
        let o = canon(out);
        if dirs.iter().any(|d| canon(d) == o) {
            return Err(format!("--out {} is the input directory", out.display()));
        }
        let non_empty = fs::read_dir(out).map(|mut r| r.next().is_some()).unwrap_or(false);
        if non_empty && !out.join(MARKER).exists() {
            return Err(format!("--out {} is not empty and was not made by bare-rust weave", out.display()));
        }
        fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    fs::write(out.join(MARKER), "").map_err(|e| e.to_string())
}

/// The position of the binding of `name` that a diagnostic at `at` (the binding itself, or a use) refers to: the
/// nearest binding of it at or before `at` whose scope (the block, function, closure, loop or arm it is bound in)
/// holds `at`. `None` if it already has `mut`.
fn binding_before(src: &str, name: &str, at: usize) -> Option<usize> {
    use tree_sitter::Node;
    let tree = crate::parse_rust(src);
    fn bound(n: Node) -> Option<Node> {
        // The construct that binds: up from the identifier through the pattern to a `pattern` field or the parameters.
        let mut cur = n;
        while let Some(p) = cur.parent() {
            let in_pattern = p.child_by_field_name("pattern") == Some(cur) || matches!(p.kind(), "closure_parameters" | "match_pattern");
            if in_pattern {
                return Some(p);
            }
            if p.kind().ends_with("_expression") || matches!(p.kind(), "block" | "arguments") || crate::is_item_kind(p.kind()) {
                return None;
            }
            cur = p;
        }
        None
    }
    fn walk<'a>(n: Node<'a>, src: &str, name: &str, out: &mut Vec<(Node<'a>, Node<'a>)>) {
        if n.kind() == "identifier" && crate::text(n, src) == name {
            if let Some(b) = bound(n) {
                out.push((n, b));
            }
        }
        for c in crate::named_children(n) {
            walk(c, src, name, out);
        }
    }
    let mut found = vec![];
    walk(tree.root_node(), src, name, &mut found);
    let scope = |b: Node| -> Option<(usize, usize)> {
        let s = match b.kind() {
            // A `let` binds to the end of its block; a parameter, for its function or closure; a pattern, for its arm,
            // loop or `if let`.
            "let_declaration" => b.parent(),
            "parameter" => b.parent().and_then(|ps| ps.parent()),
            "closure_parameters" => b.parent(),
            "match_pattern" => b.parent(),
            _ => b.parent(),
        }?;
        Some((s.start_byte(), s.end_byte()))
    };
    found
        .into_iter()
        .filter(|(n, b)| n.start_byte() <= at && scope(*b).is_some_and(|(s, e)| s <= at && at < e))
        .filter(|(n, _)| !n.prev_sibling().is_some_and(|p| p.kind() == "mutable_specifier"))
        .max_by_key(|(n, _)| n.start_byte())
        .map(|(n, _)| n.start_byte())
}

fn write(p: &Path, s: &str) -> R<()> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    fs::write(p, s).map_err(|e| format!("{}: {e}", p.display()))
}

fn is_inference(d: &Diag) -> bool {
    d.code.as_deref().is_some_and(|c| INFER_CODES.contains(&c))
        || (d.code.is_none() && (d.message.contains("type annotations needed") || d.message.contains("cannot infer")))
}

/// The unfilled hole site an error at a bare range belongs to, if any. `ok` says which sites may be chosen.
fn hole_of<'a>(sites: &'a [HoleSite], bare: &str, bs: usize, be: usize, ok: &dyn Fn(&HoleSite) -> bool, infer: bool) -> Option<&'a HoleSite> {
    let pool: Vec<&HoleSite> = sites.iter().filter(|s| !s.filled && ok(s) && s.body.0 <= bs && be <= s.body.1).collect();
    let cand = |f: &dyn Fn(&HoleSite) -> bool| pool.iter().copied().filter(|s| f(s)).collect::<Vec<_>>().into_iter();
    let within = |r: (usize, usize)| r.0 <= bs && be <= r.1;
    // 1. a site whose own range holds the error span: the smallest.
    if let Some(s) = cand(&|s: &HoleSite| within(s.range)).min_by_key(|s| s.range.1 - s.range.0) {
        return Some(s);
    }
    // 2. an error at a use of a local the file would have typed: the latest earlier `let` of that name.
    let ident = bare.get(bs..be.min(bare.len())).unwrap_or("").trim();
    let ident = ident.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("");
    if !ident.is_empty() {
        let found = cand(&|s: &HoleSite| s.kind == "local" && s.key.as_deref().is_some_and(|k| k == ident || k.split('#').next() == Some(ident)) && s.full.0 <= bs)
            .max_by_key(|s| s.full.0);
        if found.is_some() {
            return found;
        }
    }
    // 3. a cast, type argument, realization or closure site inside the error span (only for inference errors, which
    // name no site): `TryInto::<_>::try_into(x)` with the realization's arguments dropped.
    if infer {
        if let Some(s) = cand(&|s: &HoleSite| matches!(s.kind, "cast" | "targ" | "closure_param" | "realization") && bs <= s.range.0 && s.range.1 <= be).min_by_key(|s| s.range.0) {
            return Some(s);
        }
    }
    // 4. the innermost `let` that holds the error span (the type flows from its initialiser).
    if let Some(s) = cand(&|s: &HoleSite| s.kind == "local" && within(s.full)).min_by_key(|s| s.full.1 - s.full.0) {
        return Some(s);
    }
    // 5. the innermost closure that holds it.
    cand(&|s: &HoleSite| matches!(s.kind, "closure_return") && within(s.full)).min_by_key(|s| s.full.1 - s.full.0)
}

/// Identifies a hole across weaves: its subject, kind and key or ordinal. A realization is the crate's, not a
/// function's: its id has no subject.
pub fn hole_id(subject: &str, kind: &str, key: Option<&str>, n: usize) -> String {
    let subject = if kind == "realization" { "" } else { subject };
    format!("{subject}|{kind}|{}", key.map_or(n.to_string(), str::to_string))
}

/// The realizations of a crate: the tables of its files (a crate root's file lists its crate's), merged. A name with
/// two different entries is refused. Returns the table and, for each name, the file that lists it.
pub fn crate_realizations(files: &[(PathBuf, Value)]) -> R<(serde_json::Map<String, Value>, BTreeMap<String, PathBuf>)> {
    let (mut all, mut owner): (serde_json::Map<String, Value>, BTreeMap<String, PathBuf>) = (serde_json::Map::new(), BTreeMap::new());
    for (rel, js) in files {
        for (k, v) in js["realizations"].as_object().into_iter().flatten() {
            match all.get(k) {
                Some(w) if w != v => {
                    return Err(format!("realization `{k}` is listed twice with different entries: {} and {}", owner[k.as_str()].display(), rel.display()))
                }
                Some(_) => {}
                None => {
                    all.insert(k.clone(), v.clone());
                    owner.insert(k.clone(), rel.clone());
                }
            }
        }
    }
    Ok((all, owner))
}

pub fn hole_report(wf: &WovenFile, s: &HoleSite, code: &str, message: &str, how: &'static str) -> HoleReport {
    let (line, col) = wf.woven.bare_at(s.range.0);
    HoleReport {
        file: wf.bare_name(),
        subject: s.subject.clone(),
        kind: s.kind,
        n: s.n,
        key: s.key.clone(),
        line,
        col,
        clause: s.clause(),
        code: code.to_string(),
        message: message.to_string(),
        how,
    }
}

/// Report the diagnostics of a woven crate: holes, errors that follow from them, and other errors with their locus.
fn classify(files: &[WovenFile], diags: &[Diag], cw: &mut CrateWeave) {
    let errs: Vec<&Diag> = diags.iter().filter(|d| d.is_error()).collect();
    let find = |d: &Diag| files.iter().find(|f| f.rel.to_string_lossy() == d.file);
    // Pass 1: inference errors at hole sites.
    let mut handled = vec![false; errs.len()];
    for (i, d) in errs.iter().enumerate().filter(|(_, d)| is_inference(d)) {
        let Some(wf) = find(d) else { continue };
        let map = &wf.woven.map;
        let (bs, be) = (map.to_bare(d.start, false), map.to_bare(d.end, true));
        if let Some(s) = hole_of(&wf.woven.sites, &wf.woven.bare, bs, be.max(bs), &|_| true, true) {
            handled[i] = true;
            let r = hole_report(wf, s, d.code.as_deref().unwrap_or(""), &d.message, "uninferred");
            if !cw.holes.iter().any(|h| h.file == r.file && h.subject == r.subject && h.kind == r.kind && h.n == r.n && h.key == r.key) {
                cw.holes.push(r);
            }
        }
    }
    // Pass 2: the others.
    for (_, d) in errs.iter().enumerate().filter(|(i, _)| !handled[*i]) {
        let code = d.code.clone().unwrap_or_default();
        let mk = |locus: String, bare_start: usize| ErrReport { code: code.clone(), message: d.message.clone(), woven_file: d.file.clone(), woven_line: d.line, woven_col: d.col, locus, bare_start, rendered: d.rendered.clone(), mapped: crate::errmap::map_diag(files, files.iter().find(|f| f.rel.to_string_lossy() == d.file), d) };
        let Some(wf) = find(d) else {
            let locus = if d.file.is_empty() { "no source span".to_string() } else { format!("{} (not a woven file)", d.file) };
            cw.errors.push(mk(locus, 0));
            continue;
        };
        let map = &wf.woven.map;
        let bs = map.to_bare(d.start, false);
        let locus = match map.touching(d.start, d.end).first() {
            Some(sp) => {
                let (l, c) = wf.woven.bare_at(sp.bare_start);
                format!("{} (bare {}:{l}:{c})", sp.origin.describe(), wf.bare_name())
            }
            None => {
                let (l, c) = wf.woven.bare_at(bs);
                format!("bare {}:{l}:{c}", wf.bare_name())
            }
        };
        cw.errors.push(mk(locus, bs));
    }
}

/// Write the span map of every woven file to `<out>/.bare-rust-map.json`: each range of the woven text that is
/// not a copy of the bare text, with the bare range it replaces and the slot, hole or `mut` it came from; and,
/// under `@unbound`, the entries of the constraint files that bind to nothing.
fn write_map(out: &Path, files: &[WovenFile]) -> R<()> {
    let mut m = serde_json::Map::new();
    let mut unbound = vec![];
    for f in files {
        let spans: Vec<Value> = f
            .woven
            .map
            .spans
            .iter()
            .map(|s| {
                let kind = match s.origin.kind {
                    Kind::Slot => "slot",
                    Kind::Hole => "hole",
                    Kind::Mut => "mut",
                };
                serde_json::json!({"woven": [s.start, s.end], "bare": [s.bare_start, s.bare_end], "kind": kind, "subject": s.origin.subject, "slot": s.origin.slot})
            })
            .collect();
        m.insert(f.rel.display().to_string(), Value::Array(spans));
        for u in &f.woven.unbound {
            unbound.push(serde_json::json!({"file": f.bare_name(), "what": u.what, "subject": u.subject, "name": u.name}));
        }
    }
    m.insert("@unbound".into(), Value::Array(unbound));
    write(&out.join(".bare-rust-map.json"), &crate::json::ordered(&Value::Object(m)))
}

/// Weave every `x.brs` below `from` and, with `cargo`, check the crate and derive `mut`.
pub fn weave_crate_with(from: &Path, out: &Path, o: &WeaveOpts) -> R<CrateWeave> {
    let crate_dir = o.crate_dir.clone().unwrap_or_else(|| from.to_path_buf());
    let mut brs = vec![];
    find_brs(from, &mut brs);
    if brs.is_empty() {
        return Err(format!("{}: no .brs files", from.display()));
    }
    let mut cw = CrateWeave { cargo: o.cargo, out: out.to_path_buf(), ..Default::default() };
    // The crate's realizations, from every file that lists some; check 4 drops their arguments, crate-wide.
    let listed: Vec<(PathBuf, Value)> = brs
        .iter()
        .filter_map(|b| {
            let js: Value = serde_json::from_str(&fs::read_to_string(b.with_extension("constraints.json")).ok()?).ok()?;
            js.get("realizations").is_some().then(|| (b.strip_prefix(from).unwrap_or(b).with_extension("constraints.json"), js))
        })
        .collect();
    let (mut reals, owners) = crate_realizations(&listed)?;
    let mut reals_dropped = BTreeSet::new();
    if o.drop_holes {
        for (k, v) in reals.iter_mut() {
            let id = hole_id("", "realization", Some(k), 0);
            if !o.keep.contains(&id) {
                // A realization keeps its template and drops its arguments: `parse::<_>()`.
                let n = v["args"].as_array().map_or(0, Vec::len);
                v["args"] = serde_json::json!(vec!["_"; n]);
                reals_dropped.insert(id);
            }
        }
    }
    for b in brs {
        let mut rel = b.strip_prefix(from).unwrap_or(&b).with_extension("rs");
        // A crate directory whose stripped files sit in a `bare/` folder (`strip --out <crate>/bare`): the files belong
        // at the paths below `bare/`, where the crate's own sources are, not at `bare/…`.
        if let Ok(rest) = rel.strip_prefix("bare") {
            if !rest.as_os_str().is_empty() && crate_dir.join(rest).is_file() && !crate_dir.join(&rel).exists() {
                rel = rest.to_path_buf();
            }
        }
        let r = (|| {
            let bare = fs::read_to_string(&b).map_err(|e| WeaveError::from(format!("{}: {e}", b.display())))?;
            let cj = b.with_extension("constraints.json");
            let js = fs::read_to_string(&cj).map_err(|e| WeaveError::from(format!("{}: {e}", cj.display())))?;
            let mut js: Value = serde_json::from_str(&js).map_err(|e| WeaveError::from(format!("{}: {e}", cj.display())))?;
            let mut dropped = BTreeSet::new();
            if o.drop_holes {
                let mut kept = serde_json::Map::new();
                for (sub, h) in js["holes"].as_object().into_iter().flatten() {
                    let (mut locals, mut hints) = (serde_json::Map::new(), vec![]);
                    for (k, v) in h["locals"].as_object().into_iter().flatten() {
                        let id = hole_id(sub, "local", Some(k), 0);
                        if o.keep.contains(&id) {
                            locals.insert(k.clone(), v.clone());
                        } else {
                            // A cast local drops its type, not its cast: `let x: _ = e as _;`.
                            if v["cast"] == true {
                                locals.insert(k.clone(), serde_json::json!({"type": "_", "cast": true}));
                            }
                            dropped.insert(id);
                        }
                    }
                    for x in h["hints"].as_array().into_iter().flatten() {
                        let id = hole_id(sub, x["kind"].as_str().unwrap_or(""), None, x["n"].as_u64().unwrap_or(0) as usize);
                        if o.keep.contains(&id) {
                            hints.push(x.clone());
                        } else {
                            dropped.insert(id);
                        }
                    }
                    let mut e = serde_json::Map::new();
                    if !locals.is_empty() {
                        e.insert("locals".into(), Value::Object(locals));
                    }
                    if !hints.is_empty() {
                        e.insert("hints".into(), Value::Array(hints));
                    }
                    if !e.is_empty() {
                        kept.insert(sub.clone(), Value::Object(e));
                    }
                }
                js["holes"] = Value::Object(kept);
                dropped.extend(reals_dropped.iter().cloned());
            }
            Ok::<_, WeaveError>((weave_in_crate(&bare, &js, Some(&reals))?, js, dropped))
        })();
        match r {
            Ok((woven, json, dropped)) => cw.files.push(WovenFile { rel, woven, json, mut_added: 0, dropped }),
            Err(e) => {
                cw.weave_diags.push(crate::errmap::map_weave_error(&rel.with_extension("brs").display().to_string(), &rel.with_extension("constraints.json").display().to_string(), &e));
                cw.weave_errors.push((rel, e.msg));
            }
        }
    }
    // A realization that no body of the crate uses binds to nothing: reported with the file that lists it.
    let used: std::collections::HashSet<String> = cw.files.iter().flat_map(|f| f.woven.realizations_used.iter().cloned()).collect();
    for u in crate::weave::unused_realizations(Some(&reals), &used, "") {
        let file = &owners[u.name.as_str()];
        if let Some(wf) = cw.files.iter_mut().find(|f| file.ends_with(f.rel.with_extension("constraints.json"))) {
            let owner = crate::weave::base_path(&wf.json);
            wf.woven.unbound.push(crate::weave::Unbound { subject: owner, ..u });
        }
    }
    cw.realizations = reals;
    let woven_set: BTreeSet<PathBuf> = cw.files.iter().map(|f| f.rel.clone()).collect();
    cw.unwoven = corpus_files(&crate_dir, true).unwrap_or_default().into_iter().filter(|f| !woven_set.contains(f)).collect();
    prepare_out(out, &[from, &crate_dir])?;
    copy_tree(&crate_dir, &crate_dir, out, &woven_set)?;
    link_corpora(&crate_dir, out)?;
    link_workspace(&crate_dir, out)?;
    // The build goes to a target directory of its own; a test that writes under `target/`, as a crate's own build
    // would have made it, finds the directory there.
    fs::create_dir_all(out.join("target")).map_err(|e| format!("{}: {e}", out.display()))?;
    for f in &cw.files {
        write(&out.join(&f.rel), &f.woven.text)?;
    }
    if !o.cargo || !cw.weave_errors.is_empty() {
        write_map(out, &cw.files)?;
        return Ok(cw);
    }
    // Derive `mut`: apply rustc's suggestions for E0384, E0594 and E0596 that add `mut` to a binding, until none remain.
    let diags = loop {
        cw.iterations += 1;
        if cw.iterations > 40 {
            return Err("mut derivation did not converge after 40 cargo runs".into());
        }
        let run = cargo::diagnostics(out, &o.target, "check")?;
        cw.cargo_stderr = run.stderr.clone();
        let mut ins: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        for d in run.diags.iter().filter(|d| d.is_error() && d.code.as_deref().is_some_and(|c| MUT_CODES.contains(&c))) {
            let before = ins.values().map(BTreeSet::len).sum::<usize>();
            for fx in &d.fixes {
                let Some(wf) = cw.files.iter().find(|f| f.rel.to_string_lossy() == fx.file) else { continue };
                // Either an insertion of `mut ` before the binding, or the binding replaced by `mut <binding>`.
                let orig = wf.woven.text.get(fx.start..fx.end).unwrap_or("");
                let adds_mut = if fx.start == fx.end {
                    fx.replacement == "mut " && wf.woven.text.is_char_boundary(fx.start)
                } else {
                    !orig.is_empty() && fx.replacement == format!("mut {orig}")
                };
                if adds_mut {
                    ins.entry(fx.file.clone()).or_default().insert(fx.start);
                }
            }
            // rustc does not always suggest the `mut` (for `&mut x` it may suggest removing the `&mut`): the binding
            // the message names, the nearest one in scope before the use, takes it.
            if ins.values().map(BTreeSet::len).sum::<usize>() == before {
                let name = d.message.split('`').nth(1).map(|n| n.trim_start_matches('*').split(['.', '[']).next().unwrap_or(n).to_string());
                if let (Some(name), Some(wf)) = (name, cw.files.iter().find(|f| f.rel.to_string_lossy() == d.file)) {
                    if let Some(at) = binding_before(&wf.woven.text, &name, d.start) {
                        ins.entry(d.file.clone()).or_default().insert(at);
                    }
                }
            }
        }
        if ins.is_empty() {
            break run.diags;
        }
        for (file, set) in ins {
            let wf = cw.files.iter_mut().find(|f| f.rel.to_string_lossy() == file).unwrap();
            for pos in set.into_iter().rev() {
                wf.woven.text.insert_str(pos, "mut ");
                wf.woven.map.insert_mut(pos, 4, "");
                wf.mut_added += 1;
            }
            write(&out.join(&wf.rel), &wf.woven.text)?;
        }
    };
    cw.warnings = diags.iter().filter(|d| d.level == "warning").cloned().collect();
    let files = std::mem::take(&mut cw.files);
    classify(&files, &diags, &mut cw);
    cw.files = files;
    write_map(out, &cw.files)?;
    Ok(cw)
}
