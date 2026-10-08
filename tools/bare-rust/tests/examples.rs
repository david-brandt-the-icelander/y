//! The crates under `examples/` keep their stripped output (`bare/`) in the repository. This test strips each example
//! again and requires the output to equal the committed files byte for byte, then weaves the committed files and
//! checks the round trip without cargo (check 1: strip(weave(b, c)) == (b, c); check 2: the woven text equals the
//! original after rustfmt, with `mut` on bindings removed, as in `tests/corpora.rs`).
//! To refresh the committed files after a change to the tool: `cargo run -q -- strip examples/<name> --out examples/<name>/bare`.
use bare_rust::corpus::strip_crate;
use bare_rust::json::canonical;
use bare_rust::{normalise_nomut, rustfmt, strip, weave};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn examples() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root().join("examples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("Cargo.toml").exists())
        .collect();
    dirs.sort();
    dirs
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    fn go(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                go(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
    let mut out = vec![];
    go(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn there_are_six_examples() {
    assert_eq!(examples().len(), 6);
}

#[test]
fn committed_bare_and_constraint_files_are_current() {
    let scratch = root().join("target/examples-out");
    let _ = fs::remove_dir_all(&scratch);
    for dir in examples() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let out = scratch.join(&name);
        let results = strip_crate(&dir, &out, true).unwrap();
        for r in &results {
            assert!(r.outcome.is_ok(), "{name}: strip refused {}: {:?}", r.file.display(), r.outcome);
        }
        let committed = dir.join("bare");
        let (fresh, kept) = (files_under(&out), files_under(&committed));
        assert_eq!(fresh, kept, "{name}: the set of files in bare/ is stale");
        for f in fresh {
            let (a, b) = (fs::read_to_string(out.join(&f)).unwrap(), fs::read_to_string(committed.join(&f)).unwrap());
            assert_eq!(a, b, "{name}: {} is stale; rerun the strip command in the test's header", f.display());
        }
    }
}

#[test]
fn committed_files_round_trip_without_cargo() {
    for dir in examples() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let bare_dir = dir.join("bare/src");
        let brs = fs::read_to_string(bare_dir.join("lib.brs")).unwrap();
        let cjs = fs::read_to_string(bare_dir.join("lib.constraints.json")).unwrap();
        let js: Value = serde_json::from_str(&cjs).unwrap();
        let woven = weave(&brs, &js).unwrap_or_else(|e| panic!("{name}: weave: {e}"));
        let (b2, j2) = strip(&woven, js["crate"].as_str().unwrap(), js["module"].as_str().unwrap())
            .unwrap_or_else(|e| panic!("{name}: strip(woven): {e}"));
        assert_eq!(b2, brs, "{name}: check 1, bare differs");
        assert_eq!(canonical(&j2), cjs, "{name}: check 1, constraints differ");
        let original = fs::read_to_string(dir.join("src/lib.rs")).unwrap();
        let norm = |s: &str| normalise_nomut(s).and_then(|n| rustfmt(&n));
        assert_eq!(norm(&woven).unwrap(), norm(&original).unwrap(), "{name}: check 2, woven text differs from the original");
    }
}
