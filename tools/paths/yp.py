"""The execution paths through each function of a Y module, under the path rules.

A path is the sequence of named outcomes one call passes through, written as breadcrumbs: `own_pieces > placed'many >
theirs`.

- `if a = (p):` / `else b:`: the outcome is `a` or `b`, each split by the predicate's sub-outcomes.
- `conditional l:` / `condition l.x = (p):` / `else l.y:`: the outcome is the condition taken, split by the sub-outcomes
  of its own predicate. A condition found false on the way has no name of its own, and is not split, but what its
  falseness shows is known on the rest of the path.
- `compare l = a with b:` / `condition l.x = less`, `equal`, `greater`, or two of them joined by `or`: a `conditional`
  over the order of `a` and `b`. A condition that covers two orders splits by them.
- A comparison of ordered values (numbers, durations, instants) splits into `'lt`, `'eq` and `'gt` on the branch it falls
  on. A part that is the only one possible on that branch is not written.
- `and` and `or` split by the operand at which evaluation stopped: `b'1`, `b'2`. On the branch where every operand was
  evaluated, the operands' own parts are written by position: `a'2eq`.
- `for x in e:` and `while x = (p):` end in `else y` or `else y:`, taken when the loop never runs, unless the toolchain
  proves that it runs. When it runs, the path names how often, `x` (once) or `x'many` (more than once), followed by the
  path of one of its passes: the pass that leaves the loop, by `return` or `fail`, or else any pass, and then the steps
  after the loop.
- A call with `or fail` adds a path that ends there: `name'fails`, named by the name it binds, or by the call.
- `fail not_reachable` and `fail not_implemented` end a path that needs no test.

What a path shows is carried along it as facts: the interval of a number, or of a list's count. They come from the
branches taken, the conditions found false, `where` preconditions, the `ensures` of a called function, `assert`s and
constants. A sub-outcome the facts rule out is no path: that is what "proven" means here.
"""
import re, sys

ORDERED = {"int32", "uint32", "uint16", "int64", "uint64", "duration", "instant", "count", "int"}
UNSIGNED = {"uint32", "uint16", "uint64", "duration", "count"}
EXEMPT = ("not_reachable", "not_implemented")


# ---------------------------------------------------------------------------------------------------------------------
# Text at the top level of an expression

def scan(e):
    """Yields (index, char, depth) for the characters outside strings."""
    depth, q = 0, False
    for k, c in enumerate(e):
        if c == '"':
            q = not q
            continue
        if q:
            continue
        if c in "([":
            depth += 1
        elif c in ")]":
            depth -= 1
        yield k, c, depth


def strip_parens(e):
    e = e.strip()
    while e.startswith("("):
        close = next((k for k, c, d in scan(e) if c == ")" and d == 0), -1)
        if close != len(e) - 1:
            break
        e = e[1:-1].strip()
    return e


def split_top(e, sep):
    parts, last = [], 0
    for k, c, d in scan(e):
        if d == 0 and k >= last and e.startswith(sep, k):
            parts.append(e[last:k].strip())
            last = k + len(sep)
    parts.append(e[last:].strip())
    return parts


def split_at(e, op):
    for k, c, d in scan(e):
        if d == 0 and e.startswith(op, k):
            return e[:k].strip(), e[k + len(op):].strip()
    return None


def top_comparison(e):
    for k, c, d in scan(e):
        if d == 0:
            for op in (" <= ", " >= ", " == ", " != ", " < ", " > "):
                if e.startswith(op, k):
                    return e[:k].strip(), op.strip(), e[k + len(op):].strip()
    return None


def has_top_arith(e):
    return any(d == 0 and 0 < k < len(e) - 1 and e[k - 1] == " " and c in "+-*/%" and e[k + 1] == " " for k, c, d in scan(e))


def split_commas(e):
    parts, last = [], 0
    for k, c, d in scan(e):
        if d == 0 and c == ",":
            parts.append(e[last:k])
            last = k + 1
    parts.append(e[last:])
    return [p.strip() for p in parts if p.strip()]


def norm(e):
    """An expression as facts are kept on it: a count converted to another whole number is the same count."""
    e = strip_parens(e)
    m = re.fullmatch(r"(.+\.count\(\))\.to_(?:u?int32)\(\)", e)
    return m.group(1) if m else e


def root(e):
    m = re.match(r"\(*\s*(\w+)", e)
    return m.group(1) if m else ""


# ---------------------------------------------------------------------------------------------------------------------
# Intervals, with None for no bound

def meet(a, b):
    lo = a[0] if b[0] is None else b[0] if a[0] is None else max(a[0], b[0])
    hi = a[1] if b[1] is None else b[1] if a[1] is None else min(a[1], b[1])
    return lo, hi


def empty(i):
    return i[0] is not None and i[1] is not None and i[0] > i[1]


