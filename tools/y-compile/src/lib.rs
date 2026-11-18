//! The Y compiler: reads an example's Y source and writes what the lowering gives, its bare Rust bodies, its constraint
//! files and its `y-facts.json`, which `bare-rust weave` turns into Rust.
pub mod emit;
pub mod lower;
pub mod model;
pub mod ty;
pub mod bare;
pub mod facts;

use std::fs;
use std::path::{Path, PathBuf};

pub struct Options {
    /// Where the bare files and `y-facts.json` go: `<out>/src/*.brs`, `<out>/src/*.constraints.json`, `<out>/y-facts.json`.
    pub out: Option<PathBuf>,
    /// Where the typed Rust the lowering strips goes, for reading.
    pub rust: Option<PathBuf>,
}

pub fn read(p: &Path) -> Result<String, String> {
    fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn write(p: &Path, s: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    fs::write(p, s).map_err(|e| format!("{}: {e}", p.display()))
}

/// The package's name and version, from its `Cargo.toml`.
fn package(dir: &Path) -> Result<(String, String), String> {
    let toml = read(&dir.join("Cargo.toml"))?;
    let field = |k: &str| toml.lines().find_map(|l| l.strip_prefix(&format!("{k} = \"")).map(|v| v.trim_end_matches('"').to_string()));
    Ok((field("name").ok_or("Cargo.toml has no name")?, field("version").ok_or("Cargo.toml has no version")?))
}

/// Compiles the example in `dir`.
pub fn compile(dir: &Path, opts: &Options) -> Result<String, String> {
    let src = dir.join("y/src");
    let mut ys: Vec<String> = fs::read_dir(&src).map_err(|e| format!("{}: {e}", src.display()))?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|f| f.ends_with(".y")).collect();
    ys.sort();
    let module_y = ys.iter().find(|f| !f.ends_with(".test.y") && *f != "main.y").ok_or("no module in y/src")?.clone();
    let name = module_y.trim_end_matches(".y").to_string();
    let parse = |f: &str| y_parse::parse_file(&src.join(f));
    let tree = parse(&module_y)?;
    let test_tree = parse(&format!("{name}.test.y"))?;
    let main_tree = parse("main.y")?;
    let m = model::Module::new(&tree);
    let tm = model::Module::new_in(&test_tree, Some(&m));
    let mm = model::Module::new(&main_tree);
    let (pkg, version) = package(dir)?;
    let lib_crate = pkg.replace('-', "_");
    let e = emit::emit(&m, &test_tree, &tm, &mm, &lib_crate);
    let files = vec![
        (PathBuf::from("src/lib.rs"), e.lib.clone()),
        (PathBuf::from(format!("src/{name}.rs")), e.module.clone()),
        (PathBuf::from(format!("src/{name}/tests.rs")), e.tests.clone()),
        (PathBuf::from("src/main.rs"), e.main.clone()),
    ];
    if let Some(r) = &opts.rust {
        for (p, s) in &files {
            write(&r.join(p), s)?;
        }
    }
    let Some(out) = &opts.out else { return Ok(format!("y-compile: wrote the typed Rust of {name}")) };
    let stripped = bare::strip(&format!("{pkg} {version}"), &files)?;
    let lowered = bare::lower(&m, &tree, &read(&src.join(&module_y))?, &stripped, &name, &e.invariant_sites)?;
    for (p, (brs, js)) in &lowered {
        write(&out.join(p.with_extension("brs")), brs)?;
        write(&out.join(p.with_extension("constraints.json")), &bare_rust::json::canonical(js))?;
    }
    let facts = facts::facts(dir, &m, &tree, &test_tree, &main_tree, &lowered, &lib_crate, &name, &e.copies)?;
    write(&out.join("y-facts.json"), &(serde_json::to_string_pretty(&facts).unwrap() + "\n"))?;
    Ok(format!("y-compile: {name}: {} files lowered", lowered.len()))
}
