use bare_rust::corpus::{strip_crate, weave_crate, FileResult};
use bare_rust::json::canonical;
use std::{fs, path::PathBuf, process::exit};

const USAGE: &str = "usage:
  bare-rust strip <file.rs> --crate <name> --module <path> [--out <dir>]
  bare-rust strip <crate-dir> [--out <dir>] [--src-only]
  bare-rust weave <file.brs> <file.constraints.json> [--out <dir>]
  bare-rust weave <crate-dir-or-out-dir> --out <dir>
  bare-rust stats <out-dir>";

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn report(what: &str, results: &[FileResult]) -> Result<(), String> {
    let mut failed = 0;
    for r in results {
        if let Err(e) = &r.outcome {
            failed += 1;
            eprintln!("{what} refused {}: {e}", r.file.display());
        }
    }
    println!("{what}: {} files, {} refused", results.len(), failed);
    Ok(())
}

fn run(args: Vec<String>) -> Result<(), String> {
    let rd = |p: &str| fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let out_dir = arg(&args, "--out").map(PathBuf::from);
    let target = args.get(2).ok_or(USAGE)?;
    match args.get(1).map(String::as_str) {
        Some("strip") if PathBuf::from(target).is_dir() => {
            let dir = PathBuf::from(target);
            let out = out_dir.unwrap_or_else(|| dir.clone());
            report("strip", &strip_crate(&dir, &out, !args.iter().any(|a| a == "--src-only"))?)
        }
        Some("strip") => {
            let krate = arg(&args, "--crate").ok_or("--crate is required")?;
            let module = arg(&args, "--module").ok_or("--module is required")?;
            let (bare, json) = bare_rust::strip(&rd(target)?, &krate, &module)?;
            let p = PathBuf::from(target);
            let dir = out_dir.unwrap_or_else(|| p.parent().map(|d| d.to_path_buf()).unwrap_or_default());
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            let w = |path: PathBuf, s: String| fs::write(&path, s).map_err(|e| format!("{}: {e}", path.display()));
            w(dir.join(format!("{stem}.brs")), bare)?;
            w(dir.join(format!("{stem}.constraints.json")), canonical(&json))
        }
        Some("weave") if PathBuf::from(target).is_dir() => {
            let out = out_dir.ok_or("weave of a directory needs --out <dir>")?;
            report("weave", &weave_crate(&PathBuf::from(target), &out)?)
        }
        Some("weave") => {
            let cj = args.get(3).ok_or("usage: weave <file.brs> <file.constraints.json>")?;
            let json = serde_json::from_str(&rd(cj)?).map_err(|e| format!("{cj}: {e}"))?;
            let rust = bare_rust::weave(&rd(target)?, &json)?;
            match out_dir {
                Some(d) => {
                    let stem = PathBuf::from(target).file_stem().unwrap().to_string_lossy().to_string();
                    fs::write(d.join(format!("{stem}.rs")), rust).map_err(|e| e.to_string())
                }
                None => {
                    print!("{rust}");
                    Ok(())
                }
            }
        }
        Some("stats") => {
            print!("{}", bare_rust::json::ordered(&bare_rust::stats::stats(&PathBuf::from(target))?));
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

fn main() {
    if let Err(e) = run(std::env::args().collect()) {
        eprintln!("bare-rust: {e}");
        exit(1);
    }
}
