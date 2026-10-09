//! E3, edits (proposal 10): over a repository's history, strip the older version of each changed file, take the newer
//! bare body, and weave it with the older constraint file. Count the entries that no longer bind and the holes that
//! become undetermined, and say whether the woven crate still compiles.
//!
//! One record per commit is kept as JSON (`<work>/e3-<repo>.json`); the tables are computed from the records.
use crate::cargo;
use crate::corpus::{corpus_files, crate_info};
use crate::crate_weave::{weave_crate_with, CrateWeave, WeaveOpts};
use crate::json::canonical;
use crate::{normalise_nomut, rustfmt, strip, weave::weave_traced, R};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct E3Opts {
    pub from: Option<String>,
    pub to: Option<String>,
    pub work: PathBuf,
    pub target: PathBuf,
    /// Run cargo (otherwise only the strip and weave comparisons).
    pub cargo: bool,
    pub max_commits: Option<usize>,
}

fn git(repo: &Path, args: &[&str]) -> R<Vec<u8>> {
    let o = Command::new("git").arg("-C").arg(repo).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !o.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(o.stdout)
}

fn git_text(repo: &Path, args: &[&str]) -> R<String> {
    Ok(String::from_utf8_lossy(&git(repo, args)?).to_string())
}

fn export(repo: &Path, rev: &str, dir: &Path) -> R<()> {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tar = git(repo, &["archive", "--format=tar", rev])?;
    let mut child = Command::new("tar")
        .args(["-x", "-C"])
        .arg(dir)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("tar: {e}"))?;
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(&tar).map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string()).and_then(|s| if s.success() { Ok(()) } else { Err("tar failed".into()) })
}

fn reason(e: &str) -> String {
    let r = e.split_once(": ").map_or(e, |x| x.0);
    // a weave refusal's reason, without the names in it
    if e.starts_with("slot missing: no subject") {
        "new item (no entry in the old file)".into()
    } else if e.contains("the constraint file has") && e.contains("parameters, the body has") {
        "parameter count changed".into()
    } else if e.starts_with("slot missing") {
        "slot missing".into()
    } else {
        r.to_string()
    }
}

fn without_holes(j: &Value) -> Value {
    let mut j = j.clone();
    if let Some(o) = j.as_object_mut() {
        o.remove("holes");
    }
    j
}

/// The text both sides of a comparison are reduced to: lifetimes elided where Rust allows, `mut` on bindings removed
/// and rustfmt applied. When rustfmt refuses a file (some old files), whitespace is collapsed instead.
fn same_code(a: &str, b: &str) -> bool {
    let (na, nb) = (normalise_nomut(a), normalise_nomut(b));
    let (Ok(na), Ok(nb)) = (na, nb) else { return false };
    match (rustfmt(&na), rustfmt(&nb)) {
        (Ok(x), Ok(y)) => x == y,
        (Err(_), Err(_)) => crate::collapse(&na) == crate::collapse(&nb),
        _ => false,
    }
}

fn hole_ids(cw: &CrateWeave) -> BTreeSet<String> {
    cw.holes.iter().map(|h| format!("{}|{}|{}|{}", h.file, h.subject, h.kind, h.key.clone().unwrap_or_else(|| h.n.to_string()))).collect()
}

fn error_ids(cw: &CrateWeave) -> BTreeMap<String, String> {
    cw.errors.iter().map(|e| (format!("{}|{}|{}", e.code, e.message, e.woven_file), e.mapped.show())).collect()
}

fn write_pair(dir: &Path, rel: &Path, bare: &str, js: &Value) -> R<()> {
    let b = dir.join(rel.with_extension("brs"));
    fs::create_dir_all(b.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&b, bare).map_err(|e| e.to_string())?;
    fs::write(dir.join(rel.with_extension("constraints.json")), canonical(js)).map_err(|e| e.to_string())
}

/// One file of one commit, up to the weave of the new bare body with the old constraint file (no cargo).
struct Staged {
    rel: PathBuf,
    class: &'static str,
    bare_new: String,
    json_old: Value,
    json_new: Value,
    rec: Value,
}

