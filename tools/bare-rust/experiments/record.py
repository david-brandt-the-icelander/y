#!/usr/bin/env python3
"""record.py <run_id> <workspace> [--agent-file F] [--input N] [--output N] [--total N] [--duration S] [--model M]

Grades the workspace, parses the agent's self-report (the last line of F that is a JSON object with a "done" key; F is the
agent's final message), and writes results/<run_id>.json:
 {run_id, experiment, task, arm, trial, kind, e5, <grader fields>, agent:{...}, tokens:{input,output,total}, duration_s}"""
import argparse, csv, json, os, subprocess, sys
here = os.path.dirname(os.path.abspath(__file__))
ap = argparse.ArgumentParser()
ap.add_argument("run_id"); ap.add_argument("workspace")
ap.add_argument("--agent-file"); ap.add_argument("--input", type=int); ap.add_argument("--output", type=int)
ap.add_argument("--total", type=int); ap.add_argument("--duration", type=float); ap.add_argument("--model")
a = ap.parse_args()
row = next((r for r in csv.DictReader(open(here + "/matrix.tsv"), delimiter="\t") if r["run_id"] == a.run_id), None)
if not row:
    sys.exit("run_id not in matrix.tsv")
g = json.loads(subprocess.run([here + "/grade.sh", row["task"], row["arm"], a.workspace], capture_output=True, text=True, check=True).stdout.strip().splitlines()[-1])
meta = json.load(open(here + "/tasks/%s/task.json" % row["task"]))
agent = None
if a.agent_file and os.path.exists(a.agent_file):
    for line in reversed(open(a.agent_file).read().strip().splitlines()):
        line = line.strip().strip("`")
        if line.startswith("{") and '"done"' in line:
            try:
                agent = json.loads(line); break
            except Exception:
                pass
tot = a.total if a.total is not None else ((a.input or 0) + (a.output or 0) if (a.input or a.output) else None)
res = dict(run_id=a.run_id, experiment=a.run_id.split("-")[0], task=row["task"], arm=row["arm"], trial=int(row["trial"]),
           kind=meta["kind"], e5=meta["e5"], **{k: v for k, v in g.items() if k not in ("task", "arm")},
           agent=agent, tokens=dict(input=a.input, output=a.output, total=tot), duration_s=a.duration, model=a.model)
os.makedirs(here + "/results", exist_ok=True)
json.dump(res, open(here + "/results/%s.json" % a.run_id, "w"), indent=1, ensure_ascii=False)
print(json.dumps({k: res[k] for k in ("run_id", "pass", "compiles", "tests_passed", "tests_total", "json_entries_changed", "lines_changed")}))
