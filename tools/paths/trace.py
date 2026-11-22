"""Runs an example's tests with its module instrumented, and checks the paths each test says it covers.

    python3 tools/paths/trace.py examples/<example> <module name>

It works on a copy of the example in `$YTRACE_WORK` (by default `y-trace` in the system's temporary folder), and keeps
its cargo target folder there between runs. It exits with status 1 if anything below is reported.

It reports, for each test, the paths it says it covers but did not take; the paths the tests took that the enumerator
does not know (a fault in the enumerator or in the lowering); and the paths no test took.

It reads the tests and the paths they claim from the test file's tree (`ytree.py`, from `tools/y-parse`); the paths of
the module come from the enumerator, `yp.py`, which reads the Y as text.
"""
import os, re, shutil, subprocess, sys, tempfile
sys.path.insert(0, os.path.dirname(__file__))
import yp, ytree

HERE = os.path.dirname(os.path.abspath(__file__))
SCRATCH = os.environ.get("YTRACE_WORK") or os.path.join(tempfile.gettempdir(), "y-trace")


def split_rust(e, sep):
    parts, depth, last, q, k = [], 0, 0, False, 0
    while k < len(e):
        c = e[k]
        if c == '"':
            q = not q
        elif not q and c in "([{":
            depth += 1
        elif not q and c in ")]}":
            depth -= 1
        elif not q and depth == 0 and e.startswith(sep, k):
            parts.append(e[last:k].strip())
            k += len(sep)
            last = k
            continue
        k += 1
    parts.append(e[last:].strip())
    return parts


def rust_comparison(e):
    depth, q = 0, False
    for k, c in enumerate(e):
        if c == '"':
            q = not q
        elif not q and c in "([{":
            depth += 1
        elif not q and c in ")]}":
            depth -= 1
        elif not q and depth == 0:
            for op in (" <= ", " >= ", " == ", " != ", " < ", " > "):
                if e.startswith(op, k):
                    return e[:k].strip(), op.strip(), e[k + len(op):].strip()
    return None


def strip_rust_parens(e):
    e = e.strip()
    while e.startswith("(") and e.endswith(")"):
        depth = 0
        for k, c in enumerate(e):
            depth += c == "("
            depth -= c == ")"
            if depth == 0 and k < len(e) - 1:
                return e
        e = e[1:-1].strip()
    return e


def instrument_pred(e):
    """A Rust boolean expression, rewritten to record what its operands show."""
    e = strip_rust_parens(e)
    for sep, is_and in ((" || ", False), (" && ", True)):
        ops = split_rust(e, sep)
        if len(ops) > 1:
            inner = ", ".join(f"&mut || {instrument_operand(op)}" for op in ops)
            return f"crate::ytrace::chain({'true' if is_and else 'false'}, &mut [{inner}])"
    return instrument_operand(e)


def instrument_operand(e):
    e = strip_rust_parens(e)
    c = rust_comparison(e)
    if c and not e.startswith("!"):
        a, op, b = c
        return ("{ use crate::ytrace::{ViaOrder, ViaEquality}; "
                f"let probe = crate::ytrace::Probe(&({a}), &({b})); "
                f"crate::ytrace::compared((&probe).order(), (&probe).equal(), \"{op}\") }}")
    if (" && " in e or " || " in e) and not e.startswith("!"):
        return instrument_pred(e)
    return f"({e})"


def failing_sites(lines):
    """For each line with a `?` at the end of a statement: the label of its failure path, numbered as the Y source
    numbers its `or fail` sites, within each function."""
    fn_re = re.compile(r"^\s*(?:pub(?:\([\w ]+\))? )?(?:async )?fn (\w+)\b")
    sites, current = [], None
    for i, l in enumerate(lines):
        if fn_re.match(l):
            current = fn_re.match(l).group(1)
        code = l.split(" // ")[0].rstrip()
        if current and (code.endswith("?;") or (" => " in code and code.endswith("?,"))):
            expr = code.split(" => ", 1)[1] if " => " in code else code
            m = re.match(r"\s*let (?:mut )?(\w+) = ", expr) or re.match(r"\s*(\w+) = ", expr)
            name = m.group(1) if m else re.search(r"(\w+)\(", expr.strip()).group(1)
            sites.append((i, current, name))
    labels, totals, seen = {}, {}, {}
    for i, fn, name in sites:
        totals[(fn, name)] = totals.get((fn, name), 0) + 1
    for i, fn, name in sites:
        seen[(fn, name)] = seen.get((fn, name), 0) + 1
        labels[i] = name if totals[(fn, name)] == 1 else f"{name}'{seen[(fn, name)]}"
    return labels


