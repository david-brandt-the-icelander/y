//! Crate mode: find a crate's corpus files, strip them into `x.brs` and `x.constraints.json`, weave them back.
use crate::json::canonical;
use crate::{named_children, parse_rust, strip, text, weave, R};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub lib: Option<String>,
}

fn toml_value(line: &str, key: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
}

/// The crate's name and version, and its `[lib]` path, from Cargo.toml.
pub fn crate_info(dir: &Path) -> R<CrateInfo> {
    let p = dir.join("Cargo.toml");
    let s = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let (mut section, mut name, mut version, mut lib) = (String::new(), None, None, None);
    for line in s.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.to_string();
        } else if section == "[package]" {
            name = name.or_else(|| toml_value(line, "name"));
            version = version.or_else(|| toml_value(line, "version"));
        } else if section == "[lib]" {
            lib = lib.or_else(|| toml_value(line, "path"));
        }
    }
    Ok(CrateInfo {
        name: name.ok_or("Cargo.toml: no package name")?,
        version: version.unwrap_or_else(|| "0.0.0".into()),
        lib,
    })
}

fn rs_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rs")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Follow `mod x;` declarations from a file to the files they name.
fn submodules(file: &Path, root: bool, out: &mut Vec<PathBuf>) {
    let Ok(src) = fs::read_to_string(file) else { return };
    let dir = file.parent().unwrap_or(Path::new("."));
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let base = if root || stem == "mod" { dir.to_path_buf() } else { dir.join(stem) };
    let tree = parse_rust(&src);
    for m in named_children(tree.root_node()).into_iter().filter(|n| n.kind() == "mod_item" && n.child_by_field_name("body").is_none()) {
        let name = text(m.child_by_field_name("name").unwrap(), &src);
        for cand in [base.join(format!("{name}.rs")), base.join(name).join("mod.rs")] {
            if cand.exists() {
                if !out.contains(&cand) {
                    out.push(cand.clone());
                    submodules(&cand, false, out);
                }
                break;
            }
        }
    }
}

/// Every `.rs` file the crate compiles as library, binary or integration test (not benches or examples),
/// relative to the crate directory.
pub fn corpus_files(dir: &Path, include_tests: bool) -> R<Vec<PathBuf>> {
    let info = crate_info(dir)?;
    let mut roots: Vec<PathBuf> = vec![];
    let lib = dir.join(info.lib.as_deref().unwrap_or("src/lib.rs"));
    if lib.exists() {
        roots.push(lib);
    }
    let main = dir.join("src/main.rs");
    if main.exists() {
        roots.push(main);
    }
    roots.extend(rs_in(&dir.join("src/bin")));
    if include_tests {
        roots.extend(rs_in(&dir.join("tests")));
        if let Ok(rd) = fs::read_dir(dir.join("tests")) {
            let mut subs: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path().join("main.rs")).filter(|p| p.exists()).collect();
            subs.sort();
            roots.extend(subs);
        }
    }
    let mut all = roots.clone();
    for r in &roots {
        submodules(r, true, &mut all);
    }
    all.sort();
    all.dedup();
    Ok(all.into_iter().map(|p| p.strip_prefix(dir).map(Path::to_path_buf).unwrap_or(p)).collect())
}

/// What strip did to one corpus file.
pub struct FileResult {
    pub file: PathBuf,
    pub outcome: Result<(), String>,
}

fn write_file(p: &Path, s: &str) -> R<()> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    fs::write(p, s).map_err(|e| format!("{}: {e}", p.display()))
}

/// Strip every corpus file of a crate into `out` (beside the sources if `out` is the crate directory).
/// Writes `refusals.json` into `out` when `out` is not the crate directory.
pub fn strip_crate(dir: &Path, out: &Path, include_tests: bool) -> R<Vec<FileResult>> {
    let info = crate_info(dir)?;
    let krate = format!("{} {}", info.name, info.version);
    let mut results = vec![];
    for rel in corpus_files(dir, include_tests)? {
        let src = fs::read_to_string(dir.join(&rel)).map_err(|e| format!("{}: {e}", rel.display()))?;
        let module = rel.to_string_lossy().replace('\\', "/");
        let outcome = strip(&src, &krate, &module).and_then(|(bare, js)| {
            write_file(&out.join(rel.with_extension("brs")), &bare)?;
            write_file(&out.join(rel.with_extension("constraints.json")), &canonical(&js))
        });
        results.push(FileResult { file: rel, outcome });
    }
    let refused: Vec<Value> = results
        .iter()
        .filter_map(|r| {
            let e = r.outcome.as_ref().err()?;
            let (reason, detail) = e.split_once(": ").unwrap_or(("error", e));
            Some(json!({"file": r.file.to_string_lossy(), "reason": reason, "detail": detail}))
        })
        .collect();
    write_file(&out.join("refusals.json"), &canonical(&json!({"refused": refused})))?;
    Ok(results)
}

fn find_brs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if name != "target" && name != "corpora" && !name.starts_with('.') {
                find_brs(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "brs") {
            out.push(p);
        }
    }
}

/// Weave every `x.brs` below `from` (with its `x.constraints.json`) into `x.rs` below `out`.
pub fn weave_crate(from: &Path, out: &Path) -> R<Vec<FileResult>> {
    let mut brs = vec![];
    find_brs(from, &mut brs);
    if brs.is_empty() {
        return Err(format!("{}: no .brs files", from.display()));
    }
    let mut results = vec![];
    for b in brs {
        let rel = b.strip_prefix(from).unwrap_or(&b).to_path_buf();
        let outcome = (|| {
            let bare = fs::read_to_string(&b).map_err(|e| format!("{}: {e}", b.display()))?;
            let cj = b.with_extension("constraints.json");
            let js = fs::read_to_string(&cj).map_err(|e| format!("{}: {e}", cj.display()))?;
            let js: Value = serde_json::from_str(&js).map_err(|e| format!("{}: {e}", cj.display()))?;
            write_file(&out.join(rel.with_extension("rs")), &weave(&bare, &js)?)
        })();
        results.push(FileResult { file: rel, outcome });
    }
    Ok(results)
}
