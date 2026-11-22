#!/usr/bin/env python3
"""Writes matrix.tsv: run_id task arm trial.  E4: 20 tasks x {rust,bare} x 2 trials = 80; E5: the tasks with e5=true x
{bare-nolib,bare-nounexpressed} x 2 trials = 40 (E5's third variant, the full library, is E4's bare arm)."""
import glob, json, os
here = os.path.dirname(os.path.abspath(__file__))
tasks = sorted((json.load(open(p)) for p in glob.glob(here + "/tasks/*/task.json")), key=lambda t: t["id"])
rows = []
for t in tasks:
    for arm in ("rust", "bare"):
        for tr in (1, 2):
            rows.append(("e4-%s-%s-t%d" % (t["id"], arm, tr), t["id"], arm, tr))
for t in tasks:
    if t["e5"]:
        for arm in ("bare-nolib", "bare-nounexpressed"):
            for tr in (1, 2):
                rows.append(("e5-%s-%s-t%d" % (t["id"], arm, tr), t["id"], arm, tr))
with open(here + "/matrix.tsv", "w") as f:
    f.write("run_id\ttask\tarm\ttrial\n")
    for r in rows:
        f.write("%s\t%s\t%s\t%d\n" % r)
print(len(rows), "runs;", sum(r[0].startswith("e4") for r in rows), "E4,", sum(r[0].startswith("e5") for r in rows), "E5")
