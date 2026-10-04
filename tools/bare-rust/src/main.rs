use std::{fs, path::PathBuf, process::exit};

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn run(args: Vec<String>) -> Result<(), String> {
    let rd = |p: &str| fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let out_dir = arg(&args, "--out").map(PathBuf::from);
    match args.get(1).map(String::as_str) {
        Some("strip") => {
            let file = args.get(2).ok_or("usage: strip <file.rs> --crate <name> --module <path>")?;
            let krate = arg(&args, "--crate").ok_or("--crate is required")?;
            let module = arg(&args, "--module").ok_or("--module is required")?;
            let (bare, json) = bare_rust::strip(&rd(file)?, &krate, &module)?;
            let p = PathBuf::from(file);
            let dir = out_dir.unwrap_or_else(|| p.parent().map(|d| d.to_path_buf()).unwrap_or_default());
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            let w = |path: PathBuf, s: String| fs::write(&path, s).map_err(|e| format!("{}: {e}", path.display()));
            w(dir.join(format!("{stem}.brs")), bare)?;
            w(dir.join(format!("{stem}.constraints.json")), serde_json::to_string_pretty(&json).unwrap() + "\n")
        }
        Some("weave") => {
            let (brs, cj) = (args.get(2), args.get(3));
            let (brs, cj) = brs.zip(cj).ok_or("usage: weave <file.brs> <file.constraints.json>")?;
            let json = serde_json::from_str(&rd(cj)?).map_err(|e| format!("{cj}: {e}"))?;
            let rust = bare_rust::weave(&rd(brs)?, &json)?;
            match out_dir {
                Some(d) => {
                    let stem = PathBuf::from(brs).file_stem().unwrap().to_string_lossy().to_string();
                    fs::write(d.join(format!("{stem}.rs")), rust).map_err(|e| e.to_string())
                }
                None => {
                    print!("{rust}");
                    Ok(())
                }
            }
        }
        _ => Err("usage: bare-rust strip|weave ...".into()),
    }
}

fn main() {
    if let Err(e) = run(std::env::args().collect()) {
        eprintln!("bare-rust: {e}");
        exit(1);
    }
}
