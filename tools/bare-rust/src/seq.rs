//! A reference evaluator for the sequence operators of language 6.15 and the values around them (tuples, optionals,
//! variants, entities), with a JSON encoding so that another host can be run against the same table
//! (`conformance/sequences.json`, see `conformance.rs`).
//!
//! A sequence is finite or infinite: it has an optional length and an element at every position below it. Nothing here
//! fails: an operator with no value on an infinite sequence (`count`, `reverse`) says so, and an equality that no finite
//! prefix refutes is undecided, as the text has it.
use serde_json::{json, Map, Value as Json};
use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

/// Items of an infinite result that the JSON encoding shows.
pub const PREFIX: usize = 8;
/// A finite list longer than this is shown as its prefix and its size (`take(1 << 40, counter)`).
pub const SHOWN: usize = 1024;
/// How far two infinite sequences are compared.
pub const BOUND: usize = 1024;

#[derive(Clone)]
pub struct Seq {
    len: Option<usize>,
    at: Rc<dyn Fn(usize) -> Value>,
}

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Bool(bool),
    /// Text, and a Unicode scalar (a text of one scalar).
    Str(String),
    None,
    Some(Box<Value>),
    Tuple(Vec<Value>),
    List(Seq),
    Variant(String, Vec<Value>),
    /// An entity with a key that `==` and the order see and an identity that only `moved from` sees (language 6.14.3).
    Ent(i64, u64),
    /// The identity of a host reference (`same`).
    Id(u64),
}

impl fmt::Debug for Seq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", to_json(&Value::List(self.clone())))
    }
}

impl Seq {
    pub fn fin(v: Vec<Value>) -> Seq {
        let n = v.len();
        let v = Rc::new(v);
        Seq { len: Some(n), at: Rc::new(move |k| v[k].clone()) }
    }

    /// The infinite sequence `from`, `from + step`, …
    pub fn counter(from: i64, step: i64) -> Seq {
        Seq { len: None, at: Rc::new(move |k| Value::Int(from.wrapping_add(step.wrapping_mul(k as i64)))) }
    }

    pub fn empty() -> Seq {
        Seq::fin(vec![])
    }

    /// `None` for an infinite sequence.
    pub fn len(&self) -> Option<usize> {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == Some(0)
    }

    pub fn get(&self, k: usize) -> Option<Value> {
        self.len.is_none_or(|n| k < n).then(|| (self.at)(k))
    }

    /// `xs[i]`: none outside the sequence.
    pub fn index(&self, i: i64) -> Value {
        if i < 0 {
            return Value::None;
        }
        self.get(i as usize).map_or(Value::None, |v| Value::Some(Box::new(v)))
    }

    pub fn take(&self, n: i64) -> Seq {
        let m = n.max(0) as usize;
        let len = Some(self.len.map_or(m, |l| l.min(m)));
        Seq { len, at: self.at.clone() }
    }

    pub fn skip(&self, n: i64) -> Seq {
        if n <= 0 {
            return self.clone();
        }
        let k = n as usize;
        let at = self.at.clone();
        Seq { len: self.len.map(|l| l.saturating_sub(k)), at: Rc::new(move |j| at(j + k)) }
    }

    /// `xs[i..j]`, `xs[i..]`, `xs[..j]` and `xs[..]`, by the equations of the text.
    pub fn slice(&self, i: Option<i64>, j: Option<i64>) -> Seq {
        match (i, j) {
            (Some(i), Some(j)) => self.skip(i).take(j.saturating_sub(i.max(0))),
            (Some(i), None) => self.skip(i),
            (None, Some(j)) => self.take(j),
            (None, None) => self.clone(),
        }
    }

