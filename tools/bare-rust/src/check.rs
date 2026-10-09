//! Checks 1 to 4 of proposal 9 for one crate: strip it, weave it back through cargo, and compare.
use crate::cargo::{self, Diag};
use crate::corpus::strip_crate;
use crate::crate_weave::{hole_id, hole_report, weave_crate_with, CrateWeave, ErrReport, HoleReport, WeaveOpts};
use crate::weave::HoleSite;
use crate::json::canonical;
use crate::trace::Kind;
use crate::{count_mut, normalise, normalise_nomut, rustfmt, strip, R};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct CheckOpts {
    /// Where the stripped and woven copies go: `<work>/<name>/{bare,woven,holes}`.
    pub work: PathBuf,
    pub target: PathBuf,
    /// Include the crate's integration tests among the corpus files.
    pub tests: bool,
    /// Run clippy and the tests too (otherwise `cargo check` only).
    pub full: bool,
    /// Where the library files and externals lists are: check 5 runs for a crate that has an externals list there.
    pub libraries: Option<PathBuf>,
    /// Fail check 5 when external items have no entry yet (otherwise they are reported as incomplete).
    pub strict_libraries: bool,
}

#[derive(Default)]
pub struct Check1 {
    pub files: usize,
    pub failures: Vec<String>,
}

#[derive(Default)]
pub struct Check2 {
    pub files: usize,
    pub exact: usize,
    /// Files that match except for `mut`: (file, bindings with `mut` in the original, in the woven, first difference).
    pub mut_differs: Vec<(String, usize, usize, String)>,
    pub failures: Vec<String>,
    pub mut_original: usize,
    pub mut_derived: usize,
}

#[derive(Default)]
pub struct Check3 {
    /// Errors of the woven crate's `cargo check` (holes, others, weave refusals).
    pub check_failures: Vec<String>,
    pub check_warnings_new: Vec<String>,
    pub clippy: Option<(usize, usize, Vec<String>)>,
    pub test: Option<(bool, String)>,
}

#[derive(Default)]
pub struct Check4 {
    /// Holes in the constraint files that were dropped, by kind.
    pub dropped: BTreeMap<String, usize>,
    pub dropped_functions: usize,
    pub undetermined: Vec<HoleReport>,
    /// Failures that are not an undetermined hole.
    pub other_errors: Vec<String>,
    /// Hole entries printed into the woven text although the file had none: must be zero.
    pub silently_filled: usize,
    pub weave_errors: Vec<String>,
    pub undetermined_not_dropped: usize,
    /// Errors that follow from an undetermined hole in the same function.
    pub follow_on: usize,
}

pub struct CheckReport {
    pub name: String,
    pub files: usize,
    pub refused: Vec<(String, String)>,
    pub mut_added: usize,
    /// Entries of the constraint files that bind to nothing (locals, hints, subjects), from the weave of check 2.
    pub unbound: (usize, usize, usize),
    pub c1: Check1,
    pub c2: Check2,
    pub c3: Check3,
    pub c4: Check4,
    /// Check 5, mechanical half: `None` when the crate has no externals list among the libraries.
    pub c5: Option<crate::libcheck::LibReport>,
    pub strict_libraries: bool,
}

/// The number of tests that passed, from `cargo test`'s `test result:` lines.
pub fn test_count(summary: &str) -> usize {
    summary.lines().filter_map(|l| l.strip_prefix("test result: ok. ")?.split(' ').next()?.parse::<usize>().ok()).sum()
}

fn first_diff(a: &str, b: &str) -> String {
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    for i in 0..la.len().max(lb.len()) {
        if la.get(i) != lb.get(i) {
            return format!("line {}: `{}` != `{}`", i + 1, la.get(i).unwrap_or(&"<end>").trim(), lb.get(i).unwrap_or(&"<end>").trim());
        }
    }
    "differs".into()
}