fn stage(repo: &Path, parent: &str, commit: &str, rel: &Path, krate: &str) -> Result<Staged, Value> {
    let path = rel.to_string_lossy().replace('\\', "/");
    let module = path.clone();
    let rd = |rev: &str| git_text(repo, &["show", &format!("{rev}:{path}")]);
    let (old_src, new_src) = (rd(parent).map_err(|e| json!({"file": path, "outcome": "unreadable", "detail": e}))?, rd(commit).map_err(|e| json!({"file": path, "outcome": "unreadable", "detail": e}))?);
    let old = strip(&old_src, krate, &module);
    let new = strip(&new_src, krate, &module);
    let ((bare_old, json_old), (bare_new, json_new)) = match (old, new) {
        (Ok(o), Ok(n)) => (o, n),
        (o, n) => {
            let (side, e) = match (&o, &n) {
                (Err(e), Err(_)) => ("both", e.clone()),
                (Err(e), _) => ("old", e.clone()),
                (_, Err(e)) => ("new", e.clone()),
                _ => unreachable!(),
            };
            return Err(json!({"file": path, "outcome": "strip refused", "side": side, "reason": reason(&e), "detail": e}));
        }
    };
    let class = if bare_old == bare_new && canonical(&json_old) == canonical(&json_new) {
        "identical"
    } else if canonical(&json_old) == canonical(&json_new) {
        "body-only"
    } else if canonical(&without_holes(&json_old)) == canonical(&without_holes(&json_new)) {
        "body-only, holes changed"
    } else {
        "signature-changing"
    };
    let mut rec = json!({"file": path, "class": class});
    let w = match weave_traced(&bare_new, &json_old) {
        Err(e) => {
            rec["weave"] = json!("refused");
            rec["weave_reason"] = json!(reason(&e));
            rec["weave_detail"] = json!(e);
            return Ok(Staged { rel: rel.to_path_buf(), class, bare_new, json_old, json_new, rec: rec });
        }
        Ok(w) => w,
    };
    let mut by_what: BTreeMap<&str, u64> = BTreeMap::new();
    for u in &w.unbound {
        *by_what.entry(u.what).or_insert(0) += 1;
    }
    rec["weave"] = json!("woven");
    rec["unbound"] = json!(by_what);
    rec["unbound_total"] = json!(w.unbound.len());
    rec["unbound_entries"] = json!(w.unbound.iter().map(|u| u.show()).collect::<Vec<_>>());
    // Does the woven text differ from the new original only where a slot was woven in?
    let reproduces = same_code(&w.text, &new_src);
    let only_slots = match strip(&w.text, krate, &module) {
        Ok((b2, _)) => b2 == bare_new,
        Err(_) => false,
    };
    rec["woven_equals_new"] = json!(reproduces);
    rec["differs_only_in_slots"] = json!(only_slots);
    Ok(Staged { rel: rel.to_path_buf(), class, bare_new, json_old, json_new, rec })
}

fn opts(o: &E3Opts, crate_dir: &Path, drop: bool) -> WeaveOpts {
    WeaveOpts { cargo: true, crate_dir: Some(crate_dir.to_path_buf()), target: o.target.clone(), drop_holes: drop, keep: Default::default() }
}

