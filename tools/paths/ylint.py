"""The lint `type_argument_name` (Y § 3.1), as `y lint` would report it: an argument that takes a type is named
`…_type`, and one that takes a value is not.

    python3 tools/paths/ylint.py examples/hangman/y/src/*.y

It prints each error as `file:line: type_argument_name: …` and exits with status 1 if there is any.
`tools/bare-rust/tests/y_lint/mod.rs` is the same lint, which the example checks run: change both together.

- An argument is `name=value` inside `kind<…>`.
- Its value is a type when it is a type with arguments (`list<…>`), a built-in type, or a name the module declares as a
  type (`structure`, `type`, `constants`, `state`, `schema`, `enumeration`).
- Its value is a value when it is a literal (a number, with or without a unit, a string, `true`, `false`), a path
  (`game_rules.size`), or a name the module declares as a `service` or a `unit`.
- Anything else, such as a name imported from another module, is not judged.
- `-- lint: allow type_argument_name because "…"` on the line, or alone on the line before it, suppresses the error."""
import re
import sys

BUILT_IN = {"boolean", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float64", "integer",
            "decimal", "rational", "string", "rune", "scalar", "bytes", "void", "byte"}
TYPE_DECLARATIONS = ("structure", "type", "constants", "state", "schema", "enumeration")
VALUE_DECLARATIONS = ("service", "unit")
ALLOW = '-- lint: allow type_argument_name because "'


def declared(source, words):
    names = set()
    for line in source.split("\n"):
        t = line.lstrip()
        if t.startswith("public "):
            t = t[len("public "):]
        for w in words:
            m = re.match(re.escape(w) + r" (\w+)", t)
            if m:
                names.add(m.group(1))
    return names


def arguments(line):
    out = []
    for i, c in enumerate(line):
        if c != "<" or i == 0 or not (line[i - 1].islower() or line[i - 1].isdigit() or line[i - 1] == "_"):
            continue
        depth, parts, cur, closed = 0, [], "", False
        for d in line[i + 1:]:
            if d == "<":
                depth += 1
            elif d == ">" and depth == 0:
                closed = True
                break
            elif d == ">":
                depth -= 1
            if d == "," and depth == 0:
                parts.append(cur)
                cur = ""
            else:
                cur += d
        if not closed:
            continue
        parts.append(cur)
        for p in parts:
            if "=" in p:
                name, value = p.split("=", 1)
                name = name.strip()
                if name and re.fullmatch(r"[a-z0-9_]+", name):
                    out.append((name, value.strip()))
    return out


def kind(value, types, values):
    v = value.strip()
    if "<" in v:
        return "type"
    if v[:1].isdigit() or v[:1] in ('-', '"') or v in ("true", "false") or "." in v:
        return "value"
    if v in BUILT_IN or v in types:
        return "type"
    if v in values:
        return "value"
    return None


def type_argument_names(file, source):
    types, values = declared(source, TYPE_DECLARATIONS), declared(source, VALUE_DECLARATIONS)
    lines = source.split("\n")
    errors = []
    for n, line in enumerate(lines):
        if ALLOW in line or (n > 0 and lines[n - 1].lstrip().startswith(ALLOW)):
            continue
        for name, value in arguments(line):
            suffixed = name.endswith("_type")
            k = kind(value, types, values)
            if k == "type" and not suffixed:
                errors.append(f"{file}:{n + 1}: type_argument_name: `{name}` takes a type, `{value}`; name it `{name}_type`")
            elif k == "value" and suffixed:
                errors.append(f"{file}:{n + 1}: type_argument_name: `{name}` takes a value, `{value}`; name it without `_type`")
    return errors


if __name__ == "__main__":
    found = [e for path in sys.argv[1:] for e in type_argument_names(path, open(path).read())]
    print("\n".join(found) if found else "type_argument_name: no errors")
    sys.exit(1 if found else 0)
