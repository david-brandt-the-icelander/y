//! The lint `type_argument_name` (Y § 3.1), as `y lint` would report it: an argument that takes a type is named
//! `…_type`, and one that takes a value is not. There is no Y compiler yet; this reads the Y text as the examples use
//! it. `tools/paths/ylint.py` is the same lint for the command line: change both together.
//!
//! - An argument is `name=value` inside `kind<…>`.
//! - Its value is a type when it is a type with arguments (`list<…>`), a built-in type, or a name the module declares
//!   as a type (`structure`, `type`, `constants`, `state`, `schema`, `enumeration`).
//! - Its value is a value when it is a literal (a number, with or without a unit, a string, `true`, `false`), a path
//!   (`game_rules.size`), or a name the module declares as a `service` or a `unit`.
//! - Anything else, such as a name imported from another module, is not judged.
//! - `-- lint: allow type_argument_name because "…"` on the line, or alone on the line before it, suppresses the error.

const BUILT_IN: [&str; 19] = [
    "boolean", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float64", "integer", "decimal",
    "rational", "string", "rune", "scalar", "bytes", "void", "byte",
];
const TYPE_DECLARATIONS: [&str; 6] = ["structure", "type", "constants", "state", "schema", "enumeration"];
const VALUE_DECLARATIONS: [&str; 2] = ["service", "unit"];
const ALLOW: &str = "-- lint: allow type_argument_name because \"";

#[derive(PartialEq)]
enum Kind {
    Type,
    Value,
    Unknown,
}

/// The names the module declares after one of `words`: `structure game:` declares `game`.
fn declared(source: &str, words: &[&str]) -> Vec<String> {
    let mut out = vec![];
    for line in source.lines() {
        let t = line.trim_start().trim_start_matches("public ");
        for w in words {
            if let Some(rest) = t.strip_prefix(w).and_then(|r| r.strip_prefix(' ')) {
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
    }
    out
}

/// The `name=value` arguments of every `kind<…>` on a line, outermost first, with nested ones after.
fn arguments(line: &str) -> Vec<(String, String)> {
    let cs: Vec<char> = line.chars().collect();
    let mut out = vec![];
    for (i, c) in cs.iter().enumerate() {
        if *c != '<' || i == 0 || !(cs[i - 1].is_ascii_lowercase() || cs[i - 1].is_ascii_digit() || cs[i - 1] == '_') {
            continue;
        }
        let (mut depth, mut parts, mut cur) = (0, vec![], String::new());
        let mut closed = false;
        for &d in &cs[i + 1..] {
            match d {
                '<' => depth += 1,
                '>' if depth == 0 => {
                    closed = true;
                    break;
                }
                '>' => depth -= 1,
                _ => {}
            }
            if d == ',' && depth == 0 {
                parts.push(std::mem::take(&mut cur));
            } else {
                cur.push(d);
            }
        }
        if !closed {
            continue;
        }
        parts.push(cur);
        for p in parts {
            if let Some((name, value)) = p.split_once('=') {
                let name = name.trim();
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                    out.push((name.to_string(), value.trim().to_string()));
                }
            }
        }
    }
    out
}

fn kind(value: &str, types: &[String], values: &[String]) -> Kind {
    let v = value.trim();
    if v.contains('<') {
        return Kind::Type;
    }
    let first = v.chars().next().unwrap_or(' ');
    if first.is_ascii_digit() || first == '-' || first == '"' || v == "true" || v == "false" || v.contains('.') {
        return Kind::Value;
    }
    if BUILT_IN.contains(&v) || types.iter().any(|t| t == v) {
        Kind::Type
    } else if values.iter().any(|t| t == v) {
        Kind::Value
    } else {
        Kind::Unknown
    }
}

/// Every `type_argument_name` error in a Y source, as `file:line: …`.
pub fn type_argument_names(file: &str, source: &str) -> Vec<String> {
    let types = declared(source, &TYPE_DECLARATIONS);
    let values = declared(source, &VALUE_DECLARATIONS);
    let lines: Vec<&str> = source.lines().collect();
    let mut errors = vec![];
    for (n, line) in lines.iter().enumerate() {
        let allowed = line.contains(ALLOW) || (n > 0 && lines[n - 1].trim_start().starts_with(ALLOW));
        if allowed {
            continue;
        }
        for (name, value) in arguments(line) {
            let suffixed = name.ends_with("_type");
            match kind(&value, &types, &values) {
                Kind::Type if !suffixed => errors.push(format!(
                    "{file}:{}: type_argument_name: `{name}` takes a type, `{value}`; name it `{name}_type`",
                    n + 1
                )),
                Kind::Value if suffixed => errors.push(format!(
                    "{file}:{}: type_argument_name: `{name}` takes a value, `{value}`; name it without `_type`",
                    n + 1
                )),
                _ => {}
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::type_argument_names;

    #[test]
    fn the_lint_judges_types_values_and_suppressions() {
        let src = "structure word:\n    text is string\n\n\
                   service shop:\n    address = url\"https://shop\"\n\n\
                   a is list<element_type=word>\n\
                   b is list<element=word>\n\
                   c is decimal<precision=12, scale_type=2>\n\
                   d is http<service=shop>\n\
                   e is http<service_type=shop>\n\
                   f is map<key_type=word, value=list<element_type=uint32>>\n\
                   g is pair<first=imported>\n\
                   -- lint: allow type_argument_name because \"the standard library's name\"\n\
                   h is count<of=rune>\n\
                   i is count<of=rune>  -- lint: allow type_argument_name because \"as above\"\n";
        let errors = type_argument_names("t.y", src);
        let lines: Vec<&str> = errors.iter().map(|e| e.split(": type_argument_name").next().unwrap()).collect();
        assert_eq!(lines, ["t.y:8", "t.y:9", "t.y:11", "t.y:12"], "{errors:#?}");
        assert!(errors[0].contains("`element` takes a type, `word`; name it `element_type`"), "{}", errors[0]);
        assert!(errors[1].contains("`scale_type` takes a value, `2`"), "{}", errors[1]);
        assert!(errors[3].contains("`value` takes a type"), "{}", errors[3]);
    }
}
