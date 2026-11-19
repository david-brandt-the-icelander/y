//! Y sources that do not compile, each with the error y-compile gives: what the language rejects is a compile error
//! that names it, never Rust that fails to build.
use std::path::PathBuf;

/// A package of one module, `m`, its tests (none) and a `main` that does nothing, written to a folder of its own.
fn package(name: &str, module: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("y-compile-errors").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("y/src")).unwrap();
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").canonicalize().unwrap();
    let e = examples.display();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"m-{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\ny-runtime = {{ path = \"{e}/y-runtime\" }}\ny-string = {{ path = \"{e}/y-string\" }}\ny-collections = {{ path = \"{e}/y-collections\" }}\n\n[workspace]\n"),
    )
    .unwrap();
    std::fs::write(dir.join("y/src/m.y"), format!("module m\n\n\n{module}")).unwrap();
    std::fs::write(dir.join("y/src/m.test.y"), "module m\n").unwrap();
    std::fs::write(dir.join("y/src/main.y"), "module main\n\n\n-- Does nothing.\nfunction main():\n    return\n").unwrap();
    dir
}

/// The error a module gives, which must contain each of `wanted`.
fn refused(name: &str, module: &str, wanted: &[&str]) {
    let dir = package(name, module);
    let out = std::env::temp_dir().join("y-compile-errors").join(format!("{name}-rust"));
    let opts = y_compile::Options { out: None, rust: Some(out) };
    let e = y_compile::compile(&dir, &opts).expect_err(&format!("{name}: compiled, and should not have"));
    for w in wanted {
        assert!(e.contains(w), "{name}: the error does not say `{w}`:\n{e}");
    }
}

#[test]
fn a_method_a_list_does_not_have() {
    refused(
        "no_such_method",
        "-------\nCounts.\n\nitems: The items, as list<element_type=uint32>.\n\nn: How many, as uint32.\n-------\nfunction f(items) returns n:\n    return n = items.frobnicate()\n",
        &["y/src/m.y:", "`list` has no method `frobnicate`", "y_collections::List offers"],
    );
}

#[test]
fn a_method_a_structure_does_not_have() {
    refused(
        "no_such_structure_method",
        "-- A point.\nstructure point:\n    x is uint32\n\n\n-------\nReads.\n\np: The point, as point.\n\nn: A number, as uint32.\n-------\nfunction f(p) returns n:\n    return n = p.length()\n",
        &["the type `point` has no method `length`"],
    );
}

#[test]
fn a_pattern_outside_the_subset() {
    refused(
        "pattern",
        "-- A code.\ntype code is string where shaped = value.matches(pattern\"^[a-z]{3}$\")\n",
        &["pattern\"^[a-z]{3}$\"", "the subset does not have"],
    );
}

#[test]
fn a_decimal_whose_scale_would_fall() {
    refused(
        "scale",
        "-------\nRounds.\n\nprice: A price, as decimal<precision=6, scale=3>.\n\ncents: The price, as decimal<precision=6, scale=2>.\n-------\nfunction f(price) returns cents:\n    return cents = price\n",
        &["a decimal of scale 3 where one of scale 2 is wanted", "to_decimal_rounded"],
    );
}

#[test]
fn a_decimal_divided_with_a_slash() {
    refused(
        "slash",
        "-------\nDivides.\n\na: A number, as decimal<precision=6, scale=2>.\nb: Another, as decimal<precision=6, scale=2>.\n\nq: The quotient, as decimal<precision=6, scale=2>.\n-------\nfunction f(a, b) returns q:\n    return q = a / b\n",
        &["`/` is defined on rationals and `float64` only"],
    );
}

#[test]
fn an_uncancellable_region_without_a_bound() {
    refused(
        "uncancellable",
        "-------\nWaits.\n-------\nfunction f():\n    region held is uncancellable:\n        g()\n\n\n-------\nNothing.\n-------\nfunction g():\n    return\n",
        &["region `held`: `uncancellable` holds cancellation off for a bounded time, and the region is not `within`"],
    );
}

#[test]
fn a_region_within_a_bound_and_forever() {
    refused(
        "within_forever",
        "-------\nWaits.\n\nforever: The right to wait, as forever.\n-------\nfunction f(injected forever):\n    region both is within 1s, forever because \"never\":\n        g()\n\n\n-------\nNothing.\n-------\nfunction g():\n    return\n",
        &["is `within` a bound or waits `forever`, not both"],
    );
}

#[test]
fn a_compile_whose_call_fails() {
    refused(
        "compile_fails",
        "-- Why there is nothing.\nconstants nothing_error:\n    none = \"none\"\n\n\n-------\nFails.\n\nn: Never given, as uint32.\n\nfails nothing_error.none: There is never a number.\n-------\nfunction nothing() returns n fails nothing_error:\n    fail nothing_error.none\n\n\n-------\nReads a number made while building.\n\nn: The number, as uint32.\n-------\nfunction f() returns n:\n    value = compile nothing()\n    return n = value\n",
        &["`compile` of `nothing()` fails with None", "There is never a number", "a failure is a build error"],
    );
}

#[test]
fn a_compile_whose_call_faults() {
    refused(
        "compile_faults",
        "-------\nFaults.\n\nn: Never given, as uint32.\n-------\nfunction broken() returns n:\n    fail not_implemented\n\n\n-------\nReads a number made while building.\n\nn: The number, as uint32.\n-------\nfunction f() returns n:\n    value = compile broken()\n    return n = value\n",
        &["`compile` faulted while the program was built", "not implemented"],
    );
}

#[test]
fn a_compile_of_an_argument_not_known_while_building() {
    refused(
        "compile_argument",
        "-------\nDoubles.\n\nx: A number, as uint32.\n\nn: Twice it, as uint32.\n-------\nfunction twice(x) returns n:\n    return n = x + x\n\n\n-------\nDoubles at build time.\n\nx: A number, as uint32.\n\nn: Twice it, as uint32.\n-------\nfunction f(x) returns n:\n    value = compile twice(x)\n    return n = value\n",
        &["`x` is not known while the program is built"],
    );
}