fn check1(cw: &CrateWeave) -> Check1 {
    let mut c = Check1::default();
    for f in &cw.files {
        c.files += 1;
        let (krate, module) = (f.json["crate"].as_str().unwrap_or(""), f.json["module"].as_str().unwrap_or(""));
        match strip(&f.woven.text, krate, module) {
            Err(e) => c.failures.push(format!("{}: strip(woven): {e}", f.rel.display())),
            Ok((b2, j2)) => {
                if b2 != f.woven.bare {
                    c.failures.push(format!("{}: bare differs: {}", f.rel.display(), first_diff(&b2, &f.woven.bare)));
                } else if canonical(&j2) != canonical(&f.json) {
                    c.failures.push(format!("{}: json differs: {}", f.rel.display(), first_diff(&canonical(&j2), &canonical(&f.json))));
                }
            }
        }
    }
    c
}

fn check2(cw: &CrateWeave, dir: &Path) -> Check2 {
    let mut c = Check2::default();
    let norm = |s: &str| normalise(s).and_then(|n| rustfmt(&n));
    let norm_nomut = |s: &str| normalise_nomut(s).and_then(|n| rustfmt(&n));
    for f in &cw.files {
        c.files += 1;
        let name = f.rel.display().to_string();
        let Ok(original) = fs::read_to_string(dir.join(&f.rel)) else {
            c.failures.push(format!("{name}: original not readable"));
            continue;
        };
        let (mo, md) = (count_mut(&original), count_mut(&f.woven.text));
        c.mut_original += mo;
        c.mut_derived += md;
        match (norm(&f.woven.text), norm(&original)) {
            (Ok(a), Ok(b)) if a == b => c.exact += 1,
            (Ok(a), Ok(b)) => match (norm_nomut(&f.woven.text), norm_nomut(&original)) {
                (Ok(x), Ok(y)) if x == y => c.mut_differs.push((name, mo, md, first_diff(&a, &b))),
                _ => c.failures.push(format!("{name}: {}", first_diff(&a, &b))),
            },
            (Err(e), _) => c.failures.push(format!("{name}: woven: {e}")),
            (_, Err(e)) => c.failures.push(format!("{name}: original: {e}")),
        }
    }
    c
}

/// Warnings of `new` that the original does not have, comparing code, message and file as multisets.
fn new_warnings(orig: &[Diag], new: &[Diag]) -> Vec<String> {
    let mut have: BTreeMap<String, i64> = BTreeMap::new();
    for d in orig.iter().filter(|d| d.level == "warning") {
        *have.entry(d.key()).or_insert(0) += 1;
    }
    let mut out = vec![];
    for d in new.iter().filter(|d| d.level == "warning") {
        let n = have.entry(d.key()).or_insert(0);
        *n -= 1;
        if *n < 0 {
            out.push(format!("{}:{}:{} {} {}", d.file, d.line, d.col, d.code.as_deref().unwrap_or("warning"), d.message));
        }
    }
    out
}

fn describe(cw: &CrateWeave) -> Vec<String> {
    let mut v: Vec<String> = cw.weave_errors.iter().map(|(f, e)| format!("weave refused {}: {e}", f.display())).collect();
    v.extend(cw.holes.iter().map(|h| h.show()));
    v.extend(cw.errors.iter().map(|e| e.show()));
    v
}

fn check3(cw: &CrateWeave, dir: &Path, o: &CheckOpts) -> R<Check3> {
    let mut c = Check3 { check_failures: describe(cw), ..Default::default() };
    let orig = cargo::diagnostics(dir, &o.target, "check")?;
    c.check_warnings_new = new_warnings(&orig.diags, &cw.warnings);
    if o.full && c.check_failures.is_empty() {
        let before = cargo::diagnostics(dir, &o.target, "clippy")?;
        let after = cargo::diagnostics(&cw.out, &o.target, "clippy")?;
        let new = new_warnings(&before.diags, &after.diags);
        let count = |r: &cargo::Run| r.diags.iter().filter(|d| d.level == "warning").count();
        c.clippy = Some((count(&before), count(&after), new));
        c.test = Some(cargo::test(&cw.out, &o.target)?);
    }
    Ok(c)
}

