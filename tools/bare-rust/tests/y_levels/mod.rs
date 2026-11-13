//! The level each facet of a Y module needs (Y § 3.19), read from the Y source alone: a second inference, independent of
//! the one `bare_rust::level` makes from the bare bodies, so that the levels pinned in an example's constraint files
//! are checked against what the Y says and not only against the tool that proposed them.
//!
//! It reads the Y's tree, from `y-parse` (`tools/y-parse`), and decides by the kinds of its nodes.
//!
//! A function, a rule or a derived value is `calculations`. `structured` adds a `mutable` name or parameter, `:=`,
//! `for`, `while`, and recursion. `shared` adds an injected parameter its documentation gives as `shared<…>`, an
//! `atomic` region, an `arena`, and `shared.new(…)`. `effects` adds any other injected parameter, a region that is
//! `within` a bound, waits `forever` or is detached, `waits until`, is `uncancellable`, a `task`, an `inject`, and the
//! system's capabilities. A call of a function of the module needs what that function needs. A facet needs the most
//! its functions need.
use std::collections::{BTreeMap, BTreeSet};
use y_parse::Node;

pub const LEVELS: [&str; 5] = ["data", "calculations", "structured", "shared", "effects"];

struct Function {
    facet: Option<String>,
    level: usize,
    why: String,
    calls: BTreeSet<String>,
}

impl Function {
    fn up(&mut self, level: usize, why: String) {
        if level > self.level {
            (self.level, self.why) = (level, why);
        }
    }
}

/// What a function, a rule or a derived value needs, from its parameters and the nodes inside it.
fn function(d: &Node, facet: Option<&str>) -> Function {
    let mut f = Function { facet: facet.map(str::to_string), level: 1, why: format!("a {}", d.kind), calls: BTreeSet::new() };
    let doc = d.node("doc").map(|doc| doc.nodes("entries")).unwrap_or_default();
    for p in d.nodes("params") {
        let n = p.str("name").unwrap();
        if p.flag("injected") {
            // the parameter's type, from the documentation above the function: `n: …, as shared<…>.`
            let shared = doc.iter().any(|e| e.str("name") == Some(n) && e.str("text").unwrap().contains("as shared<"));
            f.up(if shared { 3 } else { 4 }, format!("injected {n}"));
        } else if p.flag("mutable") {
            f.up(2, format!("the parameter `mutable {n}`"));
        }
    }
    for key in ["where", "ensures", "body", "value"] {
        for top in d.nodes(key) {
            top.walk(&mut |n: &Node| {
                let at = |what: &str| format!("{what} at line {}", n.line);
                match n.kind {
                    "for" | "while" => f.up(2, at(&format!("`{}`", n.kind))),
                    "let" if n.flag("mutable") => f.up(2, at("a `mutable` name")),
                    "assign" => f.up(2, at("`:=`")),
                    "arena" => f.up(3, at("an arena")),
                    "task" | "inject" => f.up(4, at(&format!("`{}`", n.kind))),
                    "constraint" => match n.str("kind").unwrap() {
                        "atomic" => f.up(3, at("an atomic region")),
                        k @ ("within" | "forever" | "detach" | "waits" | "uncancellable") => f.up(4, at(&format!("a region that is `{k}`"))),
                        _ => {}
                    },
                    "member" if n.node("object").is_some_and(|o| o.kind == "name" && o.str("name") == Some("system")) => {
                        f.up(4, at("the system's capabilities"))
                    }
                    "call" => {
                        let callee = n.node("callee").unwrap();
                        let name = callee.str("name").unwrap_or("");
                        let on_shared = callee.kind == "member" && callee.node("object").is_some_and(|o| o.kind == "name" && o.str("name") == Some("shared"));
                        if on_shared && name == "new" {
                            f.up(3, at("`shared.new(…)`"));
                        }
                        if !name.is_empty() {
                            f.calls.insert(name.to_string());
                        }
                    }
                    _ => {}
                }
            });
        }
    }
    f
}

/// The level each facet of the Y module `source` needs, with the function and the reason that set it.
pub fn facet_levels(source: &str) -> BTreeMap<String, (usize, String)> {
    let tree = y_parse::parse(source).unwrap_or_else(|e| panic!("the Y source does not parse: {e}"));
    let mut fns: Vec<(String, Function)> = vec![];
    let mut add = |d: &Node, facet: Option<&str>| match d.kind {
        "function" | "rule" => fns.push((d.str("name").unwrap().to_string(), function(d, facet))),
        "structure" => {
            for m in d.nodes("members").into_iter().filter(|m| m.kind == "derived") {
                fns.push((m.str("name").unwrap().to_string(), function(m, facet)));
            }
        }
        _ => {}
    };
    for d in tree.nodes("decls") {
        if d.kind == "facet" {
            d.nodes("decls").into_iter().for_each(|x| add(x, d.str("name")));
        } else {
            add(d, None);
        }
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
