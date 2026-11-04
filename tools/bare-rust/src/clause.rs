//! A light syntactic check of the clause text of a library file: not a parser of language 15, but enough to catch
//! what a prefix check cannot. Brackets balance and quotes close; the clause starts with a keyword of its kind; a host
//! path is quoted (language 15's `STABLE`) and looks like one; a failure named by `fails` is declared in the file; and
//! every other name is a parameter, `result`, `self`, a generic, a variant of a subject of the file, a name the clause
//! binds (`all x`, `is V(x)`), or a word of the expression language; in a module's file (`src/x.rs`), also a constant of
//! the module or a function of it bound `pure` or `effects none`.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// What a clause may name besides the words of the language: the subject's own names and the file's.
#[derive(Default, Clone)]
pub struct Scope {
    /// Parameters, generics, `Self`, and the variants of the file's types.
    pub names: BTreeSet<String>,
    /// The failures the file declares.
    pub failures: BTreeSet<String>,
    /// The payload fields each failure declares: `fails F { field: e } when p` may fill only these.
    pub payload: BTreeMap<String, BTreeSet<String>>,
    /// What a `protocol` may name (language 6.13): the subject's parameters and callbacks.
    pub events: BTreeSet<String>,
    /// The methods a parameter's type is known to have, for `param.method` in a `protocol`; `None` when they cannot be checked.
    pub event_methods: BTreeMap<String, Option<BTreeSet<String>>>,
    /// The named pure functions `map` and `filter` may take (language 6.15): the file's views. The language's own
    /// (`PURE_BUILTINS`) are always allowed.
    pub pure: BTreeSet<String>,
    /// What clauses of one subject tell the clauses after them (language 4 "Narrowing follows order"): shared by every
    /// clone of the scope, so the callback clauses, which are linted with a copy, still tell the others.
    pub flow: Rc<RefCell<Flow>>,
}

/// What an earlier clause of a subject has established for the later ones.
#[derive(Default, Debug)]
pub struct Flow {
    /// The names `let` has bound (language 4: a name is bound once).
    pub lets: BTreeSet<String>,
    /// A clause has spoken of a result's later calls (`called by`, `fails … in`), so `final(e)` has a run to read (language 6.13).
    pub run: bool,
}

/// The pure functions the language itself names (language 6.15): `scalars(string)`, `deref(ref)` and `yields(it)`.
pub const PURE_BUILTINS: &[&str] = &["scalars", "deref", "yields"];

/// The operators of language 6.15 that take parentheses, and how many arguments each takes.
pub const SEQUENCE_OPS: &[(&str, usize)] = &[("yields", 1), ("reverse", 1), ("enumerate", 1), ("scalars", 1), ("deref", 1), ("take", 2), ("skip", 2), ("zip", 2), ("map", 2), ("filter", 2),
    ("present", 2), ("select", 2), ("while_true", 2), ("flatten", 1), ("final", 1), ("current", 1), ("owner", 1), ("stored_key", 2)];

/// Words of language 4 and 6, and of the clause forms that carry no expression.
const WORDS: &[&str] = &[
    "and", "or", "not", "implies", "iff", "if", "then", "else", "exists", "is", "in", "matches", "fits", "finite", "all", "any", "old",
    "result", "self", "true", "false", "none", "value", "count", "scalars", "code_units", "utf8", "utf16", "sum", "min",
    "max", "same", "approx", "total_order", "distinct", "sorted", "by", "subset", "view", "called", "exactly", "at", "most", "least",
    "once", "before", "return", "only", "on", "when", "sign_bit", "is_nan", "payload", "caller", "some", "implements", "borrows",
    "owns", "shares", "from", "for", "as", "moved", "yields", "reverse", "enumerate", "scalars", "deref", "take", "skip", "zip", "map", "filter",
    "present", "select", "while_true", "flatten", "final", "current", "owner", "stored_key", "let",
];

/// Words only a clause of one kind may use: what a `budget` bounds and what a `layout` states (language 6.7, 6.1).
fn kind_words(kind: &str) -> &'static [&'static str] {
    match kind {
        "budget" => &["allocates", "retains", "work", "bytes", "sent", "round_trips", "per", "call", "instance", "frame", "B", "KB", "MB", "O", "p50", "p99", "expected", "amortised"],
        "layout" => &["c", "packed", "transparent", "align"],
        "callback" => &["with", "returning", "by", "per", "call"],
        "hold" => &["ends", "survive", "call", "every", "item", "of", "with", "outlives", "within", "released", "at", "end"],
        "obliges" => &["because"],
        _ => &[],
    }
}

/// Type words of language 3.4, which an associated type or a bound may use.
const TYPE_WORDS: &[&str] = &["int", "scalar", "string", "bytes", "decimal", "rational", "integer", "time", "instant", "duration", "list", "set", "map", "ref"];

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Quoted(String),
    /// `'a`, after `for` or another lifetime: a name here means a lifetime of the subject, not a host path.
    Lifetime(String),
    Punct(char),
    Other,
}

