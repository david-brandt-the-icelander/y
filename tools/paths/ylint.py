"""The lint `type_argument_name` (Y § 3.1), as `y lint` would report it: an argument that takes a type is named
`…_type`, and one that takes a value is not. It reads the Y's tree (`ytree.py`, from `tools/y-parse`).

    python3 tools/paths/ylint.py examples/hangman/y/src/*.y

It prints each error as `file:line: type_argument_name: …` and exits with status 1 if there is any.
`tools/bare-rust/tests/y_lint/mod.rs` is the same lint, which the example checks run: change both together.

- An argument is `name=value` inside `kind<…>`, in the code or in the type a documentation entry ends with (`game: The
  game, as game.`).
- Its value is a type when it is a type with arguments (`list<…>`), a built-in type, or a name the module declares as a
  type (`structure`, `type`, `constants`, `schema`).
- Its value is a value when it is a literal (a number, with or without a unit, a string, `true`, `false`), a path
  (`game_rules.size`), or a name the module declares as a `service`.
- Anything else, such as a name imported from another module, is not judged.
- `-- lint: allow type_argument_name because "…"` on the line, or alone on the line before it, suppresses the error."""
import sys
import ytree

BUILT_IN = {"boolean", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float64", "integer",
            "decimal", "rational", "string", "rune", "scalar", "bytes", "void", "byte"}
TYPE_DECLARATIONS = ("structure", "type_decl", "constants", "schema")
VALUE_DECLARATIONS = ("service",)
ALLOW = 'lint: allow type_argument_name because "'


def declared(tree, kinds):
    """The names the module declares with a declaration of one of `kinds`, in a facet or not."""
    return {d["name"] for _, d in ytree.declarations(tree) if d["kind"] in kinds}


def kind(value, types, values):
    """Whether an argument's value is a type or a value: a type node is a type unless it is a path, and a literal or a
    negative number is a value."""
    if value["kind"] != "type":
        return "value"
    name = value["name"]
    if value["args"]:
        return "type"
    if "." in name:
        return "value"
    if name in BUILT_IN or name in types:
        return "type"
    if name in values:
        return "value"
    return None


def type_argument_names(file, tree):
    """Every `type_argument_name` error in a Y file's tree, as `file:line: …`, in the order of their lines."""
    types, values = declared(tree, TYPE_DECLARATIONS), declared(tree, VALUE_DECLARATIONS)
    allows = [(c["line"], c["alone"]) for c in tree["comments"] if c["text"].startswith(ALLOW)]
    errors = []
    for ty in ytree.walk(tree):
        if ty["kind"] != "type":
            continue
        for a in ty["args"]:
            line, name, text = a["line"], a["name"], a["text"]
            if any(l == line or (alone and l + 1 == line) for l, alone in allows):
                continue
            suffixed = name.endswith("_type")
            k = kind(a["value"], types, values)
            if k == "type" and not suffixed:
                errors.append((line, f"{file}:{line}: type_argument_name: `{name}` takes a type, `{text}`; name it `{name}_type`"))
            elif k == "value" and suffixed:
                errors.append((line, f"{file}:{line}: type_argument_name: `{name}` takes a value, `{text}`; name it without `_type`"))
    return [e for _, e in sorted(errors, key=lambda e: e[0])]


if __name__ == "__main__":
    trees = ytree.trees(sys.argv[1:])
    found = [e for path in sys.argv[1:] for e in type_argument_names(path, trees[path])]
    print("\n".join(found) if found else "type_argument_name: no errors")
    sys.exit(1 if found else 0)