fn kind_counts(files: &[crate::crate_weave::WovenFile]) -> (BTreeMap<String, usize>, usize, usize) {
    // Holes in the files as stripped: locals and hints, by kind; and the functions that have any.
    let (mut by, mut fns, mut silent) = (BTreeMap::new(), 0, 0);
    for f in files {
        if let Some(m) = f.json["holes"].as_object() {
            for (_, v) in m {
                let l = v["locals"].as_object().map_or(0, |o| o.len());
                let h = v["hints"].as_array().map_or(0, Vec::len);
                if l + h > 0 {
                    fns += 1;
                }
                if l > 0 {
                    *by.entry("local".to_string()).or_insert(0) += l;
                }
                for x in v["hints"].as_array().into_iter().flatten() {
                    *by.entry(x["kind"].as_str().unwrap_or("?").to_string()).or_insert(0) += 1;
                }
            }
        }
        silent += f.woven.map.spans.iter().filter(|s| s.origin.kind == Kind::Hole).count();
    }
    (by, fns, silent)
}

/// The hole of a function whose restoring removes an error that the all-dropped weave reports: found by weaving
/// again with one dropped hole of that function restored, so that the attribution is shown, not guessed.
fn attribute(bare: &Path, dir: &Path, o: &CheckOpts, name: &str, all: &mut CrateWeave) -> R<()> {
    let out = o.work.join(name).join("holes-one");
    let mut seen: BTreeMap<String, BTreeSet<(String, String, String, usize)>> = BTreeMap::new();
    let key = |e: &ErrReport| (e.woven_file.clone(), e.code.clone(), e.message.clone(), e.bare_start);
    for e in std::mem::take(&mut all.errors) {
        let Some(wf) = all.files.iter().find(|f| f.rel.to_string_lossy() == e.woven_file) else {
            all.errors.push(e);
            continue;
        };
        let inner = wf.woven.sites.iter().filter(|s| s.body.0 <= e.bare_start && e.bare_start <= s.body.1).min_by_key(|s| s.body.1 - s.body.0);
        let cands: Vec<&HoleSite> = match inner {
            Some(i) => wf.woven.sites.iter().filter(|s| s.subject == i.subject && wf.dropped.contains(&hole_id(&s.subject, s.kind, s.key.as_deref(), s.n))).take(60).collect(),
            None => vec![],
        };
        let mut found = None;
        for c in cands {
            let id = hole_id(&c.subject, c.kind, c.key.as_deref(), c.n);
            if !seen.contains_key(&id) {
                let opts = WeaveOpts { cargo: true, crate_dir: Some(dir.to_path_buf()), target: o.target.clone(), drop_holes: true, keep: [id.clone()].into() };
                let r = weave_crate_with(bare, &out, &opts)?;
                seen.insert(id.clone(), r.errors.iter().chain(&r.follow_on).map(key).collect());
            }
            if !seen[&id].contains(&key(&e)) {
                found = Some(hole_report(wf, c, &e.code, &e.message, "rejected"));
                break;
            }
        }
        match found {
            Some(h) => {
                if !all.holes.iter().any(|x| x.file == h.file && x.subject == h.subject && x.kind == h.kind && x.n == h.n && x.key == h.key) {
                    all.holes.push(h);
                }
            }
            None => {
                let sub = inner.map(|i| i.subject.clone());
                if sub.is_some_and(|sub| all.holes.iter().any(|h| h.file == wf.bare_name() && h.subject == sub)) {
                    all.follow_on.push(e);
                } else {
                    all.errors.push(e);
                }
            }
        }
    }
    Ok(())
}

