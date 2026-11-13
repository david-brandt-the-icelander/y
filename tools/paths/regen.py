"""Remake an example's bare files from a fresh strip: the clauses of the committed constraint files carry over by
subject, each re-pointed at its line in the Y source, and so does a module's `level`; `extra` adds clauses for subjects
that have none yet, placed at the function, rule or derived value of the subject's name. What strip cannot see of the
Y lowering is made from the Y source on every run: each `injected` parameter's mark, and each region (`region
subject`, language 6.23) after its function's subject, with its constraints as clauses at the region's header.

    python3 tools/paths/regen.py <example folder> <strip output> ['{"crate::module::function": [clause, ...]}']

The strip output is what `bare-rust strip <example> --out <dir> --src-only` writes. A clause's line is found by what
the clause says (see `place`), so edits to the Y source do not lose it; only an invariant's falls back to the text its
line had in git's HEAD. It prints the functions left with no clauses: give them some with `extra`.

It reads the Y source through its tree (`ytree.py`, from `tools/y-parse`): the functions, rules and derived values and
their facets, their `where` and `ensures` clauses, types' `where` clauses and structures' invariants, `injected`
parameters, and regions with their constraints as written."""
import json, os, re, shutil, subprocess, sys
import ytree
root, stripped = sys.argv[1], sys.argv[2]
extra = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {}
ys = [f"y/src/{f}" for f in sorted(os.listdir(f"{root}/y/src")) if f.endswith(".y") and not f.endswith(".test.y")]
src = {y: open(f"{root}/{y}").read().split("\n") for y in ys}
# the Y source's trees (`ytree`), and in them: each function, rule and derived value, with its file and facet; and the
# `where` clauses of types, which are invariants
TREES = {y: t for y, t in zip(ys, ytree.trees([f"{root}/{y}" for y in ys]).values())}
HEADS = [(y, facet, n) for y in ys for facet, d in ytree.declarations(TREES[y])
         for n in ([d] if d["kind"] in ("function", "rule") else [m for m in ytree.nodes(d, "members") if m["kind"] == "derived"])]
HEAD_AT = {(y, d["line"]): d for y, _, d in HEADS}
INVARIANT_WHERES = {id(w) for y in ys for n in ytree.walk(TREES[y]) if n["kind"] == "type_decl" for w in ytree.nodes(n, "where")}

def at(needle):
    hits = [(y, i) for y in ys for i, l in enumerate(src[y], 1) if needle in l]
    assert len(hits) == 1, (needle, hits)
    return f"{hits[0][0]}:{hits[0][1]}"

ex = os.path.basename(os.path.abspath(root))
old_src = {}

def moved(source):
    """The line in the new Y source with the text the old source had at `source`; the nearest one if several."""
    y, n = source.rsplit(":", 1)
    if y not in old_src:
        shown = subprocess.run(["git", "show", f"HEAD:examples/{ex}/{y}"], capture_output=True, text=True)
        # a file not committed yet, in a new example, has not moved
        old_src[y] = shown.stdout.split("\n") if shown.returncode == 0 else src[y]
    text = old_src[y][int(n) - 1]
    hits = [i for i, l in enumerate(src[y], 1) if l == text]
    if not hits:
        return None
    return f"{y}:{min(hits, key=lambda i: abs(i - int(n)))}"

def heads(subject):
    """The Y lines where the subject's function, rule or derived value starts; in its facet, when the subject names
    one (`crate::module::facet::function`)."""
    fn = subject.rsplit("::", 1)[1]
    facet = subject.split("::")[-2] if subject.count("::") >= 3 else None
    found = [(y, d["line"], f) for y, f, d in HEADS if d["name"] == fn]
    inside = [(y, i) for y, i, f in found if facet and f == facet]
    return inside or [(y, i) for y, i, _ in found]


def near(hits, source):
    """The hit nearest the clause's old line, in the same file if any is."""
    y, n = source.rsplit(":", 1) if source else ("", "0")
    hits.sort(key=lambda h: (h[0] != y, abs(h[1] - int(n))))
    return f"{hits[0][0]}:{hits[0][1]}"


def place(subject, c):
    """The Y line of a clause, found by what it says: a `require` at its `where name = `, an `ensure` at its `ensures
    name = `, both in the function the subject names; an invariant at its `name = ` (or the old line's text); any
    other clause at the head of its function, rule or derived value. When the subject's name is in more than one
    facet, the head nearest the clause's old line."""
    source = c.get("source")
    fn = subject.rsplit("::", 1)[1]
    name = c["text"].split()[1] if c["kind"] in ("require", "ensure", "invariant") else None
    if c["kind"] in ("require", "ensure"):
        key = "where" if c["kind"] == "require" else "ensures"
        hits = [(y, w["line"]) for y, h in heads(subject) for w in ytree.nodes(HEAD_AT[(y, h)], key) if w["name"] == name]
        if hits:
            return near(hits, source)
    if c["kind"] == "invariant":
        if source and moved(source):
            return moved(source)
        # a structure's `invariant name = …`, or a type's `where name = …`
        hits = [(y, n["line"]) for y in ys for n in ytree.walk(TREES[y])
                if n["kind"] == "invariant" or id(n) in INVARIANT_WHERES
                if n["name"] == name]
        if hits:
            return near(hits, source)
    if heads(subject):
        return near(heads(subject), source)
    if source and moved(source):
        return moved(source)
    return at(f"function {fn}(")


