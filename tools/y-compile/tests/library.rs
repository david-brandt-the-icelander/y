//! The runtime operations the lowering's rules call exist in the runtime crates, as their public API says.
use y_compile::library::{library, LOWERED};

#[test]
fn the_crates_are_read() {
    let lib = library();
    assert!(lib.problems.is_empty(), "{:?}", lib.problems);
    for t in ["y_collections::List", "y_collections::Set", "y_string::YString", "y_runtime::Console", "y_runtime::decimal::Decimal"] {
        assert!(lib.methods.contains_key(t), "no methods of {t}: {:?}", lib.methods.keys().collect::<Vec<_>>());
    }
}

#[test]
fn every_lowered_operation_exists() {
    let lib = library();
    let missing: Vec<String> = LOWERED.iter().filter(|(_, _, p)| !lib.has(p)).map(|(r, m, p)| format!("{r}.{m} lowers to {p}, which no runtime crate has")).collect();
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
