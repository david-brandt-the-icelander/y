//! Sampled runtime checks of the library file's clauses against the real `std` (language 6.13 to 6.15, proposal 9):
//! callbacks and protocols, moves, ends and survivals of a lifetime, `must be used`, and `yields`. A small Rust program is
//! generated for each item we know how to drive (instrumented closures that record their calls, Drop-counting types, spy
//! iterators, iterators collected to vectors) and run; its output is compared with the clause by the reference evaluator
//! (`seqexpr`, `callback`). It is evidence from samples, not proof: a clause that agrees on the samples is `checked`, one
//! that cannot be driven is `unchecked`, and only a run that contradicts the clause fails.
use crate::callback::{check_callback, check_called_by, follows, parse_callback, parse_called_by, parse_protocol, Call, Verdict};
use crate::cargo;
use crate::seq::{from_json, Value};
use crate::seqexpr::{clause_body, holds, Env, EvalErr};
use crate::{first_error, parse_rust, R};
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::process::Command;

pub const KINDS: &[&str] = &["callback", "protocol", "moved from", "lifetime", "must be used", "yields"];

#[derive(Default, Debug, Clone)]
pub struct KindStat {
    pub checked: usize,
    /// (subject and clause, why it could not be driven).
    pub unchecked: Vec<(String, String)>,
    /// Subject, clause and what the run showed.
    pub contradictions: Vec<String>,
}

#[derive(Default, Debug)]
pub struct RuntimeReport {
    pub kinds: BTreeMap<String, KindStat>,
    /// Why part of the check did not run.
    pub notes: Vec<String>,
}

impl RuntimeReport {
    fn stat(&mut self, k: &str) -> &mut KindStat {
        self.kinds.entry(k.to_string()).or_default()
    }

    pub fn contradictions(&self) -> usize {
        self.kinds.values().map(|s| s.contradictions.len()).sum()
    }

    pub fn checked(&self) -> usize {
        self.kinds.values().map(|s| s.checked).sum()
    }

    pub fn to_json(&self) -> Json {
        json!({
            "kinds": self.kinds.iter().map(|(k, s)| (k.clone(), json!({
                "checked": s.checked,
                "unchecked": s.unchecked.len(),
                "unchecked_by_reason": s.unchecked.iter().fold(BTreeMap::<String, usize>::new(), |mut m, (_, w)| { *m.entry(w.clone()).or_default() += 1; m }),
                "contradictions": s.contradictions,
            }))).collect::<serde_json::Map<_, _>>(),
            "notes": self.notes,
        })
    }

    pub fn print(&self) {
        println!("  runtime   {}  {} clauses checked against the real std, {} contradicted", if self.contradictions() == 0 { "ok" } else { "FAIL" }, self.checked(), self.contradictions());
        for (k, s) in &self.kinds {
            println!("              {k}: {} checked, {} unchecked, {} contradicted", s.checked, s.unchecked.len(), s.contradictions.len());
            for c in &s.contradictions {
                println!("                contradiction: {c}");
            }
        }
        for n in &self.notes {
            println!("              note: {n}");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The generated program

/// Helpers every generated program has: JSON printing for the values the probes record.
const PRELUDE: &str = r##"#![allow(warnings)]
use std::cell::RefCell;
use std::collections::HashMap;
trait J { fn j(&self) -> String; }
macro_rules! jnum { ($($t:ty),*) => { $(impl J for $t { fn j(&self) -> String { self.to_string() } })* } }
jnum!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize, bool);
fn esc(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) }
    }
    o.push('"');
    o
}
impl J for char { fn j(&self) -> String { esc(&self.to_string()) } }
impl J for str { fn j(&self) -> String { esc(self) } }
impl J for String { fn j(&self) -> String { esc(self) } }
impl<T: J + ?Sized> J for &T { fn j(&self) -> String { (**self).j() } }
impl<T: J> J for Option<T> { fn j(&self) -> String { match self { None => "null".into(), Some(x) => format!("{{\"some\":{}}}", x.j()) } } }
impl<T: J> J for [T] { fn j(&self) -> String { format!("[{}]", self.iter().map(|x| x.j()).collect::<Vec<_>>().join(",")) } }
impl<T: J> J for Vec<T> { fn j(&self) -> String { self[..].j() } }
impl<A: J, B: J> J for (A, B) { fn j(&self) -> String { format!("{{\"tuple\":[{},{}]}}", self.0.j(), self.1.j()) } }
fn obj(f: &[(&str, String)]) -> String { format!("{{{}}}", f.iter().map(|(k, v)| format!("{}:{}", esc(k), v)).collect::<Vec<_>>().join(",")) }
fn arr(v: &[String]) -> String { format!("[{}]", v.join(",")) }
fn line(id: &str, f: &[(&str, String)]) { println!("@@{{\"id\":{},{}", esc(id), &obj(f)[1..]); }
fn lists() -> Vec<Vec<i32>> { vec![vec![], vec![1], vec![1, 2], vec![3, 1, 4, 1, 5]] }
fn ns() -> [usize; 4] { [0, 1, 2, 10] }
fn id_json(x: usize) -> String { format!("{{\"id\":{}}}", x) }
fn variant(n: &str, f: &[String]) -> String { format!("{{\"variant\":{},\"fields\":{}}}", esc(n), arr(f)) }
fn call_json(args: &[String], result: String) -> String { format!("{{\"args\":{},\"result\":{}}}", arr(args), result) }
fn entry_json(e: &std::collections::hash_map::Entry<u8, i32>) -> String {
    match e { std::collections::hash_map::Entry::Occupied(o) => variant("Occupied", &[id_json(o.get() as *const i32 as usize)]), std::collections::hash_map::Entry::Vacant(_) => variant("Vacant", &[]) }
}
// an entity: `==` and the order see the key, only `moved from` sees the id
struct E { k: i32, id: u32 }
impl PartialEq for E { fn eq(&self, o: &E) -> bool { self.k == o.k } }
impl Eq for E {}
impl PartialOrd for E { fn partial_cmp(&self, o: &E) -> Option<std::cmp::Ordering> { Some(self.k.cmp(&o.k)) } }
impl Ord for E { fn cmp(&self, o: &E) -> std::cmp::Ordering { self.k.cmp(&o.k) } }
impl J for E { fn j(&self) -> String { format!("{{\"ent\":[{},{}]}}", self.k, self.id) } }
// its destructor records its id
thread_local! { static DROPS: RefCell<Vec<u32>> = RefCell::new(vec![]); }
impl Drop for E { fn drop(&mut self) { DROPS.with(|d| d.borrow_mut().push(self.id)); } }
fn take_drops() -> Vec<u32> { DROPS.with(|d| std::mem::take(&mut *d.borrow_mut())) }
// the run of an adapter: `next` until the first none; the callback calls made during each `next`, and the items
fn run_adapter<I: Iterator>(mut r: I, calls: &RefCell<Vec<String>>) -> (Vec<I::Item>, Vec<String>) {
    let (mut items, mut groups) = (vec![], vec![]);
    loop {
        let before = calls.borrow().len();
        let o = r.next();
        groups.push(arr(&calls.borrow()[before..]));
        match o { Some(x) => items.push(x), None => break }
    }
    (items, groups)
}
// an iterator that records its `next` calls
struct Spy<'a> { n: u32, log: &'a RefCell<Vec<String>> }
impl<'a> Iterator for Spy<'a> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> { self.log.borrow_mut().push("self.next".into()); if self.n == 0 { None } else { self.n -= 1; Some(1) } }
}
"##;

