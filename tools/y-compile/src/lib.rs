//! The Y compiler: reads an example's Y source and writes what the lowering gives, its bare Rust bodies, its constraint
//! files and its `y-facts.json`, which `bare-rust weave` turns into Rust.
pub mod emit;
pub mod lower;
pub mod model;
pub mod ty;
pub mod ware;
pub mod bare;
pub mod facts;
pub mod library;
pub mod evaluate;
pub mod kinds;
pub mod lint;

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

/// The `fails` roles of the functions each `compile` calls, by the site and the case's Rust name: what a failure while
/// building is reported with (Y § 3.17).
fn failure_roles(tree: &y_parse::Node, sites: &[evaluate::Site]) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for s in sites {
        let callee = s.call.split('(').next().unwrap_or("").trim();
        for (_, d) in model::declarations(tree) {
            if d.kind != "function" || d.str("name") != Some(callee) {
                continue;
            }
            for en in d.node("doc").map(|doc| doc.nodes("entries")).unwrap_or_default() {
                if let Some(case) = en.str("name").and_then(|n| n.strip_prefix("fails ")).and_then(|c| c.split_once('.')).map(|c| c.1) {
                    out.insert(format!("{}#{}", s.id, ty::pascal(case)), en.str("text").unwrap_or("").to_string());
                }
            }
        }
    }
    out
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
    // the lints (Y § 3.15): an error stops the build, a warning is reported with it
    let mut lint_errors = vec![];
    let mut warnings = vec![];
    // the words a package allows beyond Y's own: its glossary, and the unit symbols it declares
    let glossary = match std::fs::read_to_string(dir.join("y/glossary")) {
        Ok(text) => lint::Words::read_glossary(&text)?,
        Err(_) => vec![],
    };
    let mut units = vec![];
    for t in [&tree, &test_tree, &main_tree] {
        for u in t.find("unit") {
            units.extend(u.str("symbol").map(str::to_string));
        }
    }
    let words = lint::Words { glossary, units };
    for (f, t) in [(&module_y, &tree), (&format!("{name}.test.y"), &test_tree), (&"main.y".to_string(), &main_tree)] {
        let (e, w) = lint::lint(&format!("y/src/{f}"), &read(&src.join(f))?, t, f == &module_y, &words);
        lint_errors.extend(e);
        warnings.extend(w);
    }
    if !lint_errors.is_empty() {
        return Err(lint_errors.join("\n"));
    }
    let warned = |report: String| if warnings.is_empty() { report } else { format!("{}\n{report}", warnings.iter().map(|w| format!("warning: {w}")).collect::<Vec<_>>().join("\n")) };
    let mut m = model::Module::new(&tree);
    // the names `main` imports: `from warehouse import pricing.line, …`
    for d in main_tree.nodes("decls").into_iter().filter(|d| d.kind == "import") {
        if let Some(y_parse::Value::List(ns)) = d.get("names") {
            for n in ns {
                if let y_parse::Value::Str(n) = n {
                    m.exported.insert(n.rsplit('.').next().unwrap().to_string());
                }
            }
        }
    }
    let tm = model::Module::new_in(&test_tree, Some(&m));
    let mm = model::Module::new(&main_tree);
    let (pkg, version) = package(dir)?;
    let lib_crate = pkg.replace('-', "_");
    lower::ERRORS.with(|es| es.borrow_mut().clear());
    lower::SITES.with(|s| s.borrow_mut().clear());
    lower::COMPILED.with(|c| *c.borrow_mut() = None);
    let mut e = emit::emit(&m, &test_tree, &tm, &mm, &lib_crate);
    let errors = lower::ERRORS.with(|es| std::mem::take(&mut *es.borrow_mut()));
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    // each `compile`, evaluated by the build step, and the package lowered again with the values (Y § 3.17)
    let sites = lower::SITES.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if !sites.is_empty() {
        let module_sites: Vec<_> = sites.iter().filter(|s| s.id.starts_with(&format!("y/src/{name}.y:"))).cloned().collect();
        let main_sites: Vec<_> = sites.iter().filter(|s| s.id.starts_with("y/src/main.y:")).cloned().collect();
        let mut module_text = e.module.clone();
        if !module_sites.is_empty() {
            module_text = format!("{module_text}\n{}\n", evaluate::evaluator(&m, &module_sites));
        }
        let files = vec![
            (PathBuf::from("src/lib.rs"), e.lib.clone()),
            (PathBuf::from(format!("src/{name}.rs")), module_text),
            (PathBuf::from(format!("src/{name}/tests.rs")), e.tests.clone()),
            (PathBuf::from("src/main.rs"), e.main.clone()),
        ];
        let roles = failure_roles(&tree, &sites);
        let values = evaluate::run(dir, &files, &evaluate::evaluator(&m, &main_sites), !module_sites.is_empty(), &lib_crate, &name, &sites, &roles)?;
        lower::COMPILED.with(|c| *c.borrow_mut() = Some(values));
        e = emit::emit(&m, &test_tree, &tm, &mm, &lib_crate);
        lower::COMPILED.with(|c| *c.borrow_mut() = None);
        lower::SITES.with(|s| s.borrow_mut().clear());
        let errors = lower::ERRORS.with(|es| std::mem::take(&mut *es.borrow_mut()));
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
    }
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
    let Some(out) = &opts.out else { return Ok(warned(format!("y-compile: wrote the typed Rust of {name}"))) };
    let stripped = bare::strip(&format!("{pkg} {version}"), &files)?;
    let lowered = bare::lower(&m, &tree, &read(&src.join(&module_y))?, &stripped, &name, &e.invariant_sites, &e.regions, &e.main_regions)?;
    for (p, (brs, js)) in &lowered {
        write(&out.join(p.with_extension("brs")), brs)?;
        write(&out.join(p.with_extension("constraints.json")), &bare_rust::json::canonical(js))?;
    }
    let facts = facts::facts(dir, &m, &tree, &test_tree, &main_tree, &lowered, &lib_crate, &name, &e.copies)?;
    write(&out.join("y-facts.json"), &(serde_json::to_string_pretty(&facts).unwrap() + "\n"))?;
    Ok(warned(format!("y-compile: {name}: {} files lowered", lowered.len())))
}
