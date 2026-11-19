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

const RULE: &str = "------------------------------------------------------------------------------------------------------------------------";

/// A function of the module, documented as the parser requires.
fn function(doc: &str, text: &str) -> String {
    format!("{RULE}\n{doc}\n{RULE}\n{text}")
}

#[test]
fn quantities_of_different_units_added() {
    let units = "-- A metre.\nunit metre symbol m is uint32\n\n\n-- A second.\nunit second symbol s is uint32\n\n\n";
    let f = function("Adds.\n\na: A length, as metre.\nb: A time, as second.\n\nc: Their sum, as metre.", "function f(a, b) returns c:\n    return c = a + b\n");
    refused("units", &format!("{units}{f}"), &["quantities of different units do not mix"]);
}

#[test]
fn an_enqueue_outside_an_atomic_region() {
    let cmd = "-- Pings.\ncommand ping:\n    id is uint32\n\n    policy:\n        deduplicate_by: id\n        idempotent: true\n        retry: backoff(first: 1ms, factor: 1, attempts: 1)\n        deadline: 1s\n        on_abandon: park\n\n\n";
    let f = function("Sends.\n\nid: The id, as uint32.\n\noutbox: Where pings wait, as enqueue<command_type=ping>.", "function f(id, injected outbox):\n    enqueue ping(id: id)\n");
    refused("enqueue", &format!("{cmd}{f}"), &["is outside an atomic region"]);
}

#[test]
fn a_policy_without_a_deadline() {
    let cmd = "-- Pings.\ncommand ping:\n    id is uint32\n\n    policy:\n        deduplicate_by: id\n        idempotent: true\n        retry: backoff(first: 1ms, factor: 1, attempts: 1)\n        on_abandon: park\n";
    refused("policy", cmd, &["policy has no `deadline`"]);
}

#[test]
fn a_workflow_that_holds_a_clock() {
    let f = function("Waits.\n\nclock: The time, as clock.\n\nn: A number, as uint32.", "workflow f(injected clock) returns n:\n    return n = 1\n");
    refused("workflow", &f, &["workflow `f` holds `clock`"]);
}

#[test]
fn a_lambda_that_reads_what_it_does_not_capture() {
    let f = function("Adds.\n\na: A number, as uint32.\n\nb: The sum, as uint32.", "function f(a) returns b:\n    -------\n    Adds a.\n\n    x: A number, as uint32.\n\n    y: The sum, as uint32.\n    -------\n    add = lambda add(x) returns y:\n        return y = x + a\n    return b = add(1)\n");
    refused("capture", &f, &["reads `a`, which it does not capture"]);
}

#[test]
fn an_implementation_missing_a_method() {
    let tr = "-- Measured.\ntrait measured:\n    -------\n    Its area.\n\n    size: The area, as uint32.\n    -------\n    function area() returns size\n\n\n-- A square.\nstructure square:\n    side is uint32\n\n\n";
    refused("implementation", &format!("{tr}-- A square is measured.\nimplementation measured for square:\n    -------\n    Grows.\n    -------\n    function other():\n        return\n"), &["`other` is not a method of `measured`", "does not implement `area`"]);
}

#[test]
fn a_becomes_no_transition_declares() {
    let st = "-- A door.\nstate door:\n    id is uint32\n\n    case open\n    case shut\n\n\n";
    let f = function("Shuts.\n\nd: The door, as door.open.\n\ns: It, shut, as door.shut.", "function f(d) returns s:\n    return s = d.becomes(door.shut())\n");
    refused("becomes", &format!("{st}{f}"), &["is in no transition that returns"]);
}

/// A function `f` of `n` with the body given, each line indented once.
fn function_of(body: &str) -> String {
    let body: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("-------\nDoes it.\n\nn: A number, as uint32.\n\nm: Another, as uint32.\n-------\nfunction f(n) returns m:\n{}\n", body.join("\n"))
}