struct Probe {
    /// The statements of the probe function; `@p0@` and `@p1@` are the subject's first two parameter names.
    body: String,
}

fn param_name(sub: &Json, i: usize, default: &str) -> String {
    sub["params"][i]["name"].as_str().unwrap_or(default).to_string()
}

fn probe_for(sub: &Json) -> Option<Probe> {
    let name = sub["name"].as_str()?;
    let b = |s: &str| {
        Some(Probe { body: s.replace("@p0@", &param_name(sub, 0, "self")).replace("@p1@", &param_name(sub, 1, "other")) })
    };
    match name {
        // yields
        "std::primitive::str::chars" => b(r##"for (k, s) in ["", "a", "h\u{e9}\u{e9}\u{1f600}"].iter().enumerate() {
            let r = s.chars();
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", s.j())])), ("now", obj(&[("@p0@", s.j()), ("result", r.clone().collect::<Vec<char>>().j())]))]);
        }"##),
        "std::primitive::slice::iter" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let r = v[..].iter();
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", v.j())])), ("now", obj(&[("@p0@", v.j()), ("result", r.clone().collect::<Vec<&i32>>().j())]))]);
        }"##),
        "std::iter::Iterator::next" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let mut it = v.clone().into_iter();
            let old: Vec<i32> = it.clone().collect();
            let r = it.next();
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", old.j())])), ("now", obj(&[("@p0@", it.clone().collect::<Vec<i32>>().j()), ("result", r.j())]))]);
        }"##),
        "std::iter::Iterator::enumerate" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let it = v.clone().into_iter();
            let r = it.clone().enumerate();
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", it.clone().collect::<Vec<i32>>().j())])), ("now", obj(&[("result", r.clone().collect::<Vec<(usize, i32)>>().j())]))]);
        }"##),
        "std::iter::Iterator::skip" | "std::iter::Iterator::take" => {
            let m = if name.ends_with("skip") { "skip" } else { "take" };
            b(&r##"for (k, v) in lists().into_iter().enumerate() {
            for n in ns() {
                let it = v.clone().into_iter();
                let r = it.clone().@m@(n);
                line(&format!("#{k}/{n}"), &[("old", obj(&[("@p0@", it.clone().collect::<Vec<i32>>().j())])), ("now", obj(&[("@p1@", n.j()), ("result", r.clone().collect::<Vec<i32>>().j())]))]);
            }
        }"##.replace("@m@", m))
        }
        "std::iter::Iterator::chain" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            for (j, o) in lists().into_iter().enumerate() {
                let it = v.clone().into_iter();
                let r = it.clone().chain(o.clone());
                line(&format!("#{k}/{j}"), &[("old", obj(&[("@p0@", v.j()), ("@p1@", o.j())])), ("now", obj(&[("result", r.clone().collect::<Vec<i32>>().j())]))]);
            }
        }"##),
        "std::iter::Iterator::zip" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            for (j, o) in lists().into_iter().enumerate() {
                let it = v.clone().into_iter();
                let r = it.clone().zip(o.clone());
                line(&format!("#{k}/{j}"), &[("old", obj(&[("@p0@", v.j()), ("@p1@", o.j())])), ("now", obj(&[("result", r.clone().collect::<Vec<(i32, i32)>>().j())]))]);
            }
        }"##),
        "std::iter::Iterator::copied" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let it = v[..].iter();
            let r = it.clone().copied();
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", it.clone().collect::<Vec<&i32>>().j())])), ("now", obj(&[("result", r.clone().collect::<Vec<i32>>().j())]))]);
        }"##),
        "std::iter::IntoIterator::into_iter" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let r = IntoIterator::into_iter(v.clone());
            line(&format!("#{k}/collection"), &[("old", obj(&[("@p0@", v.j())])), ("now", obj(&[("@p0@", v.j()), ("result", r.clone().collect::<Vec<i32>>().j())]))]);
            let it = v.clone().into_iter();
            let r = IntoIterator::into_iter(it.clone());
            line(&format!("#{k}/iterator"), &[("old", obj(&[("@p0@", it.clone().collect::<Vec<i32>>().j())])), ("now", obj(&[("@p0@", it.clone().collect::<Vec<i32>>().j()), ("result", r.clone().collect::<Vec<i32>>().j())]))]);
        }"##),
        "std::vec::Vec as std::iter::IntoIterator::into_iter" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let r = IntoIterator::into_iter(v.clone());
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", v.j())])), ("now", obj(&[("@p0@", v.j()), ("result", r.clone().collect::<Vec<i32>>().j())]))]);
        }"##),
        // callbacks
        "std::option::Option::map_or_else" => b(r##"for (k, o) in [None, Some(4i32), Some(0)].into_iter().enumerate() {
            let (cd, cf) = (RefCell::new(Vec::<String>::new()), RefCell::new(Vec::<String>::new()));
            let r = o.map_or_else(
                || { cd.borrow_mut().push(call_json(&[], 7i32.j())); 7i32 },
                |x| { cf.borrow_mut().push(call_json(&[x.j()], (x * 10 + 1).j())); x * 10 + 1 });
            line(&format!("#{k}"), &[("now", obj(&[("@p0@", o.j()), ("result", r.j())])), ("calls", obj(&[("default", arr(&cd.borrow())), ("f", arr(&cf.borrow()))]))]);
        }"##),
        "std::collections::hash_map::Entry::and_modify" => b(r##"for (k, present) in [false, true].into_iter().enumerate() {
            let mut m: HashMap<u8, i32> = HashMap::new();
            if present { m.insert(1, 5); }
            let seen = RefCell::new(Vec::<String>::new());
            let e = m.entry(1);
            let before = entry_json(&e);
            let e2 = e.and_modify(|v: &mut i32| { seen.borrow_mut().push(call_json(&[id_json(v as *mut i32 as usize)], "null".into())); *v += 1; });
            line(&format!("#{k}"), &[("now", obj(&[("@p0@", before), ("result", entry_json(&e2))])), ("calls", obj(&[("f", arr(&seen.borrow()))]))]);
        }"##),
        // protocols
        "std::iter::Iterator::count" => b(r##"for n in [0u32, 1, 3] {
            let log = RefCell::new(Vec::<String>::new());
            log.borrow_mut().push("begin".into());
            let c = Spy { n, log: &log }.count();
            log.borrow_mut().push("end".into());
            let ev: Vec<String> = log.borrow().iter().map(|e| esc(e)).collect();
            line(&format!("#{n}"), &[("events", arr(&ev)), ("known", arr(&[esc("begin"), esc("end"), esc("self.next")])), ("now", obj(&[("result", c.j())]))]);
        }"##),
        "std::assert_eq!" => b(r##"{
            let log = RefCell::new(Vec::<String>::new());
            log.borrow_mut().push("begin".into());
            assert_eq!({ log.borrow_mut().push("left".into()); 1 }, { log.borrow_mut().push("right".into()); 1 });
            log.borrow_mut().push("end".into());
            let ev: Vec<String> = log.borrow().iter().map(|e| esc(e)).collect();
            line("#0", &[("events", arr(&ev)), ("known", arr(&[esc("begin"), esc("end"), esc("left"), esc("right")]))]);
        }"##),
        // calls a result makes later: the run of the adapter, the calls per `next`, and what is left of `self`
        "std::iter::Iterator::map" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let calls = RefCell::new(Vec::<String>::new());
            let mut it = v.clone().into_iter();
            let (items, groups) = run_adapter(it.by_ref().map(|x| { let y = x * 2; calls.borrow_mut().push(call_json(&[x.j()], y.j())); y }), &calls);
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", v.j())])), ("now", obj(&[("result", items.j())])), ("fin", obj(&[("@p0@", it.collect::<Vec<i32>>().j())])), ("runs", obj(&[("@p1@", arr(&groups))]))]);
        }"##),
        "std::iter::Iterator::filter" | "std::iter::Iterator::take_while" => {
            let m = if name.ends_with("filter") { "filter" } else { "take_while" };
            b(&r##"for (k, v) in lists().into_iter().enumerate() {
            let calls = RefCell::new(Vec::<String>::new());
            let mut it = v.clone().into_iter();
            let (items, groups) = run_adapter(it.by_ref().@m@(|x| { let y = x % 2 == 1; calls.borrow_mut().push(call_json(&[x.j()], y.j())); y }), &calls);
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", v.j())])), ("now", obj(&[("result", items.j())])), ("fin", obj(&[("@p0@", it.collect::<Vec<i32>>().j())])), ("runs", obj(&[("@p1@", arr(&groups))]))]);
        }"##.replace("@m@", m))
        }
        "std::iter::Iterator::flat_map" => b(r##"for (k, v) in lists().into_iter().enumerate() {
            let calls = RefCell::new(Vec::<String>::new());
            let mut it = v.clone().into_iter();
            let (items, groups) = run_adapter(it.by_ref().flat_map(|x| { let y = vec![x, x + 100]; calls.borrow_mut().push(call_json(&[x.j()], y.j())); y }), &calls);
            line(&format!("#{k}"), &[("old", obj(&[("@p0@", v.j())])), ("now", obj(&[("result", items.j())])), ("fin", obj(&[("@p0@", it.collect::<Vec<i32>>().j())])), ("runs", obj(&[("@p1@", arr(&groups))]))]);
        }"##),
        // the items an iterator has not given end with it
        "std::vec::IntoIter" => b(r##"for k in 0..3usize {
            let mut it = vec![E { k: 0, id: 1 }, E { k: 0, id: 2 }, E { k: 0, id: 3 }].into_iter();
            let given: Vec<E> = (0..k).filter_map(|_| it.next()).collect();
            let left: Vec<u32> = it.as_slice().iter().map(|e| e.id).collect();
            let given_ids: Vec<u32> = given.iter().map(|e| e.id).collect();
            take_drops();
            drop(it);
            let dropped = take_drops();
            line(&format!("#{k}"), &[("items", left.j()), ("given", given_ids.j()), ("dropped", dropped.j())]);
        }"##),
        // moves
        "std::cmp::max" | "std::cmp::min" | "std::cmp::Ord::max" | "std::cmp::Ord::min" => {
            let call = match name {
                "std::cmp::max" => "std::cmp::max(a, b)",
                "std::cmp::min" => "std::cmp::min(a, b)",
                "std::cmp::Ord::max" => "Ord::max(a, b)",
                _ => "Ord::min(a, b)",
            };
            b(&r##"for (k, (k1, k2)) in [(1, 1), (1, 2), (2, 1)].into_iter().enumerate() {
            let (a, b) = (E { k: k1, id: 10 }, E { k: k2, id: 20 });
            let (ja, jb) = (a.j(), b.j());
            let r = @call@;
            line(&format!("#{k}"), &[("now", obj(&[("@p0@", ja), ("@p1@", jb), ("result", r.j())]))]);
        }"##.replace("@call@", call))
        }
        // moves and ends together: the identities at entry, those after the call, and the ids dropped during it
        "std::mem::replace" => b(r##"{
            let mut d = E { k: 5, id: 10 };
            let s = E { k: 5, id: 20 };
            let (jd, js) = (d.j(), s.j());
            take_drops();
            let r = std::mem::replace(&mut d, s);
            let dropped = take_drops();
            line("#0", &[("old", obj(&[("@p0@", jd), ("@p1@", js)])), ("now", obj(&[("@p0@", d.j()), ("result", r.j())])),
                ("entry", obj(&[("@p0@", "10".into()), ("@p1@", "20".into())])), ("dropped", dropped.j())]);
        }"##),
        "std::mem::swap" => b(r##"{
            let (mut a, mut b) = (E { k: 0, id: 1 }, E { k: 0, id: 2 });
            take_drops();
            std::mem::swap(&mut a, &mut b);
            let dropped = take_drops();
            line("#0", &[("entry", obj(&[("@p0@", "1".into()), ("@p1@", "2".into())])), ("dropped", dropped.j())]);
        }"##),
        "std::mem::drop" => b(r##"{
            let v = E { k: 0, id: 1 };
            take_drops();
            std::mem::drop(v);
            let dropped = take_drops();
            line("#0", &[("entry", obj(&[("@p0@", "1".into())])), ("dropped", dropped.j())]);
        }"##),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Judging

