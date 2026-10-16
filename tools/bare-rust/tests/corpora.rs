//! Checks of proposal 9 over the corpora: the golden file, the tool's own source, strsim, fnv, itoa, memchr, miniz_oxide
//! and png.
//! `corpora_checks_1_and_2_without_cargo` runs strip and weave in memory (check 2 ignores `mut`); the `cargo_*` tests
//! run checks 1 to 4 with cargo and rustc, for each crate.
//! Needs the pinned crates: run `corpora/fetch.sh` first.
use bare_rust::corpus::{strip_crate, weave_crate};
use bare_rust::json::canonical;
use bare_rust::{normalise_nomut, rustfmt, strip};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

struct Unit {
    corpus: &'static str,
    rel: PathBuf,
    original: PathBuf,
}

struct Row {
    corpus: &'static str,
    file: String,
    refused: Option<String>,
    schema: Option<String>,
    check1: Option<String>,
    check2: Option<String>,
}

fn first_diff(a: &str, b: &str) -> String {
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    for i in 0..la.len().max(lb.len()) {
        if la.get(i) != lb.get(i) {
            return format!("line {}: `{}` != `{}`", i + 1, la.get(i).unwrap_or(&"<end>"), lb.get(i).unwrap_or(&"<end>"));
        }
    }
    "differs".into()
}