def wrap_failing(l, label):
    code, _, comment = l.partition(" // ")
    code = code.rstrip()
    m = re.match(r"(\s*(?:let (?:mut )?\w+ = |\w+ = |.* => )?)(.*)\?([;,])$", code)
    return f"{m.group(1)}crate::ytrace::attempt({m.group(2)}, \"{label}\")?{m.group(3)}" + (f" // {comment}" if comment else "")


def instrument(src):
    lines = src.split("\n")
    failing = failing_sites(lines)
    out = []
    loop_id = 0
    pending_ends = []  # (closing line index, text to insert after it)
    closers = {}
    arm_closers = set()
    # an alternative of an `orelse` is taken when it commits: its step goes before each `Ok(Step::Done(…))` in its closure
    commits = {}
    i = 0
    fn_re = re.compile(r"^(\s*)(?:pub(?:\([\w ]+\))? )?(?:async )?fn (\w+)\b.*\{$")
    for i, l in enumerate(lines):
        m = fn_re.match(l)
        if m and "#[test]" not in lines[i - 1]:
            out.append(l)
            out.append(f"{m.group(1)}    let _path = crate::ytrace::enter(\"{m.group(2)}\");")
            continue
        if i in commits:
            out.append(" " * (len(l) - len(l.lstrip())) + f"crate::ytrace::step_plain(\"{commits[i]}\");")
        if i in failing:
            l = wrap_failing(l, failing[i])
        marker = re.search(r"// y: #([\w.']+)(?:, else #(\w+))?", l)
        code = l.split(" // y:")[0] if marker else l
        ind = len(l) - len(l.lstrip())
        if marker:
            name, otherwise = marker.group(1), marker.group(2)
            t = code.strip()
            mm = re.match(r"(\} else if |(?:let [\w: &'\[\]]+ = )?if )(.*) \{$", t)
            loop = re.match(r"(for .* in .*|while (.*)) \{$", t)
            if re.match(r"let .* else \{$", t) and otherwise:
                code = code + f" crate::ytrace::step_plain(\"{name}\");"
                closers.setdefault(find_close(lines, i), []).append(" " * ind + f"crate::ytrace::step_plain(\"{otherwise}\");")
            elif t.startswith(("if let ", "} else if let ")):
                code = code + f" crate::ytrace::step(\"{name}\");"
            elif mm:
                pred = mm.group(2)
                code = " " * ind + f"{mm.group(1)}crate::ytrace::cond({{ crate::ytrace::begin(); {instrument_pred(pred)} }}) {{ crate::ytrace::step(\"{name}\");"
                if otherwise:
                    # a guard: the code after the `if`, which runs when it is false, is its `else`
                    closers.setdefault(find_close(lines, i), []).append(" " * ind + f"crate::ytrace::step(\"{otherwise}\");")
            elif t == "} else {":
                code = code + f" crate::ytrace::step(\"{name}\");"
            elif re.match(r"let .* else \{$", t):
                code = code + f" crate::ytrace::step_plain(\"{name}\");"
            elif loop:
                loop_id += 1
                if t.startswith("while "):
                    pred = re.match(r"while (.*) \{$", t).group(1)
                    code = " " * ind + f"while crate::ytrace::cond({{ crate::ytrace::begin(); {instrument_pred(pred)} }}) {{"
                code = code + f" crate::ytrace::pass({loop_id}, \"{name}\");"
                close = find_close(lines, i)
                other = f"Some(\"{otherwise}\")" if otherwise else "None"
                closers.setdefault(close, []).append(" " * ind + f"crate::ytrace::end_loop({loop_id}, {other});")
            elif "=>" in t:
                pat, rest = code.split("=>", 1)
                rest = rest.strip()
                kind = "step" if ("Ordering::" in pat) else "step_plain"
                if rest == "{":
                    code = f"{pat}=> {{ crate::ytrace::{kind}(\"{name}\");"
                elif rest.endswith("["):
                    code = f"{pat}=> {{ crate::ytrace::{kind}(\"{name}\"); {rest}"
                    arm_closers.add(find_bracket_close(lines, i))
                else:
                    body = rest.rstrip(",")
                    code = f"{pat}=> {{ crate::ytrace::{kind}(\"{name}\"); {body} }},"
            elif re.match(r"let (mut )?\w+ = \|.*\{$", t):
                for j in range(i + 1, find_close(lines, i)):
                    if lines[j].strip().startswith(("Ok(Step::Done(", "return Ok(Step::Done(")):
                        commits[j] = name
                code = l
            else:
                code = l  # a marker on a line the tracer does not need
            l = code
        mc = re.match(r"^(\s*)match (.+)\.cmp\((.+)\) \{$", l)
        if mc:
            l = f"{mc.group(1)}match crate::ytrace::order({mc.group(2)}.cmp({mc.group(3)})) {{"
        if i in arm_closers:
            l = l.replace("],", "]},", 1)
        out.append(l)
        for extra in closers.get(i, []):
            out.append(extra)
    return "\n".join(out)