/// The report of a module that compiles.
fn compiled(name: &str, module: &str) -> String {
    let dir = package(name, module);
    let out = std::env::temp_dir().join("y-compile-errors").join(format!("{name}-rust"));
    let opts = y_compile::Options { out: None, rust: Some(out) };
    y_compile::compile(&dir, &opts).unwrap_or_else(|e| panic!("{name}: does not compile: {e}"))
}

#[test]
fn a_lint_is_an_error_until_an_allow_suppresses_it() {
    refused("lint_parentheses", &function_of("return m = (n)"), &["y/src/m.y:", "redundant_parentheses (Y2002)", "a name"]);
    // by its code or its name, as a statement over its block, or as a comment over its line
    compiled("lint_allowed_by_code", &function_of("allow(Y2002) because \"shown\"\nreturn m = (n)"));
    compiled("lint_allowed_by_name", &function_of("allow(redundant_parentheses) because \"shown\"\nreturn m = (n)"));
    compiled("lint_allowed_inline", &function_of("return m = (n)  -- lint: allow redundant_parentheses because \"shown\""));
    compiled("lint_allowed_above", &function_of("-- lint: allow redundant_parentheses because \"shown\"\nreturn m = (n)"));
}

#[test]
fn an_allow_of_no_lint_and_one_that_suppresses_nothing() {
    refused("lint_unknown", &function_of("allow(Y9999) because \"no such lint\"\nreturn m = n"), &["`allow(Y9999)`: no lint has the code or name `Y9999`"]);
    let report = compiled("lint_unused", &function_of("allow(repeated_call) because \"nothing repeats\"\nreturn m = n"));
    assert!(report.contains("warning: y/src/m.y:") && report.contains("`allow(Y2006)` suppresses nothing"), "{report}");
}

#[test]
fn shadowing_cannot_be_suppressed() {
    refused("lint_shadowing", &function_of("k = n\nk = n + 1\nreturn m = k"), &["shadowing (Y2007): `k` is already bound here"]);
    refused("lint_shadowing_parameter", &function_of("n = 3\nreturn m = n"), &["shadowing (Y2007): `n`"]);
    refused("lint_shadowing_allowed", &function_of("allow(shadowing) because \"tried\"\nk = n\nk = n + 1\nreturn m = k"), &["`shadowing` (Y2007) cannot be suppressed"]);
}

#[test]
fn a_repeated_call_a_todo_and_a_long_function() {
    let repeated = "conditional sized:\n    condition sized.small = (n.min(9) < 2):\n        return m = 1\n    condition sized.large = (n.min(9) > 5):\n        return m = 2\n    else sized.middling:\n        return m = 3";
    refused("lint_repeated", &function_of(repeated), &["repeated_call (Y2006): `n.min(9)` is called again, as on line"]);
    refused("lint_todo", &function_of("-- TODO: something better\nreturn m = n"), &["todo_comment (Y2005)"]);
    // a body of 61 to 120 lines is a warning; of more, an error, which an `allow` in the body suppresses
    let lines = |k: usize| (0..k).map(|i| format!("v{i} = n + {i}")).collect::<Vec<_>>().join("\n") + "\nreturn m = n";
    let report = compiled("lint_long", &function_of(&lines(70)));
    assert!(report.contains("function_length (Y2003): `f`'s body has 71 lines, more than 60"), "{report}");
    refused("lint_too_long", &function_of(&lines(130)), &["function_length (Y2003): `f`'s body has 131 lines, more than 120"]);
    compiled("lint_too_long_allowed", &function_of(&format!("allow(function_length) because \"generated\"\n{}", lines(130))));
}

#[test]
fn a_type_argument_named_against_the_convention() {
    refused("lint_type_argument", "-- A box.\nstructure box:\n    items is list<element=uint32>\n", &["type_argument_name (Y2001): `element` takes a type"]);
}