    /// `xs ++ ys`: for an infinite `xs`, `xs`.
    pub fn concat(&self, ys: &Seq) -> Seq {
        let Some(xl) = self.len else { return self.clone() };
        let (xs, ys2) = (self.at.clone(), ys.at.clone());
        Seq { len: ys.len.map(|y| xl + y), at: Rc::new(move |k| if k < xl { xs(k) } else { ys2(k - xl) }) }
    }

    /// `reverse(xs)`; no value for an infinite one.
    pub fn reverse(&self) -> Option<Seq> {
        let n = self.len?;
        Some(Seq::fin((0..n).rev().map(|k| (self.at)(k)).collect()))
    }

    pub fn map(&self, f: fn(&Value) -> Value) -> Seq {
        let at = self.at.clone();
        Seq { len: self.len, at: Rc::new(move |k| f(&at(k))) }
    }

    /// `filter(p, xs)`. The result of an infinite sequence is taken to be infinite, and a position is found by scanning.
    pub fn filter(&self, p: fn(&Value) -> bool) -> Seq {
        if let Some(n) = self.len {
            return Seq::fin((0..n).map(|k| (self.at)(k)).filter(p).collect());
        }
        let at = self.at.clone();
        Seq {
            len: None,
            at: Rc::new(move |k| {
                let mut seen = 0;
                for j in 0..(1 << 20) {
                    let v = at(j);
                    if p(&v) {
                        if seen == k {
                            return v;
                        }
                        seen += 1;
                    }
                }
                panic!("filter of an infinite sequence: no element at position {k} within {} positions", 1 << 20)
            }),
        }
    }

    /// `present(xs, i)`: position `i` holds an element, whatever the element is.
    pub fn present(&self, i: i64) -> bool {
        i >= 0 && self.len.is_none_or(|n| (i as usize) < n)
    }

    /// `select(bs, xs)`: the elements at the positions where `bs` is true, for the positions both lists have.
    pub fn select(&self, xs: &Seq) -> Seq {
        let both = match (self.len, xs.len) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let (bs, at) = (self.at.clone(), xs.at.clone());
        let yes = move |k: usize| matches!(bs(k), Value::Bool(true));
        if let Some(n) = both {
            return Seq::fin((0..n).filter(|k| yes(*k)).map(|k| at(k)).collect());
        }
        Seq {
            len: None,
            at: Rc::new(move |k| {
                let mut seen = 0;
                for j in 0..(1 << 20) {
                    if yes(j) {
                        if seen == k {
                            return at(j);
                        }
                        seen += 1;
                    }
                }
                panic!("select of infinite lists: no element at position {k}")
            }),
        }
    }

    /// `while_true(bs, xs)`: the elements before the first `false` of `bs`, or as many as `bs` has.
    pub fn while_true(&self, xs: &Seq) -> Seq {
        let n = match self.len {
            Some(l) => (0..l).find(|k| !matches!((self.at)(*k), Value::Bool(true))).unwrap_or(l),
            None => match (0..BOUND).find(|k| !matches!((self.at)(*k), Value::Bool(true))) {
                Some(k) => k,
                None => return xs.clone(),
            },
        };
        xs.take(n as i64)
    }

    /// `flatten(xss)` of a finite list of lists; no value for an infinite outer list.
    pub fn flatten(&self) -> Option<Seq> {
        let n = self.len?;
        let mut out = Seq::empty();
        for k in (0..n).rev() {
            match (self.at)(k) {
                Value::List(x) => out = x.concat(&out),
                _ => return None,
            }
        }
        Some(out)
    }

    pub fn zip(&self, ys: &Seq) -> Seq {
        let len = match (self.len, ys.len) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let (xs, ys) = (self.at.clone(), ys.at.clone());
        Seq { len, at: Rc::new(move |k| Value::Tuple(vec![xs(k), ys(k)])) }
    }

    pub fn enumerate(&self) -> Seq {
        let xs = self.at.clone();
        Seq { len: self.len, at: Rc::new(move |k| Value::Tuple(vec![Value::Int(k as i64), xs(k)])) }
    }