fn lex(t: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = t.chars().collect();
    let (mut i, mut out, mut open): (usize, Vec<Tok>, Vec<char>) = (0, vec![], vec![]);
    while i < cs.len() {
        let c = cs[i];
        let regex = c == '/' && matches!(out.last(), Some(Tok::Word(w)) if w == "matches");
        if c.is_whitespace() {
            i += 1;
        } else if c == '\'' && lifetime_at(&cs, i) && lifetime_context(&out) {
            let n = cs[i + 1..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').count();
            out.push(Tok::Lifetime(cs[i + 1..i + 1 + n].iter().collect()));
            i += n + 1;
        } else if c == '\'' || c == '"' || regex {
            let end = (i + 1..cs.len()).find(|&j| cs[j] == c && cs[j - 1] != '\\').ok_or(format!("unterminated `{c}` in `{t}`"))?;
            out.push(if c == '\'' { Tok::Quoted(cs[i + 1..end].iter().collect()) } else { Tok::Other });
            i = end + 1;
        } else if c.is_alphabetic() || c == '_' {
            let n = cs[i..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').count();
            let w: String = cs[i..i + n].iter().collect();
            if cs[i + n..].starts_with(&[':', ':']) {
                return Err(format!("host path `{w}::…` is not quoted: write it as `'std::…'` (language 15's STABLE)"));
            }
            out.push(Tok::Word(w));
            i += n;
        } else if c.is_ascii_digit() {
            // a number, which may have a fraction but ends before `..`
            while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_' || (cs[i] == '.' && cs.get(i + 1).is_some_and(char::is_ascii_digit))) {
                i += 1;
            }
            out.push(Tok::Other);
        } else {
            if "([{".contains(c) {
                open.push(c);
            } else if let Some(want) = [('(', ')'), ('[', ']'), ('{', '}')].iter().find(|p| p.1 == c).map(|p| p.0) {
                if open.pop() != Some(want) {
                    return Err(format!("unbalanced `{c}` in `{t}`"));
                }
            }
            out.push(Tok::Punct(c));
            i += 1;
        }
    }
    match open.last() {
        Some(o) => Err(format!("unclosed `{o}` in `{t}`")),
        None => Ok(out),
    }
}

/// A quote that opens a lifetime name no quote closes: `'a`, and not the host path `'std::x'` or `'v1'`.
fn lifetime_at(cs: &[char], i: usize) -> bool {
    let n = cs[i + 1..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').count();
    n > 0 && !cs[i + 1].is_ascii_digit() && !matches!(cs.get(i + 1 + n), Some('\'' | ':'))
}

/// Where a lifetime may stand: after `for`, and in a list of lifetimes.
fn lifetime_context(out: &[Tok]) -> bool {
    match out.last() {
        Some(Tok::Word(w)) => w == "for",
        Some(Tok::Punct(',')) => matches!(out.len().checked_sub(2).and_then(|i| out.get(i)), Some(Tok::Lifetime(_))),
        _ => false,
    }
}

fn is_word(t: Option<&Tok>, w: &str) -> bool {
    matches!(t, Some(Tok::Word(x)) if x == w)
}

fn is_host_path(p: &str) -> bool {
    p.contains("::") && ["std::", "core::", "alloc::", "<"].iter().any(|r| p.starts_with(r))
}

/// The pieces of a callback clause, as token indices: the `with` items (ranges) and the name after `returning`.
struct CallbackParts {
    with: Vec<(usize, usize)>,
    returning: Option<usize>,
    errs: Vec<String>,
}

/// Split `callback f : called … [when p] [, with e, …] [, returning r]` (the tokens after `callback`) at its top-level
/// `, with` and `, returning`; `with` takes expressions up to the `, returning` or the end.
fn callback_parts(toks: &[Tok], text: &str) -> CallbackParts {
    let mut parts = CallbackParts { with: vec![], returning: None, errs: vec![] };
    let (mut depth, mut with_at, mut ret_comma): (i32, Option<usize>, Option<usize>) = (0, None, None);
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Punct('(' | '[' | '{') => depth += 1,
            Tok::Punct(')' | ']' | '}') => depth -= 1,
            Tok::Word(w) if depth == 0 && i > 0 && toks[i - 1] == Tok::Punct(',') && w == "with" && with_at.is_none() && ret_comma.is_none() => with_at = Some(i + 1),
            Tok::Word(w) if depth == 0 && i > 0 && toks[i - 1] == Tok::Punct(',') && w == "returning" && ret_comma.is_none() => ret_comma = Some(i - 1),
            _ => {}
        }
    }
    if let Some(c) = ret_comma {
        match (toks.get(c + 2), toks.len() == c + 3) {
            (Some(Tok::Word(_)), true) => parts.returning = Some(c + 2),
            _ => parts.errs.push(format!("`returning` names one result and ends the clause: `{text}`")),
        }
    }
    if let Some(a) = with_at {
        let end = ret_comma.unwrap_or(toks.len());
        let (mut depth, mut start) = (0, a);
        for (i, t) in toks.iter().enumerate().take(end).skip(a) {
            match t {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => depth -= 1,
                Tok::Punct(',') if depth == 0 => {
                    parts.with.push((start, i));
                    start = i + 1;
                }
                _ => {}
            }
        }
        parts.with.push((start, end));
        if parts.with.iter().any(|(a, b)| a >= b) {
            parts.errs.push(format!("`with` names the call's arguments, each an expression or a name, and none is empty: `{text}`"));
        }
    }
    let once = toks.windows(2).any(|w| (is_word(Some(&w[0]), "exactly") || is_word(Some(&w[0]), "most")) && is_word(Some(&w[1]), "once"));
    let by = by_form(toks).is_some();
    if (with_at.is_some() || ret_comma.is_some()) && !once && !by {
        parts.errs.push(format!("`with` and `returning` name the values of one call, so the clause says `called exactly once` or `called at most once`: `{text}`"));
    }
    // a `called by` clause names the logs of the calls a result makes later: a name for each, never an expression
    if by && parts.with.iter().any(|(a, b)| b - a != 1 || !matches!(toks[*a], Tok::Word(_))) {
        parts.errs.push(format!("in a `called by` clause `with` gives a name to the list of each argument, not an expression: `{text}`"));
    }
    parts
}

/// Where a `callback c : called by P.m …` clause (language 6.13) names the later calls, as the index of `P` in the tokens
/// after `callback`: `called` then `by`.
fn by_form(toks: &[Tok]) -> Option<usize> {
    let i = toks.windows(2).position(|w| is_word(Some(&w[0]), "called") && is_word(Some(&w[1]), "by"))?;
    Some(i + 2)
}

/// The path `P.m` of a later call, at token `at`: `result` or a parameter of the subject, a dot, a method; and, for a
/// parameter whose bounds are known, a method that exists. Returns the index after it.
fn later_call(toks: &[Tok], at: usize, text: &str, scope: &Scope, errs: &mut Vec<String>) -> usize {
    match (toks.get(at), toks.get(at + 1), toks.get(at + 2)) {
        (Some(Tok::Word(p)), Some(Tok::Punct('.')), Some(Tok::Word(m))) => {
            if p != "result" && !scope.events.contains(p) {
                errs.push(format!("`{p}` is neither `result` nor a parameter of the subject in `{text}`"));
            } else if let Some(Some(ms)) = scope.event_methods.get(p) {
                if !ms.contains(m) {
                    errs.push(format!("`{m}` is not a method of `{p}`'s bounds (known: {}) in `{text}`", ms.iter().cloned().collect::<Vec<_>>().join(", ")));
                }
            }
            scope.flow.borrow_mut().run = true;
            at + 3
        }
        _ => {
            errs.push(format!("a later call is `result.method` or `parameter.method`: `{text}`"));
            at
        }
    }
}

/// `callback c : called by P.m [, exactly once | at most once per call] [, with a, …] [, returning r]`.
fn lint_called_by(toks: &[Tok], text: &str, scope: &Scope) -> Vec<String> {
    let mut errs = vec![];
    let Some(at) = by_form(toks) else { return errs };
    let end = later_call(toks, at, text, scope, &mut errs);
    // up to the first top-level `, with` or `, returning`: nothing, or the count per call
    let stop = (end..toks.len()).find(|&i| toks[i] == Tok::Punct(',') && (is_word(toks.get(i + 1), "with") || is_word(toks.get(i + 1), "returning"))).unwrap_or(toks.len());
    let between: Vec<&str> = toks[end..stop].iter().map(|t| if let Tok::Word(w) = t { w.as_str() } else { "," }).collect();
    if !matches!(between.as_slice(), [] | [",", "exactly", "once", "per", "call"] | [",", "at", "most", "once", "per", "call"]) {
        errs.push(format!("a `called by` clause counts the calls per call of the method with `exactly once per call` or `at most once per call`, or not at all: `{text}`"));
    }
    if toks.iter().any(|t| is_word(Some(t), "when")) {
        errs.push(format!("a `called by` clause has no `when`: the calls are those of a later run, not of the subject's entry: `{text}`"));
    }
    errs
}

