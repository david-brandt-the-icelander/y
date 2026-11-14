//! The execution paths through each function of a Y module, under the path rules of Y § 3.5 and § 3.14. There is no Y
//! compiler yet; this is what its path enumeration would give, over the Y's tree from `y-parse` (`tools/y-parse`).
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
//! branches taken, the conditions found false, `where` preconditions, the `ensures` of a called function, and
//! constants. A sub-outcome the facts rule out is no path: that is what "proven" means here.
//!
//! The facts are kept on expressions by their text, as `text` writes a node: `game.misses.count()`. This is a port of
//! `tools/paths/yp.py`: change both together, and keep their output the same.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use y_parse::{Node, Value};

const ORDERED: [&str; 9] = ["int32", "uint32", "uint16", "int64", "uint64", "duration", "instant", "count", "int"];
const UNSIGNED: [&str; 5] = ["uint32", "uint16", "uint64", "duration", "count"];
/// How a path that needs no test ends.
pub const EXEMPT: [&str; 2] = ["not_reachable", "not_implemented"];
const ORDERS3: [&str; 3] = ["lt", "eq", "gt"];

// ---------------------------------------------------------------------------------------------------------------------
// Expressions: nodes of the tree, and nodes made here

fn name_node(n: &str) -> Node {
    Node::new("name", 0).with("name", n)
}

fn member_node(obj: Node, n: &str) -> Node {
    Node::new("member", 0).with("object", obj).with("name", n)
}

fn call_node(callee: Node) -> Node {
    Node::new("call", 0).with("callee", callee).with("args", Vec::<Node>::new())
}

/// `obj.count()`.
fn count(obj: &Node) -> Node {
    call_node(member_node(obj.clone(), "count"))
}

fn s<'a>(e: &'a Node, key: &str) -> &'a str {
    e.str(key).unwrap_or("")
}

fn n<'a>(e: &'a Node, key: &str) -> &'a Node {
    e.node(key).unwrap_or_else(|| panic!("a {} with no {key}", e.kind))
}

/// An expression as text, the key facts are kept by: brackets where the source has them, but not around the whole.
fn text(e: &Node, top: bool) -> String {
    let out = match e.kind {
        "name" => s(e, "name").to_string(),
        "literal" => s(e, "text").to_string(),
        "member" => format!("{}.{}", text(n(e, "object"), false), s(e, "name")),
        "call" => {
            let args: Vec<String> = e
                .nodes("args")
                .iter()
                .map(|a| format!("{}{}", a.str("name").map_or(String::new(), |x| format!("{x}: ")), text(n(a, "value"), false)))
                .collect();
            format!("{}({})", text(n(e, "callee"), false), args.join(", "))
        }
        "index" => format!("{}[{}]", text(n(e, "object"), false), text(n(e, "index"), false)),
        "binary" => format!("{} {} {}", text(n(e, "left"), false), s(e, "op"), text(n(e, "right"), false)),
        "unary" => format!("{}{}", if s(e, "op") == "not" { "not " } else { "-" }, text(n(e, "operand"), false)),
        "list" => format!("[{}]", e.nodes("items").iter().map(|i| text(i, false)).collect::<Vec<_>>().join(", ")),
        "mutable" | "fake" | "compile" => format!("{} {}", e.kind, text(n(e, "value"), false)),
        "or_fail" => format!("{} or fail", text(n(e, "value"), false)),
        "forever" => format!("{} forever because \"{}\"", text(n(e, "value"), false), s(e, "because")),
        k => panic!("no text for a {k}"),
    };
    if e.flag("parens") && !top {
        format!("({out})")
    } else {
        out
    }
}

fn t(e: &Node) -> String {
    text(e, true)
}

/// An expression as text, as the facts are kept on it: `game.misses.count()`, `words[chosen]`.
pub fn expression_text(e: &Node) -> String {
    text(e, true)
}

/// A name or a path of names, `game.misses`, as text; None for anything else.
fn chain(e: &Node) -> Option<String> {
    match e.kind {
        "name" => Some(s(e, "name").to_string()),
        "member" => Some(format!("{}.{}", chain(n(e, "object"))?, s(e, "name"))),
        _ => None,
    }
}

/// The name a path of members and indexes starts from: `game` of `game.squares[i]`; None when it starts otherwise.
fn root(e: &Node) -> Option<String> {
    let mut e = e;
    while e.kind == "member" || e.kind == "index" {
        e = n(e, "object");
    }
    (e.kind == "name").then(|| s(e, "name").to_string())
}

/// `obj` when `e` is `obj.m(…)` for a method `m` among `names` (any, when there are none), with `args` arguments when
/// it says how many; else None.
fn method<'a>(e: &'a Node, names: &[&str], args: Option<usize>) -> Option<&'a Node> {
    if e.kind != "call" {
        return None;
    }
    let callee = n(e, "callee");
    let named = names.is_empty() || names.contains(&s(callee, "name"));
    (callee.kind == "member" && named && args.is_none_or(|k| e.nodes("args").len() == k)).then(|| n(callee, "object"))
}

/// The argument named `name` of a call.
fn arg<'a>(e: &'a Node, name: &str) -> Option<&'a Node> {
    e.nodes("args").into_iter().find(|a| a.str("name") == Some(name)).map(|a| n(a, "value"))
}

/// The arguments' names of a call, `None` for one without.
fn arg_names(e: &Node) -> Vec<Option<&str>> {
    e.nodes("args").iter().map(|a| a.str("name")).collect()
}

fn is_literal(e: &Node, ty: &str) -> bool {
    e.kind == "literal" && s(e, "type") == ty
}

/// A whole number, `6` or `-6`.
fn is_int(e: &Node) -> bool {
    if e.kind == "unary" && s(e, "op") == "-" {
        let o = n(e, "operand");
        return is_literal(o, "integer") && !o.flag("parens");
    }
    is_literal(e, "integer")
}

