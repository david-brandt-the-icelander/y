"""Remake an example's bare files from a fresh strip: the clauses of the committed constraint files carry over by
subject, each re-pointed at its line in the Y source; `extra` adds clauses for subjects that have none yet, placed at
the function, rule or derived value of the subject's name.

    python3 tools/paths/regen.py <example folder> <strip output> ['{"crate::module::function": [clause, ...]}']

The strip output is what `bare-rust strip <example> --out <dir> --src-only` writes. Lines move by comparing the Y with
the one in git's HEAD, so run it on uncommitted edits (`rebare.sh` does). It prints the functions left with no clauses:
give them some with `extra`."""
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
        old_src[y] = subprocess.run(["git", "show", f"HEAD:examples/{ex}/{y}"], capture_output=True, text=True, check=True).stdout.split("\n")
    text = old_src[y][int(n) - 1]
    hits = [i for i, l in enumerate(src[y], 1) if l == text]
    if not hits:
        return None
    return f"{y}:{min(hits, key=lambda i: abs(i - int(n)))}"

def place(subject, c):
    if "source" in c and moved(c["source"]):
        return moved(c["source"])
    fn = subject.rsplit("::", 1)[1]
    name = c["text"].split()[1] if c["kind"] in ("require", "ensure") else None
    if c["kind"] == "require":
        return at(f"where {name} = ")
    if c["kind"] == "ensure":
        return at(f"ensures {name} = ")
    for needle in (f"function {fn}(", f"rule {fn}(", f"derived {fn} is "):
        if any(needle in l for y in ys for l in src[y]):
            return at(needle)
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
        js = json.load(open(os.path.join(dirpath, f)))
        js["status"] = old["status"]
        for s in js["subjects"]:
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