#[test]
fn corpora_checks_1_and_2_without_cargo() {
    let fetched = root().join("corpora/src/strsim/Cargo.toml").exists() && root().join("corpora/src/fnv/Cargo.toml").exists();
    assert!(fetched, "the corpora are not fetched: run tools/bare-rust/corpora/fetch.sh");
    let out = root().join("target/corpora-out");
    let _ = fs::remove_dir_all(&out);
    let woven_root = out.join("woven");
    let mut units: Vec<Unit> = vec![];

    // Crates: the tool's own src, strsim (lib and tests), fnv.
    for (corpus, dir, tests) in [
        ("tool", root(), false),
        ("strsim", root().join("corpora/src/strsim"), true),
        ("fnv", root().join("corpora/src/fnv"), true),
    ] {
        // Without cargo: the default build only (the `cargo_*` tests choose the builds).
        let results = bare_rust::corpus::strip_crate_with(&dir, &out.join(corpus), tests, None).unwrap();
        for r in results {
            units.push(Unit { corpus, original: dir.join(&r.file), rel: r.file });
        }
    }
    // The golden file of proposal 14.9.
    let golden = root().join("tests/golden/cache.rs");
    let gout = out.join("golden");
    fs::create_dir_all(&gout).unwrap();
    let mut refused = vec![];
    match strip(&fs::read_to_string(&golden).unwrap(), "eve_cache 0.1.0", "src/lib.rs") {
        Ok((b, j)) => {
            fs::write(gout.join("cache.brs"), b).unwrap();
            fs::write(gout.join("cache.constraints.json"), canonical(&j)).unwrap();
        }
        Err(e) => refused.push(json!({"file": "cache.rs", "reason": e.split(": ").next(), "detail": e})),
    }
    fs::write(gout.join("refusals.json"), canonical(&json!({"refused": refused}))).unwrap();
    units.push(Unit { corpus: "golden", rel: "cache.rs".into(), original: golden });

    // Weave everything back (check 2's woven side).
    for c in ["tool", "strsim", "fnv", "golden"] {
        let _ = weave_crate(&out.join(c), &woven_root.join(c));
    }

    // Check 1 is over a crate: the woven files stripped as `strip` strips a crate (one name per realization across it,
    // and its table in the root's file).
    let mut again: std::collections::HashMap<(&str, PathBuf), Result<(String, Value), String>> = Default::default();
    for (corpus, dir, tests) in [("tool", root(), false), ("strsim", root().join("corpora/src/strsim"), true), ("fnv", root().join("corpora/src/fnv"), true)] {
        let info = bare_rust::corpus::crate_info(&dir).unwrap();
        let sources: Vec<(PathBuf, String)> = units
            .iter()
            .filter(|u| u.corpus == corpus)
            .filter_map(|u| Some((u.rel.clone(), fs::read_to_string(woven_root.join(corpus).join(&u.rel)).ok()?)))
            .collect();
        let trees = bare_rust::corpus::crate_trees(&dir, tests).unwrap();
        let reach = bare_rust::builds::Reach::load(&out.join(corpus));
        for (rel, r) in bare_rust::corpus::strip_sources_in(&format!("{} {}", info.name, info.version), &sources, &trees, reach.as_ref()) {
            again.insert((corpus, rel), r);
        }
    }

    let schema: Value = serde_json::from_str(include_str!("../schema/y-constraints-0.schema.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut rows = vec![];
    for u in &units {
        let mut row = Row { corpus: u.corpus, file: u.rel.display().to_string(), refused: None, schema: None, check1: None, check2: None };
        let brs = out.join(u.corpus).join(u.rel.with_extension("brs"));
        let cj = out.join(u.corpus).join(u.rel.with_extension("constraints.json"));
        if !brs.exists() {
            row.refused = Some(
                fs::read_to_string(out.join(u.corpus).join("refusals.json"))
                    .ok()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .and_then(|v| v["refused"].as_array()?.iter().find(|r| r["file"].as_str() == Some(&u.rel.to_string_lossy()))
                        .map(|r| format!("{}: {}", r["reason"].as_str().unwrap_or("?"), r["detail"].as_str().unwrap_or(""))))
                    .unwrap_or_else(|| "refused".into()),
            );
            rows.push(row);
            continue;
        }
        let (bare, cjs) = (fs::read_to_string(&brs).unwrap(), fs::read_to_string(&cj).unwrap());
        let js: Value = serde_json::from_str(&cjs).unwrap();
        let errs: Vec<String> = validator.iter_errors(&js).take(3).map(|e| format!("{} at {}", e, e.instance_path())).collect();
        if !errs.is_empty() {
            row.schema = Some(errs.join("; "));
        }
        let woven_path = woven_root.join(u.corpus).join(&u.rel);
        match fs::read_to_string(&woven_path) {
            Err(_) => {
                // weave failed: report why
                row.check1 = Some("weave failed".into());
                row.check2 = Some(match bare_rust::weave(&bare, &js) {
                    Err(e) => format!("weave: {e}"),
                    Ok(_) => "weave output missing".into(),
                });
            }
            Ok(woven) => {
                // Check 1: strip(weave(b, c)) == (b, c), byte for byte.
                let stripped = match again.remove(&(u.corpus, u.rel.clone())) {
                    Some(r) => r,
                    None => strip(&woven, js["crate"].as_str().unwrap(), js["module"].as_str().unwrap()),
                };
                row.check1 = match stripped {
                    Err(e) => Some(format!("strip(woven): {e}")),
                    Ok((b2, j2)) => {
                        if b2 != bare {
                            Some(format!("bare differs: {}", first_diff(&b2, &bare)))
                        } else if canonical(&j2) != cjs {
                            Some(format!("json differs: {}", first_diff(&canonical(&j2), &cjs)))
                        } else {
                            None
                        }
                    }
                };
                // Check 2 without cargo: weave(strip(s)) == s after rustfmt, lifetimes elided and `mut` on bindings
                // removed, because this weave derives none. The checks with cargo (below) do not remove it.
                let original = fs::read_to_string(&u.original).unwrap();
                let norm = |s: &str| normalise_nomut(s).and_then(|n| rustfmt(&n));
                row.check2 = match (norm(&woven), norm(&original)) {
                    (Ok(a), Ok(b)) if a == b => None,
                    (Ok(a), Ok(b)) => Some(first_diff(&a, &b)),
                    (Err(e), _) => Some(format!("woven: {e}")),
                    (_, Err(e)) => Some(format!("original: {e}")),
                };
            }
        }
        rows.push(row);
    }

    // Report, per file.
    let mut failures = 0;
    for r in &rows {
        let mark = |x: &Option<String>| if x.is_none() { "ok" } else { "FAIL" };
        let status = match &r.refused {
            Some(why) => format!("refused ({why})"),
            None => format!("schema {} check1 {} check2 {}", mark(&r.schema), mark(&r.check1), mark(&r.check2)),
        };
        println!("{:7} {:28} {status}", r.corpus, r.file);
        for (what, x) in [("schema", &r.schema), ("check 1", &r.check1), ("check 2", &r.check2)] {
            if let Some(m) = x {
                failures += 1;
                println!("          {what}: {m}");
            }
        }
    }
    let stats = bare_rust::stats::stats(&out).unwrap();
    fs::write(out.join("stats.json"), bare_rust::json::ordered(&stats)).unwrap();
    let refused = rows.iter().filter(|r| r.refused.is_some()).count();
    println!("{} files, {} refused, {} failed checks; stats in target/corpora-out/stats.json", rows.len(), refused, failures);
    assert_eq!(failures, 0, "some checks failed");
    assert!(Path::new(&out.join("stats.json")).exists());
}

// ---------- checks 1 to 4 with cargo ----------

use bare_rust::check::{check_crate, CheckOpts, CheckReport};
use bare_rust::crate_weave::HoleReport;

/// Check 5 runs strict for a crate with an externals list: an external item without an entry fails the check.
fn opts(full: bool, tests: bool) -> CheckOpts {
    CheckOpts {
        work: root().join("target/bare-rust-check"),
        target: bare_rust::cargo::target_dir(&root()),
        tests,
        full,
        libraries: Some(root().join("libraries")),
        strict_libraries: true,
    }
}

fn run_checks(dir: PathBuf, full: bool, tests: bool) -> CheckReport {
    assert!(dir.join("Cargo.toml").exists(), "{} is not fetched: run tools/bare-rust/corpora/fetch.sh", dir.display());
    let r = check_crate(&dir, &opts(full, tests)).unwrap();
    r.print();
    assert!(r.refused.is_empty(), "files refused: {:?}", r.refused);
    assert!(r.c1_ok(), "check 1 failed");
    assert!(r.c2_ok(), "check 2 failed (mut differs: {:?})", r.c2.mut_differs);
    assert!(r.c3_ok(), "check 3 failed");
    assert!(r.c4_ok(), "check 4 failed");
    assert!(r.c5_ok(), "check 5 failed: {:?}", r.c5.as_ref().map(|c| (&c.missing, &c.invalid, c.sigs.as_ref().map(|s| &s.mismatches))));
    r
}

/// The corpora with a committed externals list were checked against the library file, and every item had an entry.
fn assert_library_complete(r: &CheckReport) {
    let c5 = r.c5.as_ref().unwrap_or_else(|| panic!("check 5 did not run for {}", r.name));
    assert!(c5.coverage_ok() && c5.missing.is_empty() && c5.valid_ok() && c5.sigs_ok(), "{}: {:?}", r.name, c5.missing);
}

/// Check 4: every undetermined hole is at a real hole site of the bare file, and nothing was filled by weave.
fn assert_holes_are_sites(r: &CheckReport) {
    let bare = root().join("target/bare-rust-check").join(&r.name).join("bare");
    assert_eq!(r.c4.silently_filled, 0, "weave printed hole entries although the file had none");
    assert!(r.c4.other_errors.is_empty(), "failures that are not holes: {:?}", r.c4.other_errors);
    // Every hole dropped, as check 4 drops them: the crate's realizations keep their templates, with `_` for each argument.
    let mut brs_files = vec![];
    bare_rust::corpus::find_brs(&bare, &mut brs_files);
    let listed: Vec<(std::path::PathBuf, Value)> = brs_files
        .iter()
        .map(|b| (b.clone(), serde_json::from_str::<Value>(&fs::read_to_string(b.with_extension("constraints.json")).unwrap()).unwrap()))
        .collect();
    let (mut reals, _) = bare_rust::crate_weave::crate_realizations(&listed).unwrap();
    for v in reals.values_mut() {
        v["args"] = json!(vec!["_"; v["args"].as_array().map_or(0, Vec::len)]);
    }
    let sites_of = |h: &HoleReport| {
        let p = bare.join(&h.file);
        let brs = fs::read_to_string(&p).unwrap();
        let mut js: Value = serde_json::from_str(&fs::read_to_string(p.with_extension("constraints.json")).unwrap()).unwrap();
        js["holes"] = json!({});
        let w = bare_rust::weave::weave_in_crate(&brs, &js, Some(&reals)).unwrap();
        // The hole as weave sees it: the woven text has nothing at the site.
        assert!(w.map.spans.iter().all(|s| s.origin.kind != bare_rust::trace::Kind::Hole));
        w.sites.iter().filter(|s| s.subject == h.subject && s.kind == h.kind && s.n == h.n && s.key == h.key && !s.filled)
            .map(|s| w.bare_at(s.range.0))
            .collect::<Vec<_>>()
    };
    for h in &r.c4.undetermined {
        assert!(sites_of(h).contains(&(h.line, h.col)), "not a hole site: {}", h.show());
        assert!(h.clause.starts_with("local ") || h.clause.starts_with("hint ") || h.clause.starts_with("realize "), "{}", h.clause);
    }
}

#[test]
fn cargo_strsim() {
    let r = run_checks(root().join("corpora/src/strsim"), true, true);
    assert_library_complete(&r);
    assert_holes_are_sites(&r);
}

#[test]
fn cargo_fnv() {
    let r = run_checks(root().join("corpora/src/fnv"), true, true);
    assert_library_complete(&r);
    assert_holes_are_sites(&r);
}

// Crates with the forms the others lack. No externals list is committed for them, so check 5 does not run.

#[test]
fn cargo_itoa() {
    // Pointer casts, as cast locals of type `ptr<T>`; an impl for a primitive type.
    let r = run_checks(root().join("corpora/src/itoa"), true, true);
    assert_holes_are_sites(&r);
}

#[test]
fn cargo_memchr() {
    // Pointers in signatures, a `const` whose whole value is a cast, a union, impls for pointer types.
    let r = run_checks(root().join("corpora/src/memchr"), true, true);
    assert_holes_are_sites(&r);
}

#[test]
fn cargo_miniz_oxide() {
    // A cast in an array length; code a `#[cfg]` leaves out, which keeps its `mut`.
    let r = run_checks(root().join("corpora/src/miniz_oxide/miniz_oxide"), true, true);
    assert_holes_are_sites(&r);
}

#[test]
fn cargo_png() {
    // Suffixed literals in patterns where the pattern decides what a generic call reads (`match buf.read_be()? { 0u8 => .. }`).
    let r = run_checks(root().join("corpora/src/png"), true, true);
    assert_holes_are_sites(&r);
}

#[test]
fn cargo_golden() {
    // The golden file with `Key` and `Entry` stubbed after it, as a small crate.
    let dir = root().join("tests/golden/crate");
    let lib = fs::read_to_string(dir.join("src/lib.rs")).unwrap();
    assert!(lib.starts_with(&fs::read_to_string(root().join("tests/golden/cache.rs")).unwrap()));
    let r = run_checks(dir, false, true);
    assert_library_complete(&r);
    assert_holes_are_sites(&r);
}

#[test]
fn cargo_tool_own_source() {
    // The tool's own `src`: `cargo check` of the woven crate is enough.
    let r = run_checks(root(), false, false);
    assert_holes_are_sites(&r);
}
