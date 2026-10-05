//! The canonical form of a constraint file: fixed key order, two-space indent, one element per line unless the
//! element fits on one line.
use serde_json::Value;

const ORDER: [&str; 35] = [
    "format", "host", "crate", "module", "status", "subjects", "holes", "kind", "n", "name", "visible", "visible_path",
    "generics", "for", "bounds", "where", "trait", "self", "type", "form", "fields", "variants", "params", "result",
    "fails", "clauses", "access", "hold", "rights", "guarantee", "from", "lifetime", "locals", "hints", "sep",
];

/// The members of a clause of a library file (`clauses`), in the order they are written: the keyword, the text,
/// where it comes from, why the language cannot say it, and its status.
const CLAUSE_ORDER: [&str; 5] = ["kind", "text", "source", "reason", "status"];

fn rank(parent: &str, k: &str) -> usize {
    if parent == "clauses" {
        return CLAUSE_ORDER.iter().position(|o| *o == k).unwrap_or(CLAUSE_ORDER.len());
    }
    ORDER.iter().position(|o| *o == k).unwrap_or(ORDER.len())
}

fn keys_of<'a>(m: &'a serde_json::Map<String, Value>, parent: &str, sort: bool) -> Vec<&'a String> {
    let mut keys: Vec<&String> = m.keys().collect();
    if sort && !matches!(parent, "holes" | "locals") {
        keys.sort_by(|a, b| rank(parent, a).cmp(&rank(parent, b)).then(a.cmp(b)));
    }
    keys
}

/// A value on one line: `{ "a": 1, "b": ["x", "y"] }`.
fn inline(v: &Value, parent: &str, sort: bool) -> String {
    match v {
        Value::Object(m) if !m.is_empty() => {
            let parts: Vec<String> =
                keys_of(m, parent, sort).iter().map(|k| format!("{}: {}", serde_json::to_string(k).unwrap(), inline(&m[*k], k, sort))).collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Array(a) if !a.is_empty() => format!("[{}]", a.iter().map(|x| inline(x, parent, sort)).collect::<Vec<_>>().join(", ")),
        _ => serde_json::to_string(v).unwrap(),
    }
}

/// The widest line a compact value may make.
const WIDTH: usize = 110;

/// `ind` is the indentation of the line the value ends on, `col` the column it starts at.
fn write(v: &Value, ind: usize, col: usize, parent: &str, sort: bool, compact: bool, out: &mut String) {
    let pad = |n: usize| " ".repeat(n);
    let nonempty = matches!(v, Value::Object(m) if !m.is_empty()) || matches!(v, Value::Array(a) if !a.is_empty());
    if compact && nonempty && ind >= 2 {
        let one = inline(v, parent, sort);
        if col + one.chars().count() < WIDTH {
            out.push_str(&one);
            return;
        }
    }
    match v {
        Value::Object(m) if !m.is_empty() => {
            let keys = keys_of(m, parent, sort);
            out.push_str("{\n");
            for (i, k) in keys.iter().enumerate() {
                out.push_str(&pad(ind + 2));
                let key = serde_json::to_string(k).unwrap();
                out.push_str(&key);
                out.push_str(": ");
                write(&m[*k], ind + 2, ind + 2 + key.chars().count() + 2, k, sort, compact, out);
                out.push_str(if i + 1 < keys.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(ind));
            out.push('}');
        }
        Value::Array(a) if !a.is_empty() => {
            out.push_str("[\n");
            for (i, x) in a.iter().enumerate() {
                out.push_str(&pad(ind + 2));
                write(x, ind + 2, ind + 2, parent, sort, compact, out);
                out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(ind));
            out.push(']');
        }
        _ => out.push_str(&serde_json::to_string(v).unwrap()),
    }
}

/// The canonical text of a constraint file, ending in a newline: keys in a fixed order, two-space indent, and a
/// value on one line when it fits in 110 columns below the top level.
pub fn canonical(v: &Value) -> String {
    let mut s = String::new();
    write(v, 0, 0, "", true, true, &mut s);
    s.push('\n');
    s
}

/// The same layout, with keys in the order they were inserted: for reports.
pub fn ordered(v: &Value) -> String {
    let mut s = String::new();
    write(v, 0, 0, "", false, false, &mut s);
    s.push('\n');
    s
}
