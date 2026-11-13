"""Rewrites an example's bare/y-facts.json for path coverage: the decisions (loops and `compare` among them), the paths
of the module, and what each test covers; the other facts carry over, each place re-pointed at the line that now has
the text it had.

    python3 tools/paths/pathfacts.py <example folder>

Places move by comparing the files with the ones in git's HEAD, so run it on uncommitted edits (`rebare.sh` does)."""
import json, os, re, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import places, yp

root = sys.argv[1]
ex = os.path.basename(os.path.abspath(root))
facts = json.load(open(f"{root}/bare/y-facts.json"))
mods = facts["modules"]
module = next(m["module"] for m in mods if m["y"].endswith(".test.y"))
code = [m for m in mods if not m["y"].endswith(".test.y")]
test = next(m for m in mods if m["y"].endswith(".test.y"))


def lines(p):
    return open(f"{root}/{p}").read().split("\n")


old = {}


def old_lines(path):
    if path not in old:
        shown = subprocess.run(["git", "show", f"HEAD:examples/{ex}/{path}"], capture_output=True, text=True, cwd=root)
        # a file not committed yet, in a new example, has not moved
        old[path] = shown.stdout.split("\n") if shown.returncode == 0 else lines(path)
    return old[path]


def moved(place):
    """A place found by its old line's text: for facts no test finds (see places.py)."""
    path, n = place.rsplit(":", 1)
    hit = places.by_text(old_lines(path), lines(path), int(n))
    if hit is None:
        text = old_lines(path)[int(n) - 1] if int(n) <= len(old_lines(path)) else ""
        # a line taken into a region's header keeps its text there: `within d or fail:` in `region r is within d or
        # fail:`, or a claim's `because "…"`
        because = re.search(r'because ".*"', text)
        key = because.group(0) if because else text.strip()
        hits = [i for i, l in enumerate(lines(path), 1) if key and "region " in l and key in l]
        hit = min(hits, key=lambda i: abs(i - int(n))) if hits else None
    if hit is None:
        raise SystemExit(f"{place}: `{old_lines(path)[int(n) - 1]}` is gone")
    return f"{path}:{hit}"


def is_place(v):
    return isinstance(v, str) and re.fullmatch(r"(y|bare)/src/[\w/.-]+:\d+", v) is not None


def relocate(v):
    if isinstance(v, dict):
        return {k: relocate(x) for k, x in v.items()}
    if isinstance(v, list):
        return [relocate(x) for x in v]
    if is_place(v):
        return moved(v)
    return v


def enclosing(path, n):
    """The name of the function, rule or `fn` the line is in, and the line it starts at."""
    head = re.compile(r"^\s*(?:public |private )?(?:function|rule|test) (\w+)\(|^\s*(?:pub(?:\(\w+\))? )?(?:async )?fn (\w+)[<(]")
    for i in range(n, 0, -1):
        m = head.match(lines(path)[i - 1])
        if m:
            return m.group(1) or m.group(2), i
    return None, 0


def find(kind, found, keep=()):
    """Re-points the places of every fact of a kind by its anchors (places.py); the rest by their old text."""
    groups = {}
    for f in found:
        for field, (key, test) in places.anchors(kind, f).items():
            if is_place(f.get(field)):
                path, n = f[field].rsplit(":", 1)
                groups.setdefault((key, path), []).append((f, field, int(n), test))
    done = set()
    for (key, path), group in groups.items():
        for (f, field, _, _), hit in zip(group, places.locate([(n, t) for _, _, n, t in group], lines(path))):
            if hit is not None:
                f[field] = f"{path}:{hit}"
                done.add((id(f), field))
    out = []
    for f in found:
        g = {k: (v if (id(f), k) in done or k in keep else relocate(v)) for k, v in f.items()}
        out.append(g)
    return out


# a copy is found by its bare marker, `// y: a copy` on the line that clones what it copies; its Y line is the line in
# the function of the same name that names what is copied, the one as far into the function as before
copies = []
try:
    found = find("copies", facts.get("copies", []), keep=("y",))
except SystemExit:
    found = []
    for c in facts.get("copies", []):
        try:
            found += find("copies", [c], keep=("y",))
        except SystemExit as e:
            print("dropped a copy whose line is gone:", c["what"], c["bare"], e)
