"""Finds the lines a Y fact is about by what the fact says, not by the text its line had.

A fact in bare/y-facts.json names its places as `file:line`. After an edit those numbers are stale. Each kind of fact
says enough to find its line again: a claim's `because "…"` is a node of the Y's tree and in the `// y:` marker of its
bare line, a facet's line is its `facet` node's, a capability's is its function's head, and so on. `anchors` turns a
fact into one finder per place; a finder gives the lines of a file that it finds. A place in the Y source is found in
the file's tree (`ytree.py`, from `tools/y-parse`); a place in a bare body, by its line's text.

When several facts share a finder (two regions `frame_shown is forever because "…"`), they take its lines in order: the
first fact, by its old line, the first line. When the number of lines and of facts differ, each fact takes the line
nearest its old one. A fact with no finder, or none that finds a line, falls back to where its old line went: in the Y
source, the node that started on it, found again in the tree (see `same_node`); in a bare body, its line's text (see
`by_text`)."""
import re
import yp, ytree


def _bare_marker(line):
    return line.split("// y:", 1)[1] if "// y:" in line else None


def _lines(test):
    """A finder from a test of a line's text: for the bare bodies."""
    return lambda lines, tree: [i for i, l in enumerate(lines, 1) if test(l)]


def _nodes(test):
    """A finder from a test of a node of the Y's tree: the lines of the nodes that pass it, each once, in order."""
    return lambda lines, tree: sorted({n["line"] for n in ytree.walk(tree) if test(n)})


def heads(name):
    """A test of the head of the function, rule or derived value `name`."""
    return lambda n: n["kind"] in ("function", "rule", "derived") and n["name"] == name


def callee(n):
    """A call's callee as written, when it is a name or a path of names (`window.next_frame`); else None."""
    if n["kind"] == "name":
        return n["name"]
    if n["kind"] == "member":
        o = callee(n["object"])
        return f"{o}.{n['name']}" if o else None
    return None


def _calls(call):
    """A test of a call of `call` (`window.next_frame`, `choose_level`), on its own or after a path (`game.choose_level`)."""
    def test(n):
        c = n["kind"] == "call" and callee(n["callee"])
        return bool(c) and (c == call or c.endswith("." + call))
    return test


def anchors(kind, fact):
    """{field: (key, finder)} for the fact's places. Facts with the same key share the lines their finder finds."""
    last = fact.get("subject", "").rsplit("::", 1)[-1]
    out = {}
    if kind == "facets":
        out["y"] = _nodes(lambda n: n["kind"] == "facet" and n["name"] == fact["facet"])
    elif kind == "capabilities":
        out["y"] = _nodes(heads(last))
    elif kind == "units":
        if "param" in fact:
            out["y"] = _nodes(heads(last))
        elif "constraint" in fact:
            # a function's `where` or `ensures`, a type's `where`, or a structure's invariant
            out["y"] = _nodes(lambda n: n["kind"] in ("where", "ensures", "invariant") and n["name"] == fact["constraint"])
        else:
            # a constant, a field or a derived value: `name is …`
            out["y"] = _nodes(lambda n: n["kind"] in ("constant", "field", "derived") and n["name"] == last.lower())
    elif kind == "claims":
        because = f'because "{fact["because"]}"'
        # `e forever because "…"`, or a region's constraint `forever because "…"` or `detach because "…"`
        out["y"] = _nodes(lambda n: n.get("because") == fact["because"] and fact["claim"] == (n["word"] if n["kind"] == "constraint" else n["kind"]))
        out["bare"] = _lines(lambda l: because in (_bare_marker(l) or ""))
    elif kind == "suspensions":
        call = fact["point"].split("(", 1)[0]
        out["y"] = _nodes(_calls(call))
        at = re.compile(rf"(?<![\w]){re.escape(call + '(')}").search
        out["bare"] = _lines(lambda l: at(l) and ".await" in l and not re.search(r"\bfn ", l))
    elif kind == "deadlines":
        out["y"] = _nodes(lambda n: n["kind"] == "region" and any(c["word"] == "within" and c["text"] == f"within {fact['bound']} or fail" for c in n["constraints"]))
        out["bare"] = _lines(lambda l: "Deadline::after(" in l)
    elif kind == "compiled":
        call = fact["call"].split("(", 1)[0]
        out["y"] = _nodes(lambda n: n["kind"] == "compile" and n["value"]["kind"] == "call" and callee(n["value"]["callee"]) == call)
        out["bare"] = _lines(re.compile(rf"\bconst {re.escape(fact['value'].rsplit('::', 1)[1])}\b").search)
    elif kind == "copies":
        what = fact["what"]
        out["bare"] = _lines(lambda l: _bare_marker(l) is not None and _bare_marker(l).strip().startswith("a copy") and f"{what}.clone()" in l)
    # the key: what the facts that share a finder have in common
    keys = {"facets": ("facet",), "capabilities": ("subject",), "units": ("subject", "param", "constraint"),
            "claims": ("claim", "because"), "suspensions": ("point",), "deadlines": ("bound",), "compiled": ("call",),
            "copies": ("what",)}[kind]
    key = (kind,) + tuple(fact.get(k) for k in keys)
    if kind == "capabilities" or kind == "units":
        key = (kind, last) + tuple(fact.get(k) for k in keys[1:])
    return {field: (key, test) for field, test in out.items()}


