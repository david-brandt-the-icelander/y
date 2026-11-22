#!/usr/bin/env python3
"""summarize.py [results-dir]: the E4 and E5 tables from results/*.json, as Markdown on stdout."""
import glob, json, os, sys
from collections import Counter, defaultdict
here = os.path.dirname(os.path.abspath(__file__))
rd = sys.argv[1] if len(sys.argv) > 1 else here + "/results"
R = [json.load(open(p)) for p in sorted(glob.glob(rd + "/*.json"))]
LIB = os.environ.get("LIBFILE", here + "/../libraries/std-1.97.0.constraints.json")


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def f(x, d=2):
    return "-" if x is None else (("%." + str(d) + "f") % x)


def pct(n, d):
    return "-" if not d else "%d/%d (%.0f%%)" % (n, d, 100.0 * n / d)


def ag(r, k):
    a = r.get("agent") or {}
    return a.get(k)


def stats(rs):
    n = len(rs)
    return dict(
        n=n,
        pass_=pct(sum(1 for r in rs if r.get("pass")), n),
        compile=pct(sum(1 for r in rs if r.get("compiles")), n),
        hidden=f(mean([r["tests_passed"] / r["tests_total"] for r in rs if r.get("tests_total")]), 2),
        attempts=f(mean([ag(r, "attempts") for r in rs]), 1),
        tokens=f(mean([(r.get("tokens") or {}).get("total") for r in rs]), 0),
        jedits=f(mean([ag(r, "json_edits") for r in rs if r["arm"] != "rust"]), 1),
        jentries=f(mean([r.get("json_entries_changed") for r in rs if r["arm"] != "rust"]), 1),
        lines=f(mean([r.get("lines_changed") for r in rs]), 1),
        selfdone=pct(sum(1 for r in rs if ag(r, "done") and r.get("pass")), sum(1 for r in rs if ag(r, "done"))),
    )


COLS = [("runs", "n"), ("pass", "pass_"), ("compiles", "compile"), ("hidden tests passed (mean fraction)", "hidden"),
        ("attempts (mean)", "attempts"), ("tokens (mean)", "tokens"), ("JSON edits, self-reported (mean)", "jedits"),
        ("constraint entries changed, graded (mean)", "jentries"), ("lines changed (mean)", "lines"),
        ("pass among self-reported done", "selfdone")]


def table(groups, label):
    out = ["| %s | %s |" % (label, " | ".join(c[0] for c in COLS)), "|%s|" % "|".join(["---"] + ["---:"] * len(COLS))]
    for name, rs in groups:
        s = stats(rs)
        out.append("| %s | %s |" % (name, " | ".join(str(s[c[1]]) for c in COLS)))
    return "\n".join(out)


e4 = [r for r in R if r["experiment"] == "e4"]
e5 = [r for r in R if r["experiment"] == "e5"]
print("# Results of E4 and E5 (%d runs read: %d E4, %d E5)\n" % (len(R), len(e4), len(e5)))

print("## E4, by arm\n")
print(table([(a, [r for r in e4 if r["arm"] == a]) for a in ("rust", "bare")], "arm"))
print("\n## E4, by arm and kind of task\n")
print(table([("%s / %s" % (a, k), [r for r in e4 if r["arm"] == a and r["kind"] == k]) for k in ("body", "signature") for a in ("rust", "bare")], "arm / kind"))
print("\n## E4, per task (pass over trials: rust | bare)\n")
tasks = sorted({r["task"] for r in e4})
print("| task | kind | rust pass | bare pass | rust attempts | bare attempts | bare JSON edits (self) | bare entries changed (graded) |\n|---|---|---|---|---|---|---|---|")
for t in tasks:
    rr = [r for r in e4 if r["task"] == t and r["arm"] == "rust"]
    bb = [r for r in e4 if r["task"] == t and r["arm"] == "bare"]
    k = (rr or bb)[0]["kind"]
    print("| %s | %s | %s | %s | %s | %s | %s | %s |" % (t, k, pct(sum(bool(r.get("pass")) for r in rr), len(rr)),
          pct(sum(bool(r.get("pass")) for r in bb), len(bb)), f(mean([ag(r, "attempts") for r in rr]), 1), f(mean([ag(r, "attempts") for r in bb]), 1),
          f(mean([ag(r, "json_edits") for r in bb]), 1), f(mean([r.get("json_entries_changed") for r in bb]), 1)))
