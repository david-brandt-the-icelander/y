//! Callback clauses (language 6.13): their parse, and the check of a recorded run against them; protocols as regular
//! expressions over the events of a call. The names `with` and `returning` give are bound in the environment for the
//! subject's other clauses.
use crate::seq::Value;
use crate::seqexpr::{eval, parse, Env, EvalErr, Ex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Count {
    ExactlyOnce,
    AtMostOnce,
    AtLeastOnce,
}

#[derive(Clone, Debug)]
pub struct CallbackClause {
    pub name: String,
    pub count: Count,
    pub when: Option<Ex>,
    pub with: Vec<Ex>,
    pub returning: Option<String>,
}

/// One recorded call of a callback.
#[derive(Clone, Debug)]
pub struct Call {
    pub args: Vec<Value>,
    pub result: Option<Value>,
}

#[derive(Debug, PartialEq)]
pub enum Verdict {
    Ok,
    /// A bug in the subject: the callback was called other than the clause says.
    Violation(String),
    /// The clause cannot be decided from this run.
    Undecided(String),
}

fn split_top(s: &str) -> Vec<String> {
    let (mut depth, mut cur, mut out) = (0i32, String::new(), vec![]);
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

/// `callback f : called exactly once [before return] [when p] [, with a, b] [, returning r]`.
pub fn parse_callback(text: &str) -> Result<CallbackClause, String> {
    let rest = text.strip_prefix("callback").ok_or("not a callback clause")?.trim_start();
    let (name, rest) = rest.split_once(':').ok_or("`callback name : called …`")?;
    let segs = split_top(rest.trim());
    let head = segs[0].strip_prefix("called").ok_or("`called` expected")?.trim();
    let (count, tail) = [("exactly once", Count::ExactlyOnce), ("at most once", Count::AtMostOnce), ("at least once", Count::AtLeastOnce)]
        .into_iter()
        .find_map(|(p, c)| head.strip_prefix(p).map(|t| (c, t.trim())))
        .ok_or_else(|| format!("a count this checker does not know: `{head}`"))?;
    let tail = tail.strip_prefix("before return").map_or(tail, str::trim_start);
    let when = match tail.strip_prefix("when") {
        Some(p) => Some(parse(p.trim())?),
        None if tail.is_empty() => None,
        None => return Err(format!("`{tail}` after the count")),
    };
    let (mut with, mut returning, mut in_with) = (vec![], None, false);
    for s in &segs[1..] {
        if let Some(w) = s.strip_prefix("with ") {
            in_with = true;
            with.push(parse(w.trim())?);
        } else if let Some(r) = s.strip_prefix("returning ") {
            in_with = false;
            returning = Some(r.trim().to_string());
        } else if in_with {
            with.push(parse(s)?);
        } else {
            return Err(format!("`{s}` in a callback clause"));
        }
    }
    Ok(CallbackClause { name: name.trim().to_string(), count, when, with, returning })
}

fn holds(e: &Ex, env: &mut Env) -> Result<bool, EvalErr> {
    match eval(e, env)? {
        Value::Bool(b) => Ok(b),
        v => Err(EvalErr::Unsupported(format!("a condition that is {v:?}"))),
    }
}

/// Check the calls recorded of a callback against its clause; the names `with` and `returning` give are bound in `env.now`
/// when the callback was called.
pub fn check_callback(c: &CallbackClause, env: &mut Env, calls: &[Call]) -> Verdict {
    let cond = match &c.when {
        None => true,
        Some(w) => match holds(w, env) {
            Ok(b) => b,
            Err(e) => return Verdict::Undecided(e.text()),
        },
    };
    let n = calls.len();
    if !cond && n > 0 {
        return Verdict::Violation(format!("`{}` was called {n} time(s) where its condition does not hold", c.name));
    }
    if cond {
        let bad = match c.count {
            Count::ExactlyOnce => n != 1,
            Count::AtMostOnce => n > 1,
            Count::AtLeastOnce => n < 1,
        };
        if bad {
            return Verdict::Violation(format!("`{}` was called {n} time(s) where its condition holds and the clause says {:?}", c.name, c.count));
        }
    }
    if n != 1 {
        return Verdict::Ok;
    }
    let call = &calls[0];
    if !c.with.is_empty() {
        if c.with.len() != call.args.len() {
            return Verdict::Violation(format!("`{}` was called with {} arguments, the clause names {}", c.name, call.args.len(), c.with.len()));
        }
        for (e, a) in c.with.iter().zip(&call.args) {
            if let Ex::Name(nm) = e {
                if !env.has(nm) {
                    env.now.insert(nm.clone(), a.clone());
                    continue;
                }
            }
            match eval(e, env) {
                Ok(v) => match v.eq_dec(a) {
                    Some(true) => {}
                    Some(false) => return Verdict::Violation(format!("`{}` was called with {a:?}, not with {e:?} = {v:?}", c.name)),
                    None => return Verdict::Undecided("the argument is an infinite sequence".into()),
                },
                Err(e) => return Verdict::Undecided(e.text()),
            }
        }
    }
    if let (Some(r), Some(res)) = (&c.returning, &call.result) {
        env.now.insert(r.clone(), res.clone());
    }
    Verdict::Ok
}

// ---------------------------------------------------------------------------------------------------------------
// Calls a result makes later (language 6.13.1)

#[derive(Clone, Debug)]
pub struct CalledBy {
    pub name: String,
    /// `result.next`: the path and method whose calls make the run.
    pub by: String,
    /// The count per call of the method; none: any number.
    pub per_call: Option<Count>,
    pub with: Vec<String>,
    pub returning: Option<String>,
}

/// `callback c : called by P.m [, exactly once per call | at most once per call] [, with a, …] [, returning r]`; none
/// for a callback clause that is not of this form.
pub fn parse_called_by(text: &str) -> Result<Option<CalledBy>, String> {
    let Some(rest) = text.strip_prefix("callback") else { return Ok(None) };
    let Some((name, rest)) = rest.split_once(':') else { return Ok(None) };
    let segs = split_top(rest.trim());
    let Some(by) = segs[0].strip_prefix("called by ") else { return Ok(None) };
    let (mut per_call, mut with, mut returning, mut in_with) = (None, vec![], None, false);
    for s in &segs[1..] {
        if s == "exactly once per call" {
            per_call = Some(Count::ExactlyOnce);
        } else if s == "at most once per call" {
            per_call = Some(Count::AtMostOnce);
        } else if let Some(w) = s.strip_prefix("with ") {
            in_with = true;
            with.push(w.trim().to_string());
        } else if let Some(r) = s.strip_prefix("returning ") {
            in_with = false;
            returning = Some(r.trim().to_string());
        } else if in_with {
            with.push(s.trim().to_string());
        } else {
            return Err(format!("`{s}` in a `called by` clause"));
        }
    }
    Ok(Some(CalledBy { name: name.trim().to_string(), by: by.trim().to_string(), per_call, with, returning }))
}

/// Check the run: the calls the callback received during each call of `P.m`, in order. `with` and `returning` bind lists
/// (one item per call of the callback) in `env.now`.
pub fn check_called_by(c: &CalledBy, env: &mut Env, groups: &[Vec<Call>]) -> Verdict {
    for (i, g) in groups.iter().enumerate() {
        let bad = match c.per_call {
            Some(Count::ExactlyOnce) => g.len() != 1,
            Some(Count::AtMostOnce) => g.len() > 1,
            _ => false,
        };
        if bad {
            return Verdict::Violation(format!("`{}` was called {} time(s) by call {i} of `{}`, the clause says {:?} per call", c.name, g.len(), c.by, c.per_call));
        }
    }
    let all: Vec<&Call> = groups.iter().flatten().collect();
    for (k, w) in c.with.iter().enumerate() {
        if let Some(bad) = all.iter().find(|call| call.args.len() != c.with.len()) {
            return Verdict::Violation(format!("`{}` was called with {} arguments, the clause names {}", c.name, bad.args.len(), c.with.len()));
        }
        env.now.insert(w.clone(), Value::list(all.iter().map(|call| call.args[k].clone()).collect()));
    }
    if let Some(r) = &c.returning {
        env.now.insert(r.clone(), Value::list(all.iter().filter_map(|call| call.result.clone()).collect()));
    }
    Verdict::Ok
}

// ---------------------------------------------------------------------------------------------------------------
// Protocols

#[derive(Clone, Debug, PartialEq)]
pub enum Proto {
    Ev(String),
    Seq(Vec<Proto>),
    Alt(Vec<Proto>),
    Star(Box<Proto>),
    Plus(Box<Proto>),
    Opt(Box<Proto>),
}

struct PP {
    t: Vec<String>,
    i: usize,
}

impl PP {
    fn alt(&mut self) -> Result<Proto, String> {
        let mut alts = vec![self.seq()?];
        while self.t.get(self.i).is_some_and(|t| t == "|") {
            self.i += 1;
            alts.push(self.seq()?);
        }
        Ok(if alts.len() == 1 { alts.remove(0) } else { Proto::Alt(alts) })
    }

    fn seq(&mut self) -> Result<Proto, String> {
        let mut items = vec![];
        while let Some(t) = self.t.get(self.i) {
            if t == "|" || t == ")" {
                break;
            }
            let mut a = if t == "(" {
                self.i += 1;
                let inner = self.alt()?;
                if self.t.get(self.i).is_none_or(|t| t != ")") {
                    return Err("`)` expected".into());
                }
                self.i += 1;
                inner
            } else {
                self.i += 1;
                Proto::Ev(t.clone())
            };
            while let Some(q) = self.t.get(self.i).filter(|q| matches!(q.as_str(), "*" | "+" | "?")) {
                a = match q.as_str() {
                    "*" => Proto::Star(Box::new(a)),
                    "+" => Proto::Plus(Box::new(a)),
                    _ => Proto::Opt(Box::new(a)),
                };
                self.i += 1;
            }
            items.push(a);
        }
        Ok(Proto::Seq(items))
    }
}

/// `protocol self.next+`, or the expression alone.
pub fn parse_protocol(text: &str) -> Result<Proto, String> {
    let body = text.strip_prefix("protocol").unwrap_or(text).trim();
    let (mut toks, mut cur) = (vec![], String::new());
    for c in body.chars() {
        if c.is_alphanumeric() || c == '_' || c == '.' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            if !c.is_whitespace() {
                if !"*+?|()".contains(c) {
                    return Err(format!("`{c}` in a protocol"));
                }
                toks.push(c.to_string());
            }
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    let mut p = PP { t: toks, i: 0 };
    let r = p.alt()?;
    if p.i < p.t.len() {
        return Err("unbalanced `)`".into());
    }
    Ok(r)
}

/// The event names a protocol mentions.
pub fn events_of(p: &Proto, out: &mut Vec<String>) {
    match p {
        Proto::Ev(e) => {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        Proto::Seq(v) | Proto::Alt(v) => v.iter().for_each(|x| events_of(x, out)),
        Proto::Star(x) | Proto::Plus(x) | Proto::Opt(x) => events_of(x, out),
    }
}

fn step(p: &Proto, evs: &[String], from: &std::collections::BTreeSet<usize>) -> std::collections::BTreeSet<usize> {
    use std::collections::BTreeSet;
    match p {
        Proto::Ev(e) => from.iter().filter(|&&i| evs.get(i).is_some_and(|x| x == e)).map(|i| i + 1).collect(),
        Proto::Seq(v) => v.iter().fold(from.clone(), |acc, x| step(x, evs, &acc)),
        Proto::Alt(v) => v.iter().flat_map(|x| step(x, evs, from)).collect(),
        Proto::Opt(x) => from.union(&step(x, evs, from)).copied().collect(),
        Proto::Star(x) => {
            let mut all: BTreeSet<usize> = from.clone();
            loop {
                let next: BTreeSet<usize> = all.union(&step(x, evs, &all)).copied().collect();
                if next == all {
                    return all;
                }
                all = next;
            }
        }
        Proto::Plus(x) => step(&Proto::Star(x.clone()), evs, &step(x, evs, from)),
    }
}

/// Whether the events, once those the protocol does not name are left out, follow it from the first to the last.
pub fn follows(p: &Proto, events: &[String]) -> bool {
    let mut named = vec![];
    events_of(p, &mut named);
    let evs: Vec<String> = events.iter().filter(|e| named.contains(e)).cloned().collect();
    step(p, &evs, &[0usize].into_iter().collect()).contains(&evs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocols_are_regular_expressions_over_events() {
        let ev = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let p = parse_protocol("protocol self.next+").unwrap();
        assert!(follows(&p, &ev(&["begin", "self.next", "self.next", "end"])));
        assert!(!follows(&p, &ev(&["begin", "end"])));
        let p = parse_protocol("left right").unwrap();
        assert!(follows(&p, &ev(&["left", "other", "right"])));
        assert!(!follows(&p, &ev(&["right", "left"])));
        let p = parse_protocol("(a | b)* c?").unwrap();
        assert!(follows(&p, &ev(&["a", "b", "a", "c"])) && follows(&p, &[]) && !follows(&p, &ev(&["c", "a"])));
    }

    #[test]
    fn a_called_by_clause_binds_logs() {
        let c = parse_called_by("callback f : called by result.next, at most once per call, with x, returning y").unwrap().unwrap();
        assert_eq!((c.by.as_str(), c.per_call, c.with.clone(), c.returning.clone()), ("result.next", Some(Count::AtMostOnce), vec!["x".to_string()], Some("y".to_string())));
        let call = |a: i64, r: i64| Call { args: vec![Value::Int(a)], result: Some(Value::Int(r)) };
        let mut env = Env::default();
        assert_eq!(check_called_by(&c, &mut env, &[vec![call(1, 2)], vec![call(2, 4)], vec![]]), Verdict::Ok);
        assert_eq!(env.now["y"].as_seq().unwrap().len(), Some(2));
        assert!(matches!(check_called_by(&c, &mut env, &[vec![call(1, 2), call(2, 3)]]), Verdict::Violation(_)));
        assert!(parse_called_by("callback f : called exactly once").unwrap().is_none());
    }

    #[test]
    fn a_callback_clause_parses_and_checks() {
        let c = parse_callback("callback f : called exactly once when exists self, with self, returning r").unwrap();
        assert_eq!((c.count, c.returning.as_deref(), c.with.len()), (Count::ExactlyOnce, Some("r"), 1));
        let mut env = Env::new([("self".to_string(), Value::some(Value::Int(4)))].into(), Default::default());
        let call = Call { args: vec![Value::Int(4)], result: Some(Value::Int(9)) };
        assert_eq!(check_callback(&c, &mut env, std::slice::from_ref(&call)), Verdict::Ok);
        assert_eq!(env.now["r"].as_int(), Some(9));
        assert!(matches!(check_callback(&c, &mut env, &[]), Verdict::Violation(_)));
        let bad = Call { args: vec![Value::Int(5)], result: None };
        assert!(matches!(check_callback(&c, &mut env, &[bad]), Verdict::Violation(_)));
    }
}
