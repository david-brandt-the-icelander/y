#!/usr/bin/env python3
"""grade.py <task> <arm> <workspace>  ->  one JSON line on stdout (see README.md).  Called by grade.sh."""
import json, os, re, shutil, subprocess, sys, tempfile

EXP = os.path.dirname(os.path.abspath(__file__))
BARE = os.environ.get("BARE_RUST", "/tmp/claude-0/-home-user-y/9673def1-2027-5ffe-9d6e-8413dc645c64/scratchpad/bin/bare-rust")
TIMEOUT = int(os.environ.get("GRADE_TIMEOUT", "900"))


def run(cmd, cwd=None, env=None, timeout=TIMEOUT):
    e = dict(os.environ)
    if env:
        e.update(env)
    try:
        r = subprocess.run(cmd, cwd=cwd, env=e, capture_output=True, text=True, timeout=timeout)
        return r.returncode, r.stdout + r.stderr
    except subprocess.TimeoutExpired as x:
        return 124, "timeout\n" + ((x.stdout or b"").decode() if isinstance(x.stdout, bytes) else (x.stdout or ""))


def sh(script, **kw):
    return run(["bash", "-c", script], **kw)


def flatten(o, path=()):
    out = {}
    if isinstance(o, dict):
        for k, v in o.items():
            out.update(flatten(v, path + (str(k),)))
    elif isinstance(o, list):
        seen = {}
        for i, v in enumerate(o):
            if isinstance(v, dict) and "name" in v:
                k = "name=" + str(v["name"])
            elif isinstance(v, dict) and "kind" in v:
                k = "kind=" + str(v["kind"])
            else:
                k = "#"
            n = seen.get(k, 0)
            seen[k] = n + 1
            key = k if k != "#" else "#%d" % i
            if k != "#" and (n > 0 or sum(1 for w in o if isinstance(w, dict) and (("name=" + str(w.get("name"))) == k or ("kind=" + str(w.get("kind"))) == k)) > 1):
                key = "%s[%d]" % (k, n)
            out.update(flatten(v, path + (key,)))
        if not o:
            out[path + ("[]",)] = "[]"
    else:
        out[path] = o
    return out


def json_changes(base_dir, ws):
    """entries (subjects/top-level keys) and leaves that differ between the base's constraint files and the workspace's"""
    entries, leaves, files = set(), 0, 0
    names = set()
    for root in (base_dir, ws):
        for dp, dn, fn in os.walk(root):
            dn[:] = [d for d in dn if d not in ("target", ".weave", "libraries", ".git")]
            for f in fn:
                if f.endswith(".constraints.json"):
                    names.add(os.path.relpath(os.path.join(dp, f), root))
    for n in sorted(names):
        def load(root):
            p = os.path.join(root, n)
            if not os.path.exists(p):
                return {}
            try:
                return flatten(json.load(open(p)))
            except Exception:
                return {("<unparsable>",): open(p).read()}
        a, b = load(base_dir), load(ws)
        diff = [k for k in set(a) | set(b) if a.get(k, object()) != b.get(k, object())]
        if diff:
            files += 1
        leaves += len(diff)
        for k in diff:
            entries.add((n,) + k[:2])
    return len(entries), leaves, files


def lines_changed(base_dir, ws, ext, skip=()):
    total = 0
    paths = set()
    for root in (base_dir, ws):
        for dp, dn, fn in os.walk(root):
            dn[:] = [d for d in dn if d not in ("target", ".weave", ".git", "libraries", "extra-tests")]
            for f in fn:
                if f.endswith(ext):
                    rel = os.path.relpath(os.path.join(dp, f), root)
                    if rel not in skip:
                        paths.add(rel)
    for rel in paths:
        pa, pb = os.path.join(base_dir, rel), os.path.join(ws, rel)
        if not os.path.exists(pa):
            pa = "/dev/null"
        if not os.path.exists(pb):
            pb = "/dev/null"
        rc, out = run(["diff", "-U0", pa, pb])
        total += sum(1 for l in out.split("\n") if (l.startswith("+") or l.startswith("-")) and not l.startswith("+++") and not l.startswith("---"))
    return total