/// The token indices of a callback clause that bind a name: the one after `returning`, and each `with` item that is a
/// single name nothing else in scope has (a name given to that argument).
fn callback_binders(toks: &[Tok], parts: &CallbackParts, scope: &Scope) -> Vec<usize> {
    let known = |w: &str| scope.names.contains(w) || WORDS.contains(&w) || kind_words("callback").contains(&w);
    let mut out: Vec<usize> = parts.with.iter().filter(|(a, b)| b - a == 1).filter(|(a, _)| matches!(&toks[*a], Tok::Word(w) if !known(w) && !toks[..*a].iter().any(|t| matches!(t, Tok::Word(x) if x == w)))).map(|(a, _)| *a).collect();
    out.extend(parts.returning);
    out
}

/// The names a callback clause binds, for the subject's other clauses (language 6.13): `returning r`, and a name given to an
/// argument by `with`. `scope` is what the subject names before any callback does.
pub fn callback_binds(text: &str, scope: &Scope) -> Vec<String> {
    let Some(rest) = text.strip_prefix("callback") else { return vec![] };
    let Ok(toks) = lex(rest) else { return vec![] };
    let parts = callback_parts(&toks, text);
    callback_binders(&toks, &parts, scope).into_iter().filter_map(|i| if let Tok::Word(w) = &toks[i] { Some(w.clone()) } else { None }).collect()
}

/// A `protocol` is a regular expression over the calls of the subject: a callback, a parameter (a macro's argument is
/// evaluated, a function's is passed), or a method of a parameter, `self.next`, with `begin` and `end` of the call.
fn lint_protocol(toks: &[Tok], text: &str, scope: &Scope) -> Vec<String> {
    let mut errs = vec![];
    let mut events = 0;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Word(w) if i > 0 && toks[i - 1] == Tok::Punct('.') => {
                let p = if i > 1 { if let Tok::Word(p) = &toks[i - 2] { p.as_str() } else { "" } } else { "" };
                if let Some(Some(ms)) = scope.event_methods.get(p) {
                    if !ms.contains(w) {
                        errs.push(format!("`{w}` is not a method of `{p}`'s bounds (known: {}) in `{text}`", ms.iter().cloned().collect::<Vec<_>>().join(", ")));
                    }
                }
                events += 1;
            }
            Tok::Word(w) if toks.get(i + 1) == Some(&Tok::Punct('.')) => {
                if !scope.events.contains(w) {
                    errs.push(format!("`{w}` is not a parameter of the subject in `{text}`"));
                }
            }
            Tok::Word(w) if w == "begin" || w == "end" => {}
            Tok::Word(w) => {
                events += 1;
                if !scope.events.contains(w) {
                    errs.push(format!("`{w}` is not a callback or a parameter of the subject in `{text}`"));
                }
            }
            Tok::Punct('(' | ')' | '*' | '+' | '?' | '|' | '.') => {}
            _ => errs.push(format!("a `protocol` is calls joined by `(` `)` `*` `+` `?` `|`: `{text}`")),
        }
    }
    if events == 0 {
        errs.push(format!("a `protocol` names at least one call: `{text}`"));
    }
    errs.dedup();
    errs
}

/// The `hold` clauses of language 6.14: `hold p : moved from q`, `lifetime v ends in call [when p]` and `lifetime a, b
/// survive call`. The other forms of a `hold` clause are not checked past their names.
fn lint_hold(head: &str, toks: &[Tok], text: &str, scope: &Scope) -> Vec<String> {
    let mut errs = vec![];
    // `hold p : …` holds the path `p` (the subject's own when there is none), which the subject has
    if let (Some(Tok::Word(p)), Some(Tok::Punct(':'))) = (toks.first(), toks.get(1)) {
        if head == "hold" && p != "result" && !scope.names.contains(p) {
            errs.push(format!("`{p}` is not a parameter of the subject in `{text}`"));
        }
    }
    let word = |i: usize| match toks.get(i) {
        Some(Tok::Word(w)) => w.as_str(),
        _ => "",
    };
    if let Some(m) = toks.iter().position(|t| matches!(t, Tok::Word(w) if w == "moved")) {
        // `moved from q`: q is a path, and ends the clause
        let mut k = m + 3;
        while toks.get(k) == Some(&Tok::Punct('.')) && matches!(toks.get(k + 1), Some(Tok::Word(_) | Tok::Other)) {
            k += 2;
        }
        let ok = head == "hold" && word(m + 1) == "from" && !word(m + 2).is_empty() && k == toks.len();
        if !ok {
            errs.push(format!("`moved from` names one path and ends a `hold` clause: `{text}`"));
        }
    }
    if head == "lifetime" && is_word(toks.first(), "every") && is_word(toks.get(1), "item") {
        // `lifetime every item of e ends with p`: the many elements a sequence holds (language 7.3)
        let ok = is_word(toks.get(2), "of") && toks.len() > 4 && toks.iter().any(|t| matches!(t, Tok::Word(w) if w == "ends" || w == "outlives" || w == "within"));
        if !ok {
            errs.push(format!("`lifetime every item of e ends with p` names a sequence and a predicate: `{text}`"));
        }
    } else if head == "lifetime" && toks.iter().any(|t| matches!(t, Tok::Word(w) if w == "released")) {
        // `lifetime self released at end`: a type whose end gives something back (language 7.3).
        let ok = toks.len() == 4 && ["self", "released", "at", "end"].iter().enumerate().all(|(i, w)| word(i) == *w);
        if !ok {
            errs.push(format!("`lifetime self released at end` is said of a type, and nothing follows: `{text}`"));
        }
    } else if head == "lifetime" {
        let survive = toks.iter().position(|t| matches!(t, Tok::Word(w) if w == "survive"));
        let ends = toks.iter().position(|t| matches!(t, Tok::Word(w) if w == "ends"));
        if let Some(s) = survive {
            let names_ok = s > 0 && toks[..s].iter().enumerate().all(|(i, t)| if i % 2 == 0 { matches!(t, Tok::Word(_)) } else { *t == Tok::Punct(',') }) && s % 2 == 1;
            if !names_ok || word(s + 1) != "call" || toks.len() != s + 2 {
                errs.push(format!("`lifetime a, b survive call` names the entities that do not end in the call, and nothing follows: `{text}`"));
            }
        } else if let Some(e) = ends.filter(|e| word(e + 1) == "in") {
            if word(e + 2) != "call" || e != 1 || !(toks.len() == e + 3 || word(e + 3) == "when" && toks.len() > e + 4) {
                errs.push(format!("`lifetime v ends in call` names one entity, and may be followed by `when` and a condition: `{text}`"));
            }
        }
    }
    errs
}