viol = [r["run_id"] for r in e4 + e5 if r.get("rs_files_in_bare")]
if viol:
    print("\nBare-arm runs that left `.rs` files in the workspace (not woven from): %s" % ", ".join(viol))

print("\n## E5, library variants (E5 tasks only; full = the E4 bare arm on the same tasks)\n")
e5tasks = {r["task"] for r in e5}
variants = [("full library (E4 bare)", [r for r in e4 if r["arm"] == "bare" and r["task"] in e5tasks]),
            ("no library files (bare-nolib)", [r for r in e5 if r["arm"] == "bare-nolib"]),
            ("library without `unexpressed` (bare-nounexpressed)", [r for r in e5 if r["arm"] == "bare-nounexpressed"])]
print(table(variants, "variant"))
print("\nDeltas against the full library (positive = better than the full library; pass rate in percentage points, attempts and tokens as the change):\n")
base = stats(variants[0][1])


def num(s):  # "x/y (z%)" -> z
    return float(s.split("(")[1].rstrip("%)")) if "(" in s else None


print("| variant | d pass (points) | d compiles (points) | d attempts | d tokens | d entries changed |\n|---|---:|---:|---:|---:|---:|")
for name, rs in variants[1:]:
    s = stats(rs)

    def d(k, nf=lambda x: float(x)):
        try:
            return "%+.1f" % (nf(s[k]) - nf(base[k]))
        except Exception:
            return "-"
    print("| %s | %s | %s | %s | %s | %s |" % (name, d("pass_", num), d("compile", num), d("attempts"), d("tokens"), d("jentries")))
print("\n### E5 by kind of task\n")
print(table([("%s / %s" % (n.split(" ")[0], k), [r for r in rs if r["kind"] == k]) for k in ("body", "signature") for n, rs in variants], "variant / kind"))

# library promises cited
try:
    lib = json.load(open(LIB))
    subj = {}

    def walk(o):
        if isinstance(o, dict):
            if "name" in o and "kind" in o and isinstance(o.get("clauses"), list):
                subj[o["name"]] = any(c.get("kind") == "unexpressed" for c in o["clauses"] if isinstance(c, dict))
            elif "name" in o and "kind" in o:
                subj.setdefault(o["name"], False)
            for v in o.values():
                walk(v)
        elif isinstance(o, list):
            for v in o:
                walk(v)
    walk(lib)
except Exception:
    subj = {}
print("\n## Library promises cited (agent self-report, `library_promises_relied_on`)\n")
print("| arm | runs | runs citing at least one | citations (total) | distinct ids | citations of entries that carry an `unexpressed` clause | ids not in the library file |\n|---|---:|---:|---:|---:|---:|---:|")
for a in ("bare", "bare-nolib", "bare-nounexpressed"):
    rs = [r for r in R if r["arm"] == a]
    cites = [c for r in rs for c in (ag(r, "library_promises_relied_on") or [])]
    cnt = Counter(cites)
    print("| %s | %d | %d | %d | %d | %d | %d |" % (a, len(rs), sum(1 for r in rs if ag(r, "library_promises_relied_on")), len(cites), len(cnt),
          sum(v for k, v in cnt.items() if subj.get(k)), sum(v for k, v in cnt.items() if k not in subj)))
cnt = Counter(c for r in R if r["arm"] == "bare" for c in (ag(r, "library_promises_relied_on") or []))
if cnt:
    print("\nMost cited in the full-library arm:\n")
    for k, v in cnt.most_common(15):
        print("- `%s`: %d%s" % (k, v, " (has `unexpressed`)" if subj.get(k) else ""))
# per-task pass by variant for E5
print("\n## E5, per task (pass over trials)\n")
print("| task | full | no library | no `unexpressed` |\n|---|---|---|---|")
for t in sorted(e5tasks):
    cells = []
    for name, rs in variants:
        x = [r for r in rs if r["task"] == t]
        cells.append(pct(sum(bool(r.get("pass")) for r in x), len(x)))
    print("| %s | %s |" % (t, " | ".join(cells)))