for c in found:
    old_y = c["y"]
    ypath, yn = old_y.rsplit(":", 1)
    name, _ = enclosing(c["bare"].rsplit(":", 1)[0], int(c["bare"].rsplit(":", 1)[1]))
    old_name, old_start = None, 0
    for i in range(int(yn), 0, -1):
        m = re.match(r"^\s*(?:public |private )?(?:function|rule) (\w+)\(", old_lines(ypath)[i - 1]) if i <= len(old_lines(ypath)) else None
        if m:
            old_name, old_start = m.group(1), i
            break
    starts = [i for i, l in enumerate(lines(ypath), 1) if re.match(rf"^\s*(?:public |private )?(?:function|rule) {name}\(", l)]
    if starts and old_name == name:
        start = min(starts, key=lambda i: abs(i - old_start))
        ends = [i for i, l in enumerate(lines(ypath), 1) if i > start and re.match(r"^\s*(?:public |private )?(?:function|rule|test|type|facet) ", l)]
        end = ends[0] if ends else len(lines(ypath)) + 1
        hits = [i for i in range(start, end) if c["what"] in lines(ypath)[i - 1]]
        if hits:
            c["y"] = f"{ypath}:{min(hits, key=lambda i: abs(i - start - (int(yn) - old_start)))}"
    if c["y"] == old_y:
        try:
            c["y"] = moved(old_y)
        except SystemExit as e:
            print("dropped a copy whose Y line is gone:", c["what"], e)
            continue
    copies.append(c)
if "copies" in facts:
    facts["copies"] = copies
    listed_bare = {c["bare"] for c in copies}
    for m in mods:
        for i, l in enumerate(lines(m["bare"]), 1):
            if "// y: a copy" in l and f"{m['bare']}:{i}" not in listed_bare:
                print(f"WARNING: {m['bare']}:{i} is marked a copy, and no entry in `copies` lists it: {l.strip()}")
for key in list(facts):
    if key in ("facets", "units", "capabilities", "claims", "suspensions", "deadlines", "compiled"):
        facts[key] = find(key, facts[key])
    elif key not in ("decisions", "tests", "paths", "modules", "format", "about", "copies"):
        facts[key] = relocate(facts[key])

# every function with injected capabilities is listed, the new ones among them
COUNT = {1: "as the last parameter", 2: "as the last two parameters, in order", 3: "as the last three parameters, in order",
         4: "as the last four parameters, in order", 5: "as the last five parameters, in order"}
listed = {c["subject"].rsplit("::", 1)[1] for c in facts["capabilities"]}
subjects = {}
for dirpath, _, files in os.walk(f"{root}/bare/src"):
    for f in files:
        if f.endswith(".constraints.json") and "tests" not in f:
            for sj in json.load(open(os.path.join(dirpath, f)))["subjects"]:
                if sj["kind"] == "function":
                    subjects.setdefault(sj["name"].rsplit("::", 1)[1], sj["name"])
for m in code:
    for i, l in enumerate(lines(m["y"]), 1):
        mm = re.match(r"\s*function (\w+)\(([^)]*)\)", l)
        if mm and "injected" in mm.group(2) and mm.group(1) not in listed:
            caps = [p.split()[-1] for p in mm.group(2).split(",") if "injected" in p]
            facts["capabilities"].append({"subject": subjects[mm.group(1)], "injected": [f"{c} as {c}" for c in caps],
                                          "lowered": COUNT[len(caps)], "y": f"{m['y']}:{i}"})
facts["capabilities"].sort(key=lambda c: (c["y"].endswith(("main.y",) ) or "main.y:" in c["y"], int(c["y"].rsplit(":", 1)[1])))

