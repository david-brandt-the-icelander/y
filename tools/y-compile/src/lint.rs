//! The lint pass (Y § 3.15, "allow(Y2031) because …"): the lints y-compile runs on every Y file of a package, each with
//! a code and a name, and the two ways a lint is suppressed.
//!
//! - `allow(code) because "…"`, a statement, takes the lint's code (`Y2006`) or its name (`repeated_call`). It applies
//!   to the block it stands in and to the statement or declaration that opens that block: in a function's body, to the
//!   whole function, so it also covers what is judged at the function's line (`function_length`, `too_many_paths`).
//! - `-- lint: allow name because "…"`, a comment, applies to its own line, or, alone on a line, to the line after it.
//!
//! A lint is an error unless it says it is a warning; an error stops the build, as a compile error does. A lint that
//! cannot be suppressed (`shadowing`, as Y § 3.15 says of the official lints) is an error that names it wherever an
//! `allow` tries; so is an `allow` of a code no lint has. An `allow` that suppresses nothing is a warning.
use crate::facts::y_paths;
use std::collections::BTreeMap;
use y_parse::Node;

#[path = "../../bare-rust/tests/y_lint/mod.rs"]
mod y_lint;

/// A lint: its code, its name, and whether an `allow` may suppress it.
pub struct Lint {
    pub code: &'static str,
    pub name: &'static str,
    pub suppressible: bool,
    pub what: &'static str,
}

pub const LINTS: &[Lint] = &[
    Lint { code: "Y2001", name: "type_argument_name", suppressible: true, what: "a type argument is named `…_type`, and a value argument is not (Y § 3.1)" },
    Lint { code: "Y2002", name: "redundant_parentheses", suppressible: true, what: "parentheses around a single element (Y § 3.1)" },
    Lint { code: "Y2003", name: "function_length", suppressible: true, what: "a body of more than 60 lines is a warning, and of more than 120 an error (Y § 3.2)" },
    Lint { code: "Y2004", name: "too_many_paths", suppressible: true, what: "a function with 100 paths or more (Y § 3.14)" },
    Lint { code: "Y2005", name: "todo_comment", suppressible: true, what: "a `TODO` or `FIXME` comment, where a `gap:` belongs (Y § 3.16)" },
    Lint { code: "Y2006", name: "repeated_call", suppressible: true, what: "the same call in more than one condition of a `conditional` (Y § 3.5)" },
    Lint { code: "Y2007", name: "shadowing", suppressible: false, what: "a name bound again where it is already bound (Y § 3.15: an official lint)" },
    Lint { code: "Y2008", name: "abbreviation", suppressible: false, what: "an abbreviation in a name the file declares, not in the project's glossary (Y § 3.1: an official lint)" },
];

/// The abbreviations the lint knows, each with the word it stands for. A name that is a word is not judged: the lint
/// knows abbreviations, not the dictionary, so it finds the ones programmers reach for.
pub const ABBREVIATIONS: &[(&str, &str)] = &[
    ("abs", "absolute"), ("acc", "accumulator"), ("addr", "address"), ("alloc", "allocation"), ("amt", "amount"),
    ("app", "application"), ("arg", "argument"), ("args", "arguments"), ("arr", "array"), ("attr", "attribute"),
    ("auth", "authentication"), ("avg", "average"), ("bool", "boolean"), ("btn", "button"), ("buf", "buffer"),
    ("calc", "calculate"), ("cb", "callback"), ("cfg", "configuration"), ("chk", "check"),
    ("cmd", "command"), ("cnt", "count"), ("col", "column"), ("cols", "columns"), ("conf", "configuration"),
    ("config", "configuration"), ("conn", "connection"), ("coord", "coordinate"), ("ctx", "context"), ("cur", "current"),
    ("curr", "current"), ("db", "database"), ("decl", "declaration"), ("def", "definition"), ("del", "delete"),
    ("desc", "description"), ("dest", "destination"), ("dict", "dictionary"), ("diff", "difference"), ("dir", "directory"),
    ("dst", "destination"), ("elem", "element"), ("env", "environment"), ("eq", "equal"), ("err", "error"),
    ("evt", "event"), ("expr", "expression"), ("ext", "extension"), ("fmt", "format"), ("fn", "function"),
    ("func", "function"), ("gen", "generate"), ("hdr", "header"), ("idx", "index"), ("img", "image"),
    ("impl", "implementation"), ("inc", "increment"), ("info", "information"), ("init", "initial"), ("int", "integer"),
    ("iter", "iteration"), ("lbl", "label"), ("len", "length"), ("lhs", "left"), ("lib", "library"), ("ln", "line"),
    ("loc", "location"), ("max", "maximum"), ("mem", "memory"), ("misc", "miscellaneous"), ("mod", "module"),
    ("msg", "message"), ("mut", "mutable"), ("nav", "navigation"), ("num", "number"), ("obj", "object"),
    ("param", "parameter"), ("params", "parameters"), ("pct", "percent"), ("pkg", "package"),
    ("pos", "position"), ("prev", "previous"), ("proc", "process"), ("ptr", "pointer"), ("pwd", "password"),
    ("qty", "quantity"), ("ref", "reference"), ("req", "request"), ("res", "result"), ("resp", "response"),
    ("ret", "return"), ("rhs", "right"), ("sel", "selection"), ("seq", "sequence"), ("sig", "signature"),
    ("src", "source"), ("srv", "server"), ("std", "standard"), ("str", "string"), ("svc", "service"),
    ("sys", "system"), ("tbl", "table"), ("tmp", "temporary"), ("tx", "transaction"),
    ("txn", "transaction"), ("usr", "user"), ("util", "utility"), ("val", "value"), ("var", "variable"),
    ("vec", "vector"), ("ver", "version"), ];