def y_functions():
    """Each function and rule of the Y source: (file, facet or None, name, its injected parameters, its regions as
    (line, name, constraints' texts)), each `orelse region` after the region before it."""
    out = []
    for y in ys:
        for facet, d in ytree.declarations(TREES[y]):
            if d["kind"] not in ("function", "rule"):
                continue
            # the walk visits each `orelse region` too, after the region before it
            regions = [(r["line"], r["name"], [c["text"] for c in r["constraints"]]) for r in ytree.walk(d) if r["kind"] == "region"]
            injected = [p["name"] for p in d["params"] if p["injected"]]
            out.append((y, facet, d["name"], injected, regions))
    return out


def subject_of(subjects, facet, fn):
    """The function subject a Y function lowers to: its name ends in `::facet::fn`, or `::fn` outside a facet."""
    tail = f"::{facet}::{fn}" if facet else f"::{fn}"
    hits = [s for s in subjects if s["kind"] == "function" and s["name"].endswith(tail)]
    return hits[0] if hits else None


def region_clauses(constraints):
    """A Y region's constraints, each as written, as the constraint language's clauses (language 6.23): `within d or
    fail` is `within d`, `waits until name = (p)` is `waits until name : p`, and `atomic` adds what it implies, `may
    run again` and `on failure unchanged`."""
    out = []
    for c in constraints:
        m = re.fullmatch(r"waits until (\w+) = \((.*)\)", c)
        if m:
            out.append(f"waits until {m.group(1)} : {m.group(2)}")
        elif c.startswith("within "):
            out.append(re.sub(r" or fail$", "", c))
        else:
            out.append(c)
    if "atomic" in out:
        out += ["may run again", "on failure unchanged"]
    return out


Y_FUNCTIONS = y_functions()

missing = []
for dirpath, _, files in os.walk(f"{stripped}/src"):
    for f in files:
        if not f.endswith(".constraints.json"):
            continue
        rel = os.path.relpath(os.path.join(dirpath, f), f"{stripped}/src")
        out = f"{root}/bare/src/{rel}"
        old = json.load(open(out)) if os.path.exists(out) else {"subjects": []}
        before = {s["name"]: s.get("clauses") for s in old["subjects"]}
        levels = {s["name"]: s["level"] for s in old["subjects"] if "level" in s}
        js = json.load(open(os.path.join(dirpath, f)))
        js["status"] = old["status"]
        # the Y lowering's own, which strip cannot see: each `injected` parameter is marked so, and each region of a
        # function is a `region` subject after the function's (language 6.23), its clauses at the region's header
        injected, regions = set(), {}
        for y, facet, fn, injected_params, regions_of_fn in Y_FUNCTIONS:
            sub = subject_of(js["subjects"], facet, fn)
            if sub is None:
                continue
            injected |= {(sub["name"], p) for p in injected_params}
            for i, r, constraints in regions_of_fn:
                regions.setdefault(sub["name"], []).append({"kind": "region", "name": f"{sub['name']}#{r}",
                    "clauses": [{"kind": "region", "text": t, "source": f"{y}:{i}"} for t in region_clauses(constraints)]})
        for fn_name, rs in regions.items():
            k = next(k for k, x in enumerate(js["subjects"]) if x["name"] == fn_name and x["kind"] == "function") + 1
            js["subjects"][k:k] = rs
        for s in js["subjects"]:
            if s["kind"] == "region":
                continue
            if s["name"] in levels:
                s["level"] = levels[s["name"]]
            for p in s.get("params", []):
                if (s["name"], p["name"]) in injected:
                    p["injected"] = True
            cs = (before.get(s["name"]) or []) + extra.pop(s["name"], [])
            cs.sort(key=lambda c: c["kind"] not in ("require", "ensure"))
            if cs:
                for c in cs:
                    c["source"] = place(s["name"], c)
                s["clauses"] = cs
            elif s["kind"] == "function" and not rel.endswith("tests.constraints.json") and s["name"].split("::")[-1] != "main":
                missing.append(s["name"])
        with open(out, "w") as fh:
            json.dump(js, fh, indent=2); fh.write("\n")
        shutil.copy(os.path.join(dirpath, f.replace(".constraints.json", ".brs")), out.replace(".constraints.json", ".brs"))
assert not extra, extra
print("no clauses:", missing)