/// The operands of a chain of `op` (`and` or `or`), as the source groups them: `(a or b) or c` has two.
fn operands<'a>(e: &'a Node, op: &str) -> Vec<&'a Node> {
    if e.kind == "binary" && s(e, "op") == op {
        let left = n(e, "left");
        let mut out = if left.flag("parens") { vec![left] } else { operands(left, op) };
        out.push(n(e, "right"));
        return out;
    }
    vec![e]
}

/// An expression without its `or fail` and its `forever because "…"`.
fn unwrap(e: &Node) -> &Node {
    let mut e = e;
    while e.kind == "or_fail" || e.kind == "forever" {
        e = n(e, "value");
    }
    e
}

/// An expression as facts are kept on it: a count converted to another whole number is the same count.
fn norm(e: &Node) -> &Node {
    match method(e, &["to_int32", "to_uint32"], Some(0)) {
        Some(inner) if method(inner, &["count"], Some(0)).is_some() => inner,
        _ => e,
    }
}

/// A type that has no arguments, by its name.
fn ty(name: &str) -> Node {
    Node::new("type", 0).with("name", name).with("args", Vec::<Node>::new()).with("text", name)
}

fn type_text(t: &Option<Node>) -> Option<&str> {
    t.as_ref().map(|t| s(t, "text"))
}

/// The element type of `list<element_type=T>`.
fn element(t: &Option<Node>) -> Option<Node> {
    let t = t.as_ref()?;
    let args = t.nodes("args");
    (s(t, "name") == "list" && args.len() == 1 && s(args[0], "name") == "element_type" && n(args[0], "value").kind == "type")
        .then(|| n(args[0], "value").clone())
}

/// A path of names and indexes whose index is a plain path or a number: `game.squares[i]`, `a[b.c]`.
fn plain(e: &Node) -> bool {
    match e.kind {
        "name" => true,
        "member" => plain(n(e, "object")),
        "index" => {
            let i = n(e, "index");
            plain(n(e, "object")) && (plain(i) || is_literal(i, "integer"))
        }
        _ => false,
    }
}

/// A path of names and indexes by a name or a number: `game.squares[i]`, not `f(x).y`.
fn simple(e: &Node) -> bool {
    match e.kind {
        "name" => true,
        "member" => simple(n(e, "object")),
        "index" => {
            let i = n(e, "index");
            simple(n(e, "object")) && (i.kind == "name" || is_literal(i, "integer"))
        }
        _ => false,
    }
}