def locate(group, lines, tree=None):
    """`group`: [(old line number, finder)] that share a key; `lines` and `tree`: the file now, and its tree when it is
    Y. Returns a line number, or None, for each, in the group's order."""
    hits = group[0][1](lines, tree)
    if not hits:
        return [None] * len(group)
    order = sorted(range(len(group)), key=lambda k: group[k][0])
    if len(hits) == len(group):
        out = [None] * len(group)
        for k, h in zip(order, hits):
            out[k] = h
        return out
    return [min(hits, key=lambda i: abs(i - n)) for n, _ in group]


DECLARATIONS = ("facet", "function", "rule", "test", "structure", "schema", "constants", "service", "type_decl", "migrate")


def ident(node):
    """What tells a node from the others of its kind: its name or label, a comment's text, a documentation block's first
    line, or an expression as text; a statement adds the expressions it writes on its own line (`yp.own`)."""
    k = node["kind"]
    if k == "schema":
        return f"{node['name']} {node['version']}"
    if k in ("comment", "path", "constraint"):
        return node["text"]
    if k == "doc":
        return node["text"].split("\n")[0]
    if k == "import":
        return f"{node['from']} {node['names']}"
    named = node.get("name") if isinstance(node.get("name"), str) else node.get("label") if isinstance(node.get("label"), str) else ""
    if k in ("binding", "arg", "constant", "setting", "derived", "invariant", "where", "ensures"):
        value = node.get("value") or node.get("condition")
        return f"{named} {yp.text(value) if value else ''}"
    own = yp.own(node)
    if own or k in ("let", "assign", "expr", "return", "fail", "task", "if", "while", "for", "conditional", "compare",
                    "condition", "region", "inject", "else", "arena", "continue", "break", "retry"):
        return " ".join([named] + [yp.text(e) if e["kind"] != "constraint" else e["text"] for e in own])
    if named:
        return named
    try:
        return yp.text(node)
    except (ValueError, KeyError):
        return ""


def located(tree):
    """Each node of a tree with the declarations it is in, as [(node, ((kind, name), …))], in source order."""
    out = []

    def go(node, outer):
        out.append((node, outer))
        inner = outer + ((node["kind"], node["name"]),) if node["kind"] in DECLARATIONS else outer
        for v in node.values():
            for x in (v if isinstance(v, list) else [v]):
                if isinstance(x, dict) and "kind" in x:
                    go(x, inner)

    go(tree, ())
    return out


def same_node(old, new, n):
    """The line in the tree `new` of the node that started at line `n` of the tree `old`: the first node there, found
    again by its kind and what tells it from others (`ident`), in the same declarations, else in any. When several are
    found, and the old tree has as many such nodes there, the one in the same place among them; else the nearest to
    `n`. None when no node started at `n`, or none is found again."""
    if old is None or new is None:
        return None
    before, after = located(old), located(new)
    at = [(node, outer) for node, outer in before if node.get("line") == n and node["kind"] != "file"]
    if not at:
        return None
    node, outer = at[0]
    key = (node["kind"], ident(node))
    for same_place in (True, False):
        def like(m, o):
            return m["kind"] != "file" and (m["kind"], ident(m)) == key and (o == outer or not same_place)
        was = [m for m, o in before if like(m, o)]
        hits = [m["line"] for m, o in after if like(m, o)]
        if hits:
            if len(was) == len(hits):
                return hits[next(i for i, m in enumerate(was) if m is node)]
            return min(hits, key=lambda i: (abs(i - n), i))
    return None


def by_text(old, new, n, path=""):
    """The line of a bare body `new` with the text `old` had at line `n`, the nearest if several; the same number if the
    file kept its length; else None."""
    text = old[n - 1] if n - 1 < len(old) else ""
    for same in (lambda l: l == text, lambda l: l.strip() == text.strip() and text.strip()):
        hits = [i for i, l in enumerate(new, 1) if same(l)]
        if hits:
            return min(hits, key=lambda i: abs(i - n))
    if "function " in text:
        head = text.split("(")[0] + "("
        hits = [i for i, l in enumerate(new, 1) if l.startswith(head)]
        if hits:
            return min(hits, key=lambda i: abs(i - n))
    if len(new) == len(old):
        return n
    return None