#[derive(Default)]
struct Verdicts {
    ok: usize,
    undecided: Vec<String>,
    unsupported: Vec<String>,
    contradictions: Vec<String>,
}

impl Verdicts {
    fn err(&mut self, id: &str, e: EvalErr) {
        match e {
            EvalErr::Undecided(w) => self.undecided.push(format!("{id}: {w}")),
            e => self.unsupported.push(format!("{id}: {}", e.text())),
        }
    }
}

fn env_side(j: &Json) -> Result<HashMap<String, Value>, String> {
    j.as_object().into_iter().flatten().map(|(k, v)| Ok((k.clone(), from_json(v)?))).collect()
}

fn calls_of(scn: &Json, name: &str) -> Option<Vec<Call>> {
    calls_from(scn["calls"].get(name)?)
}

fn calls_from(list: &Json) -> Option<Vec<Call>> {
    list.as_array()?
        .iter()
        .map(|c| {
            Some(Call { args: c["args"].as_array()?.iter().map(|a| from_json(a).ok()).collect::<Option<_>>()?, result: c.get("result").and_then(|r| from_json(r).ok()) })
        })
        .collect()
}

/// `hold p : moved from q` as an expression.
fn hold_moved(text: &str) -> Option<String> {
    let rest = text.strip_prefix("hold ")?;
    let (p, q) = rest.split_once(':')?;
    let q = q.trim().strip_prefix("moved from ")?;
    Some(format!("{} moved from {}", p.trim(), q.trim()))
}

