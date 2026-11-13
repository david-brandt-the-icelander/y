"""Remake an example's bare files from a fresh strip: the clauses of the committed constraint files carry over by
subject, each re-pointed at its line in the Y source, and so does a module's `level`; `extra` adds clauses for subjects
that have none yet, placed at the function, rule or derived value of the subject's name. Regions (`region` subjects,
language 6.23) carry over too, after their function's subject, their clauses at the region's header.

    python3 tools/paths/regen.py <example folder> <strip output> ['{"crate::module::function": [clause, ...]}']

The strip output is what `bare-rust strip <example> --out <dir> --src-only` writes. A clause's line is found by what
the clause says (see `place`), so edits to the Y source do not lose it; only an invariant's falls back to the text its
line had in git's HEAD. It prints the functions left with no clauses: give them some with `extra`."""
import json, os, re, shutil, subprocess, sys
root, stripped = sys.argv[1], sys.argv[2]
extra = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {}
ys = [f"y/src/{f}" for f in sorted(os.listdir(f"{root}/y/src")) if f.endswith(".y") and not f.endswith(".test.y")]
src = {y: open(f"{root}/{y}").read().split("\n") for y in ys}

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
    head = re.compile(rf"^\s*(?:public |private )?(?:function|rule) {re.escape(fn)}\(|^\s*(?:public |private )?derived {re.escape(fn)} is ")
    found = [(y, i) for y in ys for i, l in enumerate(src[y], 1) if head.search(l)]
    facet = subject.split("::")[-2] if subject.count("::") >= 3 else None
    starts = [(y, i) for y in ys for i, l in enumerate(src[y], 1) if facet and re.match(rf"(?:public |private )?facet {facet}:", l)]
    inside = [(y, i) for y, i in found if any(y == fy and i in body(fy, f) for fy, f in starts)]
    return inside or found


def body(y, start):
    """The lines of the declaration that starts at `start`: up to the next line indented no deeper."""
    ind = len(src[y][start - 1]) - len(src[y][start - 1].lstrip())
    end = start + 1
    while end <= len(src[y]) and (not src[y][end - 1].strip() or len(src[y][end - 1]) - len(src[y][end - 1].lstrip()) > ind):
        end += 1
    return range(start, end)


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
    if c["kind"] == "region":
        # a region's clauses are at its header, `region r is …:` or `orelse region r is …:`, in its function
        function, region = subject.split("#")
        hits = [(y, i) for y, h in heads(function) for i in body(y, h) if re.match(rf"\s*(?:orelse )?region {region} is ", src[y][i - 1])]
        if hits:
            return near(hits, source)
        raise SystemExit(f"region {subject}: no `region {region} is` in the function")
    fn = subject.rsplit("::", 1)[1]
    name = c["text"].split()[1] if c["kind"] in ("require", "ensure", "invariant") else None
    if c["kind"] in ("require", "ensure"):
        needle = f"where {name} = " if c["kind"] == "require" else f"ensures {name} = "
        hits = [(y, i) for y, h in heads(subject) for i in body(y, h) if needle in src[y][i - 1]]
        if hits:
            return near(hits, source)
    if c["kind"] == "invariant":
        if source and moved(source):
            return moved(source)
        hits = [(y, i) for y in ys for i, l in enumerate(src[y], 1) if re.search(rf"\b{re.escape(name)} = ", l)]
        if hits:
            return near(hits, source)
    if heads(subject):
        return near(heads(subject), source)
    if source and moved(source):
        return moved(source)
    return at(f"function {fn}(")

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
        # regions (language 6.23) are the Y lowering's, not strip's: each goes back after its function's subject
        for r in [r for r in old["subjects"] if r["kind"] == "region"]:
            fn = r["name"].split("#")[0]
            at_fn = [k for k, x in enumerate(js["subjects"]) if x["name"] == fn and x["kind"] == "function"]
            if not at_fn:
                print(f"dropped region {r['name']}: no function {fn}")
                continue
            k = at_fn[0] + 1
            while k < len(js["subjects"]) and js["subjects"][k]["kind"] == "region":
                k += 1
            js["subjects"].insert(k, r)
        for s in js["subjects"]:
            if s["name"] in levels:
                s["level"] = levels[s["name"]]
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