/// `e` with every name `old`, and every member named `old`, renamed `new`.
fn renamed(e: &Node, old: &str, new: &str) -> Node {
    fn fix(v: &mut Value, old: &str, new: &str) {
        match v {
            Value::Node(x) => fix_node(x, old, new),
            Value::List(vs) => vs.iter_mut().for_each(|v| fix(v, old, new)),
            _ => {}
        }
    }
    fn fix_node(x: &mut Node, old: &str, new: &str) {
        let rename = (x.kind == "name" || x.kind == "member") && x.str("name") == Some(old);
        for (k, v) in x.fields.iter_mut() {
            if *k == "name" && rename {
                *v = Value::Str(new.to_string());
            } else {
                fix(v, old, new);
            }
        }
    }
    let mut out = e.clone();
    fix_node(&mut out, old, new);
    out
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

/// What a name's type is known to be; a name with `None` is known, and its type is not.
type Env = HashMap<String, Option<Node>>;

pub struct Module {
    tree: Node,
    fields: HashMap<String, Node>,
    constants: HashMap<String, Node>,
    values: HashMap<String, Node>,
    results: HashMap<String, (Option<String>, Option<Node>)>,
    params: HashMap<String, Vec<(String, Option<Node>)>>,
    contracts: HashMap<String, Vec<(&'static str, Node)>>,
    /// a refined type's constraints on its values' counts: {type: [(op, n)]}
    refinements: HashMap<String, Vec<(String, i64)>>,
    groups: HashSet<String>,
}

/// Each declaration of a file, in a facet or not.
fn declarations(tree: &Node) -> Vec<&Node> {
    tree.nodes("decls").into_iter().flat_map(|d| if d.kind == "facet" { d.nodes("decls") } else { vec![d] }).collect()
}

impl Module {
    pub fn new(text: &str) -> Module {
        let tree = y_parse::parse(text).unwrap_or_else(|e| panic!("the Y source does not parse: {e}"));
        let mut m = Module {
            tree: Node::new("file", 1),
            fields: HashMap::new(),
            constants: HashMap::new(),
            values: HashMap::new(),
            results: HashMap::new(),
            params: HashMap::new(),
            contracts: HashMap::new(),
            refinements: HashMap::new(),
            groups: HashSet::new(),
        };
        m.collect(&tree);
        m.tree = tree;
        m.groups = m.constants.keys().map(|k| k.split('.').next().unwrap().to_string()).collect();
        m
    }

    fn collect(&mut self, tree: &Node) {
        for d in declarations(tree) {
            let name = s(d, "name");
            match d.kind {
                "structure" => {
                    for f in d.nodes("members").into_iter().filter(|f| f.kind == "field") {
                        self.fields.entry(s(f, "name").to_string()).or_insert_with(|| n(f, "type").clone());
                    }
                }
                "constants" => {
                    for c in d.nodes("items") {
                        let key = format!("{name}.{}", s(c, "name"));
                        match c.node("type") {
                            Some(t) => {
                                self.constants.insert(key.clone(), t.clone());
                                self.values.insert(key, n(c, "value").clone());
                            }
                            None => {
                                self.constants.insert(key, ty(name));
                            }
                        }
                    }
                }
                "type_decl" if d.node("type").is_some() => {
                    // `where name = (value.count() >= 4)`: what the refined type allows of its values' counts
                    for w in d.nodes("where") {
                        n(w, "condition").walk(&mut |x: &Node| {
                            let op = s(x, "op");
                            if x.kind == "binary" && [">=", ">", "<=", "<", "=="].contains(&op) && t(n(x, "left")) == "value.count()" && is_literal(n(x, "right"), "integer") {
                                let k: i64 = s(n(x, "right"), "text").parse().unwrap();
                                self.refinements.entry(name.to_string()).or_default().push((op.to_string(), k));
                            }
                        });
                    }
                }
                "function" => {
                    // a parameter's or the result's type, from a documentation entry on one line: `name: …, as type.`
                    let mut types: HashMap<&str, Node> = HashMap::new();
                    for e in d.node("doc").map(|doc| doc.nodes("entries")).unwrap_or_default() {
                        let Some(et) = e.node("type") else { continue };
                        let tt = s(et, "text");
                        let one_line = e.num("end") == Some(e.line);
                        let chars = tt.chars().all(|c| c.is_alphanumeric() || "_<>=, ".contains(c));
                        if one_line && chars && s(e, "text").ends_with(&format!(", as {tt}.")) {
                            types.insert(s(e, "name"), et.clone());
                        }
                    }
                    let params = d.nodes("params").iter().map(|p| (s(p, "name").to_string(), types.get(s(p, "name")).cloned())).collect();
                    self.params.insert(name.to_string(), params);
                    let returns = d.str("returns").map(str::to_string);
                    let rtype = returns.as_deref().and_then(|r| types.get(r).cloned());
                    self.results.insert(name.to_string(), (returns, rtype));
                    let mut contracts: Vec<(&'static str, Node)> = d.nodes("where").iter().map(|w| ("where", n(w, "condition").clone())).collect();
                    contracts.extend(d.nodes("ensures").iter().map(|w| ("ensures", n(w, "condition").clone())));
                    self.contracts.insert(name.to_string(), contracts);
                }
                _ => {}
            }
        }
    }

    /// The constant value of `e`, if it has one.
    fn value(&self, e: &Node) -> Option<i64> {
        if is_int(e) {
            return t(e).parse().ok();
        }
        if let Some(v) = self.values.get(&t(e)) {
            return self.value(v);
        }
        if e.kind == "binary" && ["+", "-", "*", "/"].contains(&s(e, "op")) {
            let (a, b) = (self.value(n(e, "left"))?, self.value(n(e, "right"))?);
            return match s(e, "op") {
                "+" => Some(a + b),
                "-" => Some(a - b),
                "*" => Some(a * b),
                _ => (b != 0).then(|| floor_div(a, b)),
            };
        }
        None
    }

    /// The constant length of a list, if it has one.
    fn length(&self, e: &Node) -> Option<i64> {
        let e = self.values.get(&t(e)).unwrap_or(e);
        (e.kind == "list").then(|| e.nodes("items").len() as i64)
    }

    fn type_of(&self, e: &Node, env: &Env) -> Option<Node> {
        let k = e.kind;
        if is_int(e) {
            return Some(ty("int"));
        }
        if k == "literal" {
            let unit = s(e, "text").trim_start_matches(|c: char| c.is_ascii_digit());
            return match s(e, "type") {
                "duration" if s(e, "text").starts_with(|c: char| c.is_ascii_digit()) && (unit == "ms" || unit == "s") => Some(ty("duration")),
                "string" => Some(ty("string")),
                "boolean" => Some(ty("boolean")),
                _ => None,
            };
        }
        if k == "unary" {
            return if s(e, "op") == "not" { Some(ty("boolean")) } else { self.type_of(n(e, "operand"), env) };
        }
        if k == "binary" && ["+", "-", "*", "/", "%"].contains(&s(e, "op")) {
            let (a, b) = (self.type_of(n(e, "left"), env), self.type_of(n(e, "right"), env));
            if type_text(&a) == Some("instant") && type_text(&b) == Some("instant") {
                return Some(ty("duration"));
            }
            return if !matches!(type_text(&a), None | Some("int")) { a } else if b.is_some() { b } else { a };
        }
        if k == "binary" {
            return Some(ty("boolean"));
        }
        if method(e, &["count"], Some(0)).is_some() {
            return Some(ty("count"));
        }
        if t(e) == "clock.now()" {
            return Some(ty("instant"));
        }
        for (m, name) in [("to_int32", "int32"), ("to_uint32", "uint32"), ("whole_seconds", "uint32"), ("is_empty", "boolean")] {
            if method(e, &[m], Some(0)).is_some() {
                return Some(ty(name));
            }
        }
        if method(e, &["contains"], None).is_some() {
            return Some(ty("boolean"));
        }
        if let Some(inner) = method(e, &["divided_rounding_up", "min", "max"], None) {
            return self.type_of(inner, env);
        }
        let inner = method(e, &["largest", "smallest", "last"], Some(0))
            .or_else(|| method(e, &["at_number"], None))
            .or_else(|| (k == "index").then(|| n(e, "object")));
        if let Some(inner) = inner {
            return element(&self.type_of(inner, env));
        }
        if k == "call" && n(e, "callee").kind == "name" {
            if let Some((_, rtype)) = self.results.get(s(n(e, "callee"), "name")) {
                return rtype.clone();
            }
        }
        let key = t(e);
        if let Some(found) = env.get(&key) {
            return found.clone();
        }
        if let Some(c) = self.constants.get(&key) {
            return Some(c.clone());
        }
        if k == "member" && simple(n(e, "object")) {
            if let Some(f) = self.fields.get(s(e, "name")) {
                return Some(f.clone());
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Facts along a path: intervals of numbers, booleans (0 or 1) and counts, kept by an expression's text

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
    fn is_constant(&self, m: &Module, e: &Node) -> bool {
        let e = t(e);
        e.starts_with("optional.") || e.starts_with("result.") || (m.groups.contains(e.split('.').next().unwrap()) && !m.values.contains_key(&e))
    }

    /// Adds that `left` is, or is not, `right`, for values with no order: false when it cannot be.
    fn equality(&mut self, m: &Module, left: &Node, right: &Node, truth: bool) -> bool {
        let (mut left, mut right) = (left, right);
        if self.is_constant(m, left) && !self.is_constant(m, right) {
            (left, right) = (right, left);
        }
        if !self.is_constant(m, right) {
            return true;
        }
        let (left, right) = (t(left), t(right));
        if truth {
            if self.same.get(&left).is_some_and(|s| *s != right) || self.differs.get(&left).is_some_and(|d| d.contains(&right)) {
                return false;
            }
            self.same.insert(left, right);
            return true;
        }
        if self.same.get(&left).is_some_and(|s| *s == right) {
            return false;
        }
        self.differs.entry(left).or_default().insert(right);
        true
    }

    fn interval(&self, m: &Module, env: &Env, e: &Node) -> I {
        let e = norm(e);
        if let Some(v) = m.value(e) {
            return (Some(v), Some(v));
        }
        if is_literal(e, "boolean") {
            let v = i64::from(s(e, "text") == "true");
            return (Some(v), Some(v));
        }
        let counted = method(e, &["count"], Some(0));
        if let Some(k) = counted.and_then(|c| m.length(c)) {
            return (Some(k), Some(k));
        }
        let mut i = self.known.get(&t(e)).copied().unwrap_or(ANY);
        if let Some(counted) = counted {
            // a refined type bounds its values' counts
            if let Some(name) = type_text(&m.type_of(counted, env)) {
                for (op, k) in m.refinements.get(name).into_iter().flatten() {
                    let k = *k;
                    i = meet(i, match op.as_str() {
                        ">=" => (Some(k), None),
                        ">" => (Some(k + 1), None),
                        "<=" => (None, Some(k)),
                        "<" => (None, Some(k - 1)),
                        _ => (Some(k), Some(k)),
                    });
                }
            }
            if let Some(runes) = method(counted, &["runes"], Some(0)) {
                i = meet(i, self.interval(m, env, &count(runes)));
            }
            // the standard library: splitting text gives at least one piece
            if method(counted, &["split"], None).is_some() {
                i = meet(i, (Some(1), None));
            }
        }
        match type_text(&m.type_of(e, env)) {
            Some(x) if UNSIGNED.contains(&x) => meet(i, (Some(0), None)),
            Some("boolean") => meet(i, (Some(0), Some(1))),
            _ => i,
        }
    }

    fn add(&mut self, m: &Module, env: &Env, e: &Node, i: I) -> bool {
        let e = norm(e);
        let new = meet(self.interval(m, env, e), i);
        self.known.insert(t(e), new);
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
    fn bind(&mut self, m: &Module, env: &Env, name: &str, rhs: &Node) {
        let r = t(rhs);
        let mut new: BTreeMap<String, I> = BTreeMap::new();
        let mut same = None;
        for (k, v) in &self.known {
            if *k == r {
                new.insert(name.to_string(), *v);
            } else if let Some(rest) = k.strip_prefix(&format!("{r}.")) {
                new.insert(format!("{name}.{rest}"), *v);
            }
        }
        if let Some(v) = m.value(rhs) {
            new.insert(name.to_string(), (Some(v), Some(v)));
        }
        if self.is_constant(m, rhs) {
            same = Some(r.clone());
        } else if let Some(x) = self.same.get(&r) {
            same = Some(x.clone());
        }
        // arithmetic on what is known
        if rhs.kind == "binary" && ["+", "-"].contains(&s(rhs, "op")) {
            if let Some(c) = m.value(n(rhs, "right")) {
                let c = if s(rhs, "op") == "+" { c } else { -c };
                new.insert(name.to_string(), shift(self.interval(m, env, n(rhs, "left")), c));
            }
        }
        // a negated value
        if rhs.kind == "unary" && s(rhs, "op") == "-" && m.value(rhs).is_none() {
            let mut inner = self.clone();
            inner.bind(m, env, "negated'value", n(rhs, "operand"));
            let i = inner.interval(m, env, &name_node("negated'value"));
            new.insert(name.to_string(), (i.1.map(|x| -x), i.0.map(|x| -x)));
        }
        // what the standard library ensures of the lists it makes
        let counted = format!("{name}.count()");
        if r == "list.new()" || (rhs.kind == "list" && rhs.nodes("items").is_empty()) {
            new.insert(counted.clone(), (Some(0), Some(0)));
        }
        if rhs.kind == "call" {
            let callee = chain(n(rhs, "callee"));
            match (callee.as_deref(), arg_names(rhs).as_slice()) {
                (Some("random.shuffled"), [None]) => {
                    new.insert(counted.clone(), self.interval(m, env, &count(n(rhs.nodes("args")[0], "value"))));
                }
                (Some("random.chosen"), [Some("from"), Some("count")]) => {
                    new.insert(counted.clone(), self.interval(m, env, arg(rhs, "count").unwrap()));
                }
                (Some("list.repeated"), [None, Some("times")]) => {
                    new.insert(counted.clone(), self.interval(m, env, arg(rhs, "times").unwrap()));
                }
                _ => {}
            }
        }
        if is_literal(rhs, "boolean") {
            let v = i64::from(s(rhs, "text") == "true");
            new.insert(name.to_string(), (Some(v), Some(v)));
        }
        self.forget(name);
        self.known.extend(new);
        if let Some(x) = same {
            self.same.insert(name.to_string(), x);
        }
        // what the function it calls ensures of it
        if rhs.kind == "call" && n(rhs, "callee").kind == "name" {
            let callee = s(n(rhs, "callee"), "name");
            let result = m.results.get(callee).and_then(|r| r.0.clone());
            for (kind, pred) in m.contracts.get(callee).into_iter().flatten() {
                if let (true, Some(result)) = (*kind == "ensures", &result) {
                    let alts = assume(m, env, &renamed(pred, result, name), true, self);
                    if !alts.is_empty() {
                        *self = join(&alts);
                    }
                }
            }
        }
    }

    fn possible_orders(&self, m: &Module, env: &Env, left: &Node, right: &Node) -> Option<BTreeSet<&'static str>> {
        let known: Vec<Node> = [m.type_of(left, env), m.type_of(right, env)].into_iter().flatten().collect();
        if known.is_empty() || !known.iter().any(|x| ORDERED.contains(&s(x, "text"))) {
            return None;
        }
        Some(order_possible(self.interval(m, env, left), self.interval(m, env, right)))
    }

    /// Adds what order `o` of `left` to `right` shows: false when it cannot be.
    fn constrain(&mut self, m: &Module, env: &Env, left: &Node, right: &Node, o: &str) -> bool {
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
fn outcomes(m: &Module, env: &Env, e: &Node, truth: bool, facts: &Facts) -> Vec<(String, Facts)> {
    if e.kind == "unary" && s(e, "op") == "not" {
        return outcomes(m, env, n(e, "operand"), !truth, facts);
    }
    for (word, stops_on) in [("or", true), ("and", false)] {
        let ops = operands(e, word);
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
                    for (suffix, f2) in outcomes(m, env, op, truth, f) {
                        let suffix = format!("'{}{}", k + 1, suffix.trim_start_matches('\''));
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
                .flat_map(|(sofar, f)| {
                    outcomes(m, env, op, truth, f).into_iter().map(move |(s2, f2)| {
                        let part = if s2.is_empty() { String::new() } else { format!("'{}{}", k + 1, s2.trim_start_matches('\'')) };
                        (format!("{sofar}{part}"), f2)
                    })
                })
                .collect();
        }
        return states;
    }
    let op = s(e, "op");
    if e.kind == "binary" && ["<", "<=", ">", ">=", "==", "!="].contains(&op) {
        let (left, right) = (n(e, "left"), n(e, "right"));
        let Some(possible) = facts.possible_orders(m, env, left, right) else {
            if op == "==" || op == "!=" {
                let mut f = facts.clone();
                return if f.equality(m, left, right, truth == (op == "==")) { vec![(String::new(), f)] } else { vec![] };
            }
            return vec![(String::new(), facts.clone())];
        };
        let mut out = vec![];
        for o in ORDERS3 {
            if orders(op, truth).contains(&o) && possible.contains(o) {
                let mut f = facts.clone();
                if f.constrain(m, env, left, right, o) {
                    out.push((format!("'{o}"), f));
                }
            }
        }
        if out.len() == 1 {
            out[0].0 = String::new();
        }
        return out;
    }
    if let Some(inner) = method(e, &["is_empty"], Some(0)) {
        let mut f = facts.clone();
        let i = if truth { (Some(0), Some(0)) } else { (Some(1), None) };
        return if f.add(m, env, &count(inner), i) { vec![(String::new(), f)] } else { vec![] };
    }
    let mut f = facts.clone();
    if type_text(&m.type_of(e, env)) == Some("boolean") || chain(e).is_some() {
        let v = i64::from(truth);
        if !f.add(m, env, e, (Some(v), Some(v))) {
            return vec![];
        }
    }
    vec![(String::new(), f)]
}

/// The facts after `e` is found `truth`, one set for each way it can be.
fn assume(m: &Module, env: &Env, e: &Node, truth: bool, facts: &Facts) -> Vec<Facts> {
    outcomes(m, env, e, truth, facts).into_iter().map(|x| x.1).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Statements

/// The expressions a statement writes on its own line, not those of the blocks it opens.
fn own(st: &Node) -> Vec<&Node> {
    let fields: &[&str] = match st.kind {
        "let" | "expr" | "return" | "fail" | "task" => &["value"],
        "assign" => &["target", "value"],
        "if" | "while" => &["condition"],
        "for" => &["iterable"],
        "conditional" => &["subject"],
        "compare" => &["left", "right"],
        "condition" => &["test"],
        _ => &[],
    };
    let mut out: Vec<&Node> = fields.iter().filter_map(|f| st.node(f)).collect();
    if st.kind == "inject" {
        out.extend(st.nodes("bindings").into_iter().map(|b| n(b, "value")));
    }
    if st.kind == "region" {
        out.extend(st.nodes("constraints"));
    }
    out
}

/// Every statement of a block, and of the blocks inside it, each before the statements inside it.
fn statements(items: &[&Node]) -> Vec<Node> {
    fn go<'a>(items: &[&'a Node], out: &mut Vec<&'a Node>) {
        for st in items {
            out.push(st);
            for c in st.nodes("conditions") {
                out.push(c);
                go(&c.nodes("body"), out);
            }
            go(&st.nodes("body"), out);
            if let Some(e) = st.node("else") {
                go(&e.nodes("body"), out);
            }
            for r in st.nodes("orelse") {
                out.push(r);
                go(&r.nodes("body"), out);
            }
        }
    }
    let mut out = vec![];
    go(items, &mut out);
    out.into_iter().cloned().collect()
}

/// The names a statement passes as `mutable`.
fn mutable_args(st: &Node) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for e in own(st) {
        e.walk(&mut |x: &Node| {
            if x.kind == "mutable" {
                if let Some(r) = root(n(x, "value")) {
                    out.insert(r);
                }
            }
        });
    }
    out
}

/// The value of an expression statement, without its `or fail`; None for any other statement.
fn expr_value(st: &Node) -> Option<&Node> {
    (st.kind == "expr").then(|| unwrap(n(st, "value")))
}

/// The names a statement changes on its own line.
fn changes(st: &Node) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if st.kind == "assign" && plain(n(st, "target")) {
        out.extend(root(n(st, "target")));
    }
    if !(st.kind == "let" && st.flag("mutable")) {
        out.extend(mutable_args(st));
    }
    if let Some(obj) = expr_value(st).and_then(|v| method(v, &["push", "extend", "set_number", "insert_first", "clear"], None)) {
        if let Some(c) = chain(obj) {
            out.insert(c.split('.').next().unwrap().to_string());
        }
    }
    if st.kind == "let" || st.kind == "for" {
        out.insert(s(st, "name").to_string());
    }
    out
}

/// The names a block changes.
fn mutated(items: &[&Node]) -> BTreeSet<String> {
    statements(items).iter().flat_map(changes).collect()
}

/// The names among `names` that a block changes only by adding to them, with `push` or `extend`.
fn grown_only(items: &[&Node], names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut other = BTreeSet::new();
    for st in statements(items) {
        for name in names {
            let grows = expr_value(&st).and_then(|v| method(v, &["push", "extend"], None));
            if grows.is_some_and(|o| o.kind == "name" && s(o, "name") == name) {
                continue;
            }
            let binds = (st.kind == "let" && s(&st, "name") == name) || (st.kind == "assign" && root(n(&st, "target")).as_deref() == Some(name) && plain(n(&st, "target")));
            if binds || mutable_args(&st).contains(name) {
                other.insert(name.clone());
            }
            let changes = expr_value(&st).and_then(|v| method(v, &["set_number", "insert_first", "clear"], None));
            if changes.and_then(chain).is_some_and(|c| c.split('.').next() == Some(name)) {
                other.insert(name.clone());
            }
        }
    }
    names.iter().filter(|x| !other.contains(*x)).cloned().collect()
}

fn widen(facts: &mut Facts, names: &BTreeSet<String>, grown: &BTreeSet<String>) {
    for name in names {
        let keep = if grown.contains(name) { facts.known.get(&format!("{name}.count()")).copied() } else { None };
        facts.forget(name);
        if let Some(k) = keep {
            facts.known.insert(format!("{name}.count()"), (k.0, None));
        }
    }
}

/// The expression a statement's line ends with, before any `:`.
fn last_expression(st: &Node) -> Option<&Node> {
    let field = match st.kind {
        "let" | "assign" | "expr" | "return" | "task" | "fail" => "value",
        "for" => "iterable",
        "conditional" => "subject",
        "compare" => "right",
        _ => return None,
    };
    st.node(field)
}

/// The name of a statement's `or fail` site: the name it binds, or the method or function it calls; None when it has
/// none.
fn failing_name(st: &Node) -> Option<String> {
    let e = last_expression(st)?;
    if e.kind != "or_fail" {
        return None;
    }
    if st.kind == "let" {
        return Some(s(st, "name").to_string());
    }
    if st.kind == "assign" && n(st, "target").kind == "name" {
        return Some(s(n(st, "target"), "name").to_string());
    }
    let inner = unwrap(e);
    if st.kind == "expr" && inner.kind == "call" {
        if let Some(c) = chain(n(inner, "callee")) {
            return Some(c.rsplit('.').next().unwrap().to_string());
        }
    }
    Some("call".to_string())
}

/// What identifies an `or fail` site: two that read the same are one.
fn site(st: &Node) -> String {
    let bound = match st.kind {
        "let" => s(st, "name").to_string(),
        "assign" => t(n(st, "target")),
        _ => String::new(),
    };
    format!("{}|{}|{bound}|{}", st.kind, st.flag("mutable"), last_expression(st).map(t).unwrap_or_default())
}

/// How a path ends, and the named outcomes along it.
type State = (String, Vec<String>, Facts);

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
    fn new(module: &'m Module, decl: &Node) -> Function<'m> {
        let name = s(decl, "name");
        let mut env: Env = module.params.get(name).into_iter().flatten().filter(|(_, x)| x.is_some()).map(|(p, x)| (p.clone(), x.clone())).collect();
        if let Some((Some(result), Some(rtype))) = module.results.get(name) {
            env.insert(result.clone(), Some(rtype.clone()));
        }
        let items = decl.nodes("body");
        let mut fails_total = HashMap::new();
        for st in statements(&items) {
            if let Some(f) = failing_name(&st) {
                *fails_total.entry(f).or_insert(0) += 1;
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
            if *kind == "where" {
                let env = f.env.borrow().clone();
                starts = starts.iter().flat_map(|x| assume(module, &env, pred, true, x)).take(1).collect();
            }
        }
        let found = match starts.into_iter().next() {
            Some(x) => f.seq(&items, x),
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

    fn outcomes(&self, e: &Node, truth: bool, facts: &Facts) -> Vec<(String, Facts)> {
        outcomes(self.module, &self.env.borrow(), e, truth, facts)
    }

    fn assume(&self, e: &Node, truth: bool, facts: &Facts) -> Vec<Facts> {
        assume(self.module, &self.env.borrow(), e, truth, facts)
    }

    /// The type of a name a statement binds, or of a loop's variable, when it is known.
    fn note_type(&self, st: &Node) {
        let m = self.module;
        if st.kind == "let" {
            let found = m.type_of(unwrap(n(st, "value")), &self.env.borrow());
            let found = if type_text(&found) == Some("int") { Some(ty("int32")) } else { found };
            let mut env = self.env.borrow_mut();
            if found.is_some() && !env.contains_key(s(st, "name")) {
                env.insert(s(st, "name").to_string(), found);
            }
        }
        if st.kind == "for" {
            let src = n(st, "iterable");
            let el = element(&m.type_of(src, &self.env.borrow()));
            if el.is_some() {
                self.env.borrow_mut().insert(s(st, "name").to_string(), el);
            } else if src.kind == "call" && chain(n(src, "callee")).as_deref() == Some("range") {
                let from = arg(src, "from").and_then(|x| m.type_of(x, &self.env.borrow())).unwrap_or_else(|| ty("int32"));
                let from = if s(&from, "text") == "int" { ty("int32") } else { from };
                self.env.borrow_mut().insert(s(st, "name").to_string(), Some(from));
            }
        }
    }

    /// The states a block entered with `facts` ends in.
    fn seq(&self, items: &[&Node], facts: Facts) -> Vec<State> {
        let (mut states, mut done) = (vec![("next".to_string(), vec![], facts)], vec![]);
        for st in items {
            self.note_type(st);
            let mut next = vec![];
            for (_, steps, f) in &states {
                for (kind2, steps2, f2) in self.statement(st, f.clone()) {
                    let all = [steps.clone(), steps2].concat();
                    if kind2 == "next" {
                        next.push((kind2, all, f2));
                    } else {
                        done.push((kind2, all, f2));
                    }
                }
            }
            states = next;
        }
        states.extend(done);
        states
    }

    fn branch(&self, name: &str, pred: &Node, truth: bool, body: &[&Node], facts: &Facts) -> Vec<State> {
        let mut out = vec![];
        for (suffix, f) in self.outcomes(pred, truth, facts) {
            for (kind, steps, f2) in self.seq(body, f) {
                out.push((kind, [vec![format!("{name}{suffix}")], steps].concat(), f2));
            }
        }
        out
    }

    /// The states of a block, each with `name` before its steps.
    fn named(&self, name: &str, body: &[&Node], facts: Facts) -> Vec<State> {
        self.seq(body, facts).into_iter().map(|(kind, steps, f2)| (kind, [vec![name.to_string()], steps].concat(), f2)).collect()
    }

    fn statement(&self, st: &Node, facts: Facts) -> Vec<State> {
        let m = self.module;
        let k = st.kind;

        // `region a is …:` with `orelse region b is …:` after it: a decision whose outcomes are the regions
        if k == "region" && !st.nodes("orelse").is_empty() {
            let mut out = vec![];
            for r in [st].into_iter().chain(st.nodes("orelse")) {
                out.extend(self.named(s(r, "name"), &r.nodes("body"), facts.clone()));
            }
            return out;
        }

        if k == "if" {
            let (name, pred, body) = (s(st, "name"), n(st, "condition"), st.nodes("body"));
            let Some(other) = st.node("else") else {
                self.problem(format!("`if {name}` has no named else block"));
                return self.branch(name, pred, true, &body, &facts);
            };
            return [self.branch(name, pred, true, &body, &facts), self.branch(s(other, "name"), pred, false, &other.nodes("body"), &facts)].concat();
        }

        if k == "conditional" {
            if let Some(subject) = st.node("subject") {
                let found = m.type_of(subject, &self.env.borrow());
                self.env.borrow_mut().entry(s(st, "label").to_string()).or_insert(found);
            }
            let (mut out, mut rest) = (vec![], vec![facts]);
            for c in st.nodes("conditions") {
                for f in &rest {
                    out.extend(self.branch(s(c, "name"), n(c, "test"), true, &c.nodes("body"), f));
                }
                rest = rest.iter().flat_map(|f| self.assume(n(c, "test"), false, f)).collect();
            }
            if let Some(e) = st.node("else") {
                for f in rest {
                    out.extend(self.named(s(e, "name"), &e.nodes("body"), f));
                }
            }
            return out;
        }

        if k == "compare" {
            let (label, a, b) = (s(st, "label"), n(st, "left"), n(st, "right"));
            let possible = match facts.possible_orders(m, &self.env.borrow(), a, b) {
                Some(p) => p,
                None => {
                    self.problem(format!("`compare {label}` compares values with no order"));
                    ORDERS3.into_iter().collect()
                }
            };
            let (mut out, mut seen) = (vec![], BTreeSet::new());
            for c in st.nodes("conditions").into_iter().chain(st.node("else")) {
                let orders: Vec<&str> = if c.kind == "condition" {
                    let words: Vec<&str> = operands(n(c, "test"), "or")
                        .iter()
                        .map(|w| match s(w, "name") {
                            "less" => "lt",
                            "equal" => "eq",
                            _ => "gt",
                        })
                        .collect();
                    if words.iter().any(|o| seen.contains(o)) {
                        self.problem(format!("`compare {label}`: an order is in two conditions"));
                    }
                    words
                } else {
                    ORDERS3.into_iter().filter(|o| !seen.contains(o)).collect()
                };
                seen.extend(orders.iter().copied());
                let mut side = vec![];
                for o in &orders {
                    let mut f = facts.clone();
                    if possible.contains(o) && f.constrain(m, &self.env.borrow(), a, b, o) {
                        side.push((*o, f));
                    }
                }
                let split = side.len() > 1;
                for (o, f) in side {
                    let name = format!("{}{}", s(c, "name"), if split { format!("'{o}") } else { String::new() });
                    out.extend(self.named(&name, &c.nodes("body"), f));
                }
            }
            if seen.len() != 3 {
                self.problem(format!("`compare {label}` leaves an order out"));
            }
            return out;
        }

        if k == "for" {
            let what = format!("for {} in {}", s(st, "name"), t(n(st, "iterable")));
            return self.run_loop(s(st, "name"), None, Some(n(st, "iterable")), st, facts, &what);
        }
        if k == "while" {
            let what = format!("while {}", s(st, "name"));
            return self.run_loop(s(st, "name"), Some(n(st, "condition")), None, st, facts, &what);
        }

        if k == "return" {
            return vec![("return".to_string(), vec![], facts)];
        }
        if k == "fail" {
            let exempt = st.node("value").filter(|v| v.kind == "name" && EXEMPT.contains(&s(v, "name")));
            return vec![(exempt.map_or("fail".to_string(), |v| s(v, "name").to_string()), vec![], facts)];
        }
        if k == "continue" || k == "break" {
            return vec![(k.to_string(), vec![], facts)];
        }

        let mut outs = if ["region", "arena", "inject"].contains(&k) { self.seq(&st.nodes("body"), facts.clone()) } else { vec![("next".to_string(), vec![], facts.clone())] };
        if let Some(name) = failing_name(st) {
            let key = (name.clone(), site(st));
            let mut order = self.order.borrow_mut();
            if !order.contains_key(&key) {
                let mut seen = self.fails_seen.borrow_mut();
                let c = seen.entry(name.clone()).or_insert(0);
                *c += 1;
                order.insert(key.clone(), *c);
            }
            let label = if self.fails_total[&name] == 1 { name.clone() } else { format!("{name}'{}", order[&key]) };
            outs.push(("fail".to_string(), vec![format!("{label}'fails")], facts.clone()));
        }
        // what the statement changes, and what a new name stands for
        let mut changed = changes(st);
        if let Some(v) = expr_value(st) {
            if let Some(obj) = method(v, &["push", "extend"], None).filter(|o| o.kind == "name") {
                let by = (s(n(v, "callee"), "name") == "push").then_some(1);
                for (_, _, f) in outs.iter_mut() {
                    f.grow(s(obj, "name"), by);
                }
                changed.remove(s(obj, "name"));
            }
        }
        let bound = match k {
            "let" => Some(s(st, "name")),
            "assign" if n(st, "target").kind == "name" => Some(s(n(st, "target"), "name")),
            _ => None,
        };
        if let Some(b) = bound {
            changed.remove(b);
        }
        let env = self.env.borrow();
        for (kind, _, f) in outs.iter_mut() {
            if kind != "next" {
                continue;
            }
            for c in &changed {
                f.forget(c);
            }
            if let Some(b) = bound {
                f.bind(m, &env, b, unwrap(n(st, "value")));
            }
        }
        outs
    }

    /// The interval of the number of elements a `for` goes over.
    fn count_of(&self, src: &Node, facts: &Facts) -> I {
        let m = self.module;
        let env = self.env.borrow();
        let args = src.nodes("args");
        let range = src.kind == "call" && chain(n(src, "callee")).as_deref() == Some("range");
        if range && args.len() == 2 && args[0].str("name") == Some("from") && matches!(args[1].str("name"), Some("below" | "through")) {
            let (start, end) = (n(args[0], "value"), n(args[1], "value"));
            let (a, b) = (facts.interval(m, &env, start), facts.interval(m, &env, end));
            let extra = i64::from(args[1].str("name") == Some("through"));
            if end.kind == "binary" && s(end, "op") == "+" && !end.flag("parens") && text(n(end, "left"), false) == text(start, false) {
                if let Some(k) = m.value(n(end, "right")) {
                    return (Some(k + extra), Some(k + extra));
                }
            }
            if let (Some(lo), Some(hi)) = a {
                if lo == hi {
                    return (b.0.map(|x| (x - lo + extra).max(0)), b.1.map(|x| (x - lo + extra).max(0)));
                }
            }
            return (Some(0), None);
        }
        if let Some(k) = m.length(src) {
            return (Some(k), Some(k));
        }
        let mut f = facts.clone();
        f.bind(m, &env, "loop'source", src);
        meet(meet(f.interval(m, &env, &count(&name_node("loop'source"))), facts.interval(m, &env, &count(src))), (Some(0), None))
    }

    fn with_count(&self, src: &Node, facts: &Facts, interval: I) -> Facts {
        let mut f = facts.clone();
        if !(src.kind == "call" && chain(n(src, "callee")).as_deref() == Some("range")) {
            f.add(self.module, &self.env.borrow(), &count(src), interval);
        }
        f
    }

    fn run_loop(&self, var: &str, pred: Option<&Node>, src: Option<&Node>, st: &Node, facts: Facts, what: &str) -> Vec<State> {
        let mut out = vec![];
        let else_name = st.node("else").map(|e| s(e, "name").to_string());
        let else_body = st.node("else").map(|e| e.nodes("body")).unwrap_or_default();
        let kids = st.nodes("body");
        let counted = src.map(|x| self.count_of(x, &facts));

        // the loop never runs
        let never = match (pred, src, counted) {
            (Some(p), _, _) => self.outcomes(p, false, &facts),
            (None, Some(x), Some(c)) => {
                if c.0.unwrap_or(0) >= 1 {
                    vec![]
                } else {
                    vec![(String::new(), self.with_count(x, &facts, (Some(0), Some(0))))]
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
            for (suffix, f) in never {
                for (kind, steps, f2) in self.seq(&else_body, f) {
                    out.push((kind, [vec![format!("{name}{suffix}")], steps].concat(), f2));
                }
            }
        }

        // the pass the path shows: the first, or a later one, after which the facts on what the body changes no longer
        // hold
        let changed = mutated(&kids);
        let grown = grown_only(&kids, &changed);
        for many in [false, true] {
            let mut base = facts.clone();
            if many {
                widen(&mut base, &changed, &grown);
            }
            let runs = if many { (Some(2), None) } else { (Some(1), None) };
            let entries = match (pred, src, counted) {
                (Some(p), _, _) => self.outcomes(p, true, &base),
                (None, Some(x), Some(c)) => {
                    if empty(meet(c, runs)) {
                        continue;
                    }
                    vec![(String::new(), self.with_count(x, &base, runs))]
                }
                _ => unreachable!(),
            };
            for (suffix, mut f) in entries {
                f.forget(var);
                for (kind, steps, mut f2) in self.seq(&kids, f) {
                    let goes_on = kind == "next" || kind == "continue";
                    let last = if many { (Some(2), None) } else { (Some(1), Some(1)) };
                    if let (Some(c), true) = (counted, goes_on) {
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
                        let step = format!("{var}{}{suffix}", if many { "'many" } else { "" });
                        out.push((kind.clone(), [vec![step], steps.clone()].concat(), f3));
                    }
                }
            }
        }
        out
    }
}

fn functions(module: &Module) -> Vec<Function<'_>> {
    declarations(&module.tree).into_iter().filter(|d| d.kind == "function").map(|d| Function::new(module, d)).collect()
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
