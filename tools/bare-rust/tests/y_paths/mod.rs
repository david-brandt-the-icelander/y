//! The execution paths through each function of a Y module, under the path rules of Y § 3.5 and § 3.14. There is no Y
//! compiler yet; this is what its path enumeration would give, written over the Y text as the examples use it.
//!
//! A path is the sequence of named outcomes one call passes through, written as breadcrumbs: `own_pieces > placed'many >
//! theirs`.
//!
//! - `if a = (p):` / `else b:`: the outcome is `a` or `b`, each split by the predicate's sub-outcomes.
//! - `conditional l:` / `condition l.x = (p):` / `else l.y:`: the outcome is the condition taken, split by the
//!   sub-outcomes of its own predicate. A condition found false on the way has no name of its own, and is not split,
//!   but what its falseness shows is known on the rest of the path.
//! - `compare l = a with b:` / `condition l.x = less`, `equal`, `greater`, or two of them joined by `or`: a
//!   `conditional` over the order of `a` and `b`. A condition that covers two orders splits by them.
//! - A comparison of ordered values (numbers, durations, instants) splits into `'lt`, `'eq` and `'gt` on the branch it
//!   falls on. A part that is the only one possible on that branch is not written.
//! - `and` and `or` split by the operand at which evaluation stopped: `b'1`, `b'2`. On the branch where every operand
//!   was evaluated, the operands' own parts are written by position: `a'2eq`.
//! - `for x in e:` and `while x = (p):` end in `else y` or `else y:`, taken when the loop never runs, unless it is
//!   proven that it runs. When it runs, the path names how often, `x` (once) or `x'many` (more than once), followed by
//!   the path of one of its passes: the pass that leaves the loop, by `return` or `fail`, or else any pass, and then the
//!   steps after the loop.
//! - A call with `or fail` adds a path that ends there: `name'fails`, named by the name it binds, or by the call.
//! - `fail not_reachable` and `fail not_implemented` end a path that needs no test.
//!
//! What a path shows is carried along it as facts: the interval of a number, or of a list's count. They come from the
//! branches taken, the conditions found false, `where` preconditions, the `ensures` of a called function, `assert`s and
//! constants. A sub-outcome the facts rule out is no path: that is what "proven" means here.
use regex::Regex;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

macro_rules! re {
    ($s:expr) => {{
        static R: LazyLock<Regex> = LazyLock::new(|| Regex::new($s).unwrap());
        &*R
    }};
}

const ORDERED: [&str; 9] = ["int32", "uint32", "uint16", "int64", "uint64", "duration", "instant", "count", "int"];
const UNSIGNED: [&str; 5] = ["uint32", "uint16", "uint64", "duration", "count"];
/// How a path that needs no test ends.
pub const EXEMPT: [&str; 2] = ["not_reachable", "not_implemented"];
const ORDERS3: [&str; 3] = ["lt", "eq", "gt"];

// ---------------------------------------------------------------------------------------------------------------------
// Text at the top level of an expression

/// (byte index, char, depth) for the characters outside strings.
fn scan(e: &str) -> Vec<(usize, char, i32)> {
    let (mut depth, mut quoted, mut out) = (0, false, vec![]);
    for (k, c) in e.char_indices() {
        if c == '"' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        if c == '(' || c == '[' {
            depth += 1;
        } else if c == ')' || c == ']' {
            depth -= 1;
        }
        out.push((k, c, depth));
    }
    out
}

fn strip_parens(e: &str) -> String {
    let mut e = e.trim().to_string();
    while e.starts_with('(') {
        let close = scan(&e).into_iter().find(|&(_, c, d)| c == ')' && d == 0).map(|x| x.0);
        if close != Some(e.len() - 1) {
            break;
        }
        e = e[1..e.len() - 1].trim().to_string();
    }
    e
}

fn split_top(e: &str, sep: &str) -> Vec<String> {
    let (mut parts, mut last) = (vec![], 0);
    for (k, _, d) in scan(e) {
        if d == 0 && k >= last && e[k..].starts_with(sep) {
            parts.push(e[last..k].trim().to_string());
            last = k + sep.len();
        }
    }
    parts.push(e[last..].trim().to_string());
    parts
}

fn split_at(e: &str, op: &str) -> Option<(String, String)> {
    scan(e).into_iter().find(|&(k, _, d)| d == 0 && e[k..].starts_with(op)).map(|(k, _, _)| (e[..k].trim().to_string(), e[k + op.len()..].trim().to_string()))
}

fn top_comparison(e: &str) -> Option<(String, String, String)> {
    for (k, _, d) in scan(e) {
        if d == 0 {
            for op in [" <= ", " >= ", " == ", " != ", " < ", " > "] {
                if e[k..].starts_with(op) {
                    return Some((e[..k].trim().to_string(), op.trim().to_string(), e[k + op.len()..].trim().to_string()));
                }
            }
        }
    }
    None
}

fn has_top_arith(e: &str) -> bool {
    let b = e.as_bytes();
    scan(e).into_iter().any(|(k, c, d)| d == 0 && 0 < k && k + 1 < e.len() && b[k - 1] == b' ' && "+-*/%".contains(c) && b[k + 1] == b' ')
}

fn split_commas(e: &str) -> Vec<String> {
    let (mut parts, mut last) = (vec![], 0);
    for (k, c, d) in scan(e) {
        if d == 0 && c == ',' {
            parts.push(e[last..k].to_string());
            last = k + 1;
        }
    }
    parts.push(e[last..].to_string());
    parts.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// An expression as facts are kept on it: a count converted to another whole number is the same count.
fn norm(e: &str) -> String {
    let e = strip_parens(e);
    match re!(r"^(.+\.count\(\))\.to_(?:u?int32)\(\)$").captures(&e) {
        Some(m) => m[1].to_string(),
        None => e,
    }
}

fn cap(m: &regex::Captures, k: usize) -> String {
    m.get(k).map_or(String::new(), |x| x.as_str().to_string())
}

// ---------------------------------------------------------------------------------------------------------------------
// Intervals, with None for no bound

type I = (Option<i64>, Option<i64>);
const ANY: I = (None, None);

fn meet(a: I, b: I) -> I {
    let lo = match (a.0, b.0) {
        (x, None) | (None, x) => x,
        (Some(x), Some(y)) => Some(x.max(y)),
    };
    let hi = match (a.1, b.1) {
        (x, None) | (None, x) => x,
        (Some(x), Some(y)) => Some(x.min(y)),
    };
    (lo, hi)
}

fn empty(i: I) -> bool {
    matches!(i, (Some(lo), Some(hi)) if lo > hi)
}

fn shift(i: I, c: i64) -> I {
    (i.0.map(|x| x + c), i.1.map(|x| x + c))
}

/// The orders a value in interval `a` may stand in to one in `b`.
fn order_possible(a: I, b: I) -> BTreeSet<&'static str> {
    let mut out = BTreeSet::new();
    if !matches!((a.0, b.1), (Some(x), Some(y)) if x >= y) {
        out.insert("lt");
    }
    if !empty(meet(a, b)) {
        out.insert("eq");
    }
    if !matches!((a.1, b.0), (Some(x), Some(y)) if x <= y) {
        out.insert("gt");
    }
    out
}

fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// A module's declarations

type Env = HashMap<String, String>;

pub struct Module {
    lines: Vec<String>,
    fields: HashMap<String, String>,
    constants: HashMap<String, String>,
    values: HashMap<String, String>,
    results: HashMap<String, (Option<String>, Option<String>)>,
    params: HashMap<String, Vec<(String, Option<String>)>>,
    contracts: HashMap<String, Vec<(String, String)>>,
    /// a refined type's constraints on its values' counts: {type: [(op, n)]}
    refinements: HashMap<String, Vec<(String, i64)>>,
    groups: HashSet<String>,
}

impl Module {
    pub fn new(text: &str) -> Module {
        let mut m = Module {
            lines: text.split('\n').map(str::to_string).collect(),
            fields: HashMap::new(),
            constants: HashMap::new(),
            values: HashMap::new(),
            results: HashMap::new(),
            params: HashMap::new(),
            contracts: HashMap::new(),
            refinements: HashMap::new(),
            groups: HashSet::new(),
        };
        m.collect();
        m.groups = m.constants.keys().map(|k| k.split('.').next().unwrap().to_string()).collect();
        m
    }

    fn collect(&mut self) {
        let raw = self.lines.clone();
        let (mut lines, mut k) = (vec![], 0);
        while k < raw.len() {
            let mut l = raw[k].clone();
            k += 1;
            while l.matches('[').count() > l.matches(']').count() && k < raw.len() {
                l = format!("{} {}", l.trim_end(), raw[k].trim());
                k += 1;
            }
            lines.push(l);
        }
        let (mut group, mut kind, mut doc, mut in_doc, mut current): (Option<String>, Option<String>, Vec<String>, bool, Option<String>) = (None, None, vec![], false, None);
        let mut refined: Option<String> = None;
        for l in &lines {
            let t = l.trim();
            if re!(r"^-{3,}$").is_match(t) {
                in_doc = !in_doc;
                if in_doc {
                    doc.clear();
                }
                continue;
            }
            if in_doc {
                doc.push(t.to_string());
                continue;
            }
            if let Some(m) = re!(r"^(?:public )?(constants|structure|type) (\w+)").captures(t) {
                kind = Some(cap(&m, 1));
                group = Some(cap(&m, 2));
                refined = if re!(r"^(?:public )?type \w+ is ").is_match(t) { group.clone() } else { None };
                continue;
            }
            if let Some(r) = &refined {
                for m in re!(r"value\.count\(\) (>=|>|<=|<|==) (\d+)").captures_iter(t) {
                    self.refinements.entry(r.clone()).or_default().push((cap(&m, 1), m[2].parse().unwrap()));
                }
            }
            if let Some(m) = re!(r"^(\w+) is ([\w<>, ]+?)(?: = (.*))?$").captures(t) {
                if let (Some(g), true) = (&group, l.starts_with(' ')) {
                    let (name, ty) = (cap(&m, 1), cap(&m, 2));
                    if kind.as_deref() == Some("structure") {
                        self.fields.entry(name).or_insert(ty);
                    } else if kind.as_deref() == Some("constants") {
                        self.constants.insert(format!("{g}.{name}"), ty);
                        if let Some(v) = m.get(3) {
                            self.values.insert(format!("{g}.{name}"), v.as_str().to_string());
                        }
                    }
                    continue;
                }
            }
            if let Some(m) = re!(r"^(\w+) = (.*)$").captures(t) {
                if let (Some(g), Some("constants"), true) = (&group, kind.as_deref(), l.starts_with(' ')) {
                    self.constants.insert(format!("{g}.{}", &m[1]), g.clone());
                    continue;
                }
            }
            if let Some(m) = re!(r"^function (\w+)\(([^)]*)\)(?: returns (\w+))?").captures(t) {
                group = None;
                let name = cap(&m, 1);
                current = Some(name.clone());
                let mut types = HashMap::new();
                for d in &doc {
                    if let Some(dm) = re!(r"^(\w+): .*, as ([\w<>, ]+)\.$").captures(d) {
                        types.insert(cap(&dm, 1), cap(&dm, 2));
                    }
                }
                let names: Vec<String> = m[2].split(',').filter(|p| !p.trim().is_empty()).map(|p| p.split_whitespace().last().unwrap().to_string()).collect();
                self.params.insert(name.clone(), names.iter().map(|p| (p.clone(), types.get(p).cloned())).collect());
                let result = m.get(3).map(|r| r.as_str().to_string());
                let rtype = result.as_ref().and_then(|r| types.get(r).cloned());
                self.results.insert(name.clone(), (result, rtype));
                self.contracts.insert(name, vec![]);
                continue;
            }
            if let Some(m) = re!(r"^(where|ensures) (\w+) = (.*?):?$").captures(t) {
                if let (Some(c), true) = (&current, l.starts_with(' ')) {
                    self.contracts.get_mut(c).unwrap().push((cap(&m, 1), cap(&m, 3)));
                    continue;
                }
            }
            if !t.is_empty() && !l.starts_with(' ') && !t.starts_with("--") {
                group = None;
                current = None;
            }
        }
    }

    /// The constant value of `e`, if it has one.
    fn value(&self, e: &str) -> Option<i64> {
        let e = strip_parens(e);
        if re!(r"^-?\d+$").is_match(&e) {
            return e.parse().ok();
        }
        if let Some(v) = self.values.get(&e) {
            return self.value(v);
        }
        for op in [" + ", " - ", " * ", " / "] {
            if let Some((a, b)) = split_at(&e, op) {
                let (a, b) = (self.value(&a)?, self.value(&b)?);
                return match op.trim() {
                    "+" => Some(a + b),
                    "-" => Some(a - b),
                    "*" => Some(a * b),
                    _ => (b != 0).then(|| floor_div(a, b)),
                };
            }
        }
        None
    }

    /// The constant length of a list, if it has one.
    fn length(&self, e: &str) -> Option<i64> {
        let mut e = strip_parens(e);
        if let Some(v) = self.values.get(&e) {
            e = v.clone();
        }
        if e.starts_with('[') && e.ends_with(']') {
            let inner = e[1..e.len() - 1].trim();
            return Some(if inner.is_empty() { 0 } else { split_commas(inner).len() as i64 });
        }
        None
    }

