//! Checks 1 and 2 of proposal 9 over the corpora: the golden file, the tool's own source, strsim and fnv.
//! Needs the pinned crates: run `corpora/fetch.sh` first.
use bare_rust::corpus::{strip_crate, weave_crate};
use bare_rust::json::canonical;
use bare_rust::{normalise, rustfmt, strip};
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
fn corpora_checks_1_and_2() {
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
        let results = strip_crate(&dir, &out.join(corpus), tests).unwrap();
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
                row.check1 = match strip(&woven, js["crate"].as_str().unwrap(), js["module"].as_str().unwrap()) {
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
                // Check 2: weave(strip(s)) == s after rustfmt, lifetimes elided and `mut` on bindings removed.
                let original = fs::read_to_string(&u.original).unwrap();
                let norm = |s: &str| normalise(s).and_then(|n| rustfmt(&n));
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