/// The few abbreviations every Y programmer learns once (Y § 3.1), beside the project's glossary: the standard unit
/// symbols and currency codes. The sized numeric types are names of their own (`int64`, `float64`), not split.
const UNIT_SYMBOLS: &[&str] = &["s", "ms", "us", "ns", "min", "h", "d", "m", "km", "cm", "mm", "kg", "g", "b", "kb", "mb", "gb", "tb"];
const CURRENCIES: &[&str] = &["isk", "eur", "usd", "gbp", "jpy", "chf", "sek", "nok", "dkk", "cad", "aud", "cny"];
const SIZED: &[&str] = &["int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float32", "float64"];

/// What a package allows beyond the lint's own lists: its glossary (`y/glossary`, one `term: what it stands for` a
/// line) and the unit symbols it declares.
#[derive(Default, Clone)]
pub struct Words {
    pub glossary: Vec<String>,
    pub units: Vec<String>,
}

impl Words {
    /// The glossary's terms, from its text: each line `term: what it stands for`; blank lines and `--` comments aside.
    pub fn read_glossary(text: &str) -> Result<Vec<String>, String> {
        let mut out = vec![];
        for (k, l) in text.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with("--") {
                continue;
            }
            match l.split_once(':') {
                Some((t, what)) if !t.trim().is_empty() && !what.trim().is_empty() => out.push(t.trim().to_string()),
                _ => return Err(format!("y/glossary:{}: a term is written `term: what it stands for`", k + 1)),
            }
        }
        Ok(out)
    }

    fn allows(&self, part: &str) -> bool {
        self.glossary.iter().any(|g| g == part) || self.units.iter().any(|u| u == part) || UNIT_SYMBOLS.contains(&part) || CURRENCIES.contains(&part) || SIZED.contains(&part)
    }
}

/// The node kinds whose `name` the file declares: a name used is judged where it is declared, and a name the runtime
/// gives (a method, a type argument) is the standard library's.
const DECLARING: &[&str] = &[
    "module", "facet", "function", "rule", "transition", "handler", "workflow", "test", "param", "field", "derived",
    "invariant", "case", "structure", "type_decl", "enumeration", "state", "trait", "command", "schema", "constants",
    "constant", "unit", "service", "let", "for", "while", "if", "else", "condition", "conditional", "compare", "region",
    "task", "assert", "assume", "where", "ensures", "binding", "arena", "metric",
];

fn lint_named(code_or_name: &str) -> Option<&'static Lint> {
    LINTS.iter().find(|l| l.code == code_or_name || l.name == code_or_name)
}

/// What a lint found: where, which lint, what, and whether it is only a warning.
struct Finding {
    line: usize,
    lint: &'static Lint,
    message: String,
    warning: bool,
}

/// The last line a node spans.
fn last_line(n: &Node) -> usize {
    let mut last = n.num("end").unwrap_or(n.line).max(n.line);
    for c in n.children() {
        last = last.max(last_line(c));
    }
    last
}