/// `obliges caller : p because "…"`: a condition, and the reason in a string that ends the clause.
fn lint_obliges(toks: &[Tok], text: &str) -> Vec<String> {
    let because = toks.iter().rposition(|t| matches!(t, Tok::Word(w) if w == "because"));
    let ok = toks.first() == Some(&Tok::Punct(':')) && because.is_some_and(|b| b > 1 && b + 2 == toks.len() && toks[b + 1] == Tok::Other);
    if ok {
        vec![]
    } else {
        vec![format!("`obliges caller : p because \"…\"` states a condition and the reason, in quotes: `{text}`")]
    }
}

/// The names a tuple pattern binds after `is`: `e is (a, b)`.
fn pattern_binders(toks: &[Tok]) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for (i, t) in toks.iter().enumerate() {
        if is_word(Some(t), "is") && toks.get(i + 1) == Some(&Tok::Punct('(')) {
            for (j, u) in toks.iter().enumerate().skip(i + 2).take_while(|(_, u)| **u != Tok::Punct(')')) {
                if matches!(u, Tok::Word(_)) {
                    out.insert(j);
                }
            }
        }
    }
    out
}

/// `let name = e` and `let (a, b) = e`: the token indices of the names bound, which are known to the clauses after this one.
/// A name is bound once (language 4), and `_` binds nothing.
fn lint_let(toks: &[Tok], text: &str, scope: &Scope, errs: &mut Vec<String>) -> BTreeSet<usize> {
    let mut at = BTreeSet::new();
    if !is_word(toks.first(), "let") {
        errs.push(format!("a `let` clause starts with `let`: `{text}`"));
        return at;
    }
    let eq = toks.iter().position(|t| *t == Tok::Punct('=')).filter(|&e| e + 1 < toks.len() && toks.get(e + 1) != Some(&Tok::Punct('=')));
    let Some(eq) = eq else {
        errs.push(format!("`let` binds a name or a pattern `(a, b)` to an expression: `let p = e`: `{text}`"));
        return at;
    };
    let pat = &toks[1..eq];
    let names: Vec<usize> = match pat {
        [Tok::Word(_)] => vec![1],
        [Tok::Punct('('), inner @ .., Tok::Punct(')')] if !inner.is_empty() && inner.iter().enumerate().all(|(i, t)| if i % 2 == 0 { matches!(t, Tok::Word(_)) } else { *t == Tok::Punct(',') }) && inner.len() % 2 == 1 => (0..inner.len()).step_by(2).map(|i| i + 2).collect(),
        _ => {
            errs.push(format!("`let` binds a name or a tuple pattern `(a, b)` of names and `_`: `{text}`"));
            return at;
        }
    };
    let mut seen = BTreeSet::new();
    for i in names {
        let Tok::Word(w) = &toks[i] else { continue };
        if w == "_" {
            continue;
        }
        if !seen.insert(w.clone()) || scope.names.contains(w) || scope.flow.borrow().lets.contains(w) {
            errs.push(format!("`{w}` is already bound: a name is bound once, in `{text}`"));
        }
        at.insert(i);
    }
    at
}

/// The sequence operators of language 6.15: each takes its arguments, and `map` and `filter` take a named pure function
/// first (a view of the file or one of the language's), never a parameter, a callback or a closure.
fn lint_sequences(toks: &[Tok], text: &str, scope: &Scope) -> Vec<String> {
    let mut errs = vec![];
    for (i, t) in toks.iter().enumerate() {
        let Tok::Word(w) = t else { continue };
        let Some(&(_, arity)) = SEQUENCE_OPS.iter().find(|(n, _)| n == w) else { continue };
        if toks.get(i + 1) != Some(&Tok::Punct('(')) || (i > 0 && toks[i - 1] == Tok::Punct('.')) {
            continue;
        }
        // the arguments: tokens up to each top-level comma, until the bracket that closes
        let (mut depth, mut args, mut start) = (0, vec![], i + 2);
        for (j, u) in toks.iter().enumerate().skip(i + 1) {
            match u {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => {
                    depth -= 1;
                    if depth == 0 {
                        args.push((start, j));
                        break;
                    }
                }
                Tok::Punct(',') if depth == 1 => {
                    args.push((start, j));
                    start = j + 1;
                }
                _ => {}
            }
        }
        if args.len() != arity || args.iter().any(|(a, b)| a >= b) {
            errs.push(format!("`{w}` takes {arity} argument{} in `{text}`", if arity == 1 { "" } else { "s" }));
            continue;
        }
        if w == "map" || w == "filter" {
            let (a, b) = args[0];
            match &toks[a..b] {
                [Tok::Word(f)] if PURE_BUILTINS.contains(&f.as_str()) || scope.pure.contains(f) => {}
                [Tok::Word(f)] if scope.names.contains(f) => errs.push(format!("`{f}` is a parameter or callback, not a named pure function: `{w}` takes a view of the file or a function of the language (language 6.15) in `{text}`")),
                [Tok::Word(f)] => errs.push(format!("`{f}` is not a named pure function (a view of the file, `scalars` or `deref`) in `{text}`")),
                _ => errs.push(format!("`{w}` takes the name of a pure function first, not an expression, in `{text}`")),
            }
        }
    }
    errs
}

