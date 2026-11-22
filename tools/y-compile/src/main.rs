use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: y-compile <example dir> --out <dir> [--rust <dir>]";
    let dir = args.get(1).map(PathBuf::from).unwrap_or_else(|| { eprintln!("{usage}"); std::process::exit(2) });
    let flag = |f: &str| args.iter().position(|a| a == f).and_then(|k| args.get(k + 1)).map(PathBuf::from);
    let opts = y_compile::Options { out: flag("--out"), rust: flag("--rust") };
    if opts.out.is_none() && opts.rust.is_none() {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    match y_compile::compile(&dir, &opts) {
        Ok(report) => println!("{report}"),
        Err(e) => {
            eprintln!("y-compile: {e}");
            std::process::exit(1);
        }
    }
}