    pub fn scalars(s: &str) -> Seq {
        Seq::fin(s.chars().map(|c| Value::Str(c.to_string())).collect())
    }

    /// The first `n` items (fewer when the sequence ends).
    pub fn prefix(&self, n: usize) -> Vec<Value> {
        (0..n).map_while(|k| self.get(k)).collect()
    }

    pub fn to_vec(&self) -> Option<Vec<Value>> {
        self.len.map(|n| self.prefix(n))
    }
}

/// Equality of sequences: exact when both end; refuted on a prefix otherwise, never confirmed (`None`).
pub fn seq_eq(a: &Seq, b: &Seq) -> Option<bool> {
    match (a.len, b.len) {
        (Some(x), Some(y)) if x != y => return Some(false),
        (Some(x), None) | (None, Some(x)) => {
            // the finite one ends, the other has an element there
            let _ = x;
            return Some(false);
        }
        _ => {}
    }
    let n = a.len.unwrap_or(BOUND);
    let mut undecided = false;
    for k in 0..n {
        match (a.at.as_ref())(k).eq_dec(&(b.at.as_ref())(k)) {
            Some(false) => return Some(false),
            Some(true) => {}
            None => undecided = true,
        }
    }
    if undecided || a.len.is_none() {
        None
    } else {
        Some(true)
    }
}

impl Value {
    pub fn list(v: Vec<Value>) -> Value {
        Value::List(Seq::fin(v))
    }

    pub fn some(v: Value) -> Value {
        Value::Some(Box::new(v))
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(i) = self {
            Some(*i)
        } else {
            None
        }
    }

