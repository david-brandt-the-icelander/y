//! The canonical form of a constraint file: fixed key order, two-space indent, one element per line.
use serde_json::Value;

const ORDER: [&str; 34] = [
    "format", "host", "crate", "module", "status", "subjects", "holes", "kind", "n", "name", "visible", "visible_path",
    "generics", "for", "bounds", "where", "trait", "self", "type", "form", "fields", "variants", "params", "result",
    "fails", "access", "hold", "rights", "guarantee", "from", "lifetime", "locals", "hints", "sep",
];

fn rank(k: &str) -> usize {
    ORDER.iter().position(|o| *o == k).unwrap_or(ORDER.len())
}

fn write(v: &Value, ind: usize, parent: &str, sort: bool, out: &mut String) {
    let pad = |n: usize| " ".repeat(n);
    match v {
        Value::Object(m) if !m.is_empty() => {
            let mut keys: Vec<&String> = m.keys().collect();
            if sort && !matches!(parent, "holes" | "locals") {
                keys.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.cmp(b)));
            }
            out.push_str("{\n");
            for (i, k) in keys.iter().enumerate() {
                out.push_str(&pad(ind + 2));
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push_str(": ");
                write(&m[*k], ind + 2, k, sort, out);
                out.push_str(if i + 1 < keys.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(ind));
            out.push('}');
        }
        Value::Array(a) if !a.is_empty() => {
            out.push_str("[\n");
            for (i, x) in a.iter().enumerate() {
                out.push_str(&pad(ind + 2));
                write(x, ind + 2, parent, sort, out);
                out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(ind));
            out.push(']');
        }
        _ => out.push_str(&serde_json::to_string(v).unwrap()),
    }
}

/// The canonical text of a constraint file, ending in a newline.
pub fn canonical(v: &Value) -> String {
    let mut s = String::new();
    write(v, 0, "", true, &mut s);
    s.push('\n');
    s
}

/// The same layout, with keys in the order they were inserted: for reports.
pub fn ordered(v: &Value) -> String {
    let mut s = String::new();
    write(v, 0, "", false, &mut s);
    s.push('\n');
    s
}