fn check4(bare: &Path, dir: &Path, o: &CheckOpts, name: &str, originals: &CrateWeave) -> R<Check4> {
    let out = o.work.join(name).join("holes");
    let opts = WeaveOpts { cargo: true, crate_dir: Some(dir.to_path_buf()), target: o.target.clone(), drop_holes: true, keep: Default::default() };
    let mut cw = weave_crate_with(bare, &out, &opts)?;
    if cw.weave_errors.is_empty() && !cw.errors.is_empty() {
        attribute(bare, dir, o, name, &mut cw)?;
    }
    let (dropped, dropped_functions, _) = kind_counts(&originals.files);
    let (_, _, silently_filled) = kind_counts(&cw.files);
    // The holes the file filled, by (file, subject, kind, n/key): to tell which undetermined holes were dropped ones.
    let mut c = Check4 { dropped, dropped_functions, silently_filled, ..Default::default() };
    let filled = |h: &HoleReport| -> bool {
        originals.files.iter().filter(|f| f.bare_name() == h.file).any(|f| {
            let e = &f.json["holes"][&h.subject];
            match &h.key {
                Some(k) => !e["locals"][k].is_null(),
                None => e["hints"].as_array().into_iter().flatten().any(|x| x["kind"] == h.kind && x["n"].as_u64() == Some(h.n as u64)),
            }
        })
    };
    for h in &cw.holes {
        if !filled(h) {
            c.undetermined_not_dropped += 1;
        }
    }
    c.undetermined = cw.holes.clone();
    c.follow_on = cw.follow_on.len();
    c.other_errors = cw.errors.iter().map(|e| e.show()).collect();
    c.weave_errors = cw.weave_errors.iter().map(|(f, e)| format!("{}: {e}", f.display())).collect();
    Ok(c)
}

/// Run checks 1 to 4 over one crate.
pub fn check_crate(dir: &Path, o: &CheckOpts) -> R<CheckReport> {
    let info = crate::corpus::crate_info(dir)?;
    let name = info.name.clone();
    let root = o.work.join(&name);
    let _ = fs::remove_dir_all(&root);
    let bare = root.join("bare");
    let results = strip_crate(dir, &bare, o.tests)?;
    let refused: Vec<(String, String)> =
        results.iter().filter_map(|r| r.outcome.as_ref().err().map(|e| (r.file.display().to_string(), e.clone()))).collect();
    let opts = WeaveOpts { cargo: true, crate_dir: Some(dir.to_path_buf()), target: o.target.clone(), drop_holes: false, keep: Default::default() };
    let cw = weave_crate_with(&bare, &root.join("woven"), &opts)?;
    let c1 = check1(&cw);
    let c2 = check2(&cw, dir);
    let c3 = check3(&cw, dir, o)?;
    let c4 = check4(&bare, dir, o, &name, &cw)?;
    let c5 = match &o.libraries {
        Some(l) if l.join("externals").join(format!("{name}.json")).is_file() => Some(crate::libcheck::libcheck(dir, l, &o.work, &o.target)?),
        _ => None,
    };
    let report = CheckReport {
        name,
        files: results.len(),
        refused,
        mut_added: cw.mut_added(),
        unbound: cw.unbound_counts(),
        c1,
        c2,
        c3,
        c4,
        c5,
        strict_libraries: o.strict_libraries,
    };
    let _ = fs::write(root.join("check.json"), crate::json::ordered(&report.to_json()));
    Ok(report)
}

fn by_kind(h: &[HoleReport]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for x in h {
        *m.entry(x.kind.to_string()).or_insert(0) += 1;
    }
    m
}