def main():
    task, arm, ws = sys.argv[1], sys.argv[2], os.path.abspath(sys.argv[3])
    tdir = os.path.join(EXP, "tasks", task)
    if not os.path.isdir(tdir):
        sys.exit("no such task " + task)
    crate = json.load(open(os.path.join(tdir, "task.json")))["crate"]
    hidden = open(os.path.join(tdir, "hidden.rs")).read()
    total = len(re.findall(r"#\[test\]", hidden))
    res = dict(task=task, arm=arm, workspace=ws)
    tmp = tempfile.mkdtemp(prefix="grade-")
    try:
        base = os.path.join(tmp, "base")
        sh('. "%s/common.sh"; materialize_base "%s" "%s"' % (EXP, task, base))
        woven = os.path.join(tmp, "w")
        env = {"CARGO_TARGET_DIR": os.path.join(tmp, "target"), "CARGO_TERM_COLOR": "never"}
        notes = []
        if arm == "rust":
            shutil.copytree(ws, woven, ignore=shutil.ignore_patterns("target", ".weave", ".git"))
            res["weaves"] = None
            res["rs_files_in_bare"] = None
            ok = True
        else:
            rs = []
            for dp, dn, fn in os.walk(ws):
                dn[:] = [d for d in dn if d not in ("target", ".weave", ".git", "extra-tests")]
                rs += [os.path.relpath(os.path.join(dp, f), ws) for f in fn if f.endswith(".rs")]
            res["rs_files_in_bare"] = rs
            rc, out = run([BARE, "weave", ws, "--out", woven, "--crate", ws], env=env)
            res["weaves"] = rc == 0
            res["weave_summary"] = (out.strip().split("\n") or [""])[-1][:300] if rc == 0 else out.strip()[-600:]
            ok = os.path.isdir(woven) and os.path.exists(os.path.join(woven, "Cargo.toml"))
        if ok:
            rc, out = run(["cargo", "build", "--lib", "--offline"], cwd=woven, env=env)
            res["compiles"] = rc == 0
            if rc != 0:
                res["compile_error"] = out.strip()[-600:]
        else:
            res["compiles"] = False
        passed = failed = 0
        res["hidden_compiled"] = False
        if ok and res["compiles"]:
            os.makedirs(os.path.join(woven, "tests"), exist_ok=True)
            open(os.path.join(woven, "tests", "hidden.rs"), "w").write(hidden)
            rc, out = run(["cargo", "test", "--offline", "--no-fail-fast", "--test", "hidden"], cwd=woven, env=env)
            m = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
            if m:
                res["hidden_compiled"] = True
                passed, failed = int(m[-1][0]), int(m[-1][1])
            else:
                res["hidden_error"] = out.strip()[-600:]
            # the crate's own tests/lib.rs, if the crate has one
            if os.path.exists(os.path.join(woven, "tests", "lib.rs")):
                rc, out = run(["cargo", "test", "--offline", "--test", "lib"], cwd=woven, env=env)
                res["existing_tests_ok"] = rc == 0
        res["tests_passed"], res["tests_failed"], res["tests_total"] = passed, failed, total
        res["pass"] = bool(total) and passed == total and failed == 0
        # edits
        if arm == "rust":
            res["json_entries_changed"] = None
            res["json_leaf_edits"] = None
            res["json_files_changed"] = None
            res["lines_changed"] = lines_changed(base, ws, ".rs")
        else:
            sdir = os.path.join(tmp, "basebare")
            sh('. "%s/common.sh"; strip_into "%s" "%s"' % (EXP, base, sdir))
            e, l, f = json_changes(sdir, ws)
            res["json_entries_changed"], res["json_leaf_edits"], res["json_files_changed"] = e, l, f
            res["lines_changed"] = lines_changed(sdir, ws, ".brs")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print(json.dumps(res, ensure_ascii=False))


main()