/// The lints of one Y file: (errors, warnings), each `file:line: name (code): …`. Paths are counted in the module's
/// own file (`module`): a test file or `main` reads the units and types the module declares.
pub fn lint(file: &str, source: &str, tree: &Node, module: bool, words: &Words) -> (Vec<String>, Vec<String>) {
    let mut found: Vec<Finding> = vec![];
    let find = |name: &str| lint_named(name).unwrap();

    // abbreviations, in each name the file declares and each function's result
    tree.walk(&mut |n: &Node| {
        let mut names: Vec<&str> = vec![];
        if DECLARING.contains(&n.kind) {
            names.extend(n.str("name"));
        }
        if matches!(n.kind, "function" | "rule" | "transition" | "handler" | "workflow") {
            names.extend(n.str("returns"));
        }
        for name in names {
            for part in name.split(['_', '.']).filter(|p| !p.is_empty() && !p.bytes().all(|b| b.is_ascii_digit())) {
                if let Some((_, word)) = ABBREVIATIONS.iter().find(|(a, _)| *a == part).filter(|_| !words.allows(part)) {
                    found.push(Finding { line: n.line, lint: find("abbreviation"), message: format!("`{name}`: `{part}` abbreviates `{word}`; write the word, or put `{part}` in the project's glossary (`y/glossary`)"), warning: false });
                }
            }
        }
    });

    // type_argument_name, by the lint the examples' checks run; it reads its own `-- lint: allow` comments
    for e in y_lint::type_argument_names_in(file, tree) {
        let (line, message) = match e.strip_prefix(&format!("{file}:")).and_then(|r| r.split_once(": ")) {
            Some((l, m)) => (l.parse().unwrap_or(0), m.trim_start_matches("type_argument_name: ").to_string()),
            None => (0, e.clone()),
        };
        found.push(Finding { line, lint: find("type_argument_name"), message, warning: false });
    }

    // parentheses around a single element
    tree.walk(&mut |n: &Node| {
        let single = matches!(n.kind, "name" | "literal" | "member" | "index" | "call" | "list") || (n.kind == "unary" && n.str("op") == Some("-"));
        if n.flag("parens") && single {
            found.push(Finding { line: n.line, lint: find("redundant_parentheses"), message: format!("parentheses around a single element, a {}", n.kind), warning: false });
        }
    });

    // TODO and FIXME
    for c in tree.nodes("comments") {
        let text = c.str("text").unwrap_or("");
        if text.split(|ch: char| !ch.is_alphanumeric()).any(|w| w == "TODO" || w == "FIXME") {
            found.push(Finding { line: c.line, lint: find("todo_comment"), message: format!("`-- {text}`: write the unfinished work as a `gap:` (Y § 3.16)"), warning: false });
        }
    }

    // each callable: its length, its paths, its conditionals and the names it binds
    let callables = y_paths::callables(tree);
    let paths = if module { y_paths::all_paths(source).0 } else { BTreeMap::new() };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for p in paths.keys() {
        *counts.entry(p.split(" > ").next().unwrap_or("").to_string()).or_default() += 1;
    }
    for f in &callables {
        let name = f.str("name").unwrap_or("");
        let body = f.nodes("body");
        if let (Some(first), Some(last)) = (body.first(), body.last()) {
            let n = last_line(last) - first.line + 1;
            if n > 60 {
                let warning = n <= 120;
                found.push(Finding { line: f.line, lint: find("function_length"), message: format!("`{name}`'s body has {n} lines, more than {}", if warning { 60 } else { 120 }), warning });
            }
        }
        if let Some(&n) = counts.get(name).filter(|&&n| n >= 100) {
            found.push(Finding { line: f.line, lint: find("too_many_paths"), message: format!("`{name}` has {n} paths, 100 or more"), warning: false });
        }
    }
    tree.walk(&mut |n: &Node| {
        if n.kind != "conditional" || n.node("subject").is_some() {
            return;
        }
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for c in n.nodes("conditions") {
            let mut calls = vec![];
            if let Some(t) = c.node("test") {
                t.walk(&mut |e: &Node| {
                    if e.kind == "call" {
                        calls.push(y_paths::expression_text(e));
                    }
                });
            }
            calls.sort();
            calls.dedup();
            for call in calls {
                if let Some(first) = seen.get(&call) {
                    found.push(Finding { line: c.line, lint: find("repeated_call"), message: format!("`{call}` is called again, as on line {first}: evaluate it once, under the label"), warning: false });
                } else {
                    seen.insert(call, c.line);
                }
            }
        }
    });
    for f in tree.find("function").into_iter().chain(tree.find("test")).chain(tree.find("handler")).chain(tree.find("workflow")).chain(tree.find("transition")) {
        let params: Vec<String> = f.nodes("params").iter().filter_map(|p| p.str("name")).map(str::to_string).collect();
        let mut scopes = vec![params];
        let blocks: Vec<Vec<&Node>> = if f.kind == "test" { vec![f.nodes("sections").iter().flat_map(|s| s.nodes("body")).collect()] } else { vec![f.nodes("body")] };
        for b in blocks {
            shadowing(&b, &mut scopes, &mut found);
        }
    }

    // the suppressions: `allow(…)` statements over their blocks, and `-- lint: allow name` comments over their lines
    let mut errors: Vec<(usize, String)> = vec![];
    let mut warnings: Vec<(usize, String)> = vec![];
    let mut allows: Vec<(usize, usize, &'static Lint, usize, bool)> = vec![];
    collect_allows(tree, None, file, &mut allows, &mut errors);
    let comments: Vec<(usize, bool, String)> = tree
        .nodes("comments")
        .iter()
        .filter_map(|c| {
            let text = c.str("text").unwrap_or("");
            let rest = text.strip_prefix("lint: allow ")?;
            Some((c.line, c.flag("alone"), rest.split_whitespace().next().unwrap_or("").to_string()))
        })
        .collect();
    for (line, _, name) in &comments {
        match lint_named(name) {
            None => errors.push((*line, format!("{file}:{line}: `-- lint: allow {name}`: no lint is named `{name}`"))),
            Some(l) if !l.suppressible => errors.push((*line, format!("{file}:{line}: `{}` ({}) cannot be suppressed (Y § 3.15)", l.name, l.code))),
            _ => {}
        }
    }
    let mut used = vec![false; allows.len()];
    for f in found {
        let by_allow = allows.iter().position(|(from, to, l, _, _)| l.code == f.lint.code && f.lint.suppressible && (*from..=*to).contains(&f.line));
        let by_comment = f.lint.suppressible && comments.iter().any(|(line, alone, name)| (name == f.lint.name || name == f.lint.code) && (*line == f.line || (*alone && line + 1 == f.line)));
        if let Some(k) = by_allow {
            used[k] = true;
            continue;
        }
        if by_comment {
            continue;
        }
        let text = format!("{file}:{}: {} ({}): {}", f.line, f.lint.name, f.lint.code, f.message);
        if f.warning { warnings.push((f.line, text)) } else { errors.push((f.line, text)) }
    }
    for (k, (_, _, l, line, ok)) in allows.iter().enumerate() {
        if *ok && !used[k] {
            warnings.push((*line, format!("{file}:{line}: `allow({})` suppresses nothing: no `{}` is found where it applies", l.code, l.name)));
        }
    }
    errors.sort();
    warnings.sort();
    (errors.into_iter().map(|e| e.1).collect(), warnings.into_iter().map(|w| w.1).collect())
}

/// The `allow` statements under `n`: (first line, last line, lint, the statement's line, whether it is valid), each
/// over the block it stands in and what opens that block.
fn collect_allows(n: &Node, owner: Option<&Node>, file: &str, out: &mut Vec<(usize, usize, &'static Lint, usize, bool)>, errors: &mut Vec<(usize, String)>) {
    for st in n.nodes("body") {
        if st.kind == "allow" {
            let code = st.str("code").unwrap_or("").trim();
            let (from, to) = (owner.unwrap_or(n).line, last_line(owner.unwrap_or(n)));
            match lint_named(code) {
                None => errors.push((st.line, format!("{file}:{}: `allow({code})`: no lint has the code or name `{code}`", st.line))),
                Some(l) if !l.suppressible => {
                    errors.push((st.line, format!("{file}:{}: `{}` ({}) cannot be suppressed (Y § 3.15)", st.line, l.name, l.code)));
                    out.push((from, to, l, st.line, false));
                }
                Some(l) => out.push((from, to, l, st.line, true)),
            }
        }
    }
    for c in n.children() {
        collect_allows(c, Some(c), file, out, errors);
    }
}

/// A `let` of a name already bound where it stands: a parameter, or a name an enclosing block or this one bound.
fn shadowing<'a>(block: &[&'a Node], scopes: &mut Vec<Vec<String>>, found: &mut Vec<Finding>) {
    scopes.push(vec![]);
    for st in block {
        if st.kind == "let" {
            let name = st.str("name").unwrap_or("");
            if scopes.iter().any(|s| s.iter().any(|x| x == name)) {
                found.push(Finding { line: st.line, lint: lint_named("shadowing").unwrap(), message: format!("`{name}` is already bound here: give the new value a name of its own"), warning: false });
            }
            scopes.last_mut().unwrap().push(name.to_string());
        }
        // the blocks the statement opens, each a scope of its own; a loop's variable is bound in its body
        let mut inner: Vec<(Option<String>, Vec<&Node>)> = vec![];
        if !st.nodes("body").is_empty() {
            let var = (st.kind == "for").then(|| st.str("name").unwrap_or("").to_string());
            inner.push((var, st.nodes("body")));
        }
        for c in st.children() {
            if matches!(c.kind, "else" | "condition" | "region") && !c.nodes("body").is_empty() {
                inner.push((None, c.nodes("body")));
            }
        }
        for c in st.nodes("conditions") {
            if !inner.iter().any(|(_, b)| b.first().map(|x| x.line) == c.nodes("body").first().map(|x| x.line)) {
                inner.push((None, c.nodes("body")));
            }
        }
        for (var, b) in inner {
            scopes.push(var.into_iter().collect());
            shadowing(&b, scopes, found);
            scopes.pop();
        }
    }
    scopes.pop();
}
