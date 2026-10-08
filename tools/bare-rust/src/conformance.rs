//! The conformance table for language 6.13 (callback `with` and `returning`), 6.14.7 (tuple parts) and 6.15
//! (sequence operators), generated from the reference evaluator and exported as JSON under `conformance/`, so that
//! another host's compilation of the operators can be run against the same cases (language 6.15, "Checked").
//!
//! A case is `{ "id", "<inputs>", "expect" }` in one of four suites; `eval_case` gives the reference output of a case,
//! and a host passes when its output for every case equals `expect`. The encoding of values is `seq::to_json`.
use crate::callback::{check_callback, check_called_by, follows, parse_callback, parse_called_by, parse_protocol, Call, Verdict};
use crate::seq::{eval_op, from_json, to_json, NAMED};
use crate::seqexpr::{holds, run_let, Env, EvalErr};
use serde_json::{json, Map, Value as Json};
use std::collections::HashMap;
use std::path::PathBuf;

pub const SUITES: [&str; 6] = ["ops", "expressions", "callbacks", "runs", "lets", "protocols"];
pub const FORMAT: &str = "y-conformance-0";

pub fn table_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("conformance/sequences.json")
}

fn env_of(j: &Json) -> Result<Env, String> {
    let side = |k: &str| -> Result<HashMap<String, crate::seq::Value>, String> {
        j[k].as_object().into_iter().flatten().map(|(n, v)| Ok((n.clone(), from_json(v)?))).collect()
    };
    Ok(Env::new(side("now")?, side("old")?))
}

fn err_json(e: &EvalErr) -> Json {
    match e {
        EvalErr::Undecided(_) => json!({ "undecided": true }),
        EvalErr::Build(_) => json!({ "refused": true }),
        EvalErr::Unbound(n) => json!({ "unbound": n }),
        EvalErr::Unsupported(s) => json!({ "unsupported": s }),
    }
}