impl CheckReport {
    pub fn c1_ok(&self) -> bool {
        self.c1.failures.is_empty()
    }
    pub fn c2_ok(&self) -> bool {
        self.c2.failures.is_empty() && self.c2.mut_differs.is_empty()
    }
    pub fn c3_ok(&self) -> bool {
        let c = &self.c3;
        c.check_failures.is_empty()
            && c.check_warnings_new.is_empty()
            && c.clippy.as_ref().is_none_or(|x| x.2.is_empty())
            && c.test.as_ref().is_none_or(|x| x.0)
    }
    pub fn c4_ok(&self) -> bool {
        let c = &self.c4;
        c.silently_filled == 0 && c.other_errors.is_empty() && c.weave_errors.is_empty()
    }
    pub fn c5_ok(&self) -> bool {
        self.c5.as_ref().is_none_or(|c| c.ok(self.strict_libraries))
    }
    pub fn ok(&self) -> bool {
        self.c1_ok() && self.c2_ok() && self.c3_ok() && self.c4_ok() && self.c5_ok()
    }

    pub fn to_json(&self) -> Value {
        let mut per_fn: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
        for h in &self.c4.undetermined {
            *per_fn.entry(h.subject.clone()).or_default().entry(h.kind.to_string()).or_insert(0) += 1;
        }
        json!({
            "crate": self.name,
            "files": self.files,
            "refused": self.refused,
            "unbound": {"locals": self.unbound.0, "hints": self.unbound.1, "subjects": self.unbound.2},
            "mut": {"original": self.c2.mut_original, "derived": self.c2.mut_derived, "inserted_by_rustc": self.mut_added,
                    "files_differing": self.c2.mut_differs.len()},
            "check1": {"files": self.c1.files, "failures": self.c1.failures},
            "check2": {"files": self.c2.files, "exact": self.c2.exact, "mut_differs": self.c2.mut_differs.iter().map(|d| json!({"file": d.0, "original": d.1, "derived": d.2, "first_difference": d.3})).collect::<Vec<_>>(), "failures": self.c2.failures},
            "check3": {
                "cargo_check_failures": self.c3.check_failures, "new_check_warnings": self.c3.check_warnings_new,
                "clippy": self.c3.clippy.as_ref().map(|c| json!({"original": c.0, "woven": c.1, "new": c.2})),
                "test": self.c3.test.as_ref().map(|t| json!({"passed": t.0, "summary": t.1})),
            },
            "check4": {
                "dropped": self.c4.dropped, "dropped_total": self.c4.dropped.values().sum::<usize>(), "functions_with_holes": self.c4.dropped_functions,
                "undetermined": self.c4.undetermined.len(), "undetermined_by_kind": by_kind(&self.c4.undetermined),
                "functions_with_undetermined": per_fn.len(), "undetermined_by_function": per_fn,
                "undetermined_by_cause": {"uninferred": self.c4.undetermined.iter().filter(|h| h.how == "uninferred").count(), "rejected": self.c4.undetermined.iter().filter(|h| h.how == "rejected").count()},
                "undetermined_holes": self.c4.undetermined.iter().map(|h| json!({
                    "file": h.file, "subject": h.subject, "kind": h.kind, "n": h.n, "key": h.key, "line": h.line, "column": h.col,
                    "how": h.how, "code": h.code, "message": h.message, "fill_with": h.clause,
                })).collect::<Vec<_>>(),
                "follow_on_errors": self.c4.follow_on,
                "undetermined_at_unfilled_sites_never_dropped": self.c4.undetermined_not_dropped,
                "other_errors": self.c4.other_errors, "silently_filled": self.c4.silently_filled, "weave_errors": self.c4.weave_errors,
            },
            "check5": self.c5.as_ref().map(crate::libcheck::LibReport::to_json),
        })
    }