/// The cargo phase of one commit: the files whose old constraint file may no longer fit are woven into a checkout of
/// the new commit and the crate is checked.
fn cargo_phase(o: &E3Opts, newdir: &Path, wd: &Path, files: &[&Staged]) -> R<Value> {
    let base = cargo::diagnostics(newdir, &o.target, "check")?;
    let base_errors = base.diags.iter().filter(|d| d.is_error()).count();
    if base_errors > 0 || (!base.success && !base.stderr.is_empty()) {
        let first = base.diags.iter().find(|d| d.is_error()).map(|d| d.message.clone()).unwrap_or(base.stderr.clone());
        return Ok(json!({"baseline": "does not build", "baseline_errors": base_errors, "baseline_first": first}));
    }
    let (in_old, in_new) = (wd.join("in-old"), wd.join("in-new"));
    let _ = fs::remove_dir_all(&in_old);
    let _ = fs::remove_dir_all(&in_new);
    for s in files {
        write_pair(&in_old, &s.rel, &s.bare_new, &s.json_old)?;
        write_pair(&in_new, &s.rel, &s.bare_new, &s.json_new)?;
    }
    // A: the old file as it is. A': old file, holes dropped. B': new file, holes dropped (the baseline of A').
    let a = weave_crate_with(&in_old, &wd.join("out-a"), &opts(o, newdir, false))?;
    let a_drop = weave_crate_with(&in_old, &wd.join("out-a-dropped"), &opts(o, newdir, true))?;
    let b_drop = weave_crate_with(&in_new, &wd.join("out-b-dropped"), &opts(o, newdir, true))?;
    let b_holes = hole_ids(&b_drop);
    let b_errs = error_ids(&b_drop);
    let new_holes: Vec<String> = hole_ids(&a_drop).difference(&b_holes).cloned().collect();
    let a_drop_errs = error_ids(&a_drop);
    let new_errs: Vec<String> = a_drop_errs.iter().filter(|(k, _)| !b_errs.contains_key(*k)).map(|(_, v)| v.clone()).collect();
    let a_errs: Vec<String> = a.errors.iter().map(|e| e.mapped.show()).collect();
    Ok(json!({
        "baseline": "builds",
        "files_woven": files.len(),
        "a_ok": a.ok(),
        "a_undetermined_holes": a.holes.len(),
        "a_errors": a_errs,
        "a_weave_errors": a.weave_errors.iter().map(|(f, e)| format!("{}: {e}", f.display())).collect::<Vec<_>>(),
        "dropped_new_undetermined_holes": new_holes,
        "dropped_new_errors": new_errs,
        "dropped_baseline_undetermined": b_holes.len(),
    }))
}

pub fn e3(repo: &Path, o: &E3Opts) -> R<Value> {
    let name = repo.canonicalize().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).unwrap_or_else(|| "repo".into());
    let to = o.to.clone().unwrap_or_else(|| "HEAD".into());
    let range = match &o.from {
        Some(f) => format!("{f}..{to}"),
        None => to.clone(),
    };
    let revs = git_text(repo, &["rev-list", "--reverse", "--no-merges", &range])?;
    let mut commits: Vec<String> = revs.lines().map(str::to_string).collect();
    if let Some(n) = o.max_commits {
        commits.truncate(n);
    }
    let wd = o.work.join(format!("e3-{name}"));
    fs::create_dir_all(&wd).map_err(|e| e.to_string())?;
    let mut recs = vec![];
    for (i, c) in commits.iter().enumerate() {
        let short = &c[..c.len().min(8)];
        let parent = format!("{c}^");
        let has_parent = git(repo, &["rev-parse", "--verify", "-q", &parent]).is_ok();
        let mut rec = json!({"commit": short, "subject": git_text(repo, &["log", "-1", "--format=%s", c]).unwrap_or_default().trim()});
        if !has_parent {
            rec["status"] = json!("root commit");
            recs.push(rec);
            continue;
        }
        let diff = git_text(repo, &["diff-tree", "-r", "--no-renames", "--name-status", "--no-commit-id", &parent, c])?;
        let (mut modified, mut added, mut deleted) = (vec![], 0, 0);
        for l in diff.lines() {
            let Some((st, p)) = l.split_once('\t') else { continue };
            if !p.ends_with(".rs") {
                continue;
            }
            match st {
                "M" => modified.push(PathBuf::from(p)),
                "A" => added += 1,
                "D" => deleted += 1,
                _ => {}
            }
        }
        rec["rs_added"] = json!(added);
        rec["rs_deleted"] = json!(deleted);
        if modified.is_empty() {
            rec["status"] = json!(if added + deleted > 0 { "rs files added or deleted only" } else { "no .rs change" });
            recs.push(rec);
            continue;
        }
        eprintln!("e3 {name}: {}/{} {short}", i + 1, commits.len());
        let newdir = wd.join("new");
        export(repo, c, &newdir)?;
        let info = crate_info(&newdir);
        let krate = info.as_ref().map(|i| format!("{} {}", i.name, i.version)).unwrap_or_else(|_| "unknown 0.0.0".into());
        let corpus: BTreeSet<PathBuf> = corpus_files(&newdir, true).unwrap_or_default().into_iter().collect();
        let mut files = vec![];
        let mut staged = vec![];
        let mut not_corpus = 0;
        for rel in &modified {
            if !corpus.contains(rel) {
                not_corpus += 1;
                continue;
            }
            match stage(repo, &parent, c, rel, &krate) {
                Ok(s) => staged.push(s),
                Err(r) => files.push(r),
            }
        }
        rec["rs_modified"] = json!(modified.len());
        rec["rs_not_in_crate"] = json!(not_corpus);
        let risky: Vec<&Staged> = staged.iter().filter(|s| s.rec["weave"] == "woven" && matches!(s.class, "signature-changing" | "body-only, holes changed")).collect();
        for s in &staged {
            files.push(s.rec.clone());
        }
        rec["files"] = Value::Array(files);
        rec["status"] = json!("compared");
        if o.cargo && !risky.is_empty() {
            if info.is_err() {
                rec["cargo"] = json!({"baseline": "no Cargo.toml"});
            } else {
                match cargo_phase(o, &newdir, &wd, &risky) {
                    Ok(v) => rec["cargo"] = v,
                    Err(e) => rec["cargo"] = json!({"baseline": "cargo failed", "detail": e}),
                }
            }
        }
        recs.push(rec);
    }
    let out = json!({"repo": name, "range": range, "commits": recs});
    fs::write(o.work.join(format!("e3-{name}.json")), serde_json::to_string_pretty(&out).unwrap()).map_err(|e| e.to_string())?;
    Ok(out)
}

