//! The level each facet of a Y module needs (Y § 3.19), read from the Y source alone: a second inference, independent of
//! the one `bare_rust::level` makes from the bare bodies, so that the levels pinned in an example's constraint files
//! are checked against what the Y says and not only against the tool that proposed them.
//!
//! It reads the Y as statements in indented blocks, each with its strings and comments taken out (`code`), and decides
//! by the statement's words (`words`): what a statement starts with, the constraints a region header lists, the names
//! it calls. It is not a parser of Y's grammar, of which there is none yet.
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

/// A line without its strings (`"…"`, kept as `""`) and its comment (`-- …`).
fn code(l: &str) -> String {
    let (mut out, mut quoted, mut chars) = (String::new(), false, l.chars().peekable());
    while let Some(c) = chars.next() {
        if c == '"' {
            quoted = !quoted;
            out.push(c);
        } else if quoted {
            continue;
        } else if c == '-' && chars.peek() == Some(&'-') {
            break;
        } else {
            out.push(c);
        }
    }
    out
}

/// A statement's words and operators, in order: `x := y.add(z)` is `x`, `:=`, `y`, `.`, `add`, `(`, `z`, `)`.
fn words(t: &str) -> Vec<String> {
    let (mut out, mut word) = (vec![], String::new());
    let cs: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if c == ':' && cs.get(i + 1) == Some(&'=') {
                out.push(":=".into());
                i += 1;
            } else if !c.is_whitespace() {
                out.push(c.to_string());
            }
        }
        i += 1;
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
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
        let c = code(l);
        let ws = words(&c);
        let first = ws.first().map(String::as_str).unwrap_or("");
        let has = |w: &str| ws.iter().any(|x| x == w);
        let seq = |a: &str, b: &str| ws.windows(2).any(|p| p[0] == a && p[1] == b);
        if matches!(first, "for" | "while" | "mutable") || has(":=") {
            up(2, t);
        }
        if first == "arena" || (seq("shared", ".") && seq(".", "new")) {
            up(3, t);
        }
        // a region's header: `region r is c1, c2, …:` or `orelse region r is …:`, its constraints by their first words
        if first == "region" || (first == "orelse" && ws.get(1).is_some_and(|w| w == "region")) {
            let constraints = c.split_once(" is ").map(|(_, cs)| cs.trim_end_matches(':')).unwrap_or("");
            for part in constraints.split(',') {
                match words(part).first().map(String::as_str).unwrap_or("") {
                    "atomic" => up(3, t),
                    "within" | "forever" | "detach" | "waits" | "uncancellable" => up(4, t),
                    _ => {}
                }
            }
        }
        if first == "task" || first == "inject" || seq("system", ".") {
            up(4, t);
        }
        f.calls.extend(called(&c));
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
