//! The conformance table of language 6.13 to 6.15: the file is current, the reference passes it, and the defining
//! equations of the text hold of the evaluator on generated lists against an oracle written with plain vectors.
use bare_rust::conformance::{check_host, eval_case, table, table_path, table_text};
use bare_rust::seq::{from_json, seq_eq, to_json, Seq, Value};
use serde_json::{json, Value as Json};

#[test]
fn the_exported_table_is_current() {
    let text = table_text();
    let path = table_path();
    if std::env::var_os("UPDATE_CONFORMANCE").is_some() {
        std::fs::write(&path, &text).unwrap();
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(have == text, "{} is out of date: run with UPDATE_CONFORMANCE=1", path.display());
    let parsed: Json = serde_json::from_str(&have).unwrap();
    assert_eq!(parsed, table());
}

#[test]
fn the_reference_passes_its_table_read_back_from_the_file() {
    let t: Json = serde_json::from_str(&std::fs::read_to_string(table_path()).unwrap()).unwrap();
    let bad = check_host(&t, &eval_case);
    assert!(bad.is_empty(), "{bad:#?}");
    let n: usize = bare_rust::conformance::SUITES.iter().map(|s| t["suites"][s].as_array().unwrap().len()).sum();
    assert!(n > 300, "{n} cases");
}

#[test]
fn a_wrong_host_is_caught() {
    let t = table();
    // a host whose slices clamp wrongly and whose index of -1 wraps
    let bad = check_host(&t, &|suite, case| {
        if suite == "ops" && case["op"] == "index" && case["args"][1] == json!(-1) {
            return Ok(json!({"some": 0}));
        }
        eval_case(suite, case)
    });
    assert!(!bad.is_empty() && bad.iter().all(|b| b.contains("index")), "{bad:?}");
}

fn ints(v: &[i64]) -> Seq {
    Seq::fin(v.iter().map(|i| Value::Int(*i)).collect())
}

fn vec_of(s: &Seq) -> Vec<i64> {
    s.to_vec().unwrap().iter().map(|v| v.as_int().unwrap()).collect()
}

fn samples() -> Vec<Vec<i64>> {
    vec![vec![], vec![7], vec![7, 8], vec![3, 1, 4, 1, 5], (0..20).collect()]
}

fn around(n: i64) -> Vec<i64> {
    vec![-1, 0, n - 1, n, n + 1, 1 << 40]
}

#[test]
fn positions_take_skip_and_slices_follow_their_definitions() {
    for l in samples() {
        let n = l.len() as i64;
        let xs = ints(&l);
        for i in around(n) {
            // xs[i]: none outside the list
            let want = if i >= 0 && i < n { Some(l[i as usize]) } else { None };
            assert_eq!(xs.index(i).eq_dec(&want.map_or(Value::None, |x| Value::some(Value::Int(x)))), Some(true), "{l:?}[{i}]");
            // take(n, xs): the first n, or all; empty for n <= 0
            let t: Vec<i64> = l.iter().copied().take(i.max(0) as usize).collect();
            assert_eq!(vec_of(&xs.take(i)), t, "take({i}, {l:?})");
            // skip(n, xs): without the first n; xs for n <= 0, empty for n >= size
            let sk: Vec<i64> = l.iter().copied().skip(i.max(0) as usize).collect();
            assert_eq!(vec_of(&xs.skip(i)), sk, "skip({i}, {l:?})");
            // skip(n, xs)[k] == xs[k + n]
            for k in 0..3 {
                if i >= 0 {
                    assert_eq!(xs.skip(i).index(k).eq_dec(&xs.index(k + i)), Some(true));
                }
            }
        }
        // slices never fail: positions outside the list are not elements
        for i in [0, 1, 2, n, n + 1, 9] {
            for j in [0, 1, 2, n, n + 1, 9] {
                let want: Vec<i64> = (i..j).filter(|p| *p >= 0 && *p < n).map(|p| l[p as usize]).collect();
                assert_eq!(vec_of(&xs.slice(Some(i), Some(j))), want, "{l:?}[{i}..{j}]");
            }
            let want: Vec<i64> = (i..n).filter(|p| *p >= 0).map(|p| l[p as usize]).collect();
            assert_eq!(vec_of(&xs.slice(Some(i), None)), want);
            let want: Vec<i64> = (0..i).filter(|p| *p < n).map(|p| l[p as usize]).collect();
            assert_eq!(vec_of(&xs.slice(None, Some(i))), want);
        }
    }
    // the example of the text: xs[2..9] of three elements has one
    assert_eq!(vec_of(&ints(&[1, 2, 3]).slice(Some(2), Some(9))), [3]);
}

#[test]
fn concat_reverse_map_filter_zip_enumerate_follow_their_definitions() {
    for l in samples() {
        let xs = ints(&l);
        for m in samples() {
            let ys = ints(&m);
            let want: Vec<i64> = l.iter().chain(&m).copied().collect();
            assert_eq!(vec_of(&xs.concat(&ys)), want);
            // associative, with [] as unit
            let zs = ints(&[100, 101]);
            assert_eq!(seq_eq(&xs.concat(&ys).concat(&zs), &xs.concat(&ys.concat(&zs))), Some(true));
            let z: Vec<(i64, i64)> = l.iter().copied().zip(m.iter().copied()).collect();
            let got = xs.zip(&ys).to_vec().unwrap();
            assert_eq!(got.len(), z.len());
            for (g, w) in got.iter().zip(&z) {
                assert_eq!(g.eq_dec(&Value::Tuple(vec![Value::Int(w.0), Value::Int(w.1)])), Some(true));
            }
        }
        assert_eq!(seq_eq(&xs.concat(&Seq::empty()), &xs), Some(true));
        assert_eq!(seq_eq(&Seq::empty().concat(&xs), &xs), Some(true));
        let rev: Vec<i64> = l.iter().rev().copied().collect();
        assert_eq!(vec_of(&xs.reverse().unwrap()), rev);
        // map(f, xs)[k] == f(xs[k]), the same size
        let mapped = xs.map(|v| Value::Int(v.as_int().unwrap() * 2));
        assert_eq!(vec_of(&mapped), l.iter().map(|x| x * 2).collect::<Vec<_>>());
        // filter(p, []) == [] and filter(p, [x] ++ r) == (if p(x) then [x] else []) ++ filter(p, r)
        let p: fn(&Value) -> bool = |v| v.as_int().unwrap() % 2 == 1;
        assert!(xs.filter(p).len().is_some());
        assert!(Seq::empty().filter(p).is_empty());
        if let Some((h, rest)) = l.split_first() {
            let head = if h % 2 == 1 { ints(&[*h]) } else { Seq::empty() };
            assert_eq!(seq_eq(&xs.filter(p), &head.concat(&ints(rest).filter(p))), Some(true));
        }
        for (k, e) in xs.enumerate().to_vec().unwrap().iter().enumerate() {
            assert_eq!(e.eq_dec(&Value::Tuple(vec![Value::Int(k as i64), Value::Int(l[k])])), Some(true));
        }
    }
}

#[test]
fn infinite_sequences_have_prefixes_not_sizes() {
    let c = Seq::counter(0, 1);
    assert_eq!(vec_of(&c.take(3)), [0, 1, 2]);
    assert_eq!(c.skip(5).index(0).eq_dec(&Value::some(Value::Int(5))), Some(true));
    assert_eq!(c.index(1 << 40).eq_dec(&Value::some(Value::Int(1 << 40))), Some(true));
    assert!(c.reverse().is_none() && c.len().is_none());
    // an infinite xs ++ ys is xs
    assert_eq!(seq_eq(&c.concat(&ints(&[1])), &c), None);
    assert_eq!(to_json(&Value::List(c.concat(&ints(&[1])))), json!({"prefix": [0, 1, 2, 3, 4, 5, 6, 7]}));
    // zip with a counter is as long as the shorter
    assert_eq!(c.zip(&ints(&[9, 9])).len(), Some(2));
    // filter keeps the order
    assert_eq!(vec_of(&c.filter(|v| v.as_int().unwrap() % 3 == 0).take(4)), [0, 3, 6, 9]);
    // equality of infinite sequences: refuted on a prefix, never confirmed
    assert_eq!(seq_eq(&c, &c.skip(1)), Some(false));
    assert_eq!(seq_eq(&c, &Seq::counter(0, 1)), None);
    assert_eq!(seq_eq(&c, &ints(&[0, 1])), Some(false));
}

#[test]
fn the_double_ended_iterator_model_reads_one_sequence_from_either_end() {
    for l in samples() {
        // repeated next_back gives reverse(yields)
        let mut rest = Json::Array(l.iter().map(|x| json!(x)).collect());
        let mut got = vec![];
        loop {
            let r = eval_case("ops", &json!({"op": "next_back", "args": [rest]})).unwrap();
            if r["result"].is_null() {
                break;
            }
            got.push(r["result"]["some"].as_i64().unwrap());
            rest = r["yields"].clone();
        }
        let want: Vec<i64> = l.iter().rev().copied().collect();
        assert_eq!(got, want);
    }
    assert!(from_json(&json!({"some": [1]})).is_ok());
}

#[test]
fn present_select_while_true_flatten_follow_their_definitions() {
    for l in samples() {
        let n = l.len() as i64;
        let xs = ints(&l);
        for i in around(n) {
            assert_eq!(xs.present(i), i >= 0 && i < n);
        }
        for pattern in 0u32..16 {
            let bs: Vec<bool> = (0..4).map(|k| pattern >> k & 1 == 1).collect();
            let bsq = Seq::fin(bs.iter().map(|b| Value::Bool(*b)).collect());
            let want: Vec<i64> = bs.iter().zip(&l).filter(|(b, _)| **b).map(|(_, x)| *x).collect();
            assert_eq!(vec_of(&bsq.select(&xs)), want, "select({bs:?}, {l:?})");
            // filter(p, xs) == select(map(p, xs), xs)
            let p: fn(&Value) -> bool = |v| v.as_int().unwrap() % 2 == 1;
            let verdicts = xs.map(|v| Value::Bool(v.as_int().unwrap() % 2 == 1));
            assert_eq!(seq_eq(&xs.filter(p), &verdicts.select(&xs)), Some(true));
            let first_false = bs.iter().position(|b| !b).unwrap_or(bs.len());
            assert_eq!(vec_of(&bsq.while_true(&xs)), l.iter().copied().take(first_false).collect::<Vec<_>>(), "while_true({bs:?}, {l:?})");
        }
    }
    let lists_of = Seq::fin(vec![Value::List(ints(&[1])), Value::List(Seq::empty()), Value::List(ints(&[2, 3]))]);
    assert_eq!(vec_of(&lists_of.flatten().unwrap()), [1, 2, 3]);
    assert!(Seq::counter(0, 1).flatten().is_none());
}