/// Everything wrong with one clause's text. `heads` are the keywords its kind may start with (none: no check).
pub fn lint(kind: &str, text: &str, heads: &[&str], scope: &Scope) -> Vec<String> {
    let head = heads.iter().filter(|h| text == **h || text.starts_with(&format!("{h} ")) || text.starts_with(&format!("{h}:"))).max_by_key(|h| h.len());
    if !heads.is_empty() && head.is_none() {
        let first = text.split_whitespace().next().unwrap_or("");
        return vec![format!("a `{kind}` clause starts with {} , not `{first}`", heads.iter().map(|h| format!("`{h}`")).collect::<Vec<_>>().join(" or "))];
    }
    let toks = match lex(&text[head.map_or(0, |h| h.len())..]) {
        Ok(t) => t,
        Err(e) => return vec![e],
    };
    if kind == "protocol" {
        return lint_protocol(&toks, text, scope);
    }
    let mut errs = vec![];
    if kind == "hold" {
        errs.extend(lint_hold(head.map_or("", |h| *h), &toks, text, scope));
    }
    if kind == "obliges" {
        errs.extend(lint_obliges(&toks, text));
    }
    if kind == "callback" {
        errs.extend(lint_called_by(&toks, text, scope));
    }
    // `final(e)` reads the state at the end of a run, which an earlier clause must have named
    if toks.windows(2).any(|w| is_word(Some(&w[0]), "final") && w[1] == Tok::Punct('(')) && !scope.flow.borrow().run {
        errs.push(format!("`final(e)` is the state at the end of a later run: an earlier `callback … called by` or `fails … in` clause of the subject names it: `{text}`"));
    }
    errs.extend(lint_sequences(&toks, text, scope));
    // `let name = e` and `let (a, b) = e` bind for the clauses after them (language 4)
    let lets = if kind == "let" { lint_let(&toks, text, scope, &mut errs) } else { BTreeSet::new() };
    let patterns = pattern_binders(&toks);
    // a callback clause's `with` and `returning` (language 6.13)
    let (binders, parts_errs) = if kind == "callback" {
        let parts = callback_parts(&toks, text);
        (callback_binders(&toks, &parts, scope), parts.errs)
    } else {
        (vec![], vec![])
    };
    errs.extend(parts_errs);
    // `fails F when p` names a declared failure; `fails panic when p` is the one the Rust host has
    let failure = if kind == "panics" { Some("panic") } else if kind == "fails" { Some(toks.first().map_or("", |t| if let Tok::Word(w) = t { w } else { "" })) } else { None };
    if let Some(f) = failure.filter(|f| !scope.failures.contains(*f)) {
        errs.push(format!("failure `{f}` is not declared in the file's `failures`: `{text}`"));
    }
    // `fails F in P.m when p`: the failure is a later call's (language 6.13)
    if kind == "fails" || kind == "panics" {
        let at = usize::from(kind == "fails");
        if is_word(toks.get(at), "in") {
            let end = later_call(&toks, at + 1, text, scope, &mut errs);
            if !is_word(toks.get(end), "when") {
                errs.push(format!("`fails F in P.m` is followed by `when` and a condition: `{text}`"));
            }
        }
    }
    // `fails F { field: e, … } when p` fills the payload F declares: its field names are not expressions
    let mut skip: BTreeSet<usize> = BTreeSet::new();
    let at = usize::from(kind == "fails");
    if toks.get(at) == Some(&Tok::Punct('{')) {
        let declared = failure.and_then(|f| scope.payload.get(f));
        let mut depth = 0;
        for (i, t) in toks.iter().enumerate().skip(at) {
            match t {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => depth -= 1,
                Tok::Word(w) if depth == 1 && toks.get(i + 1) == Some(&Tok::Punct(':')) && matches!(toks[i - 1], Tok::Punct('{' | ',')) => {
                    skip.insert(i);
                    if !declared.is_some_and(|d| d.contains(w)) {
                        errs.push(format!("`{w}` is not a payload field of failure `{}`: `{text}`", failure.unwrap_or("")));
                    }
                }
                _ => {}
            }
            if depth == 0 {
                break;
            }
        }
    }
    // a clause names itself: `ensure name :`, `law name :`, `callback name :`
    let named = matches!(toks.first(), Some(Tok::Word(_))) && toks.get(1) == Some(&Tok::Punct(':')) && kind != "fails";
    let extra = kind_words(kind);
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    // `implements 'I' { Item = T }` and `some 'I' { Item = T }` name the associated types of the interface
    let assoc_clause = kind == "implements" || toks.iter().any(|t| matches!(t, Tok::Word(w) if w == "some" || w == "any" || w == "implements"));
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Lifetime(l) if !scope.names.contains(l) => errs.push(format!("unknown lifetime `'{l}` in `{text}`")),
            Tok::Word(w) if binders.contains(&i) || patterns.contains(&i) => {
                bound.insert(w);
            }
            Tok::Word(_) if lets.contains(&i) => {}
            Tok::Word(w) if w == "_" => {}
            Tok::Word(_) if assoc_clause && i > 0 && matches!(toks[i - 1], Tok::Punct('{' | ',')) && toks.get(i + 1) == Some(&Tok::Punct('=')) && toks.get(i + 2) != Some(&Tok::Punct('=')) => {}
            // the name of a type argument: `list<element_type=u8>`, `ref<target_type=T>`
            Tok::Word(w) if is_arg_name(&toks, i, w) => {}
            Tok::Quoted(p) if !is_host_path(p) => errs.push(format!("`'{p}'` is not a quoted host path (`'std::…'`)")),
            Tok::Word(w) => {
                // `all x : T : p` and `any x in c : p` bind x; `v is Variant(x)` binds x
                if i > 0 && (is_word(toks.get(i - 1), "all") || is_word(toks.get(i - 1), "any")) || (i > 2 && is_word(toks.get(i - 3), "is") && toks[i - 1] == Tok::Punct('(')) {
                    bound.insert(w);
                }
                let own = (named && i == 0) || (kind == "fails" && i == 0);
                let field = i > 0 && toks[i - 1] == Tok::Punct('.');
                if !(skip.contains(&i) || extra.contains(&w.as_str()) || WORDS.contains(&w.as_str()) || bound.contains(w.as_str()) || scope.names.contains(w) || scope.flow.borrow().lets.contains(w.as_str()) || own || field || (assoc_clause && (crate::types::is_prim(w) || TYPE_WORDS.contains(&w.as_str())))) {
                    errs.push(format!("unknown name `{w}` in `{text}`"));
                }
            }
            _ => {}
        }
    }
    // the names a `let` binds are known to the clauses after it, not to its own expression
    for &i in &lets {
        if let Tok::Word(w) = &toks[i] {
            scope.flow.borrow_mut().lets.insert(w.clone());
        }
    }
    errs
}

/// Whether the word at `i` names a type argument (`element_type=`, after `<` or `,`), which names no value.
fn is_arg_name(toks: &[Tok], i: usize, _w: &str) -> bool {
    let assigns = toks.get(i + 1) == Some(&Tok::Punct('=')) && toks.get(i + 2) != Some(&Tok::Punct('='));
    assigns && i > 0 && matches!(toks[i - 1], Tok::Punct('<' | ','))
}

/// What a `given` item may name, for one subject: the generics with the methods their bounds are known to have, the
/// build settings the host profile lists, and whether the subject is a method of a trait (which has a default body).
pub struct GivenCtx<'a> {
    /// Each generic type parameter; `Some(methods)` when every bound of it is known to the file, `None` when a method cannot be checked.
    pub generics: BTreeMap<String, Option<BTreeSet<String>>>,
    /// `Self` of a trait, an impl or a type: a name that is not a generic.
    pub has_self: bool,
    pub trait_method: bool,
    pub builds: &'a BTreeSet<String>,
}

/// The effects and promises a `given` item may state of the implementations it names (language 6.12).
const GIVEN_PREDICATES: [&[&str]; 3] = [&["never", "panic"], &["terminate"], &["effects", "none"]];