# the decisions: `if a = (…):` / `else b:`; `conditional l:` and `compare l = a with b:`, with `condition l.x = …:` and
# `else l.y:`; `for x in …:` and `while x = (…):`, with `else y` or `else y:` when the loop may never run
ydecl = re.compile(r"^\s*(function|test) (\w+)")
decisions = {}
for m in code:
    yp_, mod = m["y"], m["module"]
    cur, open_at, regions = None, {}, {}
    for i, l in enumerate(lines(yp_), 1):
        d = ydecl.match(l)
        if d:
            cur, open_at, regions = d.group(2), {}, {}
        ind = len(l) - len(l.lstrip())
        t = l.strip()
        here = f"{yp_}:{i}"
        mm = re.match(r"(if|while) (\w+) = .*:$", t) or re.match(r"(for) (\w+) in .*:$", t)
        if mm:
            group = {"kind": mm.group(1), "branches": [{"name": mm.group(2), "y": here}]}
            decisions.setdefault(f"{mod}.{cur}", []).append(group)
            open_at[ind] = group
            continue
        # `region a is …:` then `orelse region b is …:`: alternatives, named by their regions
        mm = re.match(r"orelse region (\w+) is .*:$", t)
        if mm:
            if ind in open_at and open_at[ind]["kind"] == "orelse":
                open_at[ind]["branches"].append({"name": mm.group(1), "y": here})
            elif ind in regions:
                first, at = regions[ind]
                group = {"kind": "orelse", "branches": [{"name": first, "y": at}, {"name": mm.group(1), "y": here}]}
                decisions.setdefault(f"{mod}.{cur}", []).append(group)
                open_at[ind] = group
            continue
        mm = re.match(r"region (\w+) is .*:$", t)
        if mm:
            regions[ind] = (mm.group(1), here)
        mm = re.match(r"(conditional|compare) (\w+)", t)
        if mm:
            group = {"kind": mm.group(1), "label": mm.group(2), "y": here, "branches": []}
            decisions.setdefault(f"{mod}.{cur}", []).append(group)
            open_at[ind + 4] = group
            continue
        mm = re.match(r"else ([\w.]+):?$", t) or re.match(r"condition (\w+\.\w+) = ", t)
        if mm and ind in open_at:
            open_at[ind]["branches"].append({"name": mm.group(1), "y": here})
            if not t.startswith("condition "):
                del open_at[ind]
            continue
        # a statement at the indentation of an open loop or `if` closes it
        if t and not t.startswith("--"):
            for k in [k for k in open_at if k >= ind and open_at[k]["kind"] not in ("conditional", "compare")]:
                del open_at[k]
# where the bare bodies take each branch: the next `#name` in the function not yet given to a branch
bdecl = re.compile(r"^\s*(?:pub )?(?:async )?fn (\w+)[<(]")
for m in code:
    mod, bp = m["module"], m["bare"]
    cur, used = None, set()
    for i, l in enumerate(lines(bp), 1):
        d = bdecl.match(l)
        if d:
            cur = d.group(1)
        if "// y:" not in l:
            continue
        for name in re.findall(r"#([\w.']+)", l.split("// y:", 1)[1]):
            for g in decisions.get(f"{mod}.{cur}", []):
                for b in g["branches"]:
                    if b["name"] == name.split("'")[0] and "bare" not in b and (name, i) not in used:
                        b["bare"] = f"{bp}:{i}"
                        used.add((name, i))
missing = [(k, b["name"]) for k, v in decisions.items() for g in v for b in g["branches"] if "bare" not in b]
assert not missing, missing

# the paths of the module, and how each ends
y_module = yp.Module(f"{root}/{next(m['y'] for m in code if m['module'] == module)}")
statics, problems = yp.all_paths(y_module)
assert not problems, problems
ends = {"next": "returns", "return": "returns", "continue": "returns", "break": "returns", "fail": "fails",
        "not_reachable": "not_reachable", "not_implemented": "not_implemented"}
paths = [{"path": p, "ends": ends[k]} for p, k in statics.items()]

tests, cur, reading = {}, None, False
for i, l in enumerate(lines(test["y"]), 1):
    mm = re.match(r"^test (\w+)", l)
    if mm:
        cur = mm.group(1)
        tests[cur] = {"y": f"{test['y']}:{i}", "rust": f"{facts['facets'][0]['rust'].rsplit('::', 2)[0]}::{module}::tests::{cur}" if facts["facets"] else None, "covers": []}
    mm = re.match(r"^\s+covers (.*)", l)
    if mm and cur:
        reading = True
    if cur and reading and l.startswith("        "):
        text = mm.group(1) if mm else l.strip()
        tests[cur]["covers"] += [c.strip() for c in text.rstrip(":").split(",") if c.strip()]
        if l.rstrip().endswith(":"):
            reading = False
krate = re.search(r'^name = "([^"]+)"', open(f"{root}/Cargo.toml").read(), re.M).group(1).replace("-", "_")
for name, t in tests.items():
    t["rust"] = f"{krate}::{module}::tests::{name}"

out = {}
for k, v in facts.items():
    out[k] = v
    if k == "decisions":
        out[k] = decisions
        out["paths"] = paths
    if k == "paths":
        out[k] = paths
    if k == "tests":
        out[k] = tests
out["decisions"] = decisions
with open(f"{root}/bare/y-facts.json", "w") as fh:
    json.dump(out, fh, indent=2)
    fh.write("\n")
required = {p["path"] for p in paths if p["ends"] not in ("not_reachable", "not_implemented")}
covered = {c for t in tests.values() for c in t["covers"]}
print(ex, sum(len(v) for v in decisions.values()), "decisions;", len(paths), "paths;", len(required), "to test;",
      len(tests), "tests;", "uncovered:", sorted(required - covered), "unknown:", sorted(covered - required))
