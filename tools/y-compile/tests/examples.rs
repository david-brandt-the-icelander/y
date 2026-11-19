//! Each example, end to end: `y-compile` lowers its Y source to bare files, `bare-rust weave` (with cargo) makes Rust
//! of them, the Rust builds and passes the example's lowered tests, and `tools/paths/trace.py` finds that each test
//! takes the path it claims.
//!
//! The Y test programs of `tests/programs` go through the same chain: each proves constructs no example uses.
//!
//! The work goes in `examples/.y-compile/<example>`, beside the examples, so that the relative paths of the woven
//! crate's `Cargo.toml` (`../y-runtime`) resolve.
use std::path::PathBuf;
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{what} failed:\n{text}");
    text
}

fn end_to_end(name: &str, module: &str) {
    end_to_end_in(&repo().join("examples").join(name), name, module);
}

/// A Y test program of `tests/programs`: a module, its tests and its `main`, laid out as an example is.
fn program(name: &str) {
    let dir = repo().join("tools/y-compile/tests/programs").join(name);
    end_to_end_in(&dir, &format!("program-{name}"), &name.replace('-', "_"));
}

fn end_to_end_in(example: &std::path::Path, name: &str, module: &str) {
    let repo = repo();
    let example = example.to_path_buf();
    let work = repo.join("examples/.y-compile").join(name);
    let _ = std::fs::remove_dir_all(&work);
    let bare = work.join("bare");
    let opts = y_compile::Options { out: Some(bare.clone()), rust: None };
    y_compile::compile(&example, &opts).unwrap_or_else(|e| panic!("{name}: y-compile: {e}"));
    // the woven crate sits beside the examples, so its `../y-runtime` resolves
    let woven = repo.join("examples").join(format!(".y-compile-{name}"));
    let _ = std::fs::remove_dir_all(&woven);
    let target = repo.join("examples/.y-compile/target");
    let o = bare_rust::crate_weave::WeaveOpts { cargo: true, crate_dir: Some(example.clone()), target: target.clone(), drop_holes: false, keep: Default::default() };
    let cw = bare_rust::crate_weave::weave_crate_with(&bare, &woven, &o).unwrap_or_else(|e| panic!("{name}: weave: {e}"));
    if !cw.ok() {
        cw.print();
        for (f, e) in &cw.weave_errors {
            eprintln!("{}: {e}", f.display());
        }
        panic!("{name}: weave failed");
    }
    run(Command::new("cargo").args(["test", "-q"]).env("CARGO_TARGET_DIR", &target).current_dir(&woven), &format!("{name}: cargo test"));
    let trace = run(
        Command::new("python3").arg(repo.join("tools/paths/trace.py")).arg(&woven).arg(module).env("YTRACE_WORK", work.join("trace")).current_dir(&repo),
        &format!("{name}: trace.py"),
    );
    println!("{name}: {}", trace.lines().last().unwrap_or(""));
    let _ = std::fs::remove_dir_all(&woven);
}

#[test]
fn nim() {
    end_to_end("nim", "nim");
}

#[test]
fn tic_tac_toe() {
    end_to_end("tic-tac-toe", "tic_tac_toe");
}

#[test]
fn hangman() {
    end_to_end("hangman", "hangman");
}

#[test]
fn snake() {
    end_to_end("snake", "snake");
}

#[test]
fn memory() {
    end_to_end("memory", "memory");
}

#[test]
fn checkers() {
    end_to_end("checkers", "checkers");
}

#[test]
fn warehouse() {
    end_to_end("warehouse", "warehouse");
}

#[test]
fn program_narrowing() {
    program("narrowing");
}

#[test]
fn program_regions() {
    program("regions");
}

#[test]
fn program_decimals() {
    program("decimals");
}

#[test]
fn program_compile_time() {
    program("compile_time");
}

#[test]
fn program_ensures() {
    program("ensures");
}

#[test]
fn program_library() {
    program("library");
}

#[test]
fn program_orders() {
    program("orders");
}

#[test]
fn program_shapes() {
    program("shapes");
}

#[test]
fn program_generics() {
    program("generics");
}

#[test]
fn program_claims() {
    program("claims");
}