/// Everything wrong with the `given` items of one clause. An item is `default`, `build SETTING`, or `P.m, P.n never panic |
/// terminate | effects none`: a bound's parameter, a method of that bound, and what its implementation promises.
pub fn lint_given(items: &[&str], ctx: &GivenCtx) -> Vec<String> {
    let mut errs = vec![];
    for item in items {
        let spaced = item.replace(',', " , ");
        let w: Vec<&str> = spaced.split_whitespace().collect();
        match w.as_slice() {
            [] => errs.push("an empty `given` item".to_string()),
            ["default"] if ctx.trait_method => {}
            ["default"] => errs.push(format!("`given default`: the subject is not a method of a trait, so it has no default body to name: `{item}`")),
            ["build", setting] => {
                if !ctx.builds.contains(*setting) {
                    errs.push(format!("`given build {setting}`: not a build setting of the host profile ({})", ctx.builds.iter().cloned().collect::<Vec<_>>().join(", ")));
                }
            }
            ["build", ..] => errs.push(format!("`given build` names one setting: `{item}`")),
            _ => {
                let Some(cut) = GIVEN_PREDICATES.iter().find(|p| w.len() > p.len() && w[w.len() - p.len()..] == ***p) else {
                    errs.push(format!("a `given` item is `default`, `build SETTING`, or methods then `never panic`, `terminate` or `effects none`: `{item}`"));
                    continue;
                };
                let refs = &w[..w.len() - cut.len()];
                for (i, r) in refs.iter().enumerate() {
                    if i % 2 == 1 {
                        if *r != "," {
                            errs.push(format!("`given` names methods separated by commas: `{item}`"));
                        }
                        continue;
                    }
                    let Some((g, m)) = r.split_once('.').filter(|(g, m)| !g.is_empty() && !m.is_empty() && !m.contains('.')) else {
                        errs.push(format!("`{r}` is not `Param.method` in `given {item}`"));
                        continue;
                    };
                    match ctx.generics.get(g) {
                        None if g == "Self" && ctx.has_self => {}
                        None => errs.push(format!("`given {item}`: `{g}` is not a generic type parameter of the subject")),
                        Some(Some(ms)) if !ms.contains(m) => errs.push(format!("`given {item}`: `{m}` is not a method of a bound of `{g}` (known: {})", ms.iter().cloned().collect::<Vec<_>>().join(", "))),
                        Some(_) => {}
                    }
                }
                if refs.len().is_multiple_of(2) {
                    errs.push(format!("`given` names methods separated by commas: `{item}`"));
                }
            }
        }
    }
    errs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope {
            names: ["k", "K", "V", "msg", "Self", "Less", "Greater", "Err", "Ok"].iter().map(|s| s.to_string()).collect(),
            failures: ["panic".to_string()].into(),
            payload: [("panic".to_string(), ["message".to_string(), "location".to_string()].into())].into(),
            ..Scope::default()
        }
    }

    fn bad(kind: &str, head: &str, text: &str) -> String {
        lint(kind, text, &[head], &scope()).join("; ")
    }

    #[test]
    fn well_formed_clauses_pass() {
        for (k, h, t) in [
            ("ensure", "ensure", "ensure lookup : result == self[k]"),
            ("ensure", "ensure", "ensure stored : all k2 in old(self) : k2 != k implies self[k2] == old(self[k2])"),
            ("law", "law", "law dual : all a : Self : all b : Self : ('std::cmp::Ord::cmp'(a, b) is Less) iff ('std::cmp::Ord::cmp'(b, a) is Greater)"),
            ("panics", "fails panic when", "fails panic when self is Err(e) and e == e"),
            ("effects", "calls only", "calls only 'std::cmp::Ord::max'"),
            ("panics", "fails panic", "fails panic { message: msg, location: caller } when self is Err(e)"),
            ("ensure", "ensure", "ensure unsigned : not sign_bit(result) and not is_nan(self)"),
            ("budget", "budget", "budget allocates at most 0 B per call when k == 0"),
            ("layout", "size", "size 0"),
            ("layout", "layout", "layout transparent"),
            ("callback", "callback", "callback f : called at most once before return"),
            ("ensure", "ensure", "ensure f : self matches /\\p{White_Space}/ and finite self implies result == (if self < 0.0 then -self else self)"),
        ] {
            assert_eq!(bad(k, h, t), "", "{t}");
        }
    }

    #[test]
    fn the_forms_of_interfaces_implementations_and_lifetimes_pass() {
        let mut sc = scope();
        sc.names.extend(["a", "b", "Item"].map(String::from));
        let ok = |kind: &str, head: &str, t: &str| assert_eq!(lint(kind, t, &[head], &sc), Vec::<String>::new(), "{t}");
        ok("implements", "implements", "implements 'std::iter::Iterator' { Item = scalar }, 'std::clone::Clone'");
        ok("implements", "implements", "implements 'std::convert::From<u8>' { Item = 'std::string::String', Other = list<element_type=u8> }");
        ok("hold", "hold", "hold : borrows for 'a");
        ok("hold", "hold", "hold : borrows for 'a, 'b");
        ok("hold", "hold", "hold result : borrows from self");
        ok("ensure", "ensure", "ensure ok : result is some 'std::iter::Iterator' { Item = u8 }");
        // an associated type's name is not a name to look up, a lifetime is looked up, and a host path is still quoted
        let bad = |kind: &str, head: &str, t: &str| lint(kind, t, &[head], &sc).join("; ");
        assert!(bad("hold", "hold", "hold : borrows for 'zz").contains("unknown lifetime `'zz`"));
        assert!(bad("implements", "implements", "implements 'std::iter::Iterator' { Item = nope }").contains("unknown name `nope`"));
        assert!(bad("implements", "implements", "implements std::iter::Iterator { Item = scalar }").contains("not quoted"));
        assert!(bad("implements", "implements", "implements 'x' { Item = scalar }").contains("not a quoted host path"));
        assert!(bad("ensure", "ensure", "ensure x : result == 'v1").contains("unterminated"), "a quote that is not after `for` is not a lifetime");
    }

    #[test]
    fn what_a_prefix_check_missed_is_caught() {
        assert!(bad("ensure", "ensure", "ensure x : (result == k").contains("unclosed"));
        assert!(bad("ensure", "ensure", "ensure x : result == k)").contains("unbalanced"));
        assert!(bad("ensure", "ensure", "ensure x : result == 'k").contains("unterminated"));
        assert!(bad("law", "law", "law x : std::cmp::Ord::cmp(a, b)").contains("not quoted"));
        assert!(bad("effects", "calls only", "calls only 'max'").contains("not a quoted host path"));
        assert!(bad("ensure", "ensure", "ensure x : nope == k").contains("unknown name `nope`"));
        assert!(bad("ensure", "ensure", "require x : true").contains("starts with `ensure`"));
        assert!(bad("fails", "fails", "fails Boom when true").contains("not declared"));
        assert!(lint("panics", "fails panic when true", &["fails panic when"], &Scope::default()).join(";").contains("not declared"));
        assert!(bad("panics", "fails panic", "fails panic { text: k } when true").contains("not a payload field of failure `panic`"));
        assert!(bad("panics", "fails panic", "fails panic { message: nope } when true").contains("unknown name `nope`"));
        assert!(bad("budget", "budget", "budget c at most").contains("unknown name `c`"));
        assert!(bad("layout", "layout", "layout allocates").contains("unknown name `allocates`"));
        assert!(bad("ensure", "ensure", "ensure x : allocates == k").contains("unknown name `allocates`"));
        // a name bound by a quantifier or a pattern is known only after it
        assert!(bad("ensure", "ensure", "ensure x : e == k and (self is Err(e))").contains("unknown name `e`"));
    }

    #[test]
    fn callbacks_bind_their_results_and_protocols_name_calls() {
        let mut sc = scope();
        sc.names.insert("self".into());
        let cb = |t: &str| lint("callback", t, &["callback"], &sc).join("; ");
        assert_eq!(cb("callback f : called exactly once when exists self, with self, returning r"), "");
        assert_eq!(cb("callback f : called exactly once when self is Ok(x), with x, returning r"), "");
        assert!(cb("callback f : called exactly once when r, returning r").contains("unknown name `r`"));
        assert!(cb("callback f : called at least once, returning r").contains("one call"));
        assert_eq!(callback_binds("callback f : called exactly once when self is Ok(x), with x, y, returning r", &sc), ["y", "r"]);
        assert_eq!(callback_binds("callback f : called exactly once, with v, w, returning r", &sc), ["v", "w", "r"]);
        assert_eq!(callback_binds("callback f : called at most once before return", &sc), Vec::<String>::new());
        sc.events = ["self", "f"].map(String::from).into();
        sc.event_methods.insert("self".into(), Some(["next".to_string()].into()));
        let pr = |t: &str| lint("protocol", t, &["protocol"], &sc).join("; ");
        assert_eq!(pr("protocol begin (f | self.next)* end"), "");
        assert!(pr("protocol self.nope").contains("not a method"));
        assert!(pr("protocol g").contains("not a callback or a parameter"));
    }

    #[test]
    fn the_small_additions_have_forms_of_their_own() {
        let mut sc = scope();
        sc.names.extend(["dest", "src", "self", "mid"].map(String::from));
        let hold = |t: &str| lint("hold", t, &["hold", "lifetime"], &sc).join("; ");
        assert_eq!(hold("lifetime self released at end"), "");
        assert!(hold("lifetime self released at end of call").contains("is said of a type"));
        assert_eq!(hold("hold result : moved from dest"), "");
        assert_eq!(hold("hold dest : moved from src.0"), "");
        assert_eq!(hold("lifetime src ends in call when src == dest"), "");
        assert_eq!(hold("lifetime dest, src survive call"), "");
        assert!(hold("hold result : moved from").contains("names one path"));
        assert!(hold("hold zzz : moved from src").contains("not a parameter"));
        assert!(hold("lifetime dest, src survive").contains("survive call"));
        assert!(hold("lifetime src ends in call when").contains("ends in call"));
        assert!(hold("hold : borrows from dest").is_empty(), "the older forms are not touched");
        let ob = |t: &str| lint("obliges", t, &["obliges caller"], &sc).join("; ");
        assert_eq!(ob("obliges caller : src == dest because \"two values\""), "");
        assert!(ob("obliges caller : src == dest").contains("reason"));
        assert!(ob("obliges caller : because \"x\"").contains("reason"));
        assert_eq!(lint("never panics", "never panics except exhausted", &["never panics except exhausted", "never panics"], &sc), Vec::<String>::new());
        assert!(!lint("never panics", "never panics except broke", &["never panics except exhausted", "never panics"], &sc).is_empty());
        assert_eq!(lint("budget", "budget work expected amortised O(1)", &["budget"], &sc), Vec::<String>::new());
        let ens = |t: &str| lint("ensure", t, &["ensure"], &sc).join("; ");
        assert_eq!(ens("ensure p : count(result.0) == mid and result.1 == src"), "");
        assert_eq!(ens("ensure p : result is (a, _) implies count(a) == mid"), "");
        assert!(ens("ensure p : result is (a, b) implies count(c) == mid").contains("unknown name `c`"));
        assert_eq!(ens("ensure p : result moved from src"), "");
    }

    #[test]
    fn sequences_take_named_pure_functions() {
        let mut sc = scope();
        sc.names.extend(["self", "other", "f", "n", "mid", "capacity"].map(String::from));
        sc.pure.insert("capacity".into());
        let ens = |t: &str| lint("ensure", t, &["ensure"], &sc).join("; ");
        assert_eq!(ens("ensure a : yields(result) == scalars(self)"), "");
        assert_eq!(ens("ensure b : yields(result) == old(yields(self))[n..] ++ [other] and yields(result)[0] == none"), "");
        assert_eq!(ens("ensure c : result.0 == old(self)[..mid] and result.1 == old(self)[mid..size(old(self))]"), "");
        assert_eq!(ens("ensure d : yields(result) == zip(skip(n, old(yields(self))), take(n, enumerate(reverse(yields(other)))))"), "");
        assert_eq!(ens("ensure e : map(deref, yields(result)) == self and filter(capacity, yields(self)) == []"), "");
        assert!(ens("ensure f : yields(result) == map(f, yields(self))").contains("parameter or callback"));
        assert!(ens("ensure g : yields(result) == map(g, yields(self))").contains("not a named pure function"));
        assert!(ens("ensure h : yields(result) == map(deref + 1, yields(self))").contains("name of a pure function"));
        assert!(ens("ensure i : yields(result) == take(yields(self))").contains("takes 2 arguments"));
        assert!(ens("ensure j : yields(result, self) == []").contains("takes 1 argument"));
        assert!(ens("ensure k : yields() == []").contains("takes 1 argument"));
    }

    #[test]
    fn a_result_s_later_calls_have_a_log_a_failure_and_an_end() {
        let mut sc = scope();
        sc.names.extend(["self", "f", "n"].map(String::from));
        sc.events = ["self", "f"].map(String::from).into();
        sc.event_methods.insert("self".into(), Some(["next".to_string()].into()));
        let cb = |t: &str| lint("callback", t, &["callback"], &sc).join("; ");
        assert_eq!(cb("callback f : called by result.next, at most once per call, with x, returning y"), "");
        assert_eq!(cb("callback f : called by result.next, exactly once per call, returning y"), "");
        assert_eq!(cb("callback f : called by result.next"), "");
        assert_eq!(cb("callback f : called by self.next, with x"), "");
        assert!(cb("callback f : called by self.nope").contains("not a method"));
        assert!(cb("callback f : called by g.next").contains("neither `result` nor a parameter"));
        assert!(cb("callback f : called by result").contains("`result.method`"));
        assert!(cb("callback f : called by result.next, twice per call").contains("per call"));
        assert!(cb("callback f : called by result.next, with x + 1").contains("gives a name"));
        assert!(cb("callback f : called by result.next when n == 0").contains("no `when`"));
        assert_eq!(callback_binds("callback f : called by result.next, at most once per call, with x, returning y", &sc), ["x", "y"]);
        // a failure in a later call
        let fl = |k: &str, h: &str, t: &str| lint(k, t, &[h], &sc).join("; ");
        assert_eq!(fl("panics", "fails panic", "fails panic in result.next when n > 3"), "");
        assert_eq!(fl("fails", "fails", "fails panic in self.next when n > 3"), "");
        assert!(fl("panics", "fails panic", "fails panic in result.next n > 3").contains("followed by `when`"));
        assert!(fl("panics", "fails panic", "fails panic in result when n > 3").contains("`result.method`"));
        // `final(e)` needs a run named earlier in the subject, through any copy of the scope
        let fresh = || {
            let mut s = scope();
            s.names.extend(["self", "x", "y"].map(String::from));
            s.events = ["self", "f"].map(String::from).into();
            s
        };
        let sc2 = fresh();
        let ens = |s: &Scope, t: &str| lint("ensure", t, &["ensure"], s).join("; ");
        assert!(ens(&sc2, "ensure e : final(yields(self)) == []").contains("names it"));
        lint("callback", "callback f : called by result.next, with x", &["callback"], &sc2.clone());
        assert_eq!(ens(&sc2, "ensure e : final(yields(self)) == skip(count(y), old(yields(self)))"), "");
        assert_eq!(ens(&fresh(), "ensure e : present(old(yields(self)), 0) implies yields(self) == old(yields(self))[1..]"), "");
        // the new operators of the sequences
        let mut sc3 = fresh();
        sc3.names.insert("p".into());
        sc3.pure.insert("is_err".into());
        sc3.names.insert("is_err".into());
        for t in [
            "ensure a : yields(result) == select(y, old(yields(self))) and yields(result) == while_true(y, old(yields(self)))",
            "ensure b : yields(result) == flatten(map(yields, y)) and filter(is_err, yields(self))[0] == none",
            "ensure c : same(current(p), x) and owner(p)[x] == y and same(stored_key(self, x), old(stored_key(self, x)))",
        ] {
            assert_eq!(ens(&sc3, t), "", "{t}");
        }
        assert!(ens(&sc3, "ensure d : select(y) == []").contains("takes 2 arguments"));
        assert!(ens(&sc3, "ensure d : flatten(y, x) == []").contains("takes 1 argument"));
        assert!(ens(&sc3, "ensure d : current() == []").contains("takes 1 argument"));
        assert!(ens(&sc3, "ensure d : stored_key(self) == []").contains("takes 2 arguments"));
        assert!(ens(&sc3, "ensure d : map(y, yields(self)) == []").contains("not a named pure function") || ens(&sc3, "ensure d : map(y, yields(self)) == []").contains("parameter or callback"));
    }

    #[test]
    fn let_binds_a_name_or_a_pattern_for_the_clauses_after_it() {
        let mut sc = scope();
        sc.names.extend(["self", "mid"].map(String::from));
        let l = |s: &Scope, t: &str| lint("let", t, &[], s).join("; ");
        let ens = |s: &Scope, t: &str| lint("ensure", t, &["ensure"], s).join("; ");
        assert!(ens(&sc, "ensure a : count(head) == mid").contains("unknown name `head`"));
        assert_eq!(l(&sc, "let (head, tail) = result"), "");
        assert_eq!(ens(&sc, "ensure a : count(head) == mid and count(tail) == 0"), "");
        assert_eq!(l(&sc, "let n = count(head)"), "");
        assert_eq!(l(&sc, "let (_, m) = result"), "");
        assert_eq!(ens(&sc, "ensure b : n == m"), "");
        // a name is bound once, a pattern is names and `_`, and the expression is checked
        assert!(l(&sc, "let n = 1").contains("already bound"));
        assert!(l(&sc, "let (a, a) = result").contains("already bound"));
        assert!(l(&sc, "let mid = 1").contains("already bound"));
        assert!(l(&sc, "let (a b) = result").contains("tuple pattern"));
        assert!(l(&sc, "let () = result").contains("tuple pattern"));
        assert!(l(&sc, "let z").contains("`let p = e`"));
        assert!(l(&sc, "let z = nope").contains("unknown name `nope`"));
        assert!(lint("let", "let z = (1", &[], &sc).join(";").contains("unclosed"));
        assert!(lint("let", "x = 1", &[], &sc).join(";").contains("starts with `let`"));
        // the right side does not see the names it binds
        let fresh = Scope { names: sc.names.clone(), ..Scope::default() };
        assert!(l(&fresh, "let q = q").contains("unknown name `q`"));
    }

    #[test]
    fn the_many_elements_of_a_sequence_have_a_lifetime_together() {
        let mut sc = scope();
        sc.names.extend(["self", "Self"].map(String::from));
        let hold = |t: &str| lint("hold", t, &["hold", "lifetime"], &sc).join("; ");
        assert_eq!(hold("lifetime every item of yields(self) ends with self"), "");
        assert_eq!(hold("lifetime every item of old(yields(self)) ends in call"), "");
        assert!(hold("lifetime every item yields(self) ends with self").contains("names a sequence"));
        assert!(hold("lifetime every item of yields(self)").contains("names a sequence"));
        assert!(hold("lifetime every item of yields(zz) ends with self").contains("unknown name `zz`"));
        assert_eq!(hold("lifetime self ends in call when true"), "");
    }

    #[test]
    fn given_items_name_generics_methods_and_builds() {
        let builds: BTreeSet<String> = ["overflow_checks", "panic=unwind"].map(String::from).into();
        let ms: BTreeSet<String> = ["hash", "eq"].map(String::from).into();
        let ctx = GivenCtx { generics: [("Q".to_string(), Some(ms)), ("K".to_string(), None)].into(), has_self: false, trait_method: false, builds: &builds };
        let g = |items: &[&str]| lint_given(items, &ctx).join("; ");
        assert_eq!(g(&["Q.hash, Q.eq never panic", "K.anything terminate", "Q.eq effects none", "build overflow_checks", "build panic=unwind"]), "");
        assert!(g(&["Z.hash never panic"]).contains("`Z` is not a generic"));
        assert!(g(&["Q.nope never panic"]).contains("`nope` is not a method of a bound of `Q`"));
        assert!(g(&["build debug_assertions"]).contains("not a build setting"));
        assert!(g(&["default"]).contains("no default body"));
        assert!(g(&["Q.hash"]).contains("a `given` item is"));
        assert!(g(&["Q.hash Q.eq never panic"]).contains("separated by commas"));
        assert!(g(&["never panic"]).contains("a `given` item is"));
        assert!(g(&["Q never panic"]).contains("not `Param.method`"));
        let ctx = GivenCtx { trait_method: true, has_self: true, ..ctx };
        assert_eq!(lint_given(&["default", "Self.eq terminate"], &ctx), Vec::<String>::new());
    }
}
