//! The counts of a library file, for `results/libraries.md`: entries by kind, clauses by kind and status, where the
//! clauses come from, how the signatures checked, and the `unexpressed` promises grouped by what the language lacks.
//! Everything is read from the file, and from the signature check if one is passed in.
use crate::libcheck::SigReport;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

const KINDS: [&str; 10] = ["function", "type", "trait", "macro", "const", "impl", "alias", "static", "module", "use"];
const CLAUSE_KINDS: [&str; 23] = [
    "require", "let", "ensure", "on failure", "fails", "fails only", "panics", "never panics", "result must be used", "effects", "law", "callback", "protocol",
    "obliges", "invariant", "budget", "layout", "derive", "access", "hold", "implements", "compat", "unexpressed",
];

/// Whether a clause text takes a part of a tuple, `result.0`: a digit after a dot that follows a name, and not a number.
fn tuple_part(t: &str) -> bool {
    let cs: Vec<char> = t.chars().collect();
    (1..cs.len().saturating_sub(1)).any(|i| cs[i] == '.' && cs[i + 1].is_ascii_digit() && (cs[i - 1].is_alphabetic() || cs[i - 1] == '_' || cs[i - 1] == ')') && !cs.get(i + 2).is_some_and(|c| c.is_alphanumeric() || *c == '.'))
}

/// What a clause's `reason` says the language lacks: the text before the first `: `.
pub fn reason_label(reason: &str) -> &str {
    reason.split_once(": ").map_or(reason, |(l, _)| l)
}

/// Where a clause's `source` comes from: documentation, a body, an attribute, a list of impls, or something else.
pub fn source_kind(source: &str) -> &'static str {
    if source.contains(", doc: ") {
        "documentation"
    } else if source.contains(", body: ") {
        "body"
    } else if source.contains(", attribute ") {
        "attribute"
    } else if source.contains(", impls: ") {
        "impls"
    } else {
        "other"
    }
}

fn status_of(c: &Value, file: &Value) -> String {
    let s = if c["status"].is_null() { &file["status"] } else { &c["status"] };
    match s {
        Value::String(n) => n.clone(),
        Value::Object(m) => m.iter().next().map(|(k, v)| if k == "assumed" { format!("assumed ({})", v.as_str().unwrap_or("")) } else { k.clone() }).unwrap_or_default(),
        _ => "none".into(),
    }
}

fn row(out: &mut String, label: &str, n: usize, total: usize) {
    let _ = writeln!(out, "  {label:<48} {n:>4}  {:>5.1}%", if total == 0 { 0.0 } else { 100.0 * n as f64 / total as f64 });
}

