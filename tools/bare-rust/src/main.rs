use bare_rust::corpus::{strip_crate, FileResult};
use bare_rust::check::{check_crate, CheckOpts};
use bare_rust::crate_weave::{weave_crate_with, WeaveOpts};
use bare_rust::json::canonical;
use std::{fs, path::PathBuf, process::exit};

const USAGE: &str = "usage:
  bare-rust strip <file.rs> --crate <name> --module <path> [--out <dir>]
  bare-rust strip <crate-dir> [--out <dir>] [--src-only]
  bare-rust weave <file.brs> <file.constraints.json> [--out <dir>]
  bare-rust weave <dir> --out <dir> [--crate <crate-dir>] [--no-cargo] [--drop-holes]
      <dir> holds the .brs and .constraints.json files (the crate directory, if stripped in place); the crate's
      Cargo.toml and other files come from --crate (default <dir>). Runs cargo check, derives `mut`, reports
      undetermined holes and errors; exits non-zero on any. --no-cargo only writes the files.
  bare-rust check <crate-dir> [--check-only] [--src-only] [--work <dir>] [--libraries <dir>] [--allow-missing-libraries]
      checks 1 to 5 of the plan for one crate; --check-only skips clippy and cargo test; results in <work>/<name>/check.json;
      check 5 (library files) runs when <libraries>/externals/<crate>.json exists, and fails on an external item with no
      entry unless --allow-missing-libraries
  bare-rust externals <crate-dir> [--all-targets] [--out <file>] [--libraries <dir>]
      every external item (std) the crate uses, as canonical paths with files and use counts, from rustc's MIR and
      tree-sitter; written to <libraries>/externals/<crate>.json (default libraries/ beside Cargo.toml)
  bare-rust libcheck <crate-dir> [--libraries <dir>] [--work <dir>] [--allow-missing]
      check 5, mechanical half: coverage of the crate's externals by the library file, schema validity, a syntactic
      check of every clause's text (brackets, keyword, quoted host paths, declared failures, known names), and the
      signature compile check; exits non-zero on a missing item (unless --allow-missing), an invalid file or a
      mismatched signature
  bare-rust libfmt <library-file>...
  bare-rust libfmt --expand <library-file>     (print what each type's `derive` list expands to)
      rewrite library files in canonical form
  bare-rust libstats <library-file> [--work <dir>] [--no-signatures]
      the counts of a library file (entries and clauses by kind, status and source, the `unexpressed` promises by what
      the language lacks) and, unless --no-signatures, how its signatures checked against the real std
  bare-rust stats <out-dir>
  bare-rust report <check-dir>...
      the tables of results/e1.md, from the <work>/<name> directories that `check` wrote (check.json and bare/)";

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

/// Where the library files and the externals lists are kept: `--libraries`, else `libraries/` beside Cargo.toml.
fn libraries_dir(args: &[String]) -> PathBuf {
    arg(args, "--libraries").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("libraries"))
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
            let cargo = !args.iter().any(|a| a == "--no-cargo");
            let opts = WeaveOpts {
                cargo,
                crate_dir: arg(&args, "--crate").map(PathBuf::from),
                target: bare_rust::cargo::target_dir(&std::env::current_dir().map_err(|e| e.to_string())?),
                drop_holes: args.iter().any(|a| a == "--drop-holes"),
                keep: Default::default(),
            };
            let cw = weave_crate_with(&PathBuf::from(target), &out, &opts)?;
            cw.print();
            if cw.ok() {
                Ok(())
            } else {
                Err("weave failed".into())
            }
        }
        Some("weave") => {
            let cj = args.get(3).ok_or("usage: weave <file.brs> <file.constraints.json>")?;
            let json = serde_json::from_str(&rd(cj)?).map_err(|e| format!("{cj}: {e}"))?;
            let woven = bare_rust::weave::weave_traced(&rd(target)?, &json)?;
            for u in &woven.unbound {
                eprintln!("warning: {}", u.show());
            }
            let rust = woven.text;
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
        Some("check") => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let o = CheckOpts {
                work: arg(&args, "--work").map(PathBuf::from).unwrap_or_else(|| cwd.join("target/bare-rust-check")),
                target: bare_rust::cargo::target_dir(&cwd),
                tests: !args.iter().any(|a| a == "--src-only"),
                full: !args.iter().any(|a| a == "--check-only"),
                libraries: Some(libraries_dir(&args)),
                strict_libraries: !args.iter().any(|a| a == "--allow-missing-libraries"),
            };
            let r = check_crate(&PathBuf::from(target), &o)?;
            r.print();
            if r.ok() {
                Ok(())
            } else {
                Err(format!("check failed for {}", r.name))
            }
        }
        Some("externals") => {
            let all = args.iter().any(|a| a == "--all-targets");
            let dir = PathBuf::from(target);
            let (krate, e) = bare_rust::externals::externals(&dir, all)?;
            let out = arg(&args, "--out").map(PathBuf::from).unwrap_or_else(|| {
                let name = bare_rust::corpus::crate_info(&dir).map(|i| i.name).unwrap_or_default();
                libraries_dir(&args).join("externals").join(format!("{name}.json"))
            });
            if let Some(d) = out.parent() {
                fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            fs::write(&out, bare_rust::json::ordered(&e.to_json(&krate, all))).map_err(|e| format!("{}: {e}", out.display()))?;
            println!("{}", bare_rust::externals::summary(&krate, &e));
            println!("written to {}", out.display());
            Ok(())
        }
        Some("libcheck") => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let work = arg(&args, "--work").map(PathBuf::from).unwrap_or_else(|| cwd.join("target/bare-rust-check"));
            let strict = !args.iter().any(|a| a == "--allow-missing");
            let r = bare_rust::libcheck::libcheck(&PathBuf::from(target), &libraries_dir(&args), &work, &bare_rust::cargo::target_dir(&cwd))?;
            r.print(strict);
            if r.ok(strict) {
                Ok(())
            } else {
                Err(format!("libcheck failed for {}", r.krate))
            }
        }
        Some("libstats") => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let file: serde_json::Value = serde_json::from_str(&rd(target)?).map_err(|e| format!("{target}: {e}"))?;
            let sigs = if args.iter().any(|a| a == "--no-signatures") {
                None
            } else {
                let work = arg(&args, "--work").map(PathBuf::from).unwrap_or_else(|| cwd.join("target/bare-rust-check")).join("libstats");
                Some(bare_rust::libcheck::check_signatures(&file, &work, &bare_rust::cargo::target_dir(&cwd))?)
            };
            print!("{}", bare_rust::libstats::libstats(&file, sigs.as_ref()));
            Ok(())
        }
        Some("libfmt") => {
            if args[2] == "--expand" {
                let v: serde_json::Value = serde_json::from_str(&rd(&args[3])?).map_err(|e| format!("{}: {e}", args[3]))?;
                print!("{}", bare_rust::derive::show(&v));
                return Ok(());
            }
            let paths: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            let n = bare_rust::libcheck::format_files(&paths)?;
            println!("libfmt: {} of {} files rewritten", n, paths.len());
            Ok(())
        }
        Some("report") => {
            let dirs: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            let refs: Vec<&std::path::Path> = dirs.iter().map(PathBuf::as_path).collect();
            let git = |a: &[&str]| std::process::Command::new("git").args(a).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            let rev = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
            let dirty = git(&["status", "--porcelain", "--", ".", ":!results"]).is_some_and(|s| !s.is_empty());
            let prov = format!("Generated by `bare-rust report {}` at commit {rev}{}.", args[2..].join(" "), if dirty { " with uncommitted changes" } else { "" });
            print!("{}", bare_rust::report::report(&refs, &prov)?);
            Ok(())
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
