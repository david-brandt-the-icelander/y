"""Finds the lines a Y fact is about by what the fact says, not by the text its line had.

A fact in bare/y-facts.json names its places as `file:line`. After an edit those numbers are stale. Each kind of fact
says enough to find its line again: a claim's `because "…"` is on its Y line and in the `// y:` marker of its bare line,
a facet's line is `facet <name>:`, a capability's is its function's head, and so on. `anchors` turns a fact into one
test per place; `locate` finds the lines that pass it.

When several facts share a test (two regions `frame_shown is forever because "…"`), they take its lines in order: the
first fact, by its old line, the first line. When the number of lines and of facts differ, each fact takes the line
nearest its old one. A fact with no test, or none that passes, falls back to its old line's text (see `by_text`)."""
import re


def _bare_marker(line):
    return line.split("// y:", 1)[1] if "// y:" in line else None


def _head(name):
    return re.compile(rf"^\s*(?:public |private )?(?:function|rule) {re.escape(name)}\(|^\s*derived {re.escape(name)} is ")


def _call(point):
    """`window.next_frame()` or `choose_level(` to the text that starts the call: `window.next_frame(`."""
    return point.split("(", 1)[0] + "("


def anchors(kind, fact):
    """{field: (key, test)} for the fact's places. Facts with the same key share the lines that pass the test."""
    last = fact.get("subject", "").rsplit("::", 1)[-1]
    out = {}
    if kind == "facets":
        out["y"] = re.compile(rf"^\s*(?:public |private )?facet {re.escape(fact['facet'])}:").search
    elif kind == "capabilities":
        out["y"] = _head(last).search
    elif kind == "units":
        if "param" in fact:
            out["y"] = _head(last).search
        elif "constraint" in fact:
            name = fact["constraint"]
            out["y"] = re.compile(rf"\b{re.escape(name)} = ").search
        else:
            out["y"] = re.compile(rf"^\s*{re.escape(last.lower())} is ").search
    elif kind == "claims":
        because = f'because "{fact["because"]}"'
        out["y"] = lambda l: because in l and fact["claim"] in l.split(because)[0]
        out["bare"] = lambda l: because in (_bare_marker(l) or "")
    elif kind == "suspensions":
        call = _call(fact["point"])
        at = re.compile(rf"(?<![\w]){re.escape(call)}").search
        out["y"] = lambda l: at(l) and not _head(call[:-1].rsplit(".", 1)[-1]).search(l) and not l.lstrip().startswith("--")
        out["bare"] = lambda l: at(l) and ".await" in l and not re.search(r"\bfn ", l)
    elif kind == "deadlines":
        out["y"] = lambda l: re.match(rf"\s*region \w+ is within {re.escape(fact['bound'])} or fail:", l)
        out["bare"] = lambda l: "Deadline::after(" in l
    elif kind == "compiled":
        out["y"] = lambda l: f"compile {fact['call']}" in l
        out["bare"] = re.compile(rf"\bconst {re.escape(fact['value'].rsplit('::', 1)[1])}\b").search
    elif kind == "copies":
        what = fact["what"]
        out["bare"] = lambda l: _bare_marker(l) is not None and _bare_marker(l).strip().startswith("a copy") and f"{what}.clone()" in l
    # the key: what the facts that share a test have in common
    keys = {"facets": ("facet",), "capabilities": ("subject",), "units": ("subject", "param", "constraint"),
            "claims": ("claim", "because"), "suspensions": ("point",), "deadlines": ("bound",), "compiled": ("call",),
            "copies": ("what",)}[kind]
    key = (kind,) + tuple(fact.get(k) for k in keys)
    if kind == "capabilities" or kind == "units":
        key = (kind, last) + tuple(fact.get(k) for k in keys[1:])
    return {field: (key, test) for field, test in out.items()}


def locate(group, lines):
    """`group`: [(old line number, test)] that share a key; `lines`: the file now. Returns a line number, or None, for
    each, in the group's order."""
    hits = [i for i, l in enumerate(lines, 1) if group[0][1](l)]
    if not hits:
        return [None] * len(group)
    order = sorted(range(len(group)), key=lambda k: group[k][0])
    if len(hits) == len(group):
        out = [None] * len(group)
        for k, h in zip(order, hits):
            out[k] = h
        return out
    return [min(hits, key=lambda i: abs(i - n)) for n, _ in group]


def by_text(old, new, n, path=""):
    """The line in `new` with the text `old` had at line `n`, the nearest if several; the same number if the file kept
    its length; else None."""
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