fn lifetime_names(text: &str) -> Option<(Vec<String>, bool, bool)> {
    let rest = text.strip_prefix("lifetime ")?;
    let (names, survive, has_when) = if let Some(n) = rest.strip_suffix(" survive call") {
        (n, true, false)
    } else {
        let n = rest.split_once(" ends in call")?;
        (n.0, false, !n.1.trim().is_empty())
    };
    Some((names.split(',').map(|s| s.trim().to_string()).collect(), survive, has_when))
}

fn kind_of(c: &Json, has_callbacks: bool) -> Option<&'static str> {
    let (k, t) = (c["kind"].as_str()?, c["text"].as_str()?);
    match k {
        "callback" => Some("callback"),
        "protocol" => Some("protocol"),
        "hold" if t.contains("moved from") => Some("moved from"),
        "hold" if t.starts_with("lifetime") && (t.contains(" survive call") || t.contains(" ends in call") || t.starts_with("lifetime every item of")) => Some("lifetime"),
        "ensure" if t.contains("moved from") => Some("moved from"),
        "ensure" if t.contains("yields(") => Some("yields"),
        "ensure" if has_callbacks => Some("callback"),
        "result must be used" => Some("must be used"),
        _ => None,
    }
}

/// Judge one clause against all scenarios of its subject.
fn judge(clause: &Json, scenarios: &[Json], shared: &mut [Env], binders: &[String]) -> Verdicts {
    let mut v = Verdicts::default();
    let text = clause["text"].as_str().unwrap_or("");
    let kind = clause["kind"].as_str().unwrap_or("");
    for (i, scn) in scenarios.iter().enumerate() {
        let id = scn["id"].as_str().unwrap_or("?").to_string();
        let env = &mut shared[i];
        match kind {
            "callback" if text.contains("called by") => match parse_called_by(text) {
                Err(e) => v.unsupported.push(format!("{id}: {e}")),
                Ok(None) => v.unsupported.push(format!("{id}: not a `called by` clause")),
                Ok(Some(c)) => match scn["runs"].get(&c.name).and_then(Json::as_array) {
                    None => v.unsupported.push(format!("{id}: the probe records no run of `{}`", c.name)),
                    Some(groups) => {
                        let groups: Vec<Vec<Call>> = groups.iter().map(|g| calls_from(g).unwrap_or_default()).collect();
                        match check_called_by(&c, env, &groups) {
                            Verdict::Ok => v.ok += 1,
                            Verdict::Violation(w) => v.contradictions.push(format!("{id}: {w}")),
                            Verdict::Undecided(w) => v.unsupported.push(format!("{id}: {w}")),
                        }
                    }
                },
            },
            "callback" => match parse_callback(text) {
                Err(e) => v.unsupported.push(format!("{id}: {e}")),
                Ok(c) => match calls_of(scn, &c.name) {
                    None => v.unsupported.push(format!("{id}: the probe records no calls of `{}`", c.name)),
                    Some(calls) => match check_callback(&c, env, &calls) {
                        Verdict::Ok => v.ok += 1,
                        Verdict::Violation(w) => v.contradictions.push(format!("{id}: {w}")),
                        Verdict::Undecided(w) => v.unsupported.push(format!("{id}: {w}")),
                    },
                },
            },
            "protocol" => match parse_protocol(text) {
                Err(e) => v.unsupported.push(format!("{id}: {e}")),
                Ok(p) => {
                    let evs: Vec<String> = scn["events"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect();
                    let known: Vec<&str> = scn["known"].as_array().into_iter().flatten().filter_map(Json::as_str).collect();
                    let mut named = vec![];
                    crate::callback::events_of(&p, &mut named);
                    if let Some(u) = named.iter().find(|n| !known.contains(&n.as_str()) && !["begin", "end"].contains(&n.as_str())) {
                        v.unsupported.push(format!("{id}: the probe records no event `{u}`"));
                    } else if follows(&p, &evs) {
                        v.ok += 1;
                    } else {
                        v.contradictions.push(format!("{id}: the calls {evs:?} do not follow the protocol"));
                    }
                }
            },
            "hold" | "ensure" => {
                if text.starts_with("lifetime every item of") {
                    let items: Vec<u64> = scn["items"].as_array().into_iter().flatten().filter_map(Json::as_u64).collect();
                    let given: Vec<u64> = scn["given"].as_array().into_iter().flatten().filter_map(Json::as_u64).collect();
                    let dropped: Vec<u64> = scn["dropped"].as_array().into_iter().flatten().filter_map(Json::as_u64).collect();
                    if scn["items"].is_null() {
                        v.unsupported.push(format!("{id}: the probe records no items"));
                    } else if let Some(i) = items.iter().find(|i| !dropped.contains(i)) {
                        v.contradictions.push(format!("{id}: item {i}, not yet given, did not end with the iterator (dropped {dropped:?})"));
                    } else if let Some(g) = given.iter().find(|g| dropped.contains(g)) {
                        v.contradictions.push(format!("{id}: item {g}, already given, ended with the iterator"));
                    } else {
                        v.ok += 1;
                    }
                    continue;
                }
                if let Some((names, survive, has_when)) = lifetime_names(text) {
                    if has_when {
                        v.unsupported.push(format!("{id}: a `when` condition"));
                        continue;
                    }
                    let dropped: Vec<u64> = scn["dropped"].as_array().into_iter().flatten().filter_map(Json::as_u64).collect();
                    for n in &names {
                        match scn["entry"][n.as_str()].as_u64() {
                            None => v.unsupported.push(format!("{id}: the probe has no entity for `{n}`")),
                            Some(e) if survive && dropped.contains(&e) => v.contradictions.push(format!("{id}: `{n}` ended in the call, the clause says it survives")),
                            Some(e) if !survive && !dropped.contains(&e) => v.contradictions.push(format!("{id}: `{n}` did not end in the call (dropped {dropped:?})")),
                            Some(_) => v.ok += 1,
                        }
                    }
                    continue;
                }
                let src = if kind == "hold" { hold_moved(text).unwrap_or_default() } else { clause_body(text).to_string() };
                match holds(&src, env) {
                    Ok(true) => v.ok += 1,
                    Ok(false) => v.contradictions.push(format!("{id}: false of the run: now {}, old {}", scn["now"], scn["old"])),
                    Err(EvalErr::Unbound(n)) if binders.contains(&n) => v.contradictions.push(format!("{id}: the clause uses `{n}`, which no call of its callback gave (a bug in the clause)")),
                    Err(e) => v.err(&id, e),
                }
            }
            _ => {}
        }
    }
    v
}

/// Settle a clause from its verdicts into the report.
fn settle(rep: &mut RuntimeReport, kind: &str, who: &str, v: Verdicts) {
    let s = rep.stat(kind);
    if !v.contradictions.is_empty() {
        s.contradictions.push(format!("{who}: {}", v.contradictions.join("; ")));
    } else if v.ok > 0 && v.unsupported.is_empty() {
        s.checked += 1;
    } else {
        let why = v.unsupported.first().or(v.undecided.first()).cloned().unwrap_or_else(|| "no scenario decided it".into());
        s.unchecked.push((who.to_string(), why));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Compiling and running

fn scratch(work: &Path, name: &str) -> R<std::path::PathBuf> {
    let dir = work.join(name);
    fs::create_dir_all(dir.join("src")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&dir.to_string_lossy(), &mut h);
    let manifest = format!("[package]\nname = \"libcheck_{name}_{:x}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n", std::hash::Hasher::finish(&h));
    fs::write(dir.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// The scenarios each probe printed, by probe index; a probe that panicked or printed nothing has none.
fn run_program(work: &Path, target: &Path, probes: &mut Vec<(String, Probe)>, rep: &mut RuntimeReport) -> R<Vec<Vec<Json>>> {
    let dir = scratch(work, "runtime")?;
    for attempt in 0..4 {
        let mut src = String::from(PRELUDE);
        let mut ranges = vec![];
        let mut line = src.lines().count() + 1;
        for (i, (_, p)) in probes.iter().enumerate() {
            let code = format!("fn probe_{i}() {{\n{}\n}}\n", p.body);
            let n = code.lines().count();
            ranges.push((line, line + n - 1));
            line += n;
            src.push_str(&code);
        }
        src.push_str("fn main() {\n    std::panic::set_hook(Box::new(|_| {}));\n");
        for i in 0..probes.len() {
            src.push_str(&format!("    println!(\"@@PROBE {i}\");\n    if std::panic::catch_unwind(probe_{i}).is_err() {{ println!(\"@@PANIC {i}\"); }}\n"));
        }
        src.push_str("}\n");
        fs::write(dir.join("src/main.rs"), &src).map_err(|e| e.to_string())?;
        let run = cargo::diagnostics(&dir, target, "build")?;
        if !run.success {
            let mut failing: Vec<usize> = vec![];
            for d in run.diags.iter().filter(|d| d.is_error()) {
                match ranges.iter().position(|(a, b)| d.file == "src/main.rs" && *a <= d.line && d.line <= *b) {
                    Some(i) => failing.push(i),
                    None => return Err(format!("the runtime harness did not compile ({}:{} {})", d.file, d.line, d.message)),
                }
            }
            if failing.is_empty() || attempt == 3 {
                return Err(format!("the runtime harness did not compile: {}", run.stderr));
            }
            failing.sort_unstable();
            failing.dedup();
            for i in failing.into_iter().rev() {
                let (who, _) = probes.remove(i);
                rep.notes.push(format!("the probe for {who} did not compile and was dropped"));
            }
            continue;
        }
        let out = Command::new("cargo")
            .args(["run", "-q"])
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", target)
            .env("CARGO_TERM_COLOR", "never")
            .output()
            .map_err(|e| format!("cargo run: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut by_probe: Vec<Vec<Json>> = vec![vec![]; probes.len()];
        let (mut cur, mut panicked) = (0usize, vec![false; probes.len()]);
        for l in text.lines() {
            if let Some(i) = l.strip_prefix("@@PROBE ") {
                cur = i.trim().parse().unwrap_or(0);
            } else if let Some(i) = l.strip_prefix("@@PANIC ") {
                panicked[i.trim().parse::<usize>().unwrap_or(0)] = true;
            } else if let Some(j) = l.strip_prefix("@@") {
                by_probe[cur].push(serde_json::from_str(j).map_err(|e| format!("probe output `{j}`: {e}"))?);
            }
        }
        for (i, p) in panicked.iter().enumerate() {
            if *p {
                by_probe[i].clear();
                rep.notes.push(format!("the probe for {} panicked and was dropped", probes[i].0));
            }
        }
        return Ok(by_probe);
    }
    Err("the runtime harness did not compile".into())
}

// ---------------------------------------------------------------------------------------------------------------
// must be used

/// (type to name, constructor) of a `#[must_use]` type or trait; a probe function returns it by value and the call is discarded.
fn must_use_type(name: &str) -> Option<(&'static str, &'static str)> {
    let it = "std::slice::Iter<'static, u8>";
    Some(match name {
        "std::str::Chars" => ("std::str::Chars<'static>", "\"ab\".chars()"),
        "std::iter::Chain" => ("std::iter::Chain<std::slice::Iter<'static, u8>, std::slice::Iter<'static, u8>>", "A.iter().chain(A.iter())"),
        "std::iter::Enumerate" => ("std::iter::Enumerate<std::slice::Iter<'static, u8>>", "A.iter().enumerate()"),
        "std::iter::Filter" => ("std::iter::Filter<std::slice::Iter<'static, u8>, fn(&&u8) -> bool>", "A.iter().filter((|_| true) as fn(&&u8) -> bool)"),
        "std::iter::Map" => ("std::iter::Map<std::slice::Iter<'static, u8>, fn(&u8) -> u8>", "A.iter().map((|x| *x) as fn(&u8) -> u8)"),
        "std::iter::Skip" => ("std::iter::Skip<std::slice::Iter<'static, u8>>", "A.iter().skip(1)"),
        "std::iter::Take" => ("std::iter::Take<std::slice::Iter<'static, u8>>", "A.iter().take(1)"),
        "std::iter::TakeWhile" => ("std::iter::TakeWhile<std::slice::Iter<'static, u8>, fn(&&u8) -> bool>", "A.iter().take_while((|_| true) as fn(&&u8) -> bool)"),
        "std::iter::Zip" => ("std::iter::Zip<std::slice::Iter<'static, u8>, std::slice::Iter<'static, u8>>", "A.iter().zip(A.iter())"),
        "std::iter::Copied" => ("std::iter::Copied<std::slice::Iter<'static, u8>>", "A.iter().copied()"),
        "std::iter::FlatMap" => ("std::iter::FlatMap<std::slice::Iter<'static, u8>, Option<u8>, fn(&u8) -> Option<u8>>", "A.iter().flat_map((|x| Some(*x)) as fn(&u8) -> Option<u8>)"),
        "std::slice::Iter" => (it, "A.iter()"),
        "std::result::Result" => ("Result<u8, u8>", "Ok(1)"),
        "std::iter::Iterator" => ("Box<dyn Iterator<Item = u8>>", "Box::new(A.iter().copied())"),
        _ => return None,
    })
}

/// A statement that makes and discards the result of a function.
fn must_use_call(name: &str) -> Option<&'static str> {
    Some(match name {
        "std::mem::drop" => "std::mem::drop(1);",
        "std::cmp::max" => "std::cmp::max(1, 2);",
        "std::cmp::min" => "std::cmp::min(1, 2);",
        "std::cmp::Ord::max" => "Ord::max(1, 2);",
        "std::cmp::Ord::min" => "Ord::min(1, 2);",
        "std::mem::replace" => "let mut a = 1; std::mem::replace(&mut a, 2);",
        "std::option::Option::is_none" => "Some(1).is_none();",
        "std::option::Option::is_some" => "Some(1).is_some();",
        "std::primitive::slice::len" => "let v = [1, 2]; v[..].len();",
        "std::primitive::slice::split_at_mut" => "let mut v = [1, 2]; v[..].split_at_mut(1);",
        "std::primitive::str::is_empty" => "\"a\".is_empty();",
        "std::string::String::len" => "String::new().len();",
        "std::primitive::char::is_whitespace" => "'a'.is_whitespace();",
        "std::primitive::f64::abs" => "1.0f64.abs();",
        "std::primitive::usize::saturating_sub" => "1usize.saturating_sub(2);",
        "std::primitive::u64::wrapping_mul" => "1u64.wrapping_mul(2);",
        "std::collections::HashMap::new" => "std::collections::HashMap::<u8, u8>::new();",
        "std::collections::HashMap::with_capacity" => "std::collections::HashMap::<u8, u8>::with_capacity(1);",
        "std::iter::Iterator::collect" => "A.iter().collect::<Vec<_>>();",
        "std::convert::From::from" => "<u64 as From<u8>>::from(1u8);",
        "std::hash::Hasher::finish" => "use std::hash::Hasher; std::collections::hash_map::DefaultHasher::new().finish();",
        _ => return None,
    })
}

/// Compile `#[deny(unused_must_use)]` probes: a clause that says a result must be used is confirmed when discarding it is
/// an error of that lint.
fn check_must_use(file: &Json, work: &Path, target: &Path, rep: &mut RuntimeReport) -> R<()> {
    let mut src = String::from("#![allow(warnings)]\nconst A: &[u8] = &[1, 2];\n");
    let mut probes: Vec<(usize, String, String)> = vec![];
    let mut line = 3;
    for sub in file["subjects"].as_array().into_iter().flatten() {
        let name = sub["name"].as_str().unwrap_or("");
        for c in sub["clauses"].as_array().into_iter().flatten().filter(|c| c["kind"] == "result must be used") {
            let who = format!("{name}: {}", c["text"].as_str().unwrap_or(""));
            let code = match sub["kind"].as_str() {
                Some("function") => must_use_call(name).map(|s| format!("#[deny(unused_must_use)] fn _p{line}() {{ {s} }}")),
                Some("type" | "trait") => must_use_type(name).map(|(t, ctor)| format!("fn _m{line}() -> {t} {{ {ctor} }} #[deny(unused_must_use)] fn _p{line}() {{ _m{line}(); }}")),
                _ => None,
            };
            let Some(code) = code else {
                rep.stat("must be used").unchecked.push((who, "no probe for this item".into()));
                continue;
            };
            if let Some(e) = first_error(&parse_rust(&code)) {
                rep.stat("must be used").unchecked.push((who, format!("the probe does not parse at {e}")));
                continue;
            }
            probes.push((line, who, code.clone()));
            src.push_str(&code);
            src.push('\n');
            line += 1;
        }
    }
    if probes.is_empty() {
        return Ok(());
    }
    let dir = scratch(work, "mustuse")?;
    fs::write(dir.join("src/lib.rs"), &src).map_err(|e| e.to_string())?;
    let run = cargo::diagnostics(&dir, target, "check")?;
    for (l, who, _) in &probes {
        let ds: Vec<_> = run.diags.iter().filter(|d| d.file == "src/lib.rs" && d.line == *l && d.is_error()).collect();
        let s = rep.stat("must be used");
        if ds.iter().any(|d| d.code.as_deref() == Some("unused_must_use")) {
            s.checked += 1;
        } else if let Some(d) = ds.first() {
            s.unchecked.push((who.clone(), format!("the probe did not compile: {}", d.message)));
        } else {
            s.contradictions.push(format!("{who}: rustc does not warn when the result is discarded"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The command

/// Run the sampled runtime checks over a library file. `work` is a scratch directory and `target` the cargo target.
pub fn run(file: &Json, work: &Path, target: &Path) -> R<RuntimeReport> {
    let mut rep = RuntimeReport::default();
    for k in KINDS {
        rep.stat(k);
    }
    check_must_use(file, work, target, &mut rep)?;
    // subjects with a probe, and the clauses they are judged by
    let mut probes: Vec<(String, Probe)> = vec![];
    let mut subjects: Vec<&Json> = vec![];
    for sub in file["subjects"].as_array().into_iter().flatten() {
        let has_cb = sub["clauses"].as_array().into_iter().flatten().any(|c| c["kind"] == "callback");
        let mine = sub["clauses"].as_array().into_iter().flatten().any(|c| kind_of(c, has_cb).is_some_and(|k| k != "must be used"));
        if !mine {
            continue;
        }
        let name = sub["name"].as_str().unwrap_or("").to_string();
        match probe_for(sub) {
            Some(p) => {
                probes.push((name, p));
                subjects.push(sub);
            }
            None => {
                for c in sub["clauses"].as_array().into_iter().flatten() {
                    if let Some(k) = kind_of(c, has_cb).filter(|k| *k != "must be used") {
                        rep.stat(k).unchecked.push((format!("{name}: {}", c["text"].as_str().unwrap_or("")), "no probe for this item".into()));
                    }
                }
            }
        }
    }
    if probes.is_empty() {
        return Ok(rep);
    }
    let results = run_program(work, target, &mut probes, &mut rep)?;
    // probes may have been dropped: match the rest by name
    for (sub, _) in subjects.iter().map(|s| (*s, ())) {
        let name = sub["name"].as_str().unwrap_or("");
        let has_cb = sub["clauses"].as_array().into_iter().flatten().any(|c| c["kind"] == "callback");
        let idx = probes.iter().position(|(n, _)| n == name);
        let scenarios: &[Json] = idx.map_or(&[], |i| &results[i]);
        let clauses: Vec<&Json> = sub["clauses"].as_array().into_iter().flatten().filter(|c| kind_of(c, has_cb).is_some_and(|k| k != "must be used")).collect();
        if scenarios.is_empty() {
            for c in clauses {
                let k = kind_of(c, has_cb).unwrap_or("callback");
                rep.stat(k).unchecked.push((format!("{name}: {}", c["text"].as_str().unwrap_or("")), "the probe did not run".into()));
            }
            continue;
        }
        let binders: Vec<String> = clauses
            .iter()
            .filter(|c| c["kind"] == "callback")
            .filter_map(|c| parse_callback(c["text"].as_str()?).ok())
            .flat_map(|c| c.returning.into_iter().chain(c.with.iter().filter_map(|e| if let crate::seqexpr::Ex::Name(n) = e { Some(n.clone()) } else { None }).collect::<Vec<_>>()))
            .collect();
        let mut envs: Vec<Env> = vec![];
        let mut bad_env = None;
        for s in scenarios {
            match (env_side(&s["now"]), env_side(&s["old"])) {
                (Ok(n), Ok(o)) => {
                    let mut env = Env::new(n, o);
                    match env_side(&s["fin"]) {
                        Ok(f) => env.fin = f,
                        Err(e) => bad_env = Some(e),
                    }
                    envs.push(env);
                }
                (Err(e), _) | (_, Err(e)) => bad_env = Some(e),
            }
        }
        if let Some(e) = bad_env {
            return Err(format!("{name}: a probe printed a value this checker cannot read: {e}"));
        }
        // callbacks first: the names they bind are in scope in the others
        let mut order: Vec<&Json> = clauses.iter().copied().filter(|c| c["kind"] == "callback").collect();
        order.extend(clauses.iter().copied().filter(|c| c["kind"] != "callback"));
        for c in order {
            let k = kind_of(c, has_cb).unwrap_or("callback");
            let v = judge(c, scenarios, &mut envs, &binders);
            settle(&mut rep, k, &format!("{name}: {}", c["text"].as_str().unwrap_or("")), v);
        }
    }
    Ok(rep)
}
