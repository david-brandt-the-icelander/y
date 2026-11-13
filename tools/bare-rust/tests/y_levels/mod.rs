//! The level each facet of a Y module needs (Y § 3.19), read from the Y source alone: a second inference, independent of
//! the one `bare_rust::level` makes from the bare bodies, so that the levels pinned in an example's constraint files
//! are checked against what the Y says and not only against the tool that proposed them.
//!
//! A function, a rule or a derived value is `calculations`. `structured` adds a `mutable` name or parameter, `:=`,
//! `for`, `while`, and recursion. `shared` adds an injected parameter its documentation gives as `shared<…>`, an
//! `atomic` region, an `arena`, and `shared.new(…)`. `effects` adds any other injected parameter, a region that is
//! `within` a bound, waits `forever` or is detached, `waits until`, a `task`, and the system's capabilities. A call of
//! a function of the module needs what that function needs. A facet needs the most its functions need.
use std::collections::{BTreeMap, BTreeSet};

pub const LEVELS: [&str; 5] = ["data", "calculations", "structured", "shared", "effects"];

struct Function {
    facet: Option<String>,
    indent: usize,
    level: usize,
    why: String,
    calls: BTreeSet<String>,
}

fn indent(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

/// A word of a line followed by `(`: the names a line calls, `f(` and `.f(` alike.
fn called(t: &str) -> Vec<String> {
    let cs: Vec<char> = t.chars().collect();
    let mut out = vec![];
    for (i, c) in cs.iter().enumerate() {
        if *c == '(' {
            let mut j = i;
            while j > 0 && (cs[j - 1].is_alphanumeric() || cs[j - 1] == '_') {
                j -= 1;
            }
            if j < i {
                out.push(cs[j..i].iter().collect());
            }
        }
    }
    out
}

/// A Y declaration's head: `function f(params)`, `rule f(params)` or `derived f is`, as (name, parameters).
fn head(l: &str) -> Option<(String, String)> {
    let t = l.trim_start().trim_start_matches("public ");
    for kw in ["function ", "rule "] {
        if let Some(rest) = t.strip_prefix(kw) {
            let (name, after) = rest.split_once('(')?;
            return Some((name.to_string(), after.split(')').next().unwrap_or("").to_string()));
        }
    }
    let rest = t.strip_prefix("derived ")?;
    Some((rest.split(" is").next()?.to_string(), String::new()))
}

/// The level each facet of the Y module `source` needs, with the function and the reason that set it.
pub fn facet_levels(source: &str) -> BTreeMap<String, (usize, String)> {
    let mut fns: Vec<(String, Function)> = vec![];
    let (mut facet, mut current, mut doc): (Option<String>, Option<usize>, Vec<String>) = (None, None, vec![]);
    for l in source.lines() {
        let t = l.trim();
        if indent(l) == 0 && !t.is_empty() && !t.starts_with("--") {
            facet = t.trim_start_matches("public ").strip_prefix("facet ").map(|f| f.trim_end_matches(':').to_string());
            current = None;
            continue;
        }
        if let Some((name, params)) = head(l) {
            let mut f = Function { facet: facet.clone(), indent: indent(l), level: 1, why: "a function".into(), calls: BTreeSet::new() };
            for p in params.split(',').map(str::trim) {
                if let Some(n) = p.strip_prefix("injected ") {
                    // the parameter's type, from the documentation above the function: `n: …, as shared<…>.`
                    let shared = doc.iter().any(|d| d.trim_start().starts_with(&format!("{n}: ")) && d.contains("as shared<"));
                    let level = if shared { 3 } else { 4 };
                    if level > f.level {
                        (f.level, f.why) = (level, format!("injected {n}"));
                    }
                } else if p.starts_with("mutable ") && f.level < 2 {
                    (f.level, f.why) = (2, format!("the parameter `{p}`"));
                }
            }
            fns.push((name, f));
            current = Some(fns.len() - 1);
            doc.clear();
            continue;
        }
        if let Some(i) = current {
            if !t.is_empty() && indent(l) <= fns[i].1.indent {
                current = None;
            }
        }
        let Some(i) = current else {
            doc.push(l.to_string());
            continue;
        };
        let f = &mut fns[i].1;
        let mut up = |level: usize, why: &str| {
            if level > f.level {
                (f.level, f.why) = (level, why.chars().take(50).collect());
            }
        };
        if t.starts_with("for ") || t.starts_with("while ") || t.starts_with("mutable ") || t.contains(" := ") {
            up(2, t);
        }
        if t.contains("is atomic") || t.contains(" atomic,") || t.starts_with("arena ") || t.contains("shared.new(") {
            up(3, t);
        }
        if ["forever because", "detach because", "waits until", "system."].iter().any(|w| t.contains(w))
            || t.contains(" is within ")
            || t.contains(", within ")
            || t.starts_with("task ")
            || t.starts_with("inject(")
        {
            up(4, t);
        }
        f.calls.extend(called(t));
    }
    // a call of a function of the module needs what it needs; a call of itself is recursion
    loop {
        let mut changed = false;
        for i in 0..fns.len() {
            for c in fns[i].1.calls.clone() {
                let same: Vec<usize> = (0..fns.len()).filter(|j| fns[*j].0 == c).collect();
                for j in same {
                    let (level, why) = if i == j { (2, "recursion".to_string()) } else { (fns[j].1.level, format!("a call of {c}")) };
                    if level > fns[i].1.level {
                        (fns[i].1.level, fns[i].1.why) = (level, why);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut out: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for (name, f) in &fns {
        if let Some(facet) = &f.facet {
            if out.get(facet).is_none_or(|(l, _)| f.level > *l) {
                out.insert(facet.clone(), (f.level, format!("{name}: {}", f.why)));
            }
        }
    }
    out
}
