use std::{path::Path, process::Command};

fn run(dir: &Path, prog: &str, args: &[&str]) {
    let ok = Command::new(prog).args(args).current_dir(dir).status().map(|s| s.success()).unwrap_or(false);
    assert!(ok, "failed: {prog} {args:?}");
}

fn main() {
    let g = Path::new("grammar");
    let src = g.join("src");
    if !src.join("parser.c").exists() {
        run(g, "npm", &["ci"]);
        run(g, "npx", &["tree-sitter", "generate"]);
    }
    cc::Build::new()
        .include(&src)
        .file(src.join("parser.c"))
        .file(src.join("scanner.c"))
        .warnings(false)
        .compile("tree_sitter_bare_rust");
    for f in ["grammar/grammar.js", "grammar/src/scanner.c", "grammar/package.json", "build.rs"] {
        println!("cargo:rerun-if-changed={f}");
    }
    println!("cargo:rerun-if-changed=grammar/src/parser.c");
}