def find_close(lines, i):
    depth = 0
    for k in range(i, len(lines)):
        code = lines[k].split("//")[0]
        code = re.sub(r'"(\\.|[^"\\])*"', '""', code)
        depth += code.count("{") - code.count("}")
        if depth == 0:
            return k
    raise SystemExit(f"no close for line {i}")


def find_bracket_close(lines, i):
    depth = 0
    for k in range(i, len(lines)):
        code = re.sub(r'"(\\.|[^"\\])*"', '""', lines[k].split(" // ")[0])
        depth += code.count("[") - code.count("]")
        if depth == 0:
            return k
    raise SystemExit(f"no bracket close for line {i}")


def parse_tokens(tokens, k=0, stop=None):
    """Items from trace tokens: a step name, or (loop id, [(pass name, items)], closed)."""
    items = []
    while k < len(tokens):
        t = tokens[k]
        if t.startswith("{"):
            lid, name = t[1:].split(":", 1)
            passes, k = [], k + 1
            body, k = parse_tokens(tokens, k, lid)
            passes.append((name, body))
            closed = False
            while k < len(tokens):
                if tokens[k] == "}" + lid:
                    closed, k = True, k + 1
                    break
                if tokens[k].startswith("|" + lid + ":"):
                    name = tokens[k].split(":", 1)[1]
                    body, k = parse_tokens(tokens, k + 1, lid)
                    passes.append((name, body))
                    continue
                break
            items.append((lid, passes, closed))
            continue
        if stop is not None and (t == "}" + stop or t.startswith("|" + stop + ":")):
            return items, k
        items.append(t)
        k += 1
    return items, k


def expand(items):
    """The paths a call's items stand for: a loop is how many times it ran, then the path of one of its passes, and
    the steps after the loop only when it ended."""
    paths = {()}
    for n, item in enumerate(items):
        if isinstance(item, str):
            paths = {p + (item,) for p in paths}
            continue
        lid, passes, closed = item
        many = len(passes) > 1
        options = set()
        chosen = passes if closed else passes[-1:]
        for name, body in chosen:
            base, *parts = name.split("'", 1)
            label = base + ("'many" if many else "") + ("'" + parts[0] if parts else "")
            for p in expand(body):
                options.add((label,) + p)
        paths = {p + o for p in paths for o in options}
        if not closed:
            return paths
    return paths


def parse_step(s):
    base, *parts = s.split("'")
    tokens = set()
    for p in parts:
        m = re.fullmatch(r"(\d*)(lt|eq|gt)?|many", p)
        if p == "many":
            tokens.add(("many", None))
        elif m:
            tokens.add((int(m.group(1)) if m.group(1) else 0, m.group(2)))
    return base, tokens


def step_matches(static, raw):
    sb, st = parse_step(static)
    rb, rt = parse_step(raw)
    if sb != rb:
        return None
    for k, o in st:
        if k == "many":
            if ("many", None) not in rt:
                return None
        elif not any(k == rk and (o is None or o == ro) for rk, ro in rt):
            return None
    if ("many", None) in rt and ("many", None) not in st:
        return None
    # a raw step that stopped early must match a static step that names where
    stops = {rk for rk, ro in rt if isinstance(rk, int) and rk > 0}
    named = {k for k, o in st if isinstance(k, int) and k > 0}
    if named and not named <= stops:
        return None
    return len(st)