/// The reference output of one case of a suite: `ops`, `expressions`, `callbacks` or `protocols`.
pub fn eval_case(suite: &str, case: &Json) -> Result<Json, String> {
    match suite {
        "ops" => eval_op(case["op"].as_str().ok_or("op")?, case["args"].as_array().ok_or("args")?),
        "expressions" => {
            let mut env = env_of(&case["env"])?;
            Ok(match holds(case["clause"].as_str().ok_or("clause")?, &mut env) {
                Ok(b) => json!({ "holds": b }),
                Err(e) => err_json(&e),
            })
        }
        "callbacks" | "runs" => {
            let text = case["clause"].as_str().ok_or("clause")?;
            let mut env = env_of(&case["env"])?;
            if let Some(f) = case["final"].as_object() {
                for (k, v) in f {
                    env.fin.insert(k.clone(), from_json(v)?);
                }
            }
            let before: Vec<String> = env.now.keys().cloned().collect();
            let calls_of = |a: &Json| -> Result<Vec<Call>, String> {
                a.as_array()
                    .into_iter()
                    .flatten()
                    .map(|k| {
                        Ok(Call {
                            args: k["args"].as_array().into_iter().flatten().map(from_json).collect::<Result<_, _>>()?,
                            result: k.get("result").map(from_json).transpose()?,
                        })
                    })
                    .collect()
            };
            let verdict = if suite == "runs" {
                let c = parse_called_by(text)?.ok_or("not a `called by` clause")?;
                let groups: Vec<Vec<Call>> = case["groups"].as_array().into_iter().flatten().map(calls_of).collect::<Result<_, _>>()?;
                check_called_by(&c, &mut env, &groups)
            } else {
                check_callback(&parse_callback(text)?, &mut env, &calls_of(&case["calls"])?)
            };
            let verdict = match verdict {
                Verdict::Ok => "ok",
                Verdict::Violation(_) => "violation",
                Verdict::Undecided(_) => "undecided",
            };
            let mut binds = Map::new();
            let mut names: Vec<&String> = env.now.keys().filter(|k| !before.contains(k)).collect();
            names.sort();
            for n in names {
                binds.insert(n.clone(), to_json(&env.now[n]));
            }
            let ensures: Vec<Json> = case["ensure"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| match holds(t.as_str().unwrap_or(""), &mut env.clone()) {
                    Ok(b) => json!(b),
                    Err(e) => err_json(&e),
                })
                .collect();
            Ok(json!({ "verdict": verdict, "binds": binds, "ensures": ensures }))
        }
        "lets" => {
            let mut env = env_of(&case["env"])?;
            let before: Vec<String> = env.now.keys().cloned().collect();
            Ok(match run_let(case["clause"].as_str().ok_or("clause")?, &mut env) {
                Ok(()) => {
                    let mut names: Vec<&String> = env.now.keys().filter(|k| !before.contains(k)).collect();
                    names.sort();
                    json!({ "binds": names.into_iter().map(|n| (n.clone(), to_json(&env.now[n]))).collect::<Map<_, _>>() })
                }
                Err(e) => err_json(&e),
            })
        }
        "protocols" => {
            let p = parse_protocol(case["protocol"].as_str().ok_or("protocol")?)?;
            let evs: Vec<String> = case["events"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect();
            Ok(json!({ "follows": follows(&p, &evs) }))
        }
        s => Err(format!("unknown suite `{s}`")),
    }
}

/// Run a host against the table: every case whose output differs from `expect`, as "suite id: got … expected …".
pub fn check_host(table: &Json, host: &dyn Fn(&str, &Json) -> Result<Json, String>) -> Vec<String> {
    let mut bad = vec![];
    for suite in SUITES {
        for case in table["suites"][suite].as_array().into_iter().flatten() {
            let id = case["id"].as_str().unwrap_or("?");
            match host(suite, case) {
                Ok(got) if got == case["expect"] => {}
                Ok(got) => bad.push(format!("{suite} {id}: got {got}, expected {}", case["expect"])),
                Err(e) => bad.push(format!("{suite} {id}: {e}")),
            }
        }
    }
    bad
}

fn ints(v: &[i64]) -> Json {
    json!(v)
}

fn counter(from: i64, step: i64) -> Json {
    json!({ "counter": { "from": from, "step": step } })
}

const BIG: i64 = 1 << 40;

fn op(id: String, op: &str, args: Vec<Json>) -> Json {
    let mut c = json!({ "id": id, "op": op, "args": args });
    c["expect"] = eval_op(op, c["args"].as_array().unwrap()).unwrap_or_else(|e| panic!("{e}"));
    c
}

fn lists() -> Vec<Vec<i64>> {
    vec![vec![], vec![7], vec![7, 8], vec![3, 1, 4, 1, 5]]
}

fn infinite() -> Vec<(String, Json)> {
    vec![("counter0+1".into(), counter(0, 1)), ("counter10-3".into(), counter(10, -3))]
}

/// Indices and counts around the ends of a list of `n` elements, and very large.
fn around(n: i64) -> Vec<i64> {
    let mut v = vec![-1, 0, n - 1, n, n + 1, BIG];
    v.retain(|x| *x >= -1);
    v.sort_unstable();
    v.dedup();
    v
}

fn ops_suite() -> Vec<Json> {
    let mut cs = vec![];
    let name = |l: &[i64]| format!("{l:?}").replace(' ', "");
    for l in lists() {
        let n = l.len() as i64;
        for i in around(n) {
            cs.push(op(format!("index {} {i}", name(&l)), "index", vec![ints(&l), json!(i)]));
            cs.push(op(format!("take {i} {}", name(&l)), "take", vec![json!(i), ints(&l)]));
            cs.push(op(format!("skip {i} {}", name(&l)), "skip", vec![json!(i), ints(&l)]));
        }
        // slices: a start from 0, an end anywhere (the text defines a slice with a start below 0 by the same equation, which
        // reads positions outside the list as elements: not exercised)
        for i in [0, 1, 2, n, n + 1] {
            for j in [Some(0), Some(1), Some(2), Some(n), Some(n + 1), Some(9), None] {
                cs.push(op(format!("slice {}[{i}..{}]", name(&l), j.map_or("".into(), |j| j.to_string())), "slice", vec![ints(&l), json!(i), json!(j)]));
            }
        }
        for j in [Some(0), Some(1), Some(n), Some(n + 1), None] {
            cs.push(op(format!("slice {}[..{}]", name(&l), j.map_or("".into(), |j| j.to_string())), "slice", vec![ints(&l), Json::Null, json!(j)]));
        }
        cs.push(op(format!("reverse {}", name(&l)), "reverse", vec![ints(&l)]));
        cs.push(op(format!("size {}", name(&l)), "size", vec![ints(&l)]));
        cs.push(op(format!("enumerate {}", name(&l)), "enumerate", vec![ints(&l)]));
        cs.push(op(format!("next {}", name(&l)), "next", vec![ints(&l)]));
        cs.push(op(format!("next_back {}", name(&l)), "next_back", vec![ints(&l)]));
        for (f, _) in NAMED {
            if matches!(*f, "deref" | "succ" | "double" | "negate" | "yields") {
                cs.push(op(format!("map {f} {}", name(&l)), "map", vec![json!(f), ints(&l)]));
            } else {
                cs.push(op(format!("filter {f} {}", name(&l)), "filter", vec![json!(f), ints(&l)]));
            }
        }
        cs.push(op(format!("filter is_even {}", name(&l)), "filter", vec![json!("is_even"), ints(&l)]));
        for m in lists() {
            cs.push(op(format!("concat {} {}", name(&l), name(&m)), "concat", vec![ints(&l), ints(&m)]));
            cs.push(op(format!("zip {} {}", name(&l), name(&m)), "zip", vec![ints(&l), ints(&m)]));
            cs.push(op(format!("eq {} {}", name(&l), name(&m)), "eq", vec![ints(&l), ints(&m)]));
        }
        // a list against a sequence that does not end
        cs.push(op(format!("concat {} ++ counter", name(&l)), "concat", vec![ints(&l), counter(0, 1)]));
        cs.push(op(format!("concat counter ++ {}", name(&l)), "concat", vec![counter(0, 1), ints(&l)]));
        cs.push(op(format!("zip {} counter", name(&l)), "zip", vec![ints(&l), counter(0, 1)]));
        cs.push(op(format!("zip counter {}", name(&l)), "zip", vec![counter(0, 1), ints(&l)]));
        cs.push(op(format!("eq {} counter", name(&l)), "eq", vec![ints(&l), counter(0, 1)]));
    }
    for (nm, c) in infinite() {
        for i in [-1, 0, 1, 5, BIG] {
            cs.push(op(format!("index {nm} {i}"), "index", vec![c.clone(), json!(i)]));
            cs.push(op(format!("take {i} {nm}"), "take", vec![json!(i), c.clone()]));
            cs.push(op(format!("skip {i} {nm}"), "skip", vec![json!(i), c.clone()]));
        }
        for (i, j) in [(0, 3), (2, 5), (5, 2), (BIG, BIG + 2)] {
            cs.push(op(format!("slice {nm}[{i}..{j}]"), "slice", vec![c.clone(), json!(i), json!(j)]));
        }
        cs.push(op(format!("slice {nm}[2..]"), "slice", vec![c.clone(), json!(2), Json::Null]));
        cs.push(op(format!("slice {nm}[..3]"), "slice", vec![c.clone(), Json::Null, json!(3)]));
        cs.push(op(format!("reverse {nm}"), "reverse", vec![c.clone()]));
        cs.push(op(format!("size {nm}"), "size", vec![c.clone()]));
        cs.push(op(format!("enumerate {nm}"), "enumerate", vec![c.clone()]));
        cs.push(op(format!("next {nm}"), "next", vec![c.clone()]));
        cs.push(op(format!("next_back {nm}"), "next_back", vec![c.clone()]));
        cs.push(op(format!("map succ {nm}"), "map", vec![json!("succ"), c.clone()]));
        cs.push(op(format!("filter is_even {nm}"), "filter", vec![json!("is_even"), c.clone()]));
        cs.push(op(format!("zip {nm} {nm}"), "zip", vec![c.clone(), c.clone()]));
        cs.push(op(format!("concat {nm} ++ [1]"), "concat", vec![c.clone(), ints(&[1])]));
        cs.push(op(format!("eq {nm} {nm}"), "eq", vec![c.clone(), c.clone()]));
        cs.push(op(format!("eq {nm} counter0+2"), "eq", vec![c.clone(), counter(0, 2)]));
    }
    for s in ["", "a", "ab", "h\u{e9}\u{e9}\u{1f600}"] {
        cs.push(op(format!("scalars {s:?}"), "scalars", vec![json!(s)]));
    }
    cs.push(op("deref 3".into(), "deref", vec![json!(3)]));
    // present, select, while_true, flatten
    let t = json!(true);
    let f = json!(false);
    for l in lists() {
        let n = l.len() as i64;
        for i in around(n) {
            cs.push(op(format!("present {} {i}", name(&l)), "present", vec![ints(&l), json!(i)]));
        }
    }
    cs.push(op("present counter -1".into(), "present", vec![counter(0, 1), json!(-1)]));
    cs.push(op("present counter big".into(), "present", vec![counter(0, 1), json!(BIG)]));
    let bss = [json!([]), json!([true]), json!([false]), json!([true, false, true]), json!([true, true]), json!([true, true, true, true, true, true])];
    for bs in &bss {
        for l in lists() {
            cs.push(op(format!("select {bs} {}", name(&l)), "select", vec![bs.clone(), ints(&l)]));
            cs.push(op(format!("while_true {bs} {}", name(&l)), "while_true", vec![bs.clone(), ints(&l)]));
        }
    }
    cs.push(op("select counter bools over counter".into(), "select", vec![json!([true, false, true]), counter(0, 1)]));
    cs.push(op("while_true [t,t,f] over counter".into(), "while_true", vec![json!([t, t, f]), counter(0, 1)]));
    cs.push(op("while_true all true over counter".into(), "while_true", vec![json!([t, t]), counter(0, 1)]));
    for xss in [json!([]), json!([[]]), json!([[1], [2, 3]]), json!([[], [4], []]), json!([[1, 2], []])] {
        cs.push(op(format!("flatten {xss}"), "flatten", vec![xss]));
    }
    cs.push(op("flatten of an infinite list of lists".into(), "flatten", vec![counter(0, 1)]));
    cs.push(op("map yields over lists".into(), "map", vec![json!("yields"), json!([[1], [2, 3]])]));
    // tuple parts (6.14.7)
    for (t, k) in [(json!({"tuple": [1, 2]}), 0), (json!({"tuple": [1, 2]}), 1), (json!({"tuple": [1, 2]}), 2), (json!({"tuple": [1, 2, 3]}), 2), (json!({"tuple": [1, 2, 3]}), 3)] {
        cs.push(op(format!("part {t} .{k}"), "part", vec![t, json!(k)]));
    }
    for (pat, t) in [
        (json!(["a", "b"]), json!({"tuple": [1, 2]})),
        (json!(["_", "b"]), json!({"tuple": [1, 2]})),
        (json!(["a", "_"]), json!({"tuple": [1, 2]})),
        (json!(["a", "b", "c"]), json!({"tuple": [1, 2]})),
        (json!(["a"]), json!({"tuple": [1, 2]})),
        (json!(["h", "t"]), json!({"tuple": [[1], [2, 3]]})),
    ] {
        cs.push(op(format!("bind {pat} {t}"), "bind", vec![pat, t]));
    }
    cs
}

fn val(pairs: &[(&str, Json)]) -> Json {
    Json::Object(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

fn expr(id: &str, clause: &str, now: &[(&str, Json)], old: &[(&str, Json)]) -> Json {
    let mut c = json!({ "id": id, "clause": clause, "env": { "now": val(now), "old": val(old) } });
    c["expect"] = eval_case("expressions", &c).unwrap_or_else(|e| panic!("{id}: {e}"));
    c
}

fn expressions_suite() -> Vec<Json> {
    let tup = |a: Json, b: Json| json!({ "tuple": [a, b] });
    vec![
        // 6.14.7, the example of the text
        expr("split_at_mut sizes", "size(result.0) == mid and size(result.1) == old(size(self)) - mid", &[("result", tup(json!([1, 2]), json!([3]))), ("mid", json!(2))], &[("self", json!([1, 2, 3]))]),
        expr("split_at_mut sizes, wrong", "size(result.0) == mid and size(result.1) == old(size(self)) - mid", &[("result", tup(json!([1]), json!([2, 3]))), ("mid", json!(2))], &[("self", json!([1, 2, 3]))]),
        expr("split_at_mut pattern", "result is (head, tail) implies size(head) + size(tail) == old(size(self))", &[("result", tup(json!([1, 2]), json!([3])))], &[("self", json!([1, 2, 3]))]),
        expr("split_at_mut part elements", "result.0 == old(self)[..mid] and result.1 == old(self)[mid..]", &[("result", tup(json!([1, 2]), json!([3]))), ("mid", json!(2))], &[("self", json!([1, 2, 3]))]),
        expr("split_at_mut part elements, mid past the end", "result.0 == old(self)[..mid] and result.1 == old(self)[mid..]", &[("result", tup(json!([1, 2, 3]), json!([]))), ("mid", json!(9))], &[("self", json!([1, 2, 3]))]),
        expr("component the tuple lacks", "result.2 == 1", &[("result", tup(json!(1), json!(2)))], &[]),
        expr("pattern of the wrong length", "result is (a, b, c)", &[("result", tup(json!(1), json!(2)))], &[]),
        // 6.15, the examples of the text
        expr("chars", "yields(result) == scalars(self)", &[("result", json!(["a", "b"])), ("self", json!("ab"))], &[]),
        expr("next head", "result == old(yields(self))[0]", &[("result", json!({"some": 1}))], &[("self", json!([1, 2]))]),
        expr("next head, end", "result == old(yields(self))[0]", &[("result", Json::Null)], &[("self", json!([]))]),
        expr("next advances", "exists result implies yields(self) == old(yields(self))[1..]", &[("result", json!({"some": 1})), ("self", json!([2]))], &[("self", json!([1, 2]))]),
        expr("next advances, end", "exists result implies yields(self) == old(yields(self))[1..]", &[("result", Json::Null), ("self", json!([]))], &[("self", json!([]))]),
        expr("chain", "yields(result) == old(yields(self)) ++ old(yields('<U as std::iter::IntoIterator>::into_iter'(other)))", &[("result", json!([1, 2, 3]))], &[("self", json!([1])), ("other", json!([2, 3]))]),
        expr("take", "yields(result) == take(n, old(yields(self)))", &[("result", json!([1, 2])), ("n", json!(2))], &[("self", json!([1, 2, 3]))]),
        expr("take past the end", "yields(result) == take(n, old(yields(self)))", &[("result", json!([1, 2, 3])), ("n", json!(99))], &[("self", json!([1, 2, 3]))]),
        expr("copied", "yields(result) == map(deref, old(yields(self)))", &[("result", json!([1, 2]))], &[("self", json!([1, 2]))]),
        expr("counts of a counter", "take(3, yields(result)) == [0, 1, 2]", &[("result", counter(0, 1))], &[]),
        expr("equality of infinite sequences is undecided", "yields(result) == yields(result)", &[("result", counter(0, 1))], &[]),
        expr("size of an infinite sequence has no value", "size(result) == 3", &[("result", counter(0, 1))], &[]),
        expr("zip as many as the shorter", "yields(result) == zip(old(yields(self)), old(yields(other)))", &[("result", json!([{"tuple": [1, 4]}]))], &[("self", json!([1, 2])), ("other", json!([4]))]),
        expr("enumerate", "yields(result) == enumerate(old(yields(self)))", &[("result", json!([{"tuple": [0, 5]}, {"tuple": [1, 6]}]))], &[("self", json!([5, 6]))]),
        expr("skip", "yields(result) == skip(n, old(yields(self)))", &[("result", json!([3])), ("n", json!(2))], &[("self", json!([1, 2, 3]))]),
        expr("slice never fails", "size(self[2..9]) == 1", &[("self", json!([1, 2, 3]))], &[]),
        expr("index never fails", "self[3] == none and self[-1] == none", &[("self", json!([1, 2, 3]))], &[]),
        expr("++ is associative", "(xs ++ ys) ++ zs == xs ++ (ys ++ zs)", &[("xs", json!([1])), ("ys", json!([2])), ("zs", json!([3]))], &[]),
        expr("present tells an absent position from a none element", "present(xs, 1) and not present(xs, 2) and xs[1] == none", &[("xs", json!([1, null]))], &[]),
        expr("select by verdicts", "yields(result) == select(keep, old(yields(self)))", &[("result", json!([3])), ("keep", json!([false, false, true]))], &[("self", json!([1, 2, 3]))]),
        expr("while_true stops at the first false", "yields(result) == while_true(keep, old(yields(self)))", &[("result", json!([1, 2])), ("keep", json!([true, true, false]))], &[("self", json!([1, 2, 3, 4]))]),
        expr("flat_map", "yields(result) == flatten(map(yields, y))", &[("result", json!([1, 11, 2, 12])), ("y", json!([[1, 11], [2, 12]]))], &[]),
        expr("take_while not put back", "final(yields(self)) == skip(size(keep), old(yields(self)))", &[("keep", json!([true, true, false]))], &[("self", json!([1, 2, 3, 4]))]),
        expr("final without a run value is the current one", "final(size(xs)) == 2", &[("xs", json!([1, 2]))], &[]),
        expr("reverse", "reverse(xs)[0] == xs[size(xs) - 1]", &[("xs", json!([1, 2, 3]))], &[]),
        // 6.14.3, moves: the key is what `==` sees, the second number is the identity
        expr("max tie takes the second", "(v1 <= v2 and v1 >= v2) implies result moved from v2", &[("v1", json!({"ent": [1, 10]})), ("v2", json!({"ent": [1, 20]})), ("result", json!({"ent": [1, 20]}))], &[]),
        expr("max tie takes the first, wrong", "(v1 <= v2 and v1 >= v2) implies result moved from v2", &[("v1", json!({"ent": [1, 10]})), ("v2", json!({"ent": [1, 20]})), ("result", json!({"ent": [1, 10]}))], &[]),
        expr("max, no tie", "(v1 <= v2 and v1 >= v2) implies result moved from v2", &[("v1", json!({"ent": [1, 10]})), ("v2", json!({"ent": [2, 20]})), ("result", json!({"ent": [2, 20]}))], &[]),
        expr("replace moves", "dest moved from src", &[("dest", json!({"ent": [5, 20]}))], &[("src", json!({"ent": [5, 20]})), ("dest", json!({"ent": [5, 10]}))]),
        expr("replace moves, a copy is not", "dest moved from src", &[("dest", json!({"ent": [5, 30]}))], &[("src", json!({"ent": [5, 20]}))]),
    ]
}

fn cb(id: &str, clause: &str, now: &[(&str, Json)], calls: Vec<Json>, ensure: &[&str]) -> Json {
    let mut c = json!({ "id": id, "clause": clause, "env": { "now": val(now), "old": {} }, "calls": calls, "ensure": ensure });
    c["expect"] = eval_case("callbacks", &c).unwrap_or_else(|e| panic!("{id}: {e}"));
    c
}

fn call(args: Vec<Json>, result: Json) -> Json {
    json!({ "args": args, "result": result })
}

fn callbacks_suite() -> Vec<Json> {
    let f = "callback f : called exactly once when exists self, with self, returning r";
    let d = "callback default : called exactly once when self is none, returning d";
    let post = "result == (if exists self then r else d)";
    let occ = json!({ "variant": "Occupied", "fields": [{ "id": 1 }] });
    let vac = json!({ "variant": "Vacant", "fields": [] });
    let and_modify = "callback f : called exactly once when self is Occupied(e), with v";
    vec![
        cb("map_or_else some: f called with the value, result is r", f, &[("self", json!({"some": 4})), ("result", json!(9))], vec![call(vec![json!(4)], json!(9))], &[post]),
        cb("map_or_else some: result is not r", f, &[("self", json!({"some": 4})), ("result", json!(8))], vec![call(vec![json!(4)], json!(9))], &[post]),
        cb("map_or_else some: f not called", f, &[("self", json!({"some": 4}))], vec![], &[]),
        cb("map_or_else some: f called twice", f, &[("self", json!({"some": 4}))], vec![call(vec![json!(4)], json!(9)), call(vec![json!(4)], json!(9))], &[]),
        cb("map_or_else some: f called with another value", f, &[("self", json!({"some": 4}))], vec![call(vec![json!(5)], json!(9))], &[]),
        cb("map_or_else none: f must not be called", f, &[("self", Json::Null)], vec![call(vec![json!(4)], json!(9))], &[]),
        cb("map_or_else none: f not called", f, &[("self", Json::Null)], vec![], &[]),
        cb("map_or_else none: default called, result is d", d, &[("self", Json::Null), ("result", json!(7))], vec![call(vec![], json!(7))], &[post]),
        cb("map_or_else some: default must not be called", d, &[("self", json!({"some": 4}))], vec![call(vec![], json!(7))], &[]),
        cb("map_or_else some: the postcondition does not use d", d, &[("self", json!({"some": 4})), ("result", json!(9))], vec![], &[post]),
        cb("and_modify occupied: f called with the value", and_modify, &[("self", occ.clone())], vec![call(vec![json!({"id": 1})], Json::Null)], &["same(v, v)"]),
        cb("and_modify occupied: the entry is the one f changed", "callback f : called exactly once when self is Occupied(e), with v", &[("self", json!({ "variant": "Occupied", "fields": [{ "id": 1 }] })), ("result", json!({ "variant": "Occupied", "fields": [{ "id": 1 }] }))], vec![call(vec![json!({"id": 1})], Json::Null)], &["result is Occupied(r) implies same(current(r), v)"]),
        cb("and_modify occupied: another entry", "callback f : called exactly once when self is Occupied(e), with v", &[("self", json!({ "variant": "Occupied", "fields": [{ "id": 1 }] })), ("result", json!({ "variant": "Occupied", "fields": [{ "id": 2 }] }))], vec![call(vec![json!({"id": 1})], Json::Null)], &["result is Occupied(r) implies same(current(r), v)"]),
        cb("and_modify vacant: f not called", and_modify, &[("self", vac.clone())], vec![], &[]),
        cb("and_modify vacant: f called", and_modify, &[("self", vac)], vec![call(vec![json!({"id": 1})], Json::Null)], &[]),
        cb("and_modify occupied: f not called", and_modify, &[("self", occ)], vec![], &[]),
        cb("at most once: not called", "callback f : called at most once", &[], vec![], &[]),
        cb("at most once: twice", "callback f : called at most once", &[], vec![call(vec![], Json::Null), call(vec![], Json::Null)], &[]),
        cb("two arguments", "callback f : called exactly once, with a, b, returning r", &[], vec![call(vec![json!(1), json!(2)], json!(3))], &["r == a + b"]),
        cb("with an expression: the argument equals it", "callback f : called exactly once, with self + 1", &[("self", json!(1))], vec![call(vec![json!(2)], Json::Null)], &[]),
        cb("with an expression: it does not", "callback f : called exactly once, with self + 1", &[("self", json!(1))], vec![call(vec![json!(3)], Json::Null)], &[]),
    ]
}

fn run_case(id: &str, clause: &str, now: &[(&str, Json)], old: &[(&str, Json)], fin: &[(&str, Json)], groups: Vec<Json>, ensure: &[&str]) -> Json {
    let mut c = json!({ "id": id, "clause": clause, "env": { "now": val(now), "old": val(old) }, "final": val(fin), "groups": groups, "ensure": ensure });
    c["expect"] = eval_case("runs", &c).unwrap_or_else(|e| panic!("{id}: {e}"));
    c
}

fn runs_suite() -> Vec<Json> {
    let c1 = |a: i64, r: i64| call(vec![json!(a)], json!(r));
    let map = "callback f : called by result.next, at most once per call, with x, returning y";
    let filt = "callback predicate : called by result.next, with x, returning keep";
    let tw = "callback predicate : called by result.next, at most once per call, with x, returning keep";
    vec![
        run_case("map: one call per next, the items are the results", map, &[("result", json!([2, 4]))], &[("self", json!([1, 2]))], &[], vec![json!([c1(1, 2)]), json!([c1(2, 4)]), json!([])], &["yields(result) == y", "x == old(yields(self))"]),
        run_case("map: the items are not the results", map, &[("result", json!([2, 5]))], &[("self", json!([1, 2]))], &[], vec![json!([c1(1, 2)]), json!([c1(2, 4)]), json!([])], &["yields(result) == y"]),
        run_case("map: two calls in one next", map, &[], &[], &[], vec![json!([c1(1, 2), c1(1, 2)])], &[]),
        run_case("map: no calls at all", map, &[("result", json!([]))], &[("self", json!([]))], &[], vec![json!([])], &["yields(result) == y", "x == old(yields(self))"]),
        run_case("filter: many calls per next are allowed", filt, &[("result", json!([1, 3]))], &[("self", json!([1, 2, 3]))], &[], vec![json!([call(vec![json!(1)], json!(true))]), json!([call(vec![json!(2)], json!(false)), call(vec![json!(3)], json!(true))]), json!([])], &["yields(result) == select(keep, old(yields(self)))"]),
        run_case("take_while: items before the first false, the rest not put back", tw, &[("result", json!([1, 2]))], &[("self", json!([1, 2, 3, 4]))], &[("self", json!([4]))], vec![json!([call(vec![json!(1)], json!(true))]), json!([call(vec![json!(2)], json!(true))]), json!([call(vec![json!(3)], json!(false))])], &["yields(result) == while_true(keep, old(yields(self)))", "x == take(size(keep), old(yields(self)))", "final(yields(self)) == skip(size(keep), old(yields(self)))"]),
        run_case("take_while: the failing item is consumed too", tw, &[], &[("self", json!([1, 2, 3, 4]))], &[("self", json!([3, 4]))], vec![json!([call(vec![json!(1)], json!(true))]), json!([call(vec![json!(2)], json!(false))])], &["final(yields(self)) == skip(size(keep), old(yields(self)))"]),
    ]
}

fn lets_suite() -> Vec<Json> {
    let l = |id: &str, clause: &str, now: &[(&str, Json)]| {
        let mut c = json!({ "id": id, "clause": clause, "env": { "now": val(now), "old": {} } });
        c["expect"] = eval_case("lets", &c).unwrap_or_else(|e| panic!("{id}: {e}"));
        c
    };
    let pair = json!({"tuple": [[1], [2, 3]]});
    vec![
        l("tuple pattern", "let (head, tail) = result", &[("result", pair.clone())]),
        l("wildcard", "let (_, tail) = result", &[("result", pair.clone())]),
        l("pattern of the wrong length", "let (a, b, c) = result", &[("result", pair)]),
        l("a name", "let n = size(xs)", &[("xs", json!([1, 2]))]),
        l("a name bound twice", "let xs = size(xs)", &[("xs", json!([1, 2]))]),
    ]
}

fn pr(id: &str, protocol: &str, events: &[&str]) -> Json {
    let mut c = json!({ "id": id, "protocol": protocol, "events": events });
    c["expect"] = eval_case("protocols", &c).unwrap_or_else(|e| panic!("{id}: {e}"));
    c
}

fn protocols_suite() -> Vec<Json> {
    vec![
        pr("count: next, at least once, even if empty", "protocol self.next+", &["begin", "self.next", "end"]),
        pr("count: next several times", "protocol self.next+", &["begin", "self.next", "self.next", "self.next", "end"]),
        pr("count: never called", "protocol self.next+", &["begin", "end"]),
        pr("assert_eq: left then right", "protocol left right", &["left", "right"]),
        pr("assert_eq: right then left", "protocol left right", &["right", "left"]),
        pr("assert_eq: left twice", "protocol left right", &["left", "left", "right"]),
        pr("calls the protocol does not name are free", "protocol left right", &["left", "elsewhere", "right"]),
        pr("alternatives", "protocol (a | b)* c?", &["a", "b", "a", "c"]),
        pr("alternatives, c first", "protocol (a | b)* c?", &["c", "a"]),
        pr("begin and end are events", "protocol begin f? end", &["begin", "f", "end"]),
    ]
}

/// The whole table.
pub fn table() -> Json {
    json!({
        "format": FORMAT,
        "about": "Conformance cases for language 6.13 (callback `with` and `returning`), 6.14.7 (tuple parts) and 6.15 (sequences). Run every case of every suite through the host's own evaluation and compare with `expect`. Value encoding: numbers, booleans and strings are themselves; null is none and {\"some\": v} is some; {\"tuple\": [..]}; a finite list is an array; an infinite list is {\"counter\": {\"from\": a, \"step\": b}} as an input and {\"prefix\": [first 8 items]} as a result; {\"variant\": n, \"fields\": [..]}; {\"ent\": [key, id]} is an entity whose key `==` and the order see and whose id only `moved from` sees; {\"id\": n} is a host reference's identity. A result of {\"undefined\": true} is an operator with no value (size and reverse of an infinite list); {\"undecided\": true} is an equality no finite prefix refutes; {\"refused\": ...} is a form refused at build.",
        "prefix": crate::seq::PREFIX,
        "named_functions": NAMED.iter().map(|(n, m)| json!({ "name": n, "meaning": m })).collect::<Vec<_>>(),
        "definitions": {
            "index": "xs[i]: the element at position i; none when i < 0 or i >= size(xs) (for an infinite xs, when i < 0). Never fails.",
            "take": "take(n, xs): the first n elements, or all if fewer; empty when n <= 0.",
            "skip": "skip(n, xs): xs without its first n elements; xs when n <= 0, empty when n >= size(xs). skip(n, xs)[k] == xs[k + n].",
            "slice": "xs[i..j] is take(j - i, skip(i, xs)); xs[i..] is skip(i, xs); xs[..j] is take(j, xs). Never fails. Args: [xs, i|null, j|null]. Not exercised for i < 0, where the equation and the sentence 'positions outside the list are not elements' differ.",
            "concat": "xs ++ ys: for a finite xs, xs then ys; for an infinite xs, xs. Associative, [] is the unit.",
            "reverse": "reverse(xs)[k] == xs[size(xs) - 1 - k]; no value for an infinite xs.",
            "map": "map(f, xs)[k] == f(xs[k]); f a named pure function.",
            "filter": "filter(p, []) == []; filter(p, [x] ++ r) == (if p(x) then [x] else []) ++ filter(p, r).",
            "zip": "zip(xs, ys)[k] == (xs[k], ys[k]) while both exist.",
            "enumerate": "enumerate(xs)[k] == (k, xs[k]).",
            "scalars": "the Unicode scalar values of the text, in order.",
            "size": "the number of elements; no value for an infinite list.",
            "eq": "equal at every position; for infinite lists, of every finite prefix: refuted on a prefix, never confirmed.",
            "part": "e.k: the component k of a tuple; a component the tuple lacks is refused at build.",
            "bind": "a pattern (a, _, b) binds a name to each wanted part; a pattern of the wrong length is refused at build.",
            "next": "yields: next returns xs[0] and leaves xs[1..].",
            "present": "present(xs, i): i >= 0 and (xs infinite or i < size(xs)): position i holds an element, whatever it is.",
            "select": "select(bs, xs): the elements xs[k] where bs[k] is true, for the positions both lists have.",
            "while_true": "while_true(bs, xs): take(n, xs) with n the position of the first false in bs, or size(bs); xs when bs is infinite and has none.",
            "flatten": "flatten(xss): flatten([]) == [], flatten([xs] ++ r) == xs ++ flatten(r); no value for an infinite outer list. map(yields, ys) reads a list of collections as lists.",
            "runs": "callback c : called by P.m …: the calls made during each call of P.m (groups); `with` and `returning` bind lists, one item per call; the count per call is checked per group. final(e) is read from the case's `final` environment.",
            "lets": "let name = e / let (a, b) = e binds for the clauses after it, once; a name already bound, or a pattern of the wrong length, is refused at build.",
            "next_back": "yields: next_back returns the last element and leaves all but it; no value for an infinite list."
        },
        "suites": {
            "ops": ops_suite(),
            "expressions": expressions_suite(),
            "callbacks": callbacks_suite(),
            "runs": runs_suite(),
            "lets": lets_suite(),
            "protocols": protocols_suite(),
        }
    })
}

pub fn table_text() -> String {
    let mut s = String::new();
    let t = table();
    // one case per line, so that a change shows in a diff
    s.push_str("{\n");
    for k in ["format", "about", "prefix", "named_functions", "definitions"] {
        s.push_str(&format!("  {}: {},\n", json!(k), t[k]));
    }
    s.push_str("  \"suites\": {\n");
    let suites = SUITES;
    for (i, k) in suites.iter().enumerate() {
        s.push_str(&format!("    {}: [\n", json!(k)));
        let cs = t["suites"][k].as_array().unwrap();
        for (j, c) in cs.iter().enumerate() {
            s.push_str(&format!("      {}{}\n", c, if j + 1 < cs.len() { "," } else { "" }));
        }
        s.push_str(&format!("    ]{}\n", if i + 1 < suites.len() { "," } else { "" }));
    }
    s.push_str("  }\n}\n");
    s
}
