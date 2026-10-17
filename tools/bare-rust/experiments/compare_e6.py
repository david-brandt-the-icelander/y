#!/usr/bin/env python3
"""compare_e6.py: E6 (bare-renamed, the current bare form) against E4's bare and rust arms, per kind and per task."""
import glob, json, os, statistics as st
here = os.path.dirname(os.path.abspath(__file__))
runs = [json.load(open(f)) for f in glob.glob(here + "/results/*.json")]
def pick(exp, arm):
    return [r for r in runs if r["experiment"] == exp and r["arm"] == arm]
arms = [("E4 rust", pick("e4", "rust")), ("E4 bare", pick("e4", "bare")), ("E6 bare-renamed", pick("e6", "bare-renamed"))]
def att(r):
    return (r.get("agent") or {}).get("attempts")
def row(name, rs):
    if not rs:
        return f"| {name} | 0 | | | | |"
    a = [att(r) for r in rs if att(r) is not None]
    l = [r["lines_changed"] for r in rs if r.get("lines_changed") is not None]
    j = [r["json_entries_changed"] for r in rs if r.get("json_entries_changed") is not None]
    return (f"| {name} | {len(rs)} | {sum(r['pass'] for r in rs)} | {st.mean(a):.2f} | "
            f"{st.mean(l):.1f} | {(st.mean(j) if j else 0):.1f} |")
# Tokens are left out: E4 recorded each run's cumulative input (cache reads included), the harness now reports a
# subagent's total in another measure, so the two do not compare.
print("| arm | runs | passed | attempts | lines changed | JSON entries changed |")
print("|---|---:|---:|---:|---:|---:|")
for kind in ("body", "signature", None):
    for name, rs in arms:
        sel = [r for r in rs if kind is None or r["kind"] == kind]
        print(row(f"{name}{', ' + kind if kind else ', all'}", sel))
print()
print("| task | kind | E4 rust | E4 bare | E6 bare-renamed |")
print("|---|---|---:|---:|---:|")
tasks = sorted({r["task"] for r in runs if r["experiment"] == "e6"})
for t in tasks:
    cells = []
    for name, rs in arms:
        sel = [r for r in rs if r["task"] == t]
        a = [att(r) for r in sel if att(r) is not None]
        cells.append(f"{st.mean(a):.1f} ({sum(r['pass'] for r in sel)}/{len(sel)})" if sel else "")
    kind = next(r["kind"] for r in runs if r["task"] == t)
    print(f"| {t} | {kind} | " + " | ".join(cells) + " |")