def match_path(statics, raw_steps):
    best, score = [], -1
    for pid in statics:
        parts = pid.split(" > ")[1:]
        if len(parts) != len(raw_steps):
            continue
        total = 0
        for s, r in zip(parts, raw_steps):
            m = step_matches(s, r)
            if m is None:
                break
            total += m
        else:
            if total > score:
                best, score = [pid], total
            elif total == score:
                best.append(pid)
    return best


def claims(test_file):
    """Each test of a Y test file, with the paths it says it covers (none, or one), from the file's tree."""
    tree = ytree.tree(test_file)
    return {t["name"]: [t["covers"]["text"]] if t["covers"] else [] for t in tree["decls"] if t["kind"] == "test"}


def main():
    example, module = sys.argv[1], sys.argv[2]
    example = os.path.abspath(example)
    work = os.path.join(SCRATCH, os.path.basename(example))
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    # the package's native implementations (Y § 3.18) are compiled with its Rust
    for item in ("Cargo.toml", "src", "native"):
        src = os.path.join(example, item)
        if item == "native" and not os.path.isdir(src):
            continue
        (shutil.copytree if os.path.isdir(src) else shutil.copy)(src, os.path.join(work, item))
    toml = open(os.path.join(work, "Cargo.toml")).read().replace('path = "../', f'path = "{os.path.dirname(example)}/')
    open(os.path.join(work, "Cargo.toml"), "w").write(toml)
    shutil.copy(os.path.join(HERE, "ytrace.rs"), os.path.join(work, "src", "ytrace.rs"))
    lib = os.path.join(work, "src", "lib.rs")
    open(lib, "a").write("\n#[doc(hidden)]\npub mod ytrace;\n")
    rs = os.path.join(work, "src", f"{module}.rs")
    source = open(rs).read()
    open(rs, "w").write(instrument(source))
    out = os.path.join(work, "trace.txt")
    env = dict(os.environ, YTRACE_OUT=out, CARGO_TARGET_DIR=os.path.join(SCRATCH, "target"))
    r = subprocess.run(["cargo", "test", "--lib", "-q"], cwd=work, env=env, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stdout[-3000:], r.stderr[-6000:])
        raise SystemExit("tests failed")

    y = os.path.join(example, "y", "src", f"{module}.y")
    m = yp.Module(y)
    statics, problems = yp.all_paths(m)
    by_function = {}
    for pid in statics:
        by_function.setdefault(pid.split(" > ")[0], []).append(pid)
    taken = {}
    unknown = set()
    for line in open(out):
        test, fn, steps = line.rstrip("\n").split("\t")
        test = test.split("::")[-1]
        tokens = re.findall(r"\|\d+:[^|{}]*|[{]\d+:[^|{}]*|[}]\d+|[^|{}]+", steps)
        items, _ = parse_tokens([t for t in tokens if t])
        if fn not in by_function:
            continue
        for raw in expand(items):
            raw = list(raw)
            found = match_path(by_function[fn], raw)
            if len(found) == 1:
                taken.setdefault(test, set()).add(found[0])
            else:
                unknown.add((fn + " > " + " > ".join(raw), tuple(found)))
    import json
    json.dump({t: sorted(v) for t, v in taken.items()}, open(os.path.join(work, "taken.json"), "w"), indent=1)
    claimed = claims(os.path.join(example, "y", "src", f"{module}.test.y"))
    bad = 0
    for test, cs in claimed.items():
        missing = [c for c in cs if c not in taken.get(test, set())]
        if missing:
            bad += 1
            print(f"{test}: says it covers, but did not take: {missing}")
    for raw, found in sorted(unknown):
        print(f"taken, but {'ambiguous' if found else 'not a known path'}: {raw} {list(found)}")
    required = {p for p, k in statics.items() if k not in yp.EXEMPT}
    all_taken = set().union(*taken.values()) if taken else set()
    never = sorted(required - all_taken)
    all_claimed = set().union(*map(set, claimed.values())) if claimed else set()
    print(f"{len(required)} paths to test; {len(required & all_taken)} taken by some test; {len(required & all_claimed)} claimed")
    for p in never:
        print("  never taken:", p)
    for p in sorted(all_claimed - set(statics)):
        print("  claimed, but not a path:", p)
    for p in sorted(required - all_claimed - set(never)):
        print("  taken, but claimed by no test:", p)
    if problems:
        print("problems:", problems)
    if bad or unknown or never or problems or (all_claimed - set(statics)) or (required - all_claimed - set(never)):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
