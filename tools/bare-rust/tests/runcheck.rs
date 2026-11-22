//! Sampled runtime checks of the std library file against the real std (`runcheck`).
use bare_rust::runcheck::{run, RuntimeReport};
use serde_json::{json, Value};
use std::path::PathBuf;

fn std_file() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("libraries/std-1.97.0.constraints.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn target() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"))
}

fn go(file: &Value, tag: &str) -> RuntimeReport {
    let work = std::env::temp_dir().join(format!("bare-rust-runcheck-{tag}"));
    run(file, &work, &target().join(format!("runcheck-scratch-{tag}"))).unwrap()
}

fn f(name: &str, params: &[&str], clauses: Vec<(&str, &str)>) -> Value {
    json!({
        "kind": "function", "name": name,
        "params": params.iter().map(|p| json!({"name": p})).collect::<Vec<_>>(),
        "clauses": clauses.into_iter().map(|(k, t)| json!({"kind": k, "text": t, "source": "test"})).collect::<Vec<_>>(),
    })
}

fn lib(subjects: Vec<Value>) -> Value {
    json!({ "subjects": subjects })
}

/// Right clauses are checked, and the same clauses made wrong are contradicted, kind by kind.
#[test]
fn right_clauses_are_confirmed_and_wrong_ones_contradicted() {
    let right = lib(vec![
        f("std::cmp::max", &["v1", "v2"], vec![("ensure", "ensure tie_second : (v1 <= v2 and v1 >= v2) implies result moved from v2")]),
        f("std::cmp::min", &["v1", "v2"], vec![("ensure", "ensure tie_first : (v1 <= v2 and v1 >= v2) implies result moved from v1")]),
        f("std::mem::replace", &["dest", "src"], vec![("hold", "hold dest : moved from src"), ("hold", "hold result : moved from dest"), ("hold", "lifetime dest, src survive call")]),
        f("std::mem::drop", &["v"], vec![("hold", "lifetime v ends in call"), ("result must be used", "result must be used")]),
        f("std::iter::Iterator::take", &["self", "n"], vec![("ensure", "ensure first_items : yields(result) == take(n, old(yields(self)))")]),
        f("std::option::Option::map_or_else", &["self", "default", "f"], vec![
            ("callback", "callback default : called exactly once when self is none, returning d"),
            ("callback", "callback f : called exactly once when exists self, with self, returning r"),
            ("ensure", "ensure from_callback : result == (if exists self then r else d)"),
        ]),
        f("std::iter::Iterator::count", &["self"], vec![("protocol", "protocol self.next+")]),
        f("std::assert_eq!", &["left", "right"], vec![("protocol", "protocol left right")]),
    ]);
    // `mem::drop` is not must-use: a control that the probe can fail
    let mut wrong = right.clone();
    let s = wrong["subjects"].as_array_mut().unwrap();
    s[0]["clauses"][0]["text"] = json!("ensure tie_second : (v1 <= v2 and v1 >= v2) implies result moved from v1");
    s[2]["clauses"][2]["text"] = json!("lifetime dest, src ends in call");
    s[3]["clauses"][0]["text"] = json!("lifetime v survive call");
    s[4]["clauses"][0]["text"] = json!("ensure first_items : yields(result) == skip(n, old(yields(self)))");
    s[5]["clauses"][1]["text"] = json!("callback f : called exactly once when self is none, with self, returning r");
    s[6]["clauses"][0]["text"] = json!("protocol self.next");
    s[7]["clauses"][0]["text"] = json!("protocol right left");

    let ok = go(&right, "right");
    let mut contradicted: Vec<String> = ok.kinds.iter().flat_map(|(k, s)| s.contradictions.iter().map(move |c| format!("{k}: {c}"))).collect();
    // only the control (drop is not must-use) is contradicted
    assert_eq!(contradicted.len(), 1, "{contradicted:#?}");
    assert!(contradicted[0].starts_with("must be used: std::mem::drop"), "{contradicted:?}");
    let want: usize = 2 + 3 + 1 + 1 + 3 + 1 + 1;
    assert!(ok.checked() >= want, "{} checked, {:?}", ok.checked(), ok.kinds);

    let bad = go(&wrong, "wrong");
    contradicted = bad.kinds.iter().filter(|(_, s)| !s.contradictions.is_empty()).map(|(k, _)| k.clone()).collect();
    assert_eq!(contradicted, ["callback", "lifetime", "moved from", "must be used", "protocol", "yields"], "{:#?}", bad.kinds);
    assert!(bad.kinds["protocol"].contradictions.len() == 2 && bad.kinds["lifetime"].contradictions.len() == 2, "{:#?}", bad.kinds);
}

/// What a clause form cannot be driven for is unchecked, never failed.
#[test]
fn what_cannot_be_driven_is_unchecked() {
    let l = lib(vec![
        f("demo::unknown", &["x"], vec![("ensure", "ensure e : yields(result) == yields(x)"), ("result must be used", "result must be used")]),
        f("std::iter::Iterator::take", &["self", "n"], vec![("ensure", "ensure e : yields(result) == flatten(old(yields(self)))")]),
    ]);
    let r = go(&l, "unchecked");
    assert_eq!(r.contradictions(), 0, "{:#?}", r.kinds);
    assert_eq!(r.checked(), 0);
    assert_eq!(r.kinds["yields"].unchecked.len(), 2);
    assert_eq!(r.kinds["must be used"].unchecked.len(), 1);
}

/// The real file: no clause is contradicted by the real std.
#[test]
fn the_std_file_agrees_with_the_real_std() {
    let rep = go(&std_file(), "std");
    rep.print();
    assert_eq!(rep.contradictions(), 0, "{:#?}", rep.kinds);
    for k in ["callback", "protocol", "moved from", "lifetime", "must be used", "yields"] {
        assert!(rep.kinds[k].checked > 0, "nothing of kind {k} was checked: {:#?}", rep.kinds[k]);
    }
}