/// The counts as text. `sigs` is the result of the signature check, when it was run.
pub fn libstats(file: &Value, sigs: Option<&SigReport>) -> String {
    let subjects: Vec<&Value> = file["subjects"].as_array().into_iter().flatten().collect();
    let mut out = String::new();
    let _ = writeln!(out, "library {} ({} subjects)", file["crate"].as_str().unwrap_or("?"), subjects.len());

    // entries by kind
    let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
    let mut traitm = 0;
    for s in &subjects {
        *by_kind.entry(s["kind"].as_str().unwrap_or("?")).or_default() += 1;
        if s["kind"] == "function" && !s["trait"].is_null() {
            traitm += 1;
        }
    }
    let _ = writeln!(out, "\nentries by kind");
    for k in KINDS {
        if let Some(n) = by_kind.get(k) {
            row(&mut out, &if k == "function" && traitm > 0 { format!("function ({traitm} are trait methods)") } else { k.to_string() }, *n, subjects.len());
        }
    }

    // clauses by kind, status and source
    let clauses: Vec<(&Value, &Value)> = subjects.iter().flat_map(|s| s["clauses"].as_array().into_iter().flatten().map(move |c| (*s, c))).collect();
    let total = clauses.len();
    let with: usize = subjects.iter().filter(|s| s["clauses"].as_array().is_some_and(|c| !c.is_empty())).count();
    let _ = writeln!(out, "\nclauses by kind ({total} clauses on {with} subjects)");
    for k in CLAUSE_KINDS {
        let n = clauses.iter().filter(|(_, c)| c["kind"] == k).count();
        if n > 0 {
            row(&mut out, k, n, total);
        }
    }
    // clauses qualified by `given` hold only when what they name holds: counted apart from the unconditional ones
    let given: Vec<&(&Value, &Value)> = clauses.iter().filter(|(_, c)| c["given"].as_array().is_some_and(|g| !g.is_empty())).collect();
    let _ = writeln!(out, "\nconditional clauses, qualified by `given` ({} on {} subjects)", given.len(), given.iter().map(|(s, _)| s["name"].as_str().unwrap_or("")).collect::<BTreeSet<_>>().len());
    for k in CLAUSE_KINDS {
        let n = given.iter().filter(|(_, c)| c["kind"] == k).count();
        if n > 0 {
            row(&mut out, k, n, given.len());
        }
    }
    let mut on: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, c) in &given {
        for g in c["given"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            *on.entry(if g == "default" { "the default body" } else if g.starts_with("build ") { "a build setting" } else { "a bound's implementations" }).or_default() += 1;
        }
    }
    for (k, n) in &on {
        row(&mut out, &format!("on {k}"), *n, given.iter().map(|(_, c)| c["given"].as_array().map_or(0, Vec::len)).sum());
    }
    // callback clauses that name a call's condition, arguments or result (language 6.13), and protocols over calls
    let callbacks: Vec<&(&Value, &Value)> = clauses.iter().filter(|(_, c)| c["kind"] == "callback").collect();
    let has = |c: &Value, w: &str| c["text"].as_str().is_some_and(|t| t.contains(&format!(" {w} ")) || t.contains(&format!(", {w} ")) || t.ends_with(&format!(" {w}")));
    let _ = writeln!(out, "\ncallback clauses ({} on {} subjects), and protocols over calls ({})", callbacks.len(), callbacks.iter().map(|(s, _)| s["name"].as_str().unwrap_or("")).collect::<BTreeSet<_>>().len(), clauses.iter().filter(|(_, c)| c["kind"] == "protocol").count());
    for (label, word) in [("with a condition (`when`)", "when"), ("with the call's arguments (`with`)", "with"), ("with the call's result (`returning`)", "returning")] {
        row(&mut out, label, callbacks.iter().filter(|(_, c)| has(c, word)).count(), callbacks.len());
    }
    // the small additions of language 6.14, counted by the form that carries them
    let stated_text = |c: &Value| c["kind"] != "unexpressed" && c["kind"] != "derive";
    let texts: Vec<(&str, &str)> = clauses.iter().filter(|(_, c)| stated_text(c)).map(|(_, c)| (c["kind"].as_str().unwrap_or(""), c["text"].as_str().unwrap_or(""))).collect();
    let words = |t: &str, w: &str| t.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')).any(|x| x == w);
    let small: [(&str, usize); 9] = [
        ("`evaluable at compile time` (6.14.1)", texts.iter().filter(|(k, t)| *k == "effects" && *t == "evaluable at compile time").count()),
        ("`must be used` on a type or trait (6.14.2)", texts.iter().filter(|(k, t)| *k == "result must be used" && t.starts_with("must be used")).count()),
        ("`moved from` (6.14.3)", texts.iter().filter(|(_, t)| words(t, "moved")).count()),
        ("`ends in call`, `survive call` (6.14.4)", texts.iter().filter(|(k, t)| *k == "hold" && t.starts_with("lifetime") && (t.ends_with("survive call") || t.contains(" ends in call"))).count()),
        ("`never panics except exhausted` (6.14.5)", texts.iter().filter(|(k, t)| *k == "never panics" && t.starts_with("never panics except exhausted")).count()),
        ("`expected`, `amortised` cost (6.14.6)", texts.iter().filter(|(k, t)| *k == "budget" && (words(t, "expected") || words(t, "amortised"))).count()),
        ("tuple parts, `result.0`, `is (a, b)` (6.14.7)", texts.iter().filter(|(_, t)| tuple_part(t) || t.contains(" is (")).count()),
        ("element frame, `mutates only p[e]` (6.14.8)", texts.iter().filter(|(k, t)| *k == "access" && t.starts_with("mutates only") && t.contains('[')).count()),
        ("`obliges caller` (6.14.9)", texts.iter().filter(|(k, _)| *k == "obliges").count()),
    ];
    let _ = writeln!(out, "\nsmall additions of language 6.14 ({} clauses)", small.iter().map(|(_, n)| n).sum::<usize>());
    for (label, n) in small {
        row(&mut out, label, n, total);
    }
    // the sequences of language 6.15, counted by the operator that carries them
    let has = |t: &str, w: &str| words(t, w) && t.contains(&format!("{w}("));
    // a `[` that follows a name, `)` or `]`: a slice when it holds `..`; an index when it follows `)` or `]` or holds a number
    // (`self[k]` with a name alone is a map's lookup, which is not a sequence)
    let bracket = |t: &str, slice: bool| {
        let cs: Vec<char> = t.chars().collect();
        (1..cs.len()).filter(|&i| cs[i] == '[' && (cs[i - 1].is_alphanumeric() || matches!(cs[i - 1], '_' | ')' | ']'))).any(|i| {
            let inner: String = cs[i + 1..].iter().take_while(|c| **c != ']').collect();
            let inner = inner.trim();
            if slice {
                inner.contains("..")
            } else {
                !inner.contains("..") && (matches!(cs[i - 1], ')' | ']') || inner.starts_with(|c: char| c.is_ascii_digit()))
            }
        })
    };
    let seq: [(&str, usize); 9] = [
        ("`yields(e)` (6.15)", texts.iter().filter(|(_, t)| has(t, "yields")).count()),
        ("indexing `xs[i]`", texts.iter().filter(|(_, t)| bracket(t, false)).count()),
        ("slices `xs[i..j]`", texts.iter().filter(|(_, t)| bracket(t, true)).count()),
        ("`++`", texts.iter().filter(|(_, t)| t.contains("++")).count()),
        ("`reverse`, `take`, `skip`", texts.iter().filter(|(_, t)| ["reverse", "take", "skip"].iter().any(|w| has(t, w))).count()),
        ("`zip`, `enumerate`", texts.iter().filter(|(_, t)| has(t, "zip") || has(t, "enumerate")).count()),
        ("`map`, `filter` by a named function", texts.iter().filter(|(_, t)| has(t, "map") || has(t, "filter")).count()),
        ("`scalars`, `deref`", texts.iter().filter(|(_, t)| words(t, "scalars") || words(t, "deref")).count()),
        ("`present`, `select`, `while_true`, `flatten`", texts.iter().filter(|(_, t)| ["present", "select", "while_true", "flatten"].iter().any(|w| has(t, w))).count()),
    ];
    let seq_total = texts.iter().filter(|(_, t)| has(t, "yields") || t.contains("++") || bracket(t, true) || bracket(t, false) || ["reverse", "take", "skip", "zip", "enumerate", "map", "filter"].iter().any(|w| has(t, w)) || words(t, "scalars") || words(t, "deref") || ["present", "select", "while_true", "flatten"].iter().any(|w| has(t, w))).count();
    let _ = writeln!(out, "\nsequences of language 6.15 ({seq_total} clauses)");
    for (label, n) in seq {
        row(&mut out, label, n, total);
    }
    // what a result's later calls, a handle and a sequence's elements take (language 4, 6.13, 7.3), counted by form
    let before_when = |t: &str| t.split(" when ").next().unwrap_or("").to_string();
    let later: [(&str, usize); 7] = [
        ("`callback … called by P.m`, a log of later calls (6.13)", texts.iter().filter(|(k, t)| *k == "callback" && words(t, "called") && words(t, "by")).count()),
        ("`fails F in P.m`, a failure of a later call (6.13)", texts.iter().filter(|(k, t)| (*k == "panics" || *k == "fails") && words(&before_when(t), "in")).count()),
        ("`final(e)`, the state at the end of a run (6.13)", texts.iter().filter(|(_, t)| has(t, "final")).count()),
        ("`current`, `owner`, `stored_key`, a handle's place (4)", texts.iter().filter(|(_, t)| ["current", "owner", "stored_key"].iter().any(|w| has(t, w))).count()),
        ("`lifetime every item of`, a sequence's elements (7.3)", texts.iter().filter(|(k, t)| *k == "hold" && t.starts_with("lifetime every item of")).count()),
        ("`let`, a name or a pattern `(a, b)` (4)", texts.iter().filter(|(k, _)| *k == "let").count()),
        ("`present(xs, i)`, a position that exists (6.15)", texts.iter().filter(|(_, t)| has(t, "present")).count()),
    ];
    let _ = writeln!(out, "\nlater calls, handles and bindings ({} clauses)", later.iter().map(|(_, n)| n).sum::<usize>());
    for (label, n) in later {
        row(&mut out, label, n, total);
    }
    let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, c) in &clauses {
        if c["kind"] == "unexpressed" {
            continue;
        }
        *by_status.entry(status_of(c, file)).or_default() += 1;
        *by_source.entry(source_kind(c["source"].as_str().unwrap_or(""))).or_default() += 1;
    }
    let stated = total - clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed").count();
    let _ = writeln!(out, "\nstated clauses by status ({stated}, `unexpressed` left out; the file's own status is {})", status_of(&Value::Null, file));
    for (k, n) in &by_status {
        row(&mut out, k, *n, stated);
    }
    let _ = writeln!(out, "\nstated clauses by source");
    for (k, n) in &by_source {
        row(&mut out, k, *n, stated);
    }
    let mut un_source: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, c) in clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed") {
        *un_source.entry(source_kind(c["source"].as_str().unwrap_or(""))).or_default() += 1;
    }
    let _ = writeln!(out, "\n`unexpressed` clauses by source ({})", total - stated);
    for (k, n) in &un_source {
        row(&mut out, k, *n, total - stated);
    }

    // type slots, and the ones that stayed host text
    let mut slots = 0;
    let mut host: BTreeMap<String, usize> = BTreeMap::new();
    let mut note = |t: &Value| {
        if t.is_null() {
            return;
        }
        slots += 1;
        if t.is_object() {
            *host.entry(t["form"].as_str().unwrap_or("unnamed").to_string()).or_default() += 1;
        }
    };
    for s in &subjects {
        note(&s["type"]);
        s["params"].as_array().into_iter().flatten().for_each(|p| note(&p["type"]));
        note(&s["result"]["type"]);
        note(&s["result"]["fails"]);
        s["fields"].as_array().into_iter().flatten().for_each(|f| note(&f["type"]));
        s["variants"].as_array().into_iter().flatten().flat_map(|v| v["fields"].as_array().into_iter().flatten()).for_each(|f| note(&f["type"]));
    }
    let hosts: usize = host.values().sum();
    let _ = writeln!(out, "\ntype slots ({slots}); {} map to the type language and {hosts} are host text", slots - hosts);
    for (k, n) in &host {
        row(&mut out, k, *n, slots);
    }

    // signatures
    if let Some(r) = sigs {
        let names = |v: &mut dyn Iterator<Item = &String>| -> BTreeSet<String> { v.cloned().collect() };
        let checked = names(&mut r.checked.iter());
        let resolved = names(&mut r.resolved.iter());
        let unchecked: BTreeSet<String> = r.unchecked.values().flatten().cloned().collect();
        let _ = writeln!(out, "\nsignatures, compiled against the real std ({} mismatched)", r.mismatches.len());
        let _ = writeln!(out, "  {:<12} {:>8} {:>14} {:>10} {:>10}", "kind", "checked", "path-resolved", "unchecked", "no slots");
        let (mut tc, mut tr, mut tu, mut tn) = (0, 0, 0, 0);
        for k in KINDS {
            let of: Vec<&&Value> = subjects.iter().filter(|s| s["kind"] == k).collect();
            if of.is_empty() {
                continue;
            }
            let count = |set: &BTreeSet<String>| of.iter().filter(|s| set.contains(s["name"].as_str().unwrap_or(""))).count();
            let (c, rs, u) = (count(&checked), count(&resolved), count(&unchecked));
            let n = of.len() - c - rs - u;
            let _ = writeln!(out, "  {k:<12} {c:>8} {rs:>14} {u:>10} {n:>10}");
            (tc, tr, tu, tn) = (tc + c, tr + rs, tu + u, tn + n);
        }
        let _ = writeln!(out, "  {:<12} {tc:>8} {tr:>14} {tu:>10} {tn:>10}", "all");
        for (why, v) in &r.unchecked {
            let _ = writeln!(out, "  unchecked ({why}): {}", v.join(", "));
        }
        if !r.resolved.is_empty() {
            let _ = writeln!(out, "  path-resolved only: {}", r.resolved.join(", "));
        }
    }

    // what the language lacks
    let mut groups: BTreeMap<String, (usize, BTreeSet<&str>)> = BTreeMap::new();
    let mut unlabelled = 0;
    for (s, c) in clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed") {
        let label = reason_label(c["reason"].as_str().unwrap_or(""));
        if c["reason"].as_str().is_none_or(|r| !r.contains(": ")) {
            unlabelled += 1;
        }
        let e = groups.entry(label.to_string()).or_default();
        e.0 += 1;
        e.1.insert(s["name"].as_str().unwrap_or("?"));
    }
    let un = total - stated;
    let _ = writeln!(out, "\n`unexpressed` promises, by what the language lacks ({un} clauses on {} subjects)", {
        let set: BTreeSet<&str> = clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed").map(|(s, _)| s["name"].as_str().unwrap_or("?")).collect();
        set.len()
    });
    let mut g: Vec<_> = groups.iter().collect();
    g.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
    let _ = writeln!(out, "  {:<48} {:>7} {:>9}", "lacks", "clauses", "subjects");
    for (label, (n, subs)) in g {
        let _ = writeln!(out, "  {label:<48} {n:>7} {:>9}", subs.len());
    }
    if unlabelled > 0 {
        let _ = writeln!(out, "  ({unlabelled} reasons carry no label)");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn labels_and_sources_are_read_from_the_text() {
        assert_eq!(reason_label("no sequence value: the language has no order"), "no sequence value");
        assert_eq!(reason_label("no colon here"), "no colon here");
        assert_eq!(source_kind("1.97.0 core/src/x.rs:1, doc: \"x\""), "documentation");
        assert_eq!(source_kind("1.97.0 core/src/x.rs:1, body: x"), "body");
        assert_eq!(source_kind("1.97.0 core/src/x.rs:1, attribute #[must_use]"), "attribute");
        assert_eq!(source_kind("hashbrown"), "other");
    }

    #[test]
    fn counts_come_from_the_file() {
        let f = json!({"crate": "std 1.97.0", "status": {"assumed": "hand-written"}, "subjects": [
            {"kind": "function", "name": "std::a", "clauses": [
                {"kind": "ensure", "text": "ensure x : true", "source": "s, doc: \"d\"", "status": {"assumed": "documentation"}},
                {"kind": "unexpressed", "text": "t", "source": "s", "reason": "no sequence value: why"}]},
            {"kind": "type", "name": "std::B", "clauses": [
                {"kind": "unexpressed", "text": "t", "source": "s", "reason": "no sequence value: why"}]}]});
        let t = libstats(&f, None);
        assert!(t.contains("library std 1.97.0 (2 subjects)"), "{t}");
        assert!(t.contains("function") && t.contains("type"), "{t}");
        assert!(t.contains("no sequence value") && t.contains("2 clauses on 2 subjects"), "{t}");
        assert!(t.contains("assumed (documentation)"), "{t}");
        assert!(t.contains("conditional clauses, qualified by `given` (0 on 0 subjects)"), "{t}");
        assert!(t.contains("callback clauses (0 on 0 subjects), and protocols over calls (0)"), "{t}");
    }

    #[test]
    fn callback_clauses_are_counted_by_what_they_name() {
        let f = json!({"crate": "std 1.97.0", "status": "lifted", "subjects": [
            {"kind": "function", "name": "std::a", "clauses": [
                {"kind": "callback", "text": "callback f : called at most once before return", "source": "s"},
                {"kind": "callback", "text": "callback g : called exactly once when exists self, with self, returning r", "source": "s"},
                {"kind": "callback", "text": "callback h : called exactly once when self is none, returning d", "source": "s"},
                {"kind": "protocol", "text": "protocol self.next+", "source": "s"}]}]});
        let t = libstats(&f, None);
        assert!(t.contains("callback clauses (3 on 1 subjects), and protocols over calls (1)"), "{t}");
        let line = |l: &str| t.lines().find(|x| x.contains(l)).unwrap_or("").split_whitespace().rev().nth(1).unwrap_or("").to_string();
        assert_eq!((line("(`when`)"), line("(`with`)"), line("(`returning`)")), ("2".into(), "1".into(), "2".into()), "{t}");
    }

    #[test]
    fn later_calls_handles_and_bindings_are_counted_by_form() {
        let c = |k: &str, t: &str| json!({"kind": k, "text": t, "source": "s"});
        let f = json!({"crate": "std 1.97.0", "status": "lifted", "subjects": [
            {"kind": "function", "name": "std::a", "clauses": [
                c("callback", "callback f : called by result.next, at most once per call, with x, returning y"),
                c("callback", "callback g : called exactly once, returning r"),
                c("panics", "fails panic in result.next when n > 3"),
                c("panics", "fails panic when n > 3 and old(count(self)) in x"),
                c("ensure", "ensure a : final(yields(self)) == skip(size(y), old(yields(self)))"),
                c("ensure", "ensure b : same(current(e), v) and present(xs, 0) and yields(result) == flatten(map(yields, y))"),
                c("access", "mutates only owner(self)[key(self)]"),
                c("hold", "lifetime every item of yields(self) ends with self"),
                c("let", "let (a, b) = result")]}]});
        let t = libstats(&f, None);
        let n = |l: &str| t.lines().find(|x| x.contains(l)).unwrap_or("").split_whitespace().rev().nth(1).unwrap_or("").to_string();
        assert!(t.contains("later calls, handles and bindings (8 clauses)"), "{t}");
        let got: Vec<String> = ["called by P.m", "fails F in P.m", "final(e)", "`current`, `owner`", "every item of", "`let`, a name", "present(xs, i)"].iter().map(|l| n(l)).collect();
        assert_eq!(got, ["1", "1", "1", "2", "1", "1", "1"], "{t}");
        assert!(t.lines().any(|l| l.trim_start().starts_with("let ") && l.contains('1')), "{t}");
        assert!(t.contains("`present`, `select`, `while_true`, `flatten`"), "{t}");
    }

    #[test]
    fn conditional_clauses_are_counted_apart() {
        let f = json!({"crate": "std 1.97.0", "status": "lifted", "subjects": [
            {"kind": "function", "name": "std::a", "clauses": [
                {"kind": "never panics", "text": "never panics", "given": ["Q.hash never panic", "build overflow_checks"], "source": "s"},
                {"kind": "ensure", "text": "ensure x : true", "given": ["default"], "source": "s"},
                {"kind": "ensure", "text": "ensure y : true", "source": "s"}]}]});
        let t = libstats(&f, None);
        assert!(t.contains("conditional clauses, qualified by `given` (2 on 1 subjects)"), "{t}");
        assert!(t.contains("on a build setting") && t.contains("on the default body") && t.contains("on a bound's implementations"), "{t}");
    }
}