    pub fn print(&self) {
        let st = |ok: bool| if ok { "ok" } else { "FAIL" };
        println!("bare-rust check {}", self.name);
        println!("  strip    {} corpus files, {} refused", self.files, self.refused.len());
        for (f, e) in &self.refused {
            println!("             refused {f}: {e}");
        }
        println!("  weave    {} unbound entries (locals {}, hints {}, subjects {})", self.unbound.0 + self.unbound.1 + self.unbound.2, self.unbound.0, self.unbound.1, self.unbound.2);
        println!("  check 1  {}  bare round trip, {} files", st(self.c1_ok()), self.c1.files);
        for f in &self.c1.failures {
            println!("             {f}");
        }
        println!(
            "  check 2  {}  source round trip, {} files: {} exact, {} differ only in mut, {} other; mut on bindings: original {}, derived {} ({} inserted through rustc)",
            st(self.c2_ok()), self.c2.files, self.c2.exact, self.c2.mut_differs.len(), self.c2.failures.len(), self.c2.mut_original, self.c2.mut_derived, self.mut_added
        );
        for (f, o, d, diff) in &self.c2.mut_differs {
            println!("             {f}: mut differs (original {o}, derived {d}): {diff}");
        }
        for f in &self.c2.failures {
            println!("             {f}");
        }
        let c = &self.c3;
        let clippy = c.clippy.as_ref().map_or("not run".to_string(), |x| format!("{} warnings (original {}, {} new)", x.1, x.0, x.2.len()));
        let test = c.test.as_ref().map_or("not run".to_string(), |t| {
            if t.0 { format!("passed ({} tests)", test_count(&t.1)) } else { "FAILED".into() }
        });
        println!(
            "  check 3  {}  cargo check {} ({} new warnings); clippy: {clippy}; cargo test: {test}",
            st(self.c3_ok()), if c.check_failures.is_empty() { "passes" } else { "FAILS" }, c.check_warnings_new.len()
        );
        for f in c.check_failures.iter().chain(&c.check_warnings_new).chain(c.clippy.iter().flat_map(|x| &x.2)) {
            println!("             {}", f.replace('\n', "\n             "));
        }
        if let Some((false, s)) = &c.test {
            println!("             {}", s.replace('\n', "\n             "));
        }
        let d = &self.c4;
        let dropped: usize = d.dropped.values().sum();
        println!(
            "  check 4  {}  holes dropped: {dropped} in {} functions; undetermined {} ({}; {} uninferred, {} rejected; {} follow-on errors); silently filled {}; other errors {}",
            st(self.c4_ok()), d.dropped_functions, d.undetermined.len(),
            Some(by_kind(&d.undetermined).iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty()).unwrap_or_else(|| "none".into()),
            d.undetermined.iter().filter(|h| h.how == "uninferred").count(),
            d.undetermined.iter().filter(|h| h.how == "rejected").count(),
            d.follow_on,
            d.silently_filled, d.other_errors.len()
        );
        for e in d.other_errors.iter().chain(&d.weave_errors) {
            println!("             {}", e.replace('\n', "\n             "));
        }
        match &self.c5 {
            None => println!("  check 5  n/a  no externals list for this crate among the libraries"),
            Some(c) => {
                let sigs = c.sigs.as_ref().map_or("not run".to_string(), |s| {
                    format!("{} checked, {} path-resolved, {} unchecked, {} mismatched", s.checked.len(), s.resolved.len(), s.unchecked_total(), s.mismatches.len())
                });
                let cov = if c.coverage_ok() { "complete" } else { "incomplete" };
                println!(
                    "  check 5  {}  library coverage {cov}: {} of {} external items; schema problems {}; signatures: {sigs}",
                    st(self.c5_ok()), c.covered(), c.items, c.invalid.len()
                );
                for e in &c.invalid {
                    println!("             {e}");
                }
                if let Some(s) = &c.sigs {
                    for (n, m) in &s.mismatches {
                        println!("             mismatch {n}: {m}");
                    }
                }
                if self.strict_libraries && !c.missing.is_empty() {
                    for (k, p) in &c.missing {
                        println!("             missing {} {p}", k.name());
                    }
                }
            }
        }
    }
}