    pub fn as_seq(&self) -> Option<&Seq> {
        if let Value::List(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// `==`: `None` where no finite prefix decides. An optional is its payload where it exists (`T?` of `T?` is `T?`, as
    /// the text says of `xs[i]`), so `some(x) == x`.
    pub fn eq_dec(&self, other: &Value) -> Option<bool> {
        use Value as V;
        match (self, other) {
            (V::Int(a), V::Int(b)) => Some(a == b),
            (V::Bool(a), V::Bool(b)) => Some(a == b),
            (V::Str(a), V::Str(b)) => Some(a == b),
            (V::None, V::None) => Some(true),
            (V::Some(a), V::Some(b)) => a.eq_dec(b),
            (V::Some(a), o) | (o, V::Some(a)) if !matches!(o, V::None) => a.eq_dec(o),
            (V::None, _) | (_, V::None) => Some(false),
            (V::Tuple(a), V::Tuple(b)) => all_eq(a, b),
            (V::Variant(n, a), V::Variant(m, b)) => {
                if n != m {
                    Some(false)
                } else {
                    all_eq(a, b)
                }
            }
            (V::List(a), V::List(b)) => seq_eq(a, b),
            (V::Ent(a, _), V::Ent(b, _)) => Some(a == b),
            (V::Id(a), V::Id(b)) => Some(a == b),
            _ => Some(false),
        }
    }

    pub fn cmp_dec(&self, other: &Value) -> Option<Ordering> {
        use Value as V;
        match (self, other) {
            (V::Int(a), V::Int(b)) => Some(a.cmp(b)),
            (V::Str(a), V::Str(b)) => Some(a.cmp(b)),
            (V::Ent(a, _), V::Ent(b, _)) => Some(a.cmp(b)),
            (V::Some(a), b) if !matches!(b, V::Some(_)) => a.cmp_dec(b),
            (a, V::Some(b)) if !matches!(a, V::Some(_)) => a.cmp_dec(b),
            _ => None,
        }
    }
}

fn all_eq(a: &[Value], b: &[Value]) -> Option<bool> {
    if a.len() != b.len() {
        return Some(false);
    }
    let mut undecided = false;
    for (x, y) in a.iter().zip(b) {
        match x.eq_dec(y) {
            Some(false) => return Some(false),
            None => undecided = true,
            Some(true) => {}
        }
    }
    if undecided {
        None
    } else {
        Some(true)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Named functions: the catalog the conformance table uses for `map` and `filter` (language 6.15 "A named function")

/// (name, meaning). `deref` and `scalars` are the language's; the rest stand for the views of a file.
pub const NAMED: &[(&str, &str)] = &[
    ("deref", "the value a reference refers to: the identity on values"),
    ("succ", "x + 1"),
    ("double", "x * 2"),
    ("negate", "-x"),
    ("yields", "the items of a collection or iterator value: the identity on a list"),
    ("is_even", "x % 2 == 0"),
    ("is_positive", "x > 0"),
    ("is_zero", "x == 0"),
];

pub fn named_map(name: &str) -> Option<fn(&Value) -> Value> {
    fn int(v: &Value) -> i64 {
        v.as_int().unwrap_or(0)
    }
    Some(match name {
        "deref" | "yields" => |v| v.clone(),
        "succ" => |v| Value::Int(int(v) + 1),
        "double" => |v| Value::Int(int(v) * 2),
        "negate" => |v| Value::Int(-int(v)),
        _ => return None,
    })
}

pub fn named_pred(name: &str) -> Option<fn(&Value) -> bool> {
    Some(match name {
        "is_even" => |v| v.as_int().unwrap_or(0) % 2 == 0,
        "is_positive" => |v| v.as_int().unwrap_or(0) > 0,
        "is_zero" => |v| v.as_int() == Some(0),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// JSON

/// Encoding: numbers, booleans and strings are themselves; `null` is none and `{"some": v}` is some; `{"tuple": […]}`;
/// a finite list is an array; an infinite list is `{"counter": {"from": a, "step": b}}` in an input and
/// `{"prefix": […]}` (its first `PREFIX` items) in a result, with `"size"` when it ends but is longer than `SHOWN`; `{"variant": n, "fields": […]}`; `{"ent": [key, id]}`; `{"id": n}`.
pub fn to_json(v: &Value) -> Json {
    match v {
        Value::Int(i) => json!(i),
        Value::Bool(b) => json!(b),
        Value::Str(s) => json!(s),
        Value::None => Json::Null,
        Value::Some(x) => json!({ "some": to_json(x) }),
        Value::Tuple(t) => json!({ "tuple": t.iter().map(to_json).collect::<Vec<_>>() }),
        Value::List(s) => match s.len() {
            Some(n) if n <= SHOWN => Json::Array(s.prefix(n).iter().map(to_json).collect()),
            Some(n) => json!({ "prefix": s.prefix(PREFIX).iter().map(to_json).collect::<Vec<_>>(), "size": n }),
            None => json!({ "prefix": s.prefix(PREFIX).iter().map(to_json).collect::<Vec<_>>() }),
        },
        Value::Variant(n, f) => json!({ "variant": n, "fields": f.iter().map(to_json).collect::<Vec<_>>() }),
        Value::Ent(k, i) => json!({ "ent": [k, i] }),
        Value::Id(i) => json!({ "id": i }),
    }
}

pub fn from_json(j: &Json) -> Result<Value, String> {
    let list = |a: &Json| -> Result<Vec<Value>, String> { a.as_array().ok_or_else(|| format!("an array is expected: {a}"))?.iter().map(from_json).collect() };
    Ok(match j {
        Json::Number(n) => Value::Int(n.as_i64().ok_or_else(|| format!("not an integer: {n}"))?),
        Json::Bool(b) => Value::Bool(*b),
        Json::String(s) => Value::Str(s.clone()),
        Json::Null => Value::None,
        Json::Array(_) => Value::list(list(j)?),
        Json::Object(o) => {
            if let Some(x) = o.get("some") {
                Value::some(from_json(x)?)
            } else if let Some(t) = o.get("tuple") {
                Value::Tuple(list(t)?)
            } else if let Some(c) = o.get("counter") {
                let n = |k: &str| c[k].as_i64().ok_or_else(|| format!("counter needs `{k}`: {j}"));
                Value::List(Seq::counter(n("from")?, n("step")?))
            } else if let Some(n) = o.get("variant") {
                Value::Variant(n.as_str().ok_or("variant name")?.to_string(), list(o.get("fields").unwrap_or(&json!([])))?)
            } else if let Some(e) = o.get("ent") {
                let a = e.as_array().filter(|a| a.len() == 2).ok_or("ent is [key, id]")?;
                Value::Ent(a[0].as_i64().ok_or("ent key")?, a[1].as_u64().ok_or("ent id")?)
            } else if let Some(i) = o.get("id") {
                Value::Id(i.as_u64().ok_or("id")?)
            } else {
                return Err(format!("not a value: {j}"));
            }
        }
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The operators of the conformance table, by name

fn arg<'a>(args: &'a [Json], i: usize, op: &str) -> Result<&'a Json, String> {
    args.get(i).ok_or_else(|| format!("{op}: argument {i} is missing"))
}

fn seq_arg(args: &[Json], i: usize, op: &str) -> Result<Seq, String> {
    match from_json(arg(args, i, op)?)? {
        Value::List(s) => Ok(s),
        v => Err(format!("{op}: argument {i} is not a list: {v:?}")),
    }
}

fn int_arg(args: &[Json], i: usize, op: &str) -> Result<i64, String> {
    arg(args, i, op)?.as_i64().ok_or_else(|| format!("{op}: argument {i} is not an integer"))
}

fn opt_int(args: &[Json], i: usize, op: &str) -> Result<Option<i64>, String> {
    match arg(args, i, op)? {
        Json::Null => Ok(None),
        j => j.as_i64().map(Some).ok_or_else(|| format!("{op}: argument {i} is not an integer or null")),
    }
}

fn list_json(s: Seq) -> Json {
    to_json(&Value::List(s))
}

fn undefined() -> Json {
    json!({ "undefined": true })
}

/// Evaluate one operator of the table. The operators and their arguments:
/// `index(xs, i)`, `take(n, xs)`, `skip(n, xs)`, `slice(xs, i|null, j|null)`, `concat(xs, ys)`, `reverse(xs)`,
/// `map(f, xs)`, `filter(p, xs)`, `zip(xs, ys)`, `enumerate(xs)`, `scalars(s)`, `deref(v)`, `count(xs)`, `eq(a, b)`,
/// `part(tuple, k)`, `bind(pattern, value)`, `next(xs)`, `next_back(xs)`.
pub fn eval_op(op: &str, args: &[Json]) -> Result<Json, String> {
    Ok(match op {
        "index" => to_json(&seq_arg(args, 0, op)?.index(int_arg(args, 1, op)?)),
        "take" => list_json(seq_arg(args, 1, op)?.take(int_arg(args, 0, op)?)),
        "skip" => list_json(seq_arg(args, 1, op)?.skip(int_arg(args, 0, op)?)),
        "slice" => list_json(seq_arg(args, 0, op)?.slice(opt_int(args, 1, op)?, opt_int(args, 2, op)?)),
        "concat" => list_json(seq_arg(args, 0, op)?.concat(&seq_arg(args, 1, op)?)),
        "reverse" => seq_arg(args, 0, op)?.reverse().map_or_else(undefined, list_json),
        "map" => {
            let name = arg(args, 0, op)?.as_str().unwrap_or("");
            let f = named_map(name).ok_or_else(|| format!("map: `{name}` is not a named function"))?;
            list_json(seq_arg(args, 1, op)?.map(f))
        }
        "filter" => {
            let name = arg(args, 0, op)?.as_str().unwrap_or("");
            let p = named_pred(name).ok_or_else(|| format!("filter: `{name}` is not a named predicate"))?;
            list_json(seq_arg(args, 1, op)?.filter(p))
        }
        "zip" => list_json(seq_arg(args, 0, op)?.zip(&seq_arg(args, 1, op)?)),
        "present" => json!(seq_arg(args, 0, op)?.present(int_arg(args, 1, op)?)),
        "select" => list_json(seq_arg(args, 0, op)?.select(&seq_arg(args, 1, op)?)),
        "while_true" => list_json(seq_arg(args, 0, op)?.while_true(&seq_arg(args, 1, op)?)),
        "flatten" => seq_arg(args, 0, op)?.flatten().map_or_else(undefined, list_json),
        "enumerate" => list_json(seq_arg(args, 0, op)?.enumerate()),
        "scalars" => list_json(Seq::scalars(arg(args, 0, op)?.as_str().ok_or("scalars: a text is expected")?)),
        "deref" => arg(args, 0, op)?.clone(),
        "count" => seq_arg(args, 0, op)?.len().map_or_else(undefined, |n| json!(n)),
        "eq" => match from_json(arg(args, 0, op)?)?.eq_dec(&from_json(arg(args, 1, op)?)?) {
            Some(b) => json!(b),
            None => json!({ "undecided": true }),
        },
        "part" => {
            let (t, k) = (from_json(arg(args, 0, op)?)?, int_arg(args, 1, op)?);
            match t {
                Value::Tuple(parts) if k >= 0 && (k as usize) < parts.len() => to_json(&parts[k as usize]),
                Value::Tuple(parts) => json!({ "refused": format!("a tuple of {} has no component {k}", parts.len()) }),
                _ => json!({ "refused": "not a tuple" }),
            }
        }
        "bind" => {
            let names: Vec<&str> = arg(args, 0, op)?.as_array().ok_or("bind: a pattern is a list of names")?.iter().filter_map(Json::as_str).collect();
            match from_json(arg(args, 1, op)?)? {
                Value::Tuple(parts) if parts.len() == names.len() => {
                    let mut o = Map::new();
                    for (n, v) in names.iter().zip(&parts) {
                        if *n != "_" {
                            o.insert((*n).to_string(), to_json(v));
                        }
                    }
                    Json::Object(o)
                }
                Value::Tuple(parts) => json!({ "refused": format!("a pattern of {} names against a tuple of {}", names.len(), parts.len()) }),
                _ => json!({ "refused": "not a tuple" }),
            }
        }
        // the iterator model: `yields` is the list; `next` returns its first item and leaves the rest
        "next" => {
            let xs = seq_arg(args, 0, op)?;
            json!({ "result": to_json(&xs.index(0)), "yields": list_json(xs.skip(1)) })
        }
        "next_back" => {
            let xs = seq_arg(args, 0, op)?;
            match xs.len() {
                None => undefined(),
                Some(n) => json!({ "result": to_json(&xs.index(n as i64 - 1)), "yields": list_json(xs.take(n as i64 - 1)) }),
            }
        }
        _ => return Err(format!("unknown operator `{op}`")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &[i64]) -> Seq {
        Seq::fin(v.iter().map(|i| Value::Int(*i)).collect())
    }

    #[test]
    fn the_examples_of_the_text() {
        let three = ints(&[1, 2, 3]);
        assert_eq!(three.slice(Some(2), Some(9)).len(), Some(1));
        assert_eq!(three.index(3).eq_dec(&Value::None), Some(true));
        assert_eq!(three.index(-1).eq_dec(&Value::None), Some(true));
        let c = Seq::counter(0, 1);
        assert_eq!(seq_eq(&c.take(3), &ints(&[0, 1, 2])), Some(true));
        assert_eq!(seq_eq(&c, &c.skip(1)), Some(false));
        assert_eq!(seq_eq(&c, &c), None);
        assert!(c.reverse().is_none() && c.len().is_none());
    }
}