def order_possible(a, b):
    """The orders a value in interval `a` may stand in to one in `b`."""
    out = set()
    if not (a[0] is not None and b[1] is not None and a[0] >= b[1]):
        out.add("lt")
    if not empty(meet(a, b)):
        out.add("eq")
    if not (a[1] is not None and b[0] is not None and a[1] <= b[0]):
        out.add("gt")
    return out


# ---------------------------------------------------------------------------------------------------------------------
# A module's declarations

class Module:
    def __init__(self, path, others=()):
        self.path = path
        self.lines = open(path).read().split("\n")
        self.fields, self.constants, self.values, self.results, self.params = {}, {}, {}, {}, {}
        self.contracts = {}
        # a refined type's constraints on its values' counts: {type: [(op, n)]}
        self.refinements = {}
        self.unknown = []
        for p in (path, *others):
            self.collect(open(p).read().split("\n"))
        self.groups = {k.split(".")[0] for k in self.constants}

    def collect(self, raw):
        lines, k = [], 0
        while k < len(raw):
            l = raw[k]
            k += 1
            while l.count("[") > l.count("]") and k < len(raw):
                l = l.rstrip() + " " + raw[k].strip()
                k += 1
            lines.append(l)
        group, kind, doc, in_doc, current = None, None, [], False, None
        refined = None
        for l in lines:
            t = l.strip()
            if re.fullmatch(r"-{3,}", t):
                in_doc = not in_doc
                if in_doc:
                    doc = []
                continue
            if in_doc:
                doc.append(t)
                continue
            m = re.match(r"(?:public )?(constants|structure|type) (\w+)", t)
            if m:
                kind, group = m.group(1), m.group(2)
                refined = group if re.match(r"(?:public )?type \w+ is ", t) else None
                continue
            if refined:
                for op, n in re.findall(r"value\.count\(\) (>=|>|<=|<|==) (\d+)", t):
                    self.refinements.setdefault(refined, []).append((op, int(n)))
            m = re.match(r"(\w+) is ([\w<>=, ]+?)(?: = (.*))?$", t)
            if m and group and l.startswith(" "):
                name, ty, value = m.groups()
                if kind == "structure":
                    self.fields.setdefault(name, ty)
                elif kind == "constants":
                    self.constants[f"{group}.{name}"] = ty
                    if value is not None:
                        self.values[f"{group}.{name}"] = value
                continue
            m = re.match(r"(\w+) = (.*)$", t)
            if m and kind == "constants" and group and l.startswith(" "):
                self.constants[f"{group}.{m.group(1)}"] = group
                continue
            m = re.match(r"function (\w+)\(([^)]*)\)(?: returns (\w+))?", t)
            if m:
                group = None
                name, params, result = m.groups()
                current = name
                types = {}
                for d in doc:
                    dm = re.match(r"(\w+): .*, as ([\w<>=, ]+)\.$", d)
                    if dm:
                        types[dm.group(1)] = dm.group(2)
                names = [p.strip().split()[-1] for p in params.split(",") if p.strip()]
                self.params[name] = {p: types.get(p) for p in names}
                self.results[name] = (result, types.get(result))
                self.contracts[name] = []
                continue
            m = re.match(r"(where|ensures) (\w+) = (.*?):?$", t)
            if m and current and l.startswith(" "):
                self.contracts[current].append((m.group(1), m.group(3)))
                continue
            if t and not l.startswith(" ") and not t.startswith("--"):
                group = None
                current = None

    def value(self, e):
        """The constant value of `e`, if it has one."""
        e = strip_parens(e)
        if re.fullmatch(r"-?\d+", e):
            return int(e)
        if e in self.values:
            return self.value(self.values[e])
        for op in (" + ", " - ", " * ", " / "):
            parts = split_at(e, op)
            if parts:
                a, b = self.value(parts[0]), self.value(parts[1])
                if a is None or b is None:
                    return None
                return {"+": a + b, "-": a - b, "*": a * b, "/": a // b if b else None}[op.strip()]
        return None

    def length(self, e):
        """The constant length of a list, if it has one."""
        e = strip_parens(e)
        if e in self.values:
            e = self.values[e]
        if e.startswith("[") and e.endswith("]"):
            inner = e[1:-1].strip()
            return len(split_commas(inner)) if inner else 0
        return None

    def type_of(self, e, env):
        e = strip_parens(e)
        if re.fullmatch(r"-?\d+", e):
            return "int"
        if re.fullmatch(r"\d+(ms|s)", e):
            return "duration"
        if e.startswith('"'):
            return "string"
        if e in ("true", "false"):
            return "boolean"
        if e.startswith("not "):
            return "boolean"
        if e.startswith("-"):
            return self.type_of(e[1:], env)
        if has_top_arith(e):
            for op in (" + ", " - ", " * ", " / ", " % "):
                parts = split_at(e, op)
                if parts:
                    a, b = self.type_of(parts[0], env), self.type_of(parts[1], env)
                    if a == "instant" and b == "instant":
                        return "duration"
                    return a if a not in (None, "int") else b if b else a
        if split_top(e, " and ")[1:] or split_top(e, " or ")[1:] or top_comparison(e):
            return "boolean"
        if e.endswith(".count()"):
            return "count"
        if e == "clock.now()":
            return "instant"
        for method, ty in ((".to_int32()", "int32"), (".to_uint32()", "uint32"), (".whole_seconds()", "uint32"),
                           (".is_empty()", "boolean")):
            if e.endswith(method):
                return ty
        if re.fullmatch(r".+\.contains\(.*\)", e):
            return "boolean"
        m = re.fullmatch(r"(.+)\.divided_rounding_up\(.*\)", e) or re.fullmatch(r"(.+)\.(?:min|max)\(.*\)", e)
        if m:
            return self.type_of(m.group(1), env)
        m = re.fullmatch(r"(.+)\.(?:largest|smallest|last)\(\)", e) or re.fullmatch(r"(.+)\.at_number\(.*\)", e) \
            or re.fullmatch(r"(.+)\[[^\]]*\]", e)
        if m:
            inner = self.type_of(m.group(1), env) or ""
            mm = re.fullmatch(r"list<element_type=(.+)>", inner)
            return mm.group(1) if mm else None
        m = re.fullmatch(r"(\w+)\(.*\)", e)
        if m and m.group(1) in self.results:
            return self.results[m.group(1)][1]
        if e in env:
            return env[e]
        if e in self.constants:
            return self.constants[e]
        m = re.fullmatch(r"[\w.\[\]0-9]+\.(\w+)", e)
        if m and m.group(1) in self.fields:
            return self.fields[m.group(1)]
        return None


# ---------------------------------------------------------------------------------------------------------------------
# Facts along a path: intervals of numbers, booleans (0 or 1) and counts

class Facts:
    def __init__(self, module, env, known=None, same=None, differs=None):
        self.module, self.env = module, env
        self.known = dict(known or {})
        # values with no order: what an expression is known to be, or known not to be
        self.same = dict(same or {})
        self.differs = {k: set(v) for k, v in (differs or {}).items()}

    def copy(self):
        return Facts(self.module, self.env, self.known, self.same, self.differs)

    def is_constant(self, e):
        return e.startswith(("optional.", "result.")) or e.split(".")[0] in self.module.groups and e not in self.module.values

    def equality(self, left, right, truth):
        """Adds that `left` is, or is not, `right`, for values with no order: False when it cannot be."""
        if self.is_constant(left) and not self.is_constant(right):
            left, right = right, left
        if not self.is_constant(right):
            return True
        if truth:
            if self.same.get(left, right) != right or right in self.differs.get(left, set()):
                return False
            self.same[left] = right
            return True
        if self.same.get(left) == right:
            return False
        self.differs.setdefault(left, set()).add(right)
        return True

    def interval(self, e):
        e = norm(e)
        v = self.module.value(e)
        if v is not None:
            return v, v
        if e in ("true", "false"):
            return (1, 1) if e == "true" else (0, 0)
        m = re.fullmatch(r"(.+)\.count\(\)", e)
        if m and self.module.length(m.group(1)) is not None:
            n = self.module.length(m.group(1))
            return n, n
        i = self.known.get(e, (None, None))
        m = re.fullmatch(r"(.+)\.(?:runes\(\)\.)?count\(\)", e)
        if m:
            # a refined type bounds its values' counts
            for op, n in self.module.refinements.get(self.module.type_of(m.group(1), self.env), []):
                i = meet(i, {">=": (n, None), ">": (n + 1, None), "<=": (None, n), "<": (None, n - 1), "==": (n, n)}[op])
            if m.group(1).endswith(".runes()"):
                i = meet(i, self.interval(m.group(1)[:-len(".runes()")] + ".count()"))
            # the standard library: splitting text gives at least one piece
            if re.fullmatch(r".+\.split\(.*\)", m.group(1)):
                i = meet(i, (1, None))
        ty = self.module.type_of(e, self.env)
        if ty in UNSIGNED:
            i = meet(i, (0, None))
        if ty == "boolean":
            i = meet(i, (0, 1))
        return i

    def add(self, e, i):
        e = norm(e)
        new = meet(self.interval(e), i)
        self.known[e] = new
        return not empty(new)

    def forget(self, name):
        """Forgets what is known of `name`, and of every expression that reads it."""
        reads = re.compile(rf"(?<![\w.']){re.escape(name)}\b")
        self.known = {k: v for k, v in self.known.items() if not reads.search(k)}
        self.same = {k: v for k, v in self.same.items() if not reads.search(k)}
        self.differs = {k: v for k, v in self.differs.items() if not reads.search(k)}

    def grow(self, name, by):
        """A list `name` gains `by` elements, or an unknown number when `by` is None."""
        i = self.known.get(f"{name}.count()")
        self.forget(name)
        if i is not None and by is not None:
            self.known[f"{name}.count()"] = (None if i[0] is None else i[0] + by, None if i[1] is None else i[1] + by)

    def bind(self, name, rhs):
        """`name` now stands for `rhs`: it takes on what is known of it, and what the function it calls ensures."""
        rhs = strip_parens(rhs)
        new, same = {}, None
        for k, v in self.known.items():
            if k == rhs:
                new[name] = v
            elif k.startswith(rhs + "."):
                new[name + k[len(rhs):]] = v
        v = self.module.value(rhs)
        if v is not None:
            new[name] = (v, v)
        if self.is_constant(rhs):
            same = rhs
        elif rhs in self.same:
            same = self.same[rhs]
        # arithmetic on what is known
        for op in (" + ", " - "):
            parts = split_at(rhs, op)
            if parts and self.module.value(parts[1]) is not None:
                i, c = self.interval(parts[0]), self.module.value(parts[1])
                c = c if op == " + " else -c
                new[name] = (None if i[0] is None else i[0] + c, None if i[1] is None else i[1] + c)
        # a negated value, and what the function it calls ensures of it
        if rhs.startswith("-") and self.module.value(rhs) is None:
            inner = self.copy()
            inner.bind("negated'value", rhs[1:])
            i = inner.interval("negated'value")
            new[name] = (None if i[1] is None else -i[1], None if i[0] is None else -i[0])
        # what the standard library ensures of the lists it makes
        if rhs in ("list.new()", "[]"):
            new[f"{name}.count()"] = (0, 0)
        m = re.fullmatch(r"random\.shuffled\((.+)\)", rhs)
        if m:
            new[f"{name}.count()"] = self.interval(f"{m.group(1)}.count()")
        m = re.fullmatch(r"random\.chosen\(from: .+, count: (.+)\)", rhs)
        if m:
            new[f"{name}.count()"] = self.interval(m.group(1))
        m = re.fullmatch(r"list\.repeated\(.+, times: (.+)\)", rhs)
        if m:
            new[f"{name}.count()"] = self.interval(m.group(1))
        if rhs in ("true", "false"):
            new[name] = (1, 1) if rhs == "true" else (0, 0)
        self.forget(name)
        self.known.update(new)
        if same is not None:
            self.same[name] = same
        m = re.fullmatch(r"(\w+)\((.*)\)", rhs)
        if m:
            result = self.module.results.get(m.group(1), (None, None))[0]
            for kind, pred in self.module.contracts.get(m.group(1), []):
                if kind == "ensures" and result:
                    alts = assume(self.module, self.env, re.sub(rf"\b{result}\b", name, pred), True, self)
                    if alts:
                        merged = join(alts)
                        self.known, self.same, self.differs = merged.known, merged.same, merged.differs

    def possible_orders(self, left, right):
        lt, rt = self.module.type_of(left, self.env), self.module.type_of(right, self.env)
        known = [t for t in (lt, rt) if t is not None]
        if not known:
            if not any(x.startswith(("optional.", "result.")) or x.split(".")[0] in self.module.groups for x in (left, right)):
                self.module.unknown.append(f"{left} ? {right}")
            return None
        if not any(t in ORDERED for t in known):
            return None
        return order_possible(self.interval(left), self.interval(right))

    def constrain(self, left, right, o):
        """Adds what order `o` of `left` to `right` shows: False when it cannot be."""
        a, b = self.interval(left), self.interval(right)
        if o not in order_possible(a, b):
            return False
        if o == "eq":
            return self.add(left, b) and self.add(right, a)
        if o == "lt":
            return self.add(left, (None, None if b[1] is None else b[1] - 1)) and \
                self.add(right, (None if a[0] is None else a[0] + 1, None))
        return self.add(left, (None if b[0] is None else b[0] + 1, None)) and \
            self.add(right, (None, None if a[1] is None else a[1] - 1))


def join(alternatives):
    """What holds in every one of several sets of facts."""
    first = alternatives[0].copy()
    for other in alternatives[1:]:
        for k in list(first.known):
            if k not in other.known:
                del first.known[k]
            else:
                a, b = first.known[k], other.known[k]
                first.known[k] = (None if a[0] is None or b[0] is None else min(a[0], b[0]),
                                  None if a[1] is None or b[1] is None else max(a[1], b[1]))
        first.same = {k: v for k, v in first.same.items() if other.same.get(k) == v}
        first.differs = {k: v & other.differs.get(k, set()) for k, v in first.differs.items() if v & other.differs.get(k, set())}
    return first


ORDERS = {"<": ({"lt"}, {"eq", "gt"}), "<=": ({"lt", "eq"}, {"gt"}), ">": ({"gt"}, {"lt", "eq"}),
          ">=": ({"gt", "eq"}, {"lt"}), "==": ({"eq"}, {"lt", "gt"}), "!=": ({"lt", "gt"}, {"eq"})}


def outcomes(module, env, e, truth, facts):
    """The sub-outcomes of predicate `e` being `truth`: [(suffix, facts)], each with what it shows; [] when it cannot be."""
    e = strip_parens(e)
    if e.startswith("not "):
        return outcomes(module, env, e[4:], not truth, facts)
    for word, stops_on in ((" or ", True), (" and ", False)):
        ops = split_top(e, word)
        if len(ops) > 1:
            if truth == stops_on:
                # evaluation stopped at operand k; the ones before it went the other way
                out, seen = [], set()
                for k, op in enumerate(ops, 1):
                    befores = [facts.copy()]
                    for prior in ops[:k - 1]:
                        befores = [f2 for f in befores for _, f2 in outcomes(module, env, prior, not truth, f)]
                    for f in befores:
                        for s, f2 in outcomes(module, env, op, truth, f):
                            suffix = f"'{k}" + s.lstrip("'")
                            if suffix not in seen:
                                seen.add(suffix)
                                out.append((suffix, f2))
                return out
            states = [("", facts.copy())]
            for k, op in enumerate(ops, 1):
                states = [(s + ((f"'{k}" + s2.lstrip("'")) if s2 else ""), f2) for s, f in states
                          for s2, f2 in outcomes(module, env, op, truth, f)]
            return states
    c = top_comparison(e)
    if c:
        left, op, right = c
        possible = facts.possible_orders(left, right)
        if possible is None:
            if op in ("==", "!="):
                f = facts.copy()
                return [("", f)] if f.equality(left, right, truth == (op == "==")) else []
            return [("", facts.copy())]
        out = []
        for o in ("lt", "eq", "gt"):
            if o in ORDERS[op][0 if truth else 1] and o in possible:
                f = facts.copy()
                if f.constrain(left, right, o):
                    out.append((f"'{o}", f))
        if len(out) == 1:
            out = [("", out[0][1])]
        return out
    m = re.fullmatch(r"(.+)\.is_empty\(\)", e)
    if m:
        f = facts.copy()
        return [("", f)] if f.add(f"{m.group(1)}.count()", (0, 0) if truth else (1, None)) else []
    f = facts.copy()
    if module.type_of(e, env) == "boolean" or re.fullmatch(r"[\w.]+", e):
        if not f.add(e, (1, 1) if truth else (0, 0)):
            return []
    return [("", f)]


def assume(module, env, e, truth, facts):
    """The facts after `e` is found `truth`, one set for each way it can be."""
    return [f for _, f in outcomes(module, env, e, truth, facts)]


# ---------------------------------------------------------------------------------------------------------------------
# Blocks and statements

def blocks(lines):
    i = 0

    def parse(ind):
        nonlocal i
        out = []
        while i < len(lines):
            l = lines[i]
            d = len(l) - len(l.lstrip())
            if d < ind:
                break
            i += 1
            text = l.strip()
            while text.count("(") + text.count("[") > text.count(")") + text.count("]") and i < len(lines):
                text += " " + lines[i].strip()
                i += 1
            text = text.replace("( ", "(").replace("[ ", "[").replace(", )", ")").replace(", ]", "]")
            kids = []
            if text.endswith(":") and i < len(lines):
                nd = len(lines[i]) - len(lines[i].lstrip())
                if nd > d:
                    kids = parse(nd)
            out.append((text, kids))
        return out

    return parse(len(lines[0]) - len(lines[0].lstrip()) if lines else 0)


def mutated(items):
    """The names a block changes."""
    out = set()
    for t, kids in items:
        m = re.match(r"(\w+)[\w.\[\]]* := ", t)
        if m:
            out.add(m.group(1))
        if not t.startswith("mutable "):
            for mm in re.finditer(r"mutable (\w+)", t):
                out.add(mm.group(1))
        m = re.match(r"(\w+)[\w.]*\.(push|extend|set_number|insert_first|clear)\(", t)
        if m:
            out.add(m.group(1))
        m = re.match(r"(?:mutable )?(\w+) = ", t)
        if m:
            out.add(m.group(1))
        m = re.match(r"for (\w+) in", t)
        if m:
            out.add(m.group(1))
        out |= mutated(kids)
    return out


def grown_only(items, names):
    """The names among `names` that a block changes only by adding to them, with `push` or `extend`."""
    other = set()
    for t, kids in walk(items):
        for n in names:
            if re.match(rf"{n}\.(push|extend)\(", t):
                continue
            if re.match(rf"(mutable )?{n}\b[\w.\[\]]* :?= ", t) or re.search(rf"mutable {n}\b", t) or \
                    re.match(rf"{n}[\w.]*\.(set_number|insert_first|clear)\(", t):
                other.add(n)
    return {n for n in names if n not in other}


def widen(facts, names, grown):
    """Forgets what passes of a loop may have changed; a list that only grows keeps its least count."""
    for n in names:
        keep = facts.known.get(f"{n}.count()") if n in grown else None
        facts.forget(n)
        if keep is not None:
            facts.known[f"{n}.count()"] = (keep[0], None)


def walk(items):
    for t, kids in items:
        yield t, kids
        yield from walk(kids)


def failing_name(text):
    if not re.search(r" or fail(:)?$", text) or text.startswith(("region ", "within ", "if ", "while ")):
        return None
    m = re.match(r"(?:mutable )?(\w+) :?= ", text)
    if m:
        return m.group(1)
    m = re.match(r"([\w.]+)\(", text)
    return m.group(1).split(".")[-1] if m else "call"


class Function:
    def __init__(self, module, name, body):
        self.module, self.name = module, name
        self.env = {p: t for p, t in module.params.get(name, {}).items() if t}
        result, rtype = module.results.get(name, (None, None))
        if result and rtype:
            self.env[result] = rtype
        self.problems = []
        items = blocks(body)
        self.fails_total, self.fails_seen, self.order = {}, {}, {}
        for t, _ in walk(items):
            n = failing_name(t)
            if n:
                self.fails_total[n] = self.fails_total.get(n, 0) + 1
        starts = [Facts(module, self.env)]
        for kind, pred in module.contracts.get(name, []):
            if kind == "where":
                starts = [f for s in starts for f in assume(module, self.env, pred, True, s)][:1]
        found = self.seq(items, starts[0]) if starts else []
        seen, self.paths = set(), []
        for kind, steps, _ in found:
            key = (kind, tuple(steps))
            if key not in seen:
                seen.add(key)
                self.paths.append((kind, steps))

    def problem(self, text):
        if text not in self.problems:
            self.problems.append(text)

    def note_type(self, text):
        m = re.match(r"(?:mutable )?(\w+) = (.*)$", text)
        if m and not text.startswith(("if ", "while ", "conditional ", "compare ", "for ", "return ", "assert ")):
            name, rhs = m.groups()
            rhs = re.sub(r" or fail$", "", rhs)
            rhs = re.sub(r" forever because \".*\"", "", rhs)
            ty = self.module.type_of(rhs, self.env)
            if ty == "int":
                ty = "int32"
            if ty and name not in self.env:
                self.env[name] = ty
        m = re.match(r"for (\w+) in (.*):$", text)
        if m:
            src = m.group(2)
            ty = self.module.type_of(src, self.env) or ""
            mm = re.fullmatch(r"list<element_type=(.+)>", ty)
            if mm:
                self.env[m.group(1)] = mm.group(1)
            elif src.startswith("range("):
                ty = self.module.type_of(re.search(r"from: ([^,]+)", src).group(1), self.env) or "int32"
                self.env[m.group(1)] = "int32" if ty == "int" else ty

    def seq(self, items, facts):
        """[(kind, steps, facts)] for a block entered with `facts`."""
        states, done, k = [("next", [], facts)], [], 0
        while k < len(items):
            self.note_type(items[k][0])
            j = k + 1
            while j < len(items) and items[j][0].startswith("else "):
                j += 1
            nxt = []
            for kind, steps, f in states:
                for kind2, steps2, f2 in self.statement(items, k, f.copy()):
                    (nxt if kind2 == "next" else done).append((kind2, steps + steps2, f2))
            states, k = nxt, j
        return states + done

    def branch(self, name, pred, truth, body, facts):
        out = []
        for s, f in outcomes(self.module, self.env, pred, truth, facts):
            for kind, steps, f2 in self.seq(body, f):
                out.append((kind, [name + s] + steps, f2))
        return out

    def statement(self, items, k, facts):
        text, kids = items[k]
        elses = []
        j = k + 1
        while j < len(items) and items[j][0].startswith("else "):
            elses.append(items[j])
            j += 1
        module, env = self.module, self.env

        m = re.match(r"if (\w+) = (.*):$", text)
        if m:
            name, pred = m.groups()
            if len(elses) != 1 or not elses[0][0].endswith(":"):
                self.problem(f"`if {name}` has no named else block")
                return self.branch(name, pred, True, kids, facts)
            other = elses[0][0][5:].rstrip(":")
            return self.branch(name, pred, True, kids, facts) + self.branch(other, pred, False, elses[0][1], facts)

        m = re.match(r"conditional (\w+)(?: = (.*))?:$", text)
        if m:
            label = m.group(1)
            if m.group(2):
                env.setdefault(label, module.type_of(m.group(2), env))
            out, rest = [], [facts]
            for t, body in kids:
                cm = re.match(r"condition ([\w.]+) = (.*):$", t)
                if cm:
                    for f in rest:
                        out += self.branch(cm.group(1), cm.group(2), True, body, f)
                    rest = [f2 for f in rest for f2 in assume(module, env, cm.group(2), False, f)]
                    continue
                em = re.match(r"else ([\w.]+):$", t)
                if em:
                    for f in rest:
                        out += [(kind, [em.group(1)] + steps, f2) for kind, steps, f2 in self.seq(body, f)]
            return out

        m = re.match(r"compare (\w+) = (.*) with (.*):$", text)
        if m:
            label, a, b = m.groups()
            possible = facts.possible_orders(a, b)
            if possible is None:
                self.problem(f"`compare {label}` compares values with no order")
                possible = {"lt", "eq", "gt"}
            names = {"less": "lt", "equal": "eq", "greater": "gt"}
            out, seen = [], set()
            for t, body in kids:
                cm = re.match(r"condition ([\w.]+) = \(?([a-z ]+?)\)?:$", t)
                em = re.match(r"else ([\w.]+):$", t)
                if cm:
                    orders = [names[w] for w in cm.group(2).split(" or ")]
                    if seen & set(orders):
                        self.problem(f"`compare {label}`: an order is in two conditions")
                    name = cm.group(1)
                elif em:
                    orders = [o for o in ("lt", "eq", "gt") if o not in seen]
                    name = em.group(1)
                else:
                    continue
                seen |= set(orders)
                side = []
                for o in orders:
                    f = facts.copy()
                    if o in possible and f.constrain(a, b, o):
                        side.append((o, f))
                for o, f in side:
                    for kind, steps, f2 in self.seq(body, f):
                        out.append((kind, [name + (f"'{o}" if len(side) > 1 else "")] + steps, f2))
            if seen != {"lt", "eq", "gt"}:
                self.problem(f"`compare {label}` leaves an order out")
            return out

        m = re.match(r"for (\w+) in (.*):$", text)
        if m:
            return self.loop(m.group(1), None, m.group(2), kids, elses, facts, f"for {m.group(1)} in {m.group(2)}")

        m = re.match(r"while (\w+) = (.*):$", text)
        if m:
            return self.loop(m.group(1), m.group(2), None, kids, elses, facts, f"while {m.group(1)}")

        m = re.match(r"assert \w+ = (.*)$", text)
        if m:
            return [("next", [], f) for f in assume(module, env, m.group(1), True, facts)]

        if text.startswith("return"):
            return [("return", [], facts)]
        if text in ("fail not_reachable", "fail not_implemented"):
            return [(text[5:], [], facts)]
        if text.startswith("fail "):
            return [("fail", [], facts)]
        if text in ("continue", "break"):
            return [(text, [], facts)]

        outs = [("next", [], facts)]
        if kids and re.match(r"(region |within |inject\(|inject:)", text):
            outs = self.seq(kids, facts)
        n = failing_name(text)
        if n:
            key = (n, text)
            if key not in self.order:
                self.fails_seen[n] = self.fails_seen.get(n, 0) + 1
                self.order[key] = self.fails_seen[n]
            label = n if self.fails_total[n] == 1 else f"{n}'{self.order[key]}"
            outs = outs + [("fail", [f"{label}'fails"], facts.copy())]
        # what the statement changes, and what a new name stands for
        changed = mutated([(text, [])])
        grown = re.match(r"(\w+)\.(push|extend)\(", text)
        if grown:
            for kind, steps, f in outs:
                f.grow(grown.group(1), 1 if grown.group(2) == "push" else None)
            changed.discard(grown.group(1))
        mm = re.match(r"(?:mutable )?(\w+) :?= (.*?)( or fail)?$", text)
        if mm:
            changed.discard(mm.group(1))
        for kind, steps, f in outs:
            if kind != "next":
                continue
            for name in changed:
                f.forget(name)
            if mm and not text.startswith(("if ", "while ", "for ", "conditional ", "compare ", "return ", "assert ")):
                f.bind(mm.group(1), re.sub(r" forever because \".*\"", "", mm.group(2)))
        return outs

    def count_of(self, src, facts):
        """The interval of the number of elements a `for` goes over."""
        m = re.fullmatch(r"range\(from: (.+), (below|through): (.+)\)", src)
        if m:
            a, b = facts.interval(m.group(1)), facts.interval(m.group(3))
            extra = 1 if m.group(2) == "through" else 0
            same = split_at(m.group(3), " + ")
            if same and same[0] == m.group(1).strip() and self.module.value(same[1]) is not None:
                n = self.module.value(same[1]) + extra
                return n, n
            if a[0] is not None and a[0] == a[1]:
                return (None if b[0] is None else max(0, b[0] - a[0] + extra),
                        None if b[1] is None else max(0, b[1] - a[0] + extra))
            return 0, None
        n = self.module.length(src)
        if n is not None:
            return n, n
        f = facts.copy()
        f.bind("loop'source", src)
        return meet(meet(f.interval("loop'source.count()"), facts.interval(f"{src}.count()")), (0, None))

    def loop(self, var, pred, src, kids, elses, facts, what):
        module, env = self.module, self.env
        out = []
        else_name = elses[0][0][5:].rstrip(":") if elses else None
        else_body = elses[0][1] if elses else []

        # the loop never runs
        if pred is None:
            count = self.count_of(src, facts)
            never = [] if (count[0] or 0) >= 1 else [("", self.with_count(src, facts, (0, 0)))]
        else:
            never = outcomes(module, env, pred, False, facts)
        if never and else_name is None:
            self.problem(f"`{what}` may never run, and has no named else")
        if not never and else_name is not None:
            self.problem(f"`{what}` runs for sure, and needs no else")
        if else_name is not None:
            for s, f in never:
                for kind, steps, f2 in self.seq(else_body, f):
                    out.append((kind, [else_name + s] + steps, f2))

        # the last pass: the first, or a later one, after which the facts on what the body changes no longer hold
        changed = mutated(kids)
        grown = grown_only(kids, changed)
        count = self.count_of(src, facts) if pred is None else None
        for many in (False, True):
            base = facts.copy()
            if many:
                widen(base, changed, grown)
            if pred is None:
                if empty(meet(count, (2, None) if many else (1, None))):
                    continue
                entries = [("", self.with_count(src, base, (2, None) if many else (1, None)))]
            else:
                entries = outcomes(module, env, pred, True, base)
            for s, f in entries:
                f.forget(var)
                for kind, steps, f2 in self.seq(kids, f):
                    goes_on = kind in ("next", "continue")
                    if pred is None and goes_on and empty(meet(count, (2, None) if many else (1, 1))):
                        # a pass that goes on is the last only when the elements run out
                        continue
                    ends = [f2]
                    if many:
                        # other passes come before or after this one: what they change is not known after the loop
                        widen(f2, changed, grown)
                    elif pred is not None and goes_on:
                        # the only pass is the last, so the condition is then false
                        ends = assume(module, env, pred, False, f2)[:1]
                    for f3 in ends:
                        out.append(("next" if kind in ("next", "continue", "break") else kind,
                                    [var + ("'many" if many else "") + s] + steps, f3))
        return out

    def with_count(self, src, facts, interval):
        f = facts.copy()
        if not src.startswith("range("):
            f.add(f"{src}.count()", interval)
        return f


def functions(module):
    lines = module.lines
    i = 0
    while i < len(lines):
        m = re.match(r"\s*function (\w+)\(", lines[i])
        if m:
            d = len(lines[i]) - len(lines[i].lstrip())
            j = i
            while j < len(lines) and not lines[j].rstrip().endswith(":"):
                j += 1
            body, j = [], j + 1
            while j < len(lines) and (not lines[j].strip() or len(lines[j]) - len(lines[j].lstrip()) > d):
                if lines[j].strip() and not lines[j].strip().startswith("--"):
                    body.append(lines[j])
                j += 1
            yield Function(module, m.group(1), body)
            i = j
        else:
            i += 1


def path_id(name, steps):
    return " > ".join([name] + steps)


def all_paths(module):
    """{path id: kind} for every path of every function, and {function: problems}."""
    paths, problems = {}, {}
    for f in functions(module):
        for kind, steps in f.paths:
            paths[path_id(f.name, steps)] = kind
        if f.problems:
            problems[f.name] = f.problems
    return paths, problems


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--with=")]
    module = Module(args[0], [a[7:] for a in sys.argv[1:] if a.startswith("--with=")])
    show = args[1] if len(args) > 1 else None
    rows = list(functions(module))
    total = sum(len(f.paths) for f in rows)
    exempt = sum(1 for f in rows for kind, _ in f.paths if kind in EXEMPT)
    for f in sorted(rows, key=lambda f: -len(f.paths)):
        if show in (None, f.name, "all"):
            print(f"{f.name:22} {len(f.paths):4} paths" + (f"   PROBLEMS: {f.problems}" if f.problems else ""))
            if show:
                for kind, p in f.paths:
                    print("      ", path_id(f.name, p), "" if kind in ("next", "return") else f"[{kind}]")
    print("total", total, "paths;", exempt, "exempt")
    if module.unknown:
        print("comparisons of unknown type:", sorted(set(module.unknown)))
