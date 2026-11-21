//! The lint `type_argument_name` (Y § 3.1), as `y lint` would report it: an argument that takes a type is named
//! `…_type`, and one that takes a value is not. It reads the Y's tree, from `y-parse` (`tools/y-parse`).
//! `tools/paths/ylint.py` is the same lint for the command line: change both together.
//!
//! - An argument is `name=value` inside `kind<…>`, in the code or in the type a documentation entry ends with (`game:
//!   The game, as game.`).
//! - Its value is a type when it is a type with arguments (`list<…>`), a built-in type, or a name the module declares as
//!   a type (`structure`, `type`, `constants`, `schema`).
//! - Its value is a value when it is a literal (a number, with or without a unit, a string, `true`, `false`), a path
//!   (`game_rules.size`), or a name the module declares as a `service`.
//! - Anything else, such as a name imported from another module, is not judged.
//! - `-- lint: allow type_argument_name because "…"` on the line, or alone on the line before it, suppresses the error.
use y_parse::Node;

const BUILT_IN: [&str; 19] = [
    "boolean", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float64", "integer", "decimal",
    "rational", "string", "rune", "scalar", "bytes", "void", "byte",
];
const TYPE_DECLARATIONS: [&str; 4] = ["structure", "type_decl", "constants", "schema"];
const VALUE_DECLARATIONS: [&str; 1] = ["service"];
const ALLOW: &str = "lint: allow type_argument_name because \"";

#[derive(PartialEq)]
enum Kind {
    Type,
    Value,
    Unknown,
}

/// The names the module declares with a declaration of one of `kinds`, in a facet or not.
fn declared(tree: &Node, kinds: &[&str]) -> Vec<String> {
    let mut out = vec![];
    for d in tree.nodes("decls") {
        let inner = if d.kind == "facet" { d.nodes("decls") } else { vec![d] };
        out.extend(inner.into_iter().filter(|x| kinds.contains(&x.kind)).map(|x| x.str("name").unwrap().to_string()));
    }
    out
}

/// Whether an argument's value is a type or a value: a type node is a type unless it is a path, and a literal or a
/// negative number is a value.
fn kind(value: &Node, types: &[String], values: &[String]) -> Kind {
    if value.kind != "type" {
        return Kind::Value;
    }
    let name = value.str("name").unwrap();
    if !value.nodes("args").is_empty() {
        Kind::Type
    } else if name.contains('.') {
        Kind::Value
    } else if BUILT_IN.contains(&name) || types.iter().any(|t| t == name) {
        Kind::Type
    } else if values.iter().any(|t| t == name) {
        Kind::Value
    } else {
        Kind::Unknown
    }
}

/// Every `type_argument_name` error in a Y source, as `file:line: …`, in the order of their lines; a source that does
/// not parse gives its parse error.
#[allow(dead_code)]
pub fn type_argument_names(file: &str, source: &str) -> Vec<String> {
    match y_parse::parse(source) {
        Ok(tree) => type_argument_names_in(file, &tree),
        Err(e) => vec![format!("{file}:{}: {}", e.line, e.message)],
    }
}

/// `type_argument_names`, of a source already parsed: a test file, say, which reads the units its module declares.
#[allow(dead_code)]
pub fn type_argument_names_in(file: &str, tree: &Node) -> Vec<String> {
    let (types, values) = (declared(&tree, &TYPE_DECLARATIONS), declared(&tree, &VALUE_DECLARATIONS));
    let allows: Vec<(usize, bool)> = tree.nodes("comments").iter().filter(|c| c.str("text").unwrap().starts_with(ALLOW)).map(|c| (c.line, c.flag("alone"))).collect();
    let allowed = |line: usize| allows.iter().any(|&(l, alone)| l == line || (alone && l + 1 == line));
    let mut errors: Vec<(usize, String)> = vec![];
    for ty in tree.find("type") {
        for a in ty.nodes("args") {
            let (line, name, text) = (a.line, a.str("name").unwrap(), a.str("text").unwrap());
            if allowed(line) {
                continue;
            }
            let suffixed = name.ends_with("_type");
            match kind(a.node("value").unwrap(), &types, &values) {
                Kind::Type if !suffixed => errors.push((line, format!("{file}:{line}: type_argument_name: `{name}` takes a type, `{text}`; name it `{name}_type`"))),
                Kind::Value if suffixed => errors.push((line, format!("{file}:{line}: type_argument_name: `{name}` takes a value, `{text}`; name it without `_type`"))),
                _ => {}
            }
        }
    }
    errors.sort_by_key(|(line, _)| *line);
    errors.into_iter().map(|(_, e)| e).collect()
}

#[cfg(test)]
mod tests {
    use super::type_argument_names;

    #[test]
    fn the_lint_judges_types_values_and_suppressions() {
        let src = "structure word:\n    text is string\n\n\
                   service shop:\n    address = url\"https://shop\"\n\n\
                   structure judged:\n\
                   \x20   a is list<element_type=word>\n\
                   \x20   b is list<element=word>\n\
                   \x20   c is decimal<precision=12, scale_type=2>\n\
                   \x20   d is http<service=shop>\n\
                   \x20   e is http<service_type=shop>\n\
                   \x20   f is map<key_type=word, value=list<element_type=uint32>>\n\
                   \x20   g is pair<first=imported>\n\
                   \x20   -- lint: allow type_argument_name because \"the standard library's name\"\n\
                   \x20   h is count<of=rune>\n\
                   \x20   i is count<of=rune>  -- lint: allow type_argument_name because \"as above\"\n\
                   \x20   j is count<of=-1>\n\n\n\
                   ----------\n\
                   Does it.\n\n\
                   k: The k, as list<element=word>.\n\
                   ----------\n\
                   function f(k):\n    return\n";
        let errors = type_argument_names("t.y", src);
        let lines: Vec<&str> = errors.iter().map(|e| e.split(": type_argument_name").next().unwrap()).collect();
        assert_eq!(lines, ["t.y:9", "t.y:10", "t.y:12", "t.y:13", "t.y:24"], "{errors:#?}");
        assert!(errors[0].contains("`element` takes a type, `word`; name it `element_type`"), "{}", errors[0]);
        assert!(errors[1].contains("`scale_type` takes a value, `2`"), "{}", errors[1]);
        assert!(errors[3].contains("`value` takes a type, `list<element_type=uint32>`"), "{}", errors[3]);
        assert!(type_argument_names("t.y", "function f(:\n")[0].starts_with("t.y:1: "));
    }
}
