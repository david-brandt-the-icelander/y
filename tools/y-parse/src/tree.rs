//! The tree the parser makes: nodes, each with a kind, the line it starts on and named fields.

/// A field's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(usize),
    Str(String),
    Node(Node),
    List(Vec<Value>),
}

/// A node of the tree: what it is (`kind`), the line it starts on (from 1), and its fields in a fixed order.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: &'static str,
    pub line: usize,
    pub fields: Vec<(&'static str, Value)>,
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}
impl From<usize> for Value {
    fn from(n: usize) -> Value {
        Value::Num(n)
    }
}
impl From<Node> for Value {
    fn from(n: Node) -> Value {
        Value::Node(n)
    }
}
impl From<Vec<Node>> for Value {
    fn from(ns: Vec<Node>) -> Value {
        Value::List(ns.into_iter().map(Value::Node).collect())
    }
}
impl From<Vec<String>> for Value {
    fn from(ss: Vec<String>) -> Value {
        Value::List(ss.into_iter().map(Value::Str).collect())
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(o: Option<T>) -> Value {
        o.map_or(Value::Null, Into::into)
    }
}

impl Node {
    pub fn new(kind: &'static str, line: usize) -> Node {
        Node { kind, line, fields: vec![] }
    }

    /// The node with a field added, or replaced.
    pub fn with(mut self, key: &'static str, v: impl Into<Value>) -> Node {
        self.set(key, v);
        self
    }

    pub fn set(&mut self, key: &'static str, v: impl Into<Value>) {
        assert!(key != "kind" && key != "line", "`{key}` is every node's own, and no field's name");
        let v = v.into();
        match self.fields.iter_mut().find(|(k, _)| *k == key) {
            Some(f) => f.1 = v,
            None => self.fields.push((key, v)),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.fields.iter_mut().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// A string field, or `None` when it is missing or not a string.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// A boolean field; a missing one is false.
    pub fn flag(&self, key: &str) -> bool {
        matches!(self.get(key), Some(Value::Bool(true)))
    }

    /// A number field, such as `end`.
    pub fn num(&self, key: &str) -> Option<usize> {
        match self.get(key) {
            Some(Value::Num(n)) => Some(*n),
            _ => None,
        }
    }

    /// A field that holds one node.
    pub fn node(&self, key: &str) -> Option<&Node> {
        match self.get(key) {
            Some(Value::Node(n)) => Some(n),
            _ => None,
        }
    }

    /// The nodes of a field: those of a list, or the one node it holds; none when it is missing.
    pub fn nodes(&self, key: &str) -> Vec<&Node> {
        match self.get(key) {
            Some(Value::Node(n)) => vec![n],
            Some(Value::List(vs)) => vs.iter().filter_map(|v| if let Value::Node(n) = v { Some(n) } else { None }).collect(),
            _ => vec![],
        }
    }

    /// The nodes directly inside this one, in the order of its fields.
    pub fn children(&self) -> Vec<&Node> {
        fn go<'a>(v: &'a Value, out: &mut Vec<&'a Node>) {
            match v {
                Value::Node(n) => out.push(n),
                Value::List(vs) => vs.iter().for_each(|v| go(v, out)),
                _ => {}
            }
        }
        let mut out = vec![];
        self.fields.iter().for_each(|(_, v)| go(v, &mut out));
        out
    }

    /// Visits this node and every node inside it, each before the nodes inside it, in source order.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Node)) {
        f(self);
        for c in self.children() {
            c.walk(f);
        }
    }

    /// Every node of a kind, this one included, in the order `walk` visits them.
    pub fn find(&self, kind: &str) -> Vec<&Node> {
        let mut out = vec![];
        self.walk(&mut |n| {
            if n.kind == kind {
                out.push(n)
            }
        });
        out
    }

    /// The node as JSON: an object whose first members are `kind` and `line`, then its fields; indented by two spaces.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        write_node(self, 0, &mut out);
        out
    }
}

impl Value {
    /// The value as JSON.
    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        write_value(self, 0, &mut out);
        out
    }
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn pad(depth: usize, out: &mut String) {
    out.push('\n');
    out.push_str(&"  ".repeat(depth));
}

fn write_node(n: &Node, depth: usize, out: &mut String) {
    out.push('{');
    pad(depth + 1, out);
    out.push_str("\"kind\": ");
    quote(n.kind, out);
    out.push(',');
    pad(depth + 1, out);
    out.push_str(&format!("\"line\": {}", n.line));
    for (k, v) in &n.fields {
        out.push(',');
        pad(depth + 1, out);
        quote(k, out);
        out.push_str(": ");
        write_value(v, depth + 1, out);
    }
    pad(depth, out);
    out.push('}');
}

fn write_value(v: &Value, depth: usize, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Num(n) => out.push_str(&n.to_string()),
        Value::Str(s) => quote(s, out),
        Value::Node(n) => write_node(n, depth, out),
        Value::List(vs) if vs.is_empty() => out.push_str("[]"),
        Value::List(vs) => {
            out.push('[');
            for (i, v) in vs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                pad(depth + 1, out);
                write_value(v, depth + 1, out);
            }
            pad(depth, out);
            out.push(']');
        }
    }
}