fn n(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

/// The tables of results/e3.md for one or more `e3-<repo>.json` records.
pub fn render(runs: &[Value]) -> String {
    let names: Vec<String> = runs.iter().map(|r| r["repo"].as_str().unwrap_or("?").to_string()).collect();
    let mut out = String::new();
    let mut head = vec![String::new()];
    head.extend(names.iter().cloned());
    head.push("all".into());
    let mut rows: Vec<(String, Vec<u64>)> = vec![];
    let mut add = |label: &str, f: &dyn Fn(&Value) -> u64| rows.push((label.to_string(), runs.iter().map(|r| f(r)).collect()));
    let commits = |r: &Value| r["commits"].as_array().cloned().unwrap_or_default();
    let files = |r: &Value| -> Vec<Value> { commits(r).iter().flat_map(|c| c["files"].as_array().cloned().unwrap_or_default()).collect() };
    let cnt_files = |r: &Value, f: &dyn Fn(&Value) -> bool| files(r).iter().filter(|x| f(x)).count() as u64;
    let cnt_commits = |r: &Value, f: &dyn Fn(&Value) -> bool| commits(r).iter().filter(|x| f(x)).count() as u64;
    add("commits in range (no merges)", &|r| commits(r).len() as u64);
    add("commits that modify a .rs file", &|r| cnt_commits(r, &|c| n(&c["rs_modified"]) > 0 || c["status"] == "compared"));
    add("commits with a modified file in the crate", &|r| cnt_commits(r, &|c| c["status"] == "compared" && c["rs_modified"].as_u64().unwrap_or(0) > c["rs_not_in_crate"].as_u64().unwrap_or(0)));
    add("modified files in the crate", &|r| commits(r).iter().map(|c| n(&c["rs_modified"]) - n(&c["rs_not_in_crate"])).sum());
    add("  refused by strip (old or new version)", &|r| cnt_files(r, &|x| x["outcome"] == "strip refused"));
    add("  compared", &|r| cnt_files(r, &|x| x.get("class").is_some()));
    for cl in ["identical", "body-only", "body-only, holes changed", "signature-changing"] {
        add(&format!("    {cl}"), &|r| cnt_files(r, &|x| x["class"] == cl));
    }
    add("weave of new bare + old file refused (of compared)", &|r| cnt_files(r, &|x| x["weave"] == "refused"));
    add("  of which signature-changing", &|r| cnt_files(r, &|x| x["weave"] == "refused" && x["class"] == "signature-changing"));
    add("woven files with unbound entries", &|r| cnt_files(r, &|x| n(&x["unbound_total"]) > 0));
    add("unbound entries: locals", &|r| files(r).iter().map(|x| n(&x["unbound"]["unbound local"])).sum());
    add("unbound entries: hints", &|r| files(r).iter().map(|x| n(&x["unbound"]["unbound hint"])).sum());
    add("unbound entries: subjects", &|r| files(r).iter().map(|x| n(&x["unbound"]["unbound subject"])).sum());
    add("unbound entries in body-only files", &|r| files(r).iter().filter(|x| matches!(x["class"].as_str(), Some("body-only" | "identical"))).map(|x| n(&x["unbound_total"])).sum());
    add("woven file equals the new original (after rustfmt)", &|r| cnt_files(r, &|x| x["woven_equals_new"] == true));
    add("  differs, and only where a slot is woven in", &|r| cnt_files(r, &|x| x["woven_equals_new"] == false && x["differs_only_in_slots"] == true));
    add("  differs elsewhere (strip of the woven file is not the new bare body)", &|r| cnt_files(r, &|x| x["weave"] == "woven" && x["differs_only_in_slots"] == false));
    add("body-only or identical files whose woven file is not the new original", &|r| cnt_files(r, &|x| matches!(x["class"].as_str(), Some("body-only" | "identical")) && x["weave"] == "woven" && x["woven_equals_new"] == false));
    add("commits sent to cargo (a signature or hole change that wove)", &|r| cnt_commits(r, &|c| !c["cargo"].is_null()));
    add("  baseline does not build", &|r| cnt_commits(r, &|c| c["cargo"]["baseline"] != "builds" && !c["cargo"].is_null()));
    add("  baseline builds", &|r| cnt_commits(r, &|c| c["cargo"]["baseline"] == "builds"));
    add("    old constraint file kept: crate compiles", &|r| cnt_commits(r, &|c| c["cargo"]["a_ok"] == true));
    add("    old constraint file kept: crate fails", &|r| cnt_commits(r, &|c| c["cargo"]["a_ok"] == false));
    add("      compile errors", &|r| commits(r).iter().map(|c| c["cargo"]["a_errors"].as_array().map_or(0, |a| a.len() as u64)).sum());
    add("      holes undetermined", &|r| commits(r).iter().map(|c| n(&c["cargo"]["a_undetermined_holes"])).sum());
    add("    holes dropped: commits with a new undetermined hole", &|r| cnt_commits(r, &|c| c["cargo"]["dropped_new_undetermined_holes"].as_array().is_some_and(|a| !a.is_empty())));
    add("      new undetermined holes", &|r| commits(r).iter().map(|c| c["cargo"]["dropped_new_undetermined_holes"].as_array().map_or(0, |a| a.len() as u64)).sum());
    add("      new compile errors", &|r| commits(r).iter().map(|c| c["cargo"]["dropped_new_errors"].as_array().map_or(0, |a| a.len() as u64)).sum());
    let mut body: Vec<Vec<String>> = vec![];
    for (l, v) in &rows {
        let mut r = vec![l.clone()];
        r.extend(v.iter().map(u64::to_string));
        r.push(v.iter().sum::<u64>().to_string());
        body.push(r);
    }
    crate::report::table(&mut out, &head, &body);

    // refusal reasons
    let mut reasons: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for (i, r) in runs.iter().enumerate() {
        for x in files(r) {
            let (key, bucket) = if x["outcome"] == "strip refused" {
                (format!("strip refused ({}): {}", x["side"].as_str().unwrap_or(""), x["reason"].as_str().unwrap_or("")), true)
            } else if x["weave"] == "refused" {
                (format!("weave refused: {}", x["weave_reason"].as_str().unwrap_or("")), true)
            } else {
                (String::new(), false)
            };
            if bucket {
                reasons.entry(key).or_insert_with(|| vec![0; runs.len()])[i] += 1;
            }
        }
    }
    if !reasons.is_empty() {
        out.push_str("**Files not compared or not woven, by reason.**\n\n");
        let rows: Vec<Vec<String>> = reasons
            .iter()
            .map(|(k, v)| {
                let mut r = vec![k.clone()];
                r.extend(v.iter().map(u64::to_string));
                r
            })
            .collect();
        let mut h = vec![String::new()];
        h.extend(names.iter().cloned());
        crate::report::table(&mut out, &h, &rows);
    }
    out
}