    fn type_of(&self, e: &str, env: &Env) -> Option<String> {
        let e = strip_parens(e);
        let s = |x: &str| Some(x.to_string());
        if re!(r"^-?\d+$").is_match(&e) {
            return s("int");
        }
        if re!(r"^\d+(ms|s)$").is_match(&e) {
            return s("duration");
        }
        if e.starts_with('"') {
            return s("string");
        }
        if e == "true" || e == "false" || e.starts_with("not ") {
            return s("boolean");
        }
        if let Some(rest) = e.strip_prefix('-') {
            return self.type_of(rest, env);
        }
        if has_top_arith(&e) {
            for op in [" + ", " - ", " * ", " / ", " % "] {
                if let Some((l, r)) = split_at(&e, op) {
                    let (a, b) = (self.type_of(&l, env), self.type_of(&r, env));
                    if a.as_deref() == Some("instant") && b.as_deref() == Some("instant") {
                        return s("duration");
                    }
                    return if a.is_some() && a.as_deref() != Some("int") { a } else if b.is_some() { b } else { a };
                }
            }
        }
        if split_top(&e, " and ").len() > 1 || split_top(&e, " or ").len() > 1 || top_comparison(&e).is_some() {
            return s("boolean");
        }
        if e.ends_with(".count()") {
            return s("count");
        }
        if e == "clock.now()" {
            return s("instant");
        }
        for (method, ty) in [(".to_int32()", "int32"), (".to_uint32()", "uint32"), (".whole_seconds()", "uint32"), (".is_empty()", "boolean")] {
            if e.ends_with(method) {
                return s(ty);
            }
        }
        if re!(r"^.+\.contains\(.*\)$").is_match(&e) {
            return s("boolean");
        }
        if let Some(m) = re!(r"^(.+)\.divided_rounding_up\(.*\)$").captures(&e).or_else(|| re!(r"^(.+)\.(?:min|max)\(.*\)$").captures(&e)) {
            return self.type_of(&m[1], env);
        }
        let element = re!(r"^(.+)\.(?:largest|smallest|last)\(\)$").captures(&e).or_else(|| re!(r"^(.+)\.at_number\(.*\)$").captures(&e)).or_else(|| re!(r"^(.+)\[[^\]]*\]$").captures(&e));
        if let Some(m) = element {
            let inner = self.type_of(&m[1], env).unwrap_or_default();
            return re!(r"^list<(.+)>$").captures(&inner).map(|mm| cap(&mm, 1));
        }
        if let Some(m) = re!(r"^(\w+)\(.*\)$").captures(&e) {
            if let Some((_, ty)) = self.results.get(&m[1]) {
                return ty.clone();
            }
        }
        if let Some(t) = env.get(&e) {
            return Some(t.clone());
        }
        if let Some(t) = self.constants.get(&e) {
            return Some(t.clone());
        }
        if let Some(m) = re!(r"^[\w.\[\]0-9]+\.(\w+)$").captures(&e) {
            if let Some(t) = self.fields.get(&m[1]) {
                return Some(t.clone());
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Facts along a path: intervals of numbers, booleans (0 or 1) and counts

#[derive(Clone, Default)]
struct Facts {
    known: BTreeMap<String, I>,
    /// values with no order: what an expression is known to be, or known not to be
    same: BTreeMap<String, String>,
    differs: BTreeMap<String, BTreeSet<String>>,
}

/// Whether `key` reads `name`: the name, not inside a longer name or after a `.`.
fn reads(key: &str, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    key.match_indices(name).any(|(k, _)| {
        let before = key[..k].chars().next_back();
        let after = key[k + name.len()..].chars().next();
        !before.is_some_and(|c| word(c) || c == '.' || c == '\'') && !after.is_some_and(word)
    })
}

impl Facts {
    fn is_constant(&self, m: &Module, e: &str) -> bool {
        e.starts_with("optional.") || e.starts_with("result.") || (m.groups.contains(e.split('.').next().unwrap()) && !m.values.contains_key(e))
    }

    /// Adds that `left` is, or is not, `right`, for values with no order: false when it cannot be.
    fn equality(&mut self, m: &Module, left: &str, right: &str, truth: bool) -> bool {
        let (mut left, mut right) = (left, right);
        if self.is_constant(m, left) && !self.is_constant(m, right) {
            (left, right) = (right, left);
        }
        if !self.is_constant(m, right) {
            return true;
        }
        if truth {
            if self.same.get(left).is_some_and(|s| s != right) || self.differs.get(left).is_some_and(|d| d.contains(right)) {
                return false;
            }
            self.same.insert(left.to_string(), right.to_string());
            return true;
        }
        if self.same.get(left).is_some_and(|s| s == right) {
            return false;
        }
        self.differs.entry(left.to_string()).or_default().insert(right.to_string());
        true
    }

    fn interval(&self, m: &Module, env: &Env, e: &str) -> I {
        let e = norm(e);
        if let Some(v) = m.value(&e) {
            return (Some(v), Some(v));
        }
        if e == "true" || e == "false" {
            let v = i64::from(e == "true");
            return (Some(v), Some(v));
        }
        if let Some(n) = re!(r"^(.+)\.count\(\)$").captures(&e).and_then(|c| m.length(&c[1])) {
            return (Some(n), Some(n));
        }
        let mut i = self.known.get(&e).copied().unwrap_or(ANY);
        if let Some(c) = re!(r"^(.+)\.(?:runes\(\)\.)?count\(\)$").captures(&e) {
            let counted = cap(&c, 1);
            // a refined type bounds its values' counts
            if let Some(ty) = m.type_of(&counted, env) {
                for (op, n) in m.refinements.get(&ty).into_iter().flatten() {
                    let n = *n;
                    i = meet(i, match op.as_str() {
                        ">=" => (Some(n), None),
                        ">" => (Some(n + 1), None),
                        "<=" => (None, Some(n)),
                        "<" => (None, Some(n - 1)),
                        _ => (Some(n), Some(n)),
                    });
                }
            }
            if let Some(text) = counted.strip_suffix(".runes()") {
                i = meet(i, self.interval(m, env, &format!("{text}.count()")));
            }
            // the standard library: splitting text gives at least one piece
            if re!(r"^.+\.split\(.*\)$").is_match(&counted) {
                i = meet(i, (Some(1), None));
            }
        }
        match m.type_of(&e, env).as_deref() {
            Some(t) if UNSIGNED.contains(&t) => meet(i, (Some(0), None)),
            Some("boolean") => meet(i, (Some(0), Some(1))),
            _ => i,
        }
    }

    fn add(&mut self, m: &Module, env: &Env, e: &str, i: I) -> bool {
        let e = norm(e);
        let new = meet(self.interval(m, env, &e), i);
        self.known.insert(e, new);
        !empty(new)
    }

    /// Forgets what is known of `name`, and of every expression that reads it.
    fn forget(&mut self, name: &str) {
        self.known.retain(|k, _| !reads(k, name));
        self.same.retain(|k, _| !reads(k, name));
        self.differs.retain(|k, _| !reads(k, name));
    }

    /// A list `name` gains `by` elements, or an unknown number when `by` is None.
    fn grow(&mut self, name: &str, by: Option<i64>) {
        let i = self.known.get(&format!("{name}.count()")).copied();
        self.forget(name);
        if let (Some(i), Some(by)) = (i, by) {
            self.known.insert(format!("{name}.count()"), shift(i, by));
        }
    }

    /// `name` now stands for `rhs`: it takes on what is known of it, and what the function it calls ensures.
    fn bind(&mut self, m: &Module, env: &Env, name: &str, rhs: &str) {
        let rhs = strip_parens(rhs);
        let mut new: BTreeMap<String, I> = BTreeMap::new();
        let mut same = None;
        for (k, v) in &self.known {
            if *k == rhs {
                new.insert(name.to_string(), *v);
            } else if let Some(rest) = k.strip_prefix(&format!("{rhs}.")) {
                new.insert(format!("{name}.{rest}"), *v);
            }
        }
        if let Some(v) = m.value(&rhs) {
            new.insert(name.to_string(), (Some(v), Some(v)));
        }
        if self.is_constant(m, &rhs) {
            same = Some(rhs.clone());
        } else if let Some(s) = self.same.get(&rhs) {
            same = Some(s.clone());
        }
        // arithmetic on what is known
        for (op, sign) in [(" + ", 1), (" - ", -1)] {
            if let Some((a, b)) = split_at(&rhs, op) {
                if let Some(c) = m.value(&b) {
                    new.insert(name.to_string(), shift(self.interval(m, env, &a), sign * c));
                }
            }
        }
        // a negated value, and what the function it calls ensures of it
        if rhs.starts_with('-') && m.value(&rhs).is_none() {
            let mut inner = self.clone();
            inner.bind(m, env, "negated'value", &rhs[1..]);
            let i = inner.interval(m, env, "negated'value");
            new.insert(name.to_string(), (i.1.map(|x| -x), i.0.map(|x| -x)));
        }
        // what the standard library ensures of the lists it makes
        let count = format!("{name}.count()");
        if rhs == "list.new()" || rhs == "[]" {
            new.insert(count.clone(), (Some(0), Some(0)));
        }
        if let Some(c) = re!(r"^random\.shuffled\((.+)\)$").captures(&rhs) {
            new.insert(count.clone(), self.interval(m, env, &format!("{}.count()", &c[1])));
        }
        if let Some(c) = re!(r"^random\.chosen\(from: .+, count: (.+)\)$").captures(&rhs) {
            new.insert(count.clone(), self.interval(m, env, &c[1]));
        }
        if let Some(c) = re!(r"^list\.repeated\(.+, times: (.+)\)$").captures(&rhs) {
            new.insert(count.clone(), self.interval(m, env, &c[1]));
        }
        if rhs == "true" || rhs == "false" {
            let v = i64::from(rhs == "true");
            new.insert(name.to_string(), (Some(v), Some(v)));
        }
        self.forget(name);
        self.known.extend(new);
        if let Some(s) = same {
            self.same.insert(name.to_string(), s);
        }
        if let Some(c) = re!(r"^(\w+)\((.*)\)$").captures(&rhs) {
            let callee = cap(&c, 1);
            let result = m.results.get(&callee).and_then(|r| r.0.clone());
            for (kind, pred) in m.contracts.get(&callee).into_iter().flatten() {
                if let (true, Some(result)) = (kind == "ensures", &result) {
                    let said = Regex::new(&format!(r"\b{}\b", regex::escape(result))).unwrap().replace_all(pred, name).to_string();
                    let alts = assume(m, env, &said, true, self);
                    if !alts.is_empty() {
                        *self = join(&alts);
                    }
                }
            }
        }
    }

    fn possible_orders(&self, m: &Module, env: &Env, left: &str, right: &str) -> Option<BTreeSet<&'static str>> {
        let known: Vec<String> = [m.type_of(left, env), m.type_of(right, env)].into_iter().flatten().collect();
        if known.is_empty() || !known.iter().any(|t| ORDERED.contains(&t.as_str())) {
            return None;
        }
        Some(order_possible(self.interval(m, env, left), self.interval(m, env, right)))
    }

    /// Adds what order `o` of `left` to `right` shows: false when it cannot be.
    fn constrain(&mut self, m: &Module, env: &Env, left: &str, right: &str, o: &str) -> bool {
        let (a, b) = (self.interval(m, env, left), self.interval(m, env, right));
        if !order_possible(a, b).contains(o) {
            return false;
        }
        match o {
            "eq" => self.add(m, env, left, b) && self.add(m, env, right, a),
            "lt" => self.add(m, env, left, (None, b.1.map(|x| x - 1))) && self.add(m, env, right, (a.0.map(|x| x + 1), None)),
            _ => self.add(m, env, left, (b.0.map(|x| x + 1), None)) && self.add(m, env, right, (None, a.1.map(|x| x - 1))),
        }
    }
}

/// What holds in every one of several sets of facts.
fn join(alternatives: &[Facts]) -> Facts {
    let mut first = alternatives[0].clone();
    for other in &alternatives[1..] {
        first.known = first
            .known
            .iter()
            .filter_map(|(k, a)| {
                let b = other.known.get(k)?;
                let lo = match (a.0, b.0) {
                    (Some(x), Some(y)) => Some(x.min(y)),
                    _ => None,
                };
                let hi = match (a.1, b.1) {
                    (Some(x), Some(y)) => Some(x.max(y)),
                    _ => None,
                };
                Some((k.clone(), (lo, hi)))
            })
            .collect();
        first.same.retain(|k, v| other.same.get(k) == Some(v));
        first.differs = first
            .differs
            .iter()
            .filter_map(|(k, v)| {
                let both: BTreeSet<String> = v.intersection(other.differs.get(k)?).cloned().collect();
                (!both.is_empty()).then(|| (k.clone(), both))
            })
            .collect();
    }
    first
}

/// The orders that make a comparison true, and the ones that make it false.
fn orders(op: &str, truth: bool) -> &'static [&'static str] {
    match (op, truth) {
        ("<", true) | (">=", false) => &["lt"],
        ("<", false) | (">=", true) => &["eq", "gt"],
        ("<=", true) | (">", false) => &["lt", "eq"],
        ("<=", false) | (">", true) => &["gt"],
        ("==", true) | ("!=", false) => &["eq"],
        _ => &["lt", "gt"],
    }
}

/// The sub-outcomes of predicate `e` being `truth`: [(suffix, facts)], each with what it shows; [] when it cannot be.
fn outcomes(m: &Module, env: &Env, e: &str, truth: bool, facts: &Facts) -> Vec<(String, Facts)> {
    let e = strip_parens(e);
    if let Some(rest) = e.strip_prefix("not ") {
        return outcomes(m, env, rest, !truth, facts);
    }
    for (word, stops_on) in [(" or ", true), (" and ", false)] {
        let ops = split_top(&e, word);
        if ops.len() < 2 {
            continue;
        }
        if truth == stops_on {
            // evaluation stopped at operand k; the ones before it went the other way
            let (mut out, mut seen) = (vec![], HashSet::new());
            for (k, op) in ops.iter().enumerate() {
                let mut befores = vec![facts.clone()];
                for prior in &ops[..k] {
                    befores = befores.iter().flat_map(|f| outcomes(m, env, prior, !truth, f).into_iter().map(|x| x.1)).collect();
                }
                for f in &befores {
                    for (s, f2) in outcomes(m, env, op, truth, f) {
                        let suffix = format!("'{}{}", k + 1, s.trim_start_matches('\''));
                        if seen.insert(suffix.clone()) {
                            out.push((suffix, f2));
                        }
                    }
                }
            }
            return out;
        }
        let mut states = vec![(String::new(), facts.clone())];
        for (k, op) in ops.iter().enumerate() {
            states = states
                .iter()
                .flat_map(|(s, f)| {
                    outcomes(m, env, op, truth, f).into_iter().map(move |(s2, f2)| {
                        let part = if s2.is_empty() { String::new() } else { format!("'{}{}", k + 1, s2.trim_start_matches('\'')) };
                        (format!("{s}{part}"), f2)
                    })
                })
                .collect();
        }
        return states;
    }
    if let Some((left, op, right)) = top_comparison(&e) {
        let Some(possible) = facts.possible_orders(m, env, &left, &right) else {
            if op == "==" || op == "!=" {
                let mut f = facts.clone();
                return if f.equality(m, &left, &right, truth == (op == "==")) { vec![(String::new(), f)] } else { vec![] };
            }
            return vec![(String::new(), facts.clone())];
        };
        let mut out = vec![];
        for o in ORDERS3 {
            if orders(&op, truth).contains(&o) && possible.contains(o) {
                let mut f = facts.clone();
                if f.constrain(m, env, &left, &right, o) {
                    out.push((format!("'{o}"), f));
                }
            }
        }
        if out.len() == 1 {
            out[0].0 = String::new();
        }
        return out;
    }
    if let Some(c) = re!(r"^(.+)\.is_empty\(\)$").captures(&e) {
        let mut f = facts.clone();
        let i = if truth { (Some(0), Some(0)) } else { (Some(1), None) };
        return if f.add(m, env, &format!("{}.count()", &c[1]), i) { vec![(String::new(), f)] } else { vec![] };
    }
    let mut f = facts.clone();
    if m.type_of(&e, env).as_deref() == Some("boolean") || re!(r"^[\w.]+$").is_match(&e) {
        let v = i64::from(truth);
        if !f.add(m, env, &e, (Some(v), Some(v))) {
            return vec![];
        }
    }
    vec![(String::new(), f)]
}

/// The facts after `e` is found `truth`, one set for each way it can be.
fn assume(m: &Module, env: &Env, e: &str, truth: bool, facts: &Facts) -> Vec<Facts> {
    outcomes(m, env, e, truth, facts).into_iter().map(|x| x.1).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Blocks and statements

#[derive(Clone)]
struct Item {
    text: String,
    kids: Vec<Item>,
}

fn indent(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

fn blocks(lines: &[String]) -> Vec<Item> {
    fn parse(lines: &[String], i: &mut usize, ind: usize) -> Vec<Item> {
        let mut out = vec![];
        while *i < lines.len() {
            let l = &lines[*i];
            let d = indent(l);
            if d < ind {
                break;
            }
            *i += 1;
            let mut text = l.trim().to_string();
            let open = |t: &str| t.matches('(').count() + t.matches('[').count() > t.matches(')').count() + t.matches(']').count();
            while open(&text) && *i < lines.len() {
                text = format!("{text} {}", lines[*i].trim());
                *i += 1;
            }
            let text = text.replace("( ", "(").replace("[ ", "[").replace(", )", ")").replace(", ]", "]");
            let mut kids = vec![];
            if text.ends_with(':') && *i < lines.len() {
                let nd = indent(&lines[*i]);
                if nd > d {
                    kids = parse(lines, i, nd);
                }
            }
            out.push(Item { text, kids });
        }
        out
    }
    let mut i = 0;
    parse(lines, &mut i, lines.first().map_or(0, |l| indent(l)))
}

fn walk(items: &[Item]) -> Vec<&Item> {
    let mut out = vec![];
    for it in items {
        out.push(it);
        out.extend(walk(&it.kids));
    }
    out
}

/// The names a block changes.
fn mutated(items: &[Item]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for it in items {
        let t = it.text.as_str();
        if let Some(c) = re!(r"^(\w+)[\w.\[\]]* := ").captures(t) {
            out.insert(cap(&c, 1));
        }
        if !t.starts_with("mutable ") {
            for c in re!(r"mutable (\w+)").captures_iter(t) {
                out.insert(cap(&c, 1));
            }
        }
        for r in [re!(r"^(\w+)[\w.]*\.(push|extend|set_number|insert_first|clear)\("), re!(r"^(?:mutable )?(\w+) = "), re!(r"^for (\w+) in")] {
            if let Some(c) = r.captures(t) {
                out.insert(cap(&c, 1));
            }
        }
        out.extend(mutated(&it.kids));
    }
    out
}

/// The names among `names` that a block changes only by adding to them, with `push` or `extend`.
fn grown_only(items: &[Item], names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut other = BTreeSet::new();
    for it in walk(items) {
        let t = it.text.as_str();
        for n in names {
            if t.starts_with(&format!("{n}.push(")) || t.starts_with(&format!("{n}.extend(")) {
                continue;
            }
            let n_ = regex::escape(n);
            let assigned = Regex::new(&format!(r"^(mutable )?{n_}\b[\w.\[\]]* :?= ")).unwrap().is_match(t);
            let passed = Regex::new(&format!(r"mutable {n_}\b")).unwrap().is_match(t);
            let changed = Regex::new(&format!(r"^{n_}[\w.]*\.(set_number|insert_first|clear)\(")).unwrap().is_match(t);
            if assigned || passed || changed {
                other.insert(n.clone());
            }
        }
    }
    names.difference(&other).cloned().collect()
}

/// Forgets what passes of a loop may have changed; a list that only grows keeps its least count.
fn widen(facts: &mut Facts, names: &BTreeSet<String>, grown: &BTreeSet<String>) {
    for n in names {
        let keep = if grown.contains(n) { facts.known.get(&format!("{n}.count()")).copied() } else { None };
        facts.forget(n);
        if let Some(keep) = keep {
            facts.known.insert(format!("{n}.count()"), (keep.0, None));
        }
    }
}

fn failing_name(text: &str) -> Option<String> {
    if !re!(r" or fail(:)?$").is_match(text) || ["within ", "if ", "while "].iter().any(|p| text.starts_with(p)) {
        return None;
    }
    if let Some(c) = re!(r"^(?:mutable )?(\w+) :?= ").captures(text) {
        return Some(cap(&c, 1));
    }
    Some(re!(r"^([\w.]+)\(").captures(text).map_or("call".to_string(), |c| c[1].rsplit('.').next().unwrap().to_string()))
}

/// How a path ends, and the named outcomes along it.
type State = (String, Vec<String>, Facts);

const NOT_BOUND: [&str; 7] = ["if ", "while ", "for ", "conditional ", "compare ", "return ", "assert "];

struct Function<'m> {
    module: &'m Module,
    name: String,
    env: RefCell<Env>,
    problems: RefCell<Vec<String>>,
    fails_total: HashMap<String, usize>,
    fails_seen: RefCell<HashMap<String, usize>>,
    order: RefCell<HashMap<(String, String), usize>>,
    paths: Vec<(String, Vec<String>)>,
}

impl<'m> Function<'m> {
    fn new(module: &'m Module, name: &str, body: &[String]) -> Function<'m> {
        let mut env: Env = module.params.get(name).into_iter().flatten().filter_map(|(p, t)| Some((p.clone(), t.clone()?))).collect();
        if let Some((Some(result), Some(rtype))) = module.results.get(name) {
            env.insert(result.clone(), rtype.clone());
        }
        let items = blocks(body);
        let mut fails_total = HashMap::new();
        for it in walk(&items) {
            if let Some(n) = failing_name(&it.text) {
                *fails_total.entry(n).or_insert(0) += 1;
            }
        }
        let mut f = Function {
            module,
            name: name.to_string(),
            env: RefCell::new(env),
            problems: RefCell::new(vec![]),
            fails_total,
            fails_seen: RefCell::new(HashMap::new()),
            order: RefCell::new(HashMap::new()),
            paths: vec![],
        };
        let mut starts = vec![Facts::default()];
        for (kind, pred) in module.contracts.get(name).into_iter().flatten() {
            if kind == "where" {
                let env = f.env.borrow().clone();
                starts = starts.iter().flat_map(|s| assume(module, &env, pred, true, s)).take(1).collect();
            }
        }
        let found = match starts.into_iter().next() {
            Some(s) => f.seq(&items, s),
            None => vec![],
        };
        let mut seen = HashSet::new();
        for (kind, steps, _) in found {
            if seen.insert((kind.clone(), steps.clone())) {
                f.paths.push((kind, steps));
            }
        }
        f
    }

    fn problem(&self, text: String) {
        let mut p = self.problems.borrow_mut();
        if !p.contains(&text) {
            p.push(text);
        }
    }

    fn outcomes(&self, e: &str, truth: bool, facts: &Facts) -> Vec<(String, Facts)> {
        outcomes(self.module, &self.env.borrow(), e, truth, facts)
    }

    fn assume(&self, e: &str, truth: bool, facts: &Facts) -> Vec<Facts> {
        assume(self.module, &self.env.borrow(), e, truth, facts)
    }

    fn note_type(&self, text: &str) {
        let m = self.module;
        if let Some(c) = re!(r"^(?:mutable )?(\w+) = (.*)$").captures(text) {
            if !NOT_BOUND.iter().any(|p| text.starts_with(p)) && !text.starts_with("while ") {
                let rhs = re!(r" or fail$").replace(&c[2], "").to_string();
                let rhs = re!(r#" forever because ".*""#).replace(&rhs, "").to_string();
                let ty = m.type_of(&rhs, &self.env.borrow()).map(|t| if t == "int" { "int32".to_string() } else { t });
                let mut env = self.env.borrow_mut();
                if let (Some(ty), false) = (ty, env.contains_key(&c[1])) {
                    env.insert(cap(&c, 1), ty);
                }
            }
        }
        if let Some(c) = re!(r"^for (\w+) in (.*):$").captures(text) {
            let src = cap(&c, 2);
            let ty = m.type_of(&src, &self.env.borrow()).unwrap_or_default();
            if let Some(mm) = re!(r"^list<(.+)>$").captures(&ty) {
                self.env.borrow_mut().insert(cap(&c, 1), cap(&mm, 1));
            } else if src.starts_with("range(") {
                let from = re!(r"from: ([^,]+)").captures(&src).map(|x| cap(&x, 1)).unwrap_or_default();
                let ty = m.type_of(&from, &self.env.borrow()).unwrap_or_else(|| "int32".to_string());
                self.env.borrow_mut().insert(cap(&c, 1), if ty == "int" { "int32".to_string() } else { ty });
            }
        }
    }

    /// The states a block entered with `facts` ends in.
    fn seq(&self, items: &[Item], facts: Facts) -> Vec<State> {
        let (mut states, mut done, mut k) = (vec![("next".to_string(), vec![], facts)], vec![], 0);
        while k < items.len() {
            self.note_type(&items[k].text);
            let mut j = k + 1;
            while j < items.len() && items[j].text.starts_with("else ") {
                j += 1;
            }
            let mut next = vec![];
            for (_, steps, f) in &states {
                for (kind2, steps2, f2) in self.statement(items, k, f.clone()) {
                    let all = [steps.clone(), steps2].concat();
                    if kind2 == "next" {
                        next.push((kind2, all, f2));
                    } else {
                        done.push((kind2, all, f2));
                    }
                }
            }
            (states, k) = (next, j);
        }
        states.extend(done);
        states
    }

    fn branch(&self, name: &str, pred: &str, truth: bool, body: &[Item], facts: &Facts) -> Vec<State> {
        let mut out = vec![];
        for (s, f) in self.outcomes(pred, truth, facts) {
            for (kind, steps, f2) in self.seq(body, f) {
                out.push((kind, [vec![format!("{name}{s}")], steps].concat(), f2));
            }
        }
        out
    }

    fn statement(&self, items: &[Item], k: usize, facts: Facts) -> Vec<State> {
        let (text, kids) = (items[k].text.as_str(), &items[k].kids);
        let elses: Vec<&Item> = items[k + 1..].iter().take_while(|it| it.text.starts_with("else ")).collect();
        let m = self.module;
        let say = |s: &str| s.to_string();

        if let Some(c) = re!(r"^if (\w+) = (.*):$").captures(text) {
            let (name, pred) = (cap(&c, 1), cap(&c, 2));
            if elses.len() != 1 || !elses[0].text.ends_with(':') {
                self.problem(format!("`if {name}` has no named else block"));
                return self.branch(&name, &pred, true, kids, &facts);
            }
            let other = elses[0].text[5..].trim_end_matches(':').to_string();
            return [self.branch(&name, &pred, true, kids, &facts), self.branch(&other, &pred, false, &elses[0].kids, &facts)].concat();
        }

        if let Some(c) = re!(r"^conditional (\w+)(?: = (.*))?:$").captures(text) {
            if let Some(subject) = c.get(2) {
                let ty = m.type_of(subject.as_str(), &self.env.borrow());
                if let Some(ty) = ty {
                    self.env.borrow_mut().entry(cap(&c, 1)).or_insert(ty);
                }
            }
            let (mut out, mut rest) = (vec![], vec![facts]);
            for it in kids {
                if let Some(cm) = re!(r"^condition ([\w.]+) = (.*):$").captures(&it.text) {
                    for f in &rest {
                        out.extend(self.branch(&cm[1], &cm[2], true, &it.kids, f));
                    }
                    rest = rest.iter().flat_map(|f| self.assume(&cm[2], false, f)).collect();
                    continue;
                }
                if let Some(em) = re!(r"^else ([\w.]+):$").captures(&it.text) {
                    for f in &rest {
                        for (kind, steps, f2) in self.seq(&it.kids, f.clone()) {
                            out.push((kind, [vec![cap(&em, 1)], steps].concat(), f2));
                        }
                    }
                }
            }
            return out;
        }

        if let Some(c) = re!(r"^compare (\w+) = (.*) with (.*):$").captures(text) {
            let (label, a, b) = (cap(&c, 1), cap(&c, 2), cap(&c, 3));
            let possible = facts.possible_orders(m, &self.env.borrow(), &a, &b).unwrap_or_else(|| {
                self.problem(format!("`compare {label}` compares values with no order"));
                ORDERS3.into_iter().collect()
            });
            let word = |w: &str| match w {
                "less" => "lt",
                "equal" => "eq",
                _ => "gt",
            };
            let (mut out, mut seen) = (vec![], BTreeSet::new());
            for it in kids {
                let (ords, name): (Vec<&str>, String) = if let Some(cm) = re!(r"^condition ([\w.]+) = \(?([a-z ]+?)\)?:$").captures(&it.text) {
                    let ords: Vec<&str> = cm[2].split(" or ").map(word).collect();
                    if ords.iter().any(|o| seen.contains(o)) {
                        self.problem(format!("`compare {label}`: an order is in two conditions"));
                    }
                    (ords, cap(&cm, 1))
                } else if let Some(em) = re!(r"^else ([\w.]+):$").captures(&it.text) {
                    (ORDERS3.into_iter().filter(|o| !seen.contains(o)).collect(), cap(&em, 1))
                } else {
                    continue;
                };
                seen.extend(ords.iter().copied());
                let mut side = vec![];
                for o in &ords {
                    let mut f = facts.clone();
                    if possible.contains(o) && f.constrain(m, &self.env.borrow(), &a, &b, o) {
                        side.push((*o, f));
                    }
                }
                let split = side.len() > 1;
                for (o, f) in side {
                    for (kind, steps, f2) in self.seq(&it.kids, f) {
                        let step = if split { format!("{name}'{o}") } else { name.clone() };
                        out.push((kind, [vec![step], steps].concat(), f2));
                    }
                }
            }
            if seen.len() != 3 {
                self.problem(format!("`compare {label}` leaves an order out"));
            }
            return out;
        }

        if let Some(c) = re!(r"^for (\w+) in (.*):$").captures(text) {
            return self.run_loop(&c[1], None, Some(&c[2]), kids, &elses, facts, &format!("for {} in {}", &c[1], &c[2]));
        }

        if let Some(c) = re!(r"^while (\w+) = (.*):$").captures(text) {
            return self.run_loop(&c[1], Some(&c[2]), None, kids, &elses, facts, &format!("while {}", &c[1]));
        }

        if let Some(c) = re!(r"^assert \w+ = (.*)$").captures(text) {
            return self.assume(&c[1], true, &facts).into_iter().map(|f| (say("next"), vec![], f)).collect();
        }

        if text.starts_with("return") {
            return vec![(say("return"), vec![], facts)];
        }
        if text == "fail not_reachable" || text == "fail not_implemented" {
            return vec![(text[5..].to_string(), vec![], facts)];
        }
        if text.starts_with("fail ") {
            return vec![(say("fail"), vec![], facts)];
        }
        if text == "continue" || text == "break" {
            return vec![(text.to_string(), vec![], facts)];
        }

        let mut outs = if !kids.is_empty() && re!(r"^(within |inject\(|inject:)").is_match(text) {
            self.seq(kids, facts.clone())
        } else {
            vec![(say("next"), vec![], facts.clone())]
        };
        if let Some(n) = failing_name(text) {
            let key = (n.clone(), text.to_string());
            let mut order = self.order.borrow_mut();
            if !order.contains_key(&key) {
                let mut seen = self.fails_seen.borrow_mut();
                let count = seen.entry(n.clone()).or_insert(0);
                *count += 1;
                order.insert(key.clone(), *count);
            }
            let label = if self.fails_total[&n] == 1 { n.clone() } else { format!("{n}'{}", order[&key]) };
            outs.push((say("fail"), vec![format!("{label}'fails")], facts.clone()));
        }
        // what the statement changes, and what a new name stands for
        let mut changed = mutated(&[Item { text: text.to_string(), kids: vec![] }]);
        if let Some(g) = re!(r"^(\w+)\.(push|extend)\(").captures(text) {
            for (_, _, f) in &mut outs {
                f.grow(&g[1], if &g[2] == "push" { Some(1) } else { None });
            }
            changed.remove(&g[1]);
        }
        let bound = re!(r"^(?:mutable )?(\w+) :?= (.*?)( or fail)?$").captures(text);
        if let Some(b) = &bound {
            changed.remove(&b[1]);
        }
        let env = self.env.borrow();
        for (kind, _, f) in &mut outs {
            if kind != "next" {
                continue;
            }
            for name in &changed {
                f.forget(name);
            }
            if let (Some(b), false) = (&bound, NOT_BOUND.iter().any(|p| text.starts_with(p))) {
                let rhs = re!(r#" forever because ".*""#).replace(&b[2], "").to_string();
                f.bind(m, &env, &b[1], &rhs);
            }
        }
        outs
    }

    /// The interval of the number of elements a `for` goes over.
    fn count_of(&self, src: &str, facts: &Facts) -> I {
        let m = self.module;
        let env = self.env.borrow();
        if let Some(c) = re!(r"^range\(from: (.+), (below|through): (.+)\)$").captures(src) {
            let (a, b) = (facts.interval(m, &env, &c[1]), facts.interval(m, &env, &c[3]));
            let extra = i64::from(&c[2] == "through");
            if let Some((base, plus)) = split_at(&c[3], " + ") {
                if base == c[1].trim() {
                    if let Some(n) = m.value(&plus) {
                        return (Some(n + extra), Some(n + extra));
                    }
                }
            }
            if let (Some(lo), Some(hi)) = a {
                if lo == hi {
                    return (b.0.map(|x| (x - lo + extra).max(0)), b.1.map(|x| (x - lo + extra).max(0)));
                }
            }
            return (Some(0), None);
        }
        if let Some(n) = m.length(src) {
            return (Some(n), Some(n));
        }
        let mut f = facts.clone();
        f.bind(m, &env, "loop'source", src);
        meet(meet(f.interval(m, &env, "loop'source.count()"), facts.interval(m, &env, &format!("{src}.count()"))), (Some(0), None))
    }

    fn with_count(&self, src: &str, facts: &Facts, interval: I) -> Facts {
        let mut f = facts.clone();
        if !src.starts_with("range(") {
            f.add(self.module, &self.env.borrow(), &format!("{src}.count()"), interval);
        }
        f
    }

    #[allow(clippy::too_many_arguments)]
    fn run_loop(&self, var: &str, pred: Option<&str>, src: Option<&str>, kids: &[Item], elses: &[&Item], facts: Facts, what: &str) -> Vec<State> {
        let mut out = vec![];
        let else_name = elses.first().map(|e| e.text[5..].trim_end_matches(':').to_string());
        let else_body: &[Item] = elses.first().map_or(&[], |e| &e.kids);
        let count = src.map(|s| self.count_of(s, &facts));

        // the loop never runs
        let never = match (pred, src, count) {
            (Some(p), _, _) => self.outcomes(p, false, &facts),
            (None, Some(s), Some(c)) => {
                if c.0.unwrap_or(0) >= 1 {
                    vec![]
                } else {
                    vec![(String::new(), self.with_count(s, &facts, (Some(0), Some(0))))]
                }
            }
            _ => unreachable!(),
        };
        if !never.is_empty() && else_name.is_none() {
            self.problem(format!("`{what}` may never run, and has no named else"));
        }
        if never.is_empty() && else_name.is_some() {
            self.problem(format!("`{what}` runs for sure, and needs no else"));
        }
        if let Some(name) = &else_name {
            for (s, f) in never {
                for (kind, steps, f2) in self.seq(else_body, f) {
                    out.push((kind, [vec![format!("{name}{s}")], steps].concat(), f2));
                }
            }
        }

        // the pass the path shows: the first, or a later one, after which the facts on what the body changes no longer
        // hold
        let changed = mutated(kids);
        let grown = grown_only(kids, &changed);
        for many in [false, true] {
            let mut base = facts.clone();
            if many {
                widen(&mut base, &changed, &grown);
            }
            let runs = if many { (Some(2), None) } else { (Some(1), None) };
            let entries = match (pred, src, count) {
                (Some(p), _, _) => self.outcomes(p, true, &base),
                (None, Some(s), Some(c)) => {
                    if empty(meet(c, runs)) {
                        continue;
                    }
                    vec![(String::new(), self.with_count(s, &base, runs))]
                }
                _ => unreachable!(),
            };
            for (s, mut f) in entries {
                f.forget(var);
                for (kind, steps, mut f2) in self.seq(kids, f) {
                    let goes_on = kind == "next" || kind == "continue";
                    let last = if many { (Some(2), None) } else { (Some(1), Some(1)) };
                    if let (Some(c), true) = (count, goes_on) {
                        // a pass that goes on is the last only when the elements run out
                        if empty(meet(c, last)) {
                            continue;
                        }
                    }
                    let ends = if many {
                        // other passes come before or after this one: what they change is not known after the loop
                        widen(&mut f2, &changed, &grown);
                        vec![f2]
                    } else if let (Some(p), true) = (pred, goes_on) {
                        // the only pass is the last, so the condition is then false
                        self.assume(p, false, &f2).into_iter().take(1).collect()
                    } else {
                        vec![f2]
                    };
                    let kind = if ["next", "continue", "break"].contains(&kind.as_str()) { "next".to_string() } else { kind };
                    for f3 in ends {
                        let step = format!("{var}{}{s}", if many { "'many" } else { "" });
                        out.push((kind.clone(), [vec![step], steps.clone()].concat(), f3));
                    }
                }
            }
        }
        out
    }
}

fn functions(module: &Module) -> Vec<Function<'_>> {
    let lines = &module.lines;
    let (mut i, mut out) = (0, vec![]);
    while i < lines.len() {
        let Some(c) = re!(r"^\s*function (\w+)\(").captures(&lines[i]) else {
            i += 1;
            continue;
        };
        let d = indent(&lines[i]);
        let mut j = i;
        while j < lines.len() && !lines[j].trim_end().ends_with(':') {
            j += 1;
        }
        let mut body = vec![];
        j += 1;
        while j < lines.len() && (lines[j].trim().is_empty() || indent(&lines[j]) > d) {
            if !lines[j].trim().is_empty() && !lines[j].trim().starts_with("--") {
                body.push(lines[j].clone());
            }
            j += 1;
        }
        out.push(Function::new(module, &c[1], &body));
        i = j;
    }
    out
}

/// A path's identifier: the function, then each named outcome along it.
fn path_id(name: &str, steps: &[String]) -> String {
    [vec![name.to_string()], steps.to_vec()].concat().join(" > ")
}

/// {path: how it ends} for every path of every function of a Y module, and {function: problems}. A path ends in `next`
/// or `return` (it returns), `fail`, `continue` or `break` at the top of a function, `not_reachable` or `not_implemented`.
pub fn all_paths(text: &str) -> (BTreeMap<String, String>, BTreeMap<String, Vec<String>>) {
    let module = Module::new(text);
    let (mut paths, mut problems) = (BTreeMap::new(), BTreeMap::new());
    for f in functions(&module) {
        for (kind, steps) in &f.paths {
            paths.insert(path_id(&f.name, steps), kind.clone());
        }
        let p = f.problems.into_inner();
        if !p.is_empty() {
            problems.insert(f.name.clone(), p);
        }
    }
    (paths, problems)
}
