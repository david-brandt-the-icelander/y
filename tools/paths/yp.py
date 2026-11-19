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
branches taken, the conditions found false, `where` preconditions, the `ensures` of a called function, and constants.
A sub-outcome the facts rule out is no path: that is what "proven" means here.

It reads the Y source through its tree (`ytree.py`, from `tools/y-parse`): declarations, statements and expressions
are its nodes. The facts are kept on expressions by their text, as `text` writes a node: `game.misses.count()`.
`tools/bare-rust/tests/y_paths/mod.rs` is a port of it to Rust: change both together, and keep their output the same.
"""
import re, sys
import ytree

ORDERED = {"int32", "uint32", "uint16", "int64", "uint64", "duration", "instant", "count", "int"}
UNSIGNED = {"uint32", "uint16", "uint64", "duration", "count"}
EXEMPT = ("not_reachable", "not_implemented")
PRECEDENCE = {"or": 1, "and": 2, "==": 4, "!=": 4, "<": 4, "<=": 4, ">": 4, ">=": 4, "+": 5, "-": 5, "*": 6, "/": 6, "%": 6}


# ---------------------------------------------------------------------------------------------------------------------
# Expressions: nodes of the tree, and nodes made here

def name(n):
    return {"kind": "name", "name": n}


def member(obj, n):
    return {"kind": "member", "object": obj, "name": n}


def call(callee, args=()):
    return {"kind": "call", "callee": callee, "args": [{"kind": "arg", "name": None, "value": a} for a in args]}


def count(obj):
    """`obj.count()`."""
    return call(member(obj, "count"))


def text(e, top=True):
    """An expression as text, the key facts are kept by: brackets where the source has them, but not around the
    whole."""
    k = e["kind"]
    if k == "name":
        out = e["name"]
    elif k == "literal":
        out = e["text"]
    elif k == "member":
        out = f"{text(e['object'], False)}.{e['name']}"
    elif k == "call":
        args = ", ".join((f"{a['name']}: " if a["name"] else "") + text(a["value"], False) for a in e["args"])
        out = f"{text(e['callee'], False)}({args})"
    elif k == "index":
        out = f"{text(e['object'], False)}[{text(e['index'], False)}]"
    elif k == "binary":
        out = f"{text(e['left'], False)} {e['op']} {text(e['right'], False)}"
    elif k == "unary":
        out = ("not " if e["op"] == "not" else "-") + text(e["operand"], False)
    elif k == "list":
        out = "[" + ", ".join(text(i, False) for i in e["items"]) + "]"
    elif k in ("mutable", "fake", "compile", "submit"):
        out = f"{k} {text(e['value'], False)}"
    elif k == "lambda":
        out = f"lambda {e['name']}"
    elif k == "old":
        out = f"old({text(e['value'], False)})"
    elif k == "reveal":
        out = f"reveal({text(e['value'], False)}) because \"{e['because']}\""
    elif k == "or_fail":
        out = f"{text(e['value'], False)} or fail"
    elif k == "forever":
        out = f"{text(e['value'], False)} forever because \"{e['because']}\""
    else:
        raise ValueError(k)
    return f"({out})" if e.get("parens") and not top else out


def chain(e):
    """A name or a path of names, `game.misses`, as text; None for anything else."""
    if e["kind"] == "name":
        return e["name"]
    if e["kind"] == "member":
        o = chain(e["object"])
        return f"{o}.{e['name']}" if o else None
    return None


def root(e):
    """The name a path of members and indexes starts from: `game` of `game.squares[i]`; None when it starts otherwise."""
    while e["kind"] in ("member", "index"):
        e = e["object"]
    return e["name"] if e["kind"] == "name" else None


def method(e, *names, args=None):
    """`obj` when `e` is `obj.m(…)` for a method `m` among `names` (any, when none are given), with `args` arguments
    when it says how many; else None."""
    if e["kind"] == "call" and e["callee"]["kind"] == "member" and (not names or e["callee"]["name"] in names):
        if args is None or len(e["args"]) == args:
            return e["callee"]["object"]
    return None


def arg(e, n):
    """The argument named `n` of a call."""
    return next((a["value"] for a in e["args"] if a["name"] == n), None)


def is_int(e):
    """A whole number, `6` or `-6`."""
    if e["kind"] == "unary" and e["op"] == "-":
        e = e["operand"]
        return e["kind"] == "literal" and e["type"] == "integer" and not e.get("parens")
    return e["kind"] == "literal" and e["type"] == "integer"


def operands(e, op):
    """The operands of a chain of `op` (`and` or `or`), as the source groups them: `(a or b) or c` has two."""
    if e["kind"] == "binary" and e["op"] == op:
        left = e["left"]
        return (operands(left, op) if not left.get("parens") else [left]) + [e["right"]]
    return [e]


def unwrap(e):
    """An expression without its `or fail` and its `forever because "…"`."""
    while e["kind"] in ("or_fail", "forever"):
        e = e["value"]
    return e


def norm(e):
    """An expression as facts are kept on it: a count converted to another whole number is the same count."""
    inner = method(e, "to_int32", "to_uint32", args=0)
    if inner is not None and method(inner, "count", args=0) is not None:
        return inner
    return e


def ty(n):
    """A type that has no arguments, by its name."""
    return {"kind": "type", "name": n, "args": [], "text": n}


def type_text(t):
    return t["text"] if t else None


def element(t):
    """The element type of `list<element_type=T>`."""
    if t and t["name"] == "list" and len(t["args"]) == 1 and t["args"][0]["name"] == "element_type" and t["args"][0]["value"]["kind"] == "type":
        return t["args"][0]["value"]
    return None


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
        self.tree = ytree.tree(path)
        self.fields, self.constants, self.values, self.results, self.params = {}, {}, {}, {}, {}
        self.contracts = {}
        # a refined type's constraints on its values' counts: {type: [(op, n)]}
        self.refinements = {}
        self.unknown = []
        for p in (path, *others):
            self.collect(ytree.tree(p))
        self.groups = {k.split(".")[0] for k in self.constants}

    def collect(self, tree):
        for _, d in ytree.declarations(tree):
            if d["kind"] == "structure":
                for m in d["members"]:
                    if m["kind"] == "field":
                        self.fields.setdefault(m["name"], m["type"])
            elif d["kind"] == "constants":
                for c in d["items"]:
                    key = f"{d['name']}.{c['name']}"
                    if c["type"]:
                        self.constants[key] = c["type"]
                        self.values[key] = c["value"]
                    else:
                        self.constants[key] = ty(d["name"])
            elif d["kind"] == "type_decl" and d["type"]:
                # `where name = (value.count() >= 4)`: what the refined type allows of its values' counts
                for w in d["where"]:
                    for n in ytree.walk(w["condition"]):
                        if n["kind"] == "binary" and n["op"] in (">=", ">", "<=", "<", "==") and text(n["left"]) == "value.count()" \
                                and n["right"]["kind"] == "literal" and n["right"]["type"] == "integer":
                            self.refinements.setdefault(d["name"], []).append((n["op"], int(n["right"]["text"])))
            elif d["kind"] == "enumeration":
                # an enumeration's members, in order: what `for x in it` goes over, so it runs for sure
                self.values[d["name"]] = {"kind": "list", "items": [{"kind": "name", "name": i["name"]} for i in d["items"]]}
        for d in callables(tree):
            if True:
                # a parameter's or the result's type, from a documentation entry on one line: `name: …, as type.`
                types = {}
                for e in (d["doc"] or {}).get("entries", []):
                    if e["type"] and e["end"] == e["line"] and e["text"].endswith(f", as {e['type']['text']}.") \
                            and re.fullmatch(r"[\w<>=, ]+", e["type"]["text"]):
                        types[e["name"]] = e["type"]
                self.params[d["name"]] = {p["name"]: types.get(p["name"]) for p in d["params"]}
                self.results[d["name"]] = (d["returns"], types.get(d["returns"]))
                self.contracts[d["name"]] = [("where", w["condition"]) for w in d["where"]] + [("ensures", w["condition"]) for w in d["ensures"]]

    def value(self, e):
        """The constant value of `e`, if it has one."""
        if is_int(e):
            return int(text(e))
        key = text(e)
        if key in self.values:
            return self.value(self.values[key])
        if e["kind"] == "binary" and e["op"] in ("+", "-", "*", "/"):
            a, b = self.value(e["left"]), self.value(e["right"])
            if a is None or b is None:
                return None
            return {"+": a + b, "-": a - b, "*": a * b, "/": a // b if b else None}[e["op"]]
        return None

    def length(self, e):
        """The constant length of a list, if it has one."""
        e = self.values.get(text(e), e)
        return len(e["items"]) if e["kind"] == "list" else None

    def type_of(self, e, env):
        k = e["kind"]
        if is_int(e):
            return ty("int")
        if k == "literal":
            if e["type"] == "duration" and re.fullmatch(r"\d+(ms|s)", e["text"]):
                return ty("duration")
            if e["type"] == "string":
                return ty("string")
            if e["type"] == "boolean":
                return ty("boolean")
            return None
        if k == "unary":
            return ty("boolean") if e["op"] == "not" else self.type_of(e["operand"], env)
        if k == "binary" and e["op"] in ("+", "-", "*", "/", "%"):
            a, b = self.type_of(e["left"], env), self.type_of(e["right"], env)
            if type_text(a) == "instant" and type_text(b) == "instant":
                return ty("duration")
            return a if type_text(a) not in (None, "int") else b if b else a
        if k == "binary":
            return ty("boolean")
        if method(e, "count", args=0) is not None:
            return ty("count")
        if text(e) == "clock.now()":
            return ty("instant")
        for m, t in (("to_int32", "int32"), ("to_uint32", "uint32"), ("whole_seconds", "uint32"), ("is_empty", "boolean")):
            if method(e, m, args=0) is not None:
                return ty(t)
        if method(e, "contains") is not None:
            return ty("boolean")
        inner = method(e, "divided_rounding_up", "min", "max")
        if inner is not None:
            return self.type_of(inner, env)
        inner = method(e, "largest", "smallest", "last", args=0)
        if inner is None:
            inner = method(e, "at_number")
        if inner is None and k == "index":
            inner = e["object"]
        if inner is not None:
            return element(self.type_of(inner, env))
        if k == "call" and e["callee"]["kind"] == "name" and e["callee"]["name"] in self.results:
            return self.results[e["callee"]["name"]][1]
        key = text(e)
        if key in env:
            return env[key]
        if key in self.constants:
            return self.constants[key]
        if k == "member" and simple(e["object"]) and e["name"] in self.fields:
            return self.fields[e["name"]]
        return None


def plain(e):
    """A path of names and indexes whose index is a plain path or a number: `game.squares[i]`, `a[b.c]`."""
    if e["kind"] == "name":
        return True
    if e["kind"] == "member":
        return plain(e["object"])
    if e["kind"] == "index":
        i = e["index"]
        return plain(e["object"]) and (plain(i) or (i["kind"] == "literal" and i["type"] == "integer"))
    return False


def simple(e):
    """A path of names and indexes by a name or a number: `game.squares[i]`, not `f(x).y`."""
    if e["kind"] == "name":
        return True
    if e["kind"] == "member":
        return simple(e["object"])
    if e["kind"] == "index":
        i = e["index"]
        return simple(e["object"]) and (i["kind"] == "name" or (i["kind"] == "literal" and i["type"] == "integer"))
    return False


# ---------------------------------------------------------------------------------------------------------------------
# Facts along a path: intervals of numbers, booleans (0 or 1) and counts, kept by an expression's text

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
        t = text(e)
        return t.startswith(("optional.", "result.")) or t.split(".")[0] in self.module.groups and t not in self.module.values

    def equality(self, left, right, truth):
        """Adds that `left` is, or is not, `right`, for values with no order: False when it cannot be."""
        if self.is_constant(left) and not self.is_constant(right):
            left, right = right, left
        if not self.is_constant(right):
            return True
        left, right = text(left), text(right)
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
        if e["kind"] == "literal" and e["type"] == "boolean":
            return (1, 1) if e["text"] == "true" else (0, 0)
        counted = method(e, "count", args=0)
        if counted is not None and self.module.length(counted) is not None:
            n = self.module.length(counted)
            return n, n
        i = self.known.get(text(e), (None, None))
        if counted is not None:
            # a refined type bounds its values' counts
            for op, n in self.module.refinements.get(type_text(self.module.type_of(counted, self.env)), []):
                i = meet(i, {">=": (n, None), ">": (n + 1, None), "<=": (None, n), "<": (None, n - 1), "==": (n, n)}[op])
            runes = method(counted, "runes", args=0)
            if runes is not None:
                i = meet(i, self.interval(count(runes)))
            # the standard library: splitting text gives at least one piece
            if method(counted, "split") is not None:
                i = meet(i, (1, None))
        t = type_text(self.module.type_of(e, self.env))
        if t in UNSIGNED:
            i = meet(i, (0, None))
        if t == "boolean":
            i = meet(i, (0, 1))
        return i

    def add(self, e, i):
        e = norm(e)
        new = meet(self.interval(e), i)
        self.known[text(e)] = new
        return not empty(new)

    def forget(self, n):
        """Forgets what is known of `n`, and of every expression that reads it."""
        reads = re.compile(rf"(?<![\w.']){re.escape(n)}\b")
        self.known = {k: v for k, v in self.known.items() if not reads.search(k)}
        self.same = {k: v for k, v in self.same.items() if not reads.search(k)}
        self.differs = {k: v for k, v in self.differs.items() if not reads.search(k)}

    def grow(self, n, by):
        """A list `n` gains `by` elements, or an unknown number when `by` is None."""
        i = self.known.get(f"{n}.count()")
        self.forget(n)
        if i is not None and by is not None:
            self.known[f"{n}.count()"] = (None if i[0] is None else i[0] + by, None if i[1] is None else i[1] + by)

    def bind(self, n, rhs):
        """`n` now stands for `rhs`: it takes on what is known of it, and what the function it calls ensures."""
        r = text(rhs)
        new, same = {}, None
        for k, v in self.known.items():
            if k == r:
                new[n] = v
            elif k.startswith(r + "."):
                new[n + k[len(r):]] = v
        v = self.module.value(rhs)
        if v is not None:
            new[n] = (v, v)
        if self.is_constant(rhs):
            same = r
        elif r in self.same:
            same = self.same[r]
        # arithmetic on what is known
        if rhs["kind"] == "binary" and rhs["op"] in ("+", "-") and self.module.value(rhs["right"]) is not None:
            i, c = self.interval(rhs["left"]), self.module.value(rhs["right"])
            c = c if rhs["op"] == "+" else -c
            new[n] = (None if i[0] is None else i[0] + c, None if i[1] is None else i[1] + c)
        # a negated value
        if rhs["kind"] == "unary" and rhs["op"] == "-" and self.module.value(rhs) is None:
            inner = self.copy()
            inner.bind("negated'value", rhs["operand"])
            i = inner.interval(name("negated'value"))
            new[n] = (None if i[1] is None else -i[1], None if i[0] is None else -i[0])
        # what the standard library ensures of the lists it makes
        if r == "list.new()" or (rhs["kind"] == "list" and not rhs["items"]):
            new[f"{n}.count()"] = (0, 0)
        if rhs["kind"] == "call" and chain(rhs["callee"]) == "random.shuffled" and len(rhs["args"]) == 1 and rhs["args"][0]["name"] is None:
            new[f"{n}.count()"] = self.interval(count(rhs["args"][0]["value"]))
        if rhs["kind"] == "call" and chain(rhs["callee"]) == "random.chosen" and [a["name"] for a in rhs["args"]] == ["from", "count"]:
            new[f"{n}.count()"] = self.interval(arg(rhs, "count"))
        if rhs["kind"] == "call" and chain(rhs["callee"]) == "list.repeated" and [a["name"] for a in rhs["args"]] == [None, "times"]:
            new[f"{n}.count()"] = self.interval(arg(rhs, "times"))
        if rhs["kind"] == "literal" and rhs["type"] == "boolean":
            new[n] = (1, 1) if rhs["text"] == "true" else (0, 0)
        self.forget(n)
        self.known.update(new)
        if same is not None:
            self.same[n] = same
        # what the function it calls ensures of it
        if rhs["kind"] == "call" and rhs["callee"]["kind"] == "name":
            f = rhs["callee"]["name"]
            result = self.module.results.get(f, (None, None))[0]
            for kind, pred in self.module.contracts.get(f, []):
                if kind == "ensures" and result:
                    alts = assume(self.module, self.env, renamed(pred, result, n), True, self)
                    if alts:
                        merged = join(alts)
                        self.known, self.same, self.differs = merged.known, merged.same, merged.differs

    def possible_orders(self, left, right):
        lt, rt = self.module.type_of(left, self.env), self.module.type_of(right, self.env)
        known = [t for t in (lt, rt) if t is not None]
        if not known:
            if not any(text(x).startswith(("optional.", "result.")) or text(x).split(".")[0] in self.module.groups for x in (left, right)):
                self.module.unknown.append(f"{text(left, False)} ? {text(right, False)}")
            return None
        if not any(type_text(t) in ORDERED for t in known):
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


def renamed(e, old, new):
    """`e` with every name `old`, and every member named `old`, renamed `new`."""
    if isinstance(e, list):
        return [renamed(x, old, new) for x in e]
    if not isinstance(e, dict):
        return e
    out = {k: renamed(v, old, new) for k, v in e.items()}
    if e.get("kind") in ("name", "member") and e.get("name") == old:
        out["name"] = new
    return out


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
    if e["kind"] == "unary" and e["op"] == "not":
        return outcomes(module, env, e["operand"], not truth, facts)
    for word, stops_on in (("or", True), ("and", False)):
        ops = operands(e, word)
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
    if e["kind"] == "binary" and e["op"] in ORDERS:
        left, op, right = e["left"], e["op"], e["right"]
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
    inner = method(e, "is_empty", args=0)
    if inner is not None:
        f = facts.copy()
        return [("", f)] if f.add(count(inner), (0, 0) if truth else (1, None)) else []
    f = facts.copy()
    if type_text(module.type_of(e, env)) == "boolean" or chain(e) is not None:
        if not f.add(e, (1, 1) if truth else (0, 0)):
            return []
    return [("", f)]


def assume(module, env, e, truth, facts):
    """The facts after `e` is found `truth`, one set for each way it can be."""
    return [f for _, f in outcomes(module, env, e, truth, facts)]


# ---------------------------------------------------------------------------------------------------------------------
# Statements

def own(s):
    """The expressions a statement writes on its own line, not those of the blocks it opens."""
    k = s["kind"]
    fields = {"let": ("value",), "assign": ("target", "value"), "expr": ("value",), "return": ("value",),
              "fail": ("value",), "if": ("condition",), "while": ("condition",), "for": ("iterable",),
              "conditional": ("subject",), "compare": ("left", "right"), "condition": ("test",), "task": ("value",)}.get(k, ())
    out = [s[f] for f in fields if s.get(f)]
    if k == "inject":
        out += [b["value"] for b in s["bindings"]]
    if k == "region":
        out += [c for c in s["constraints"]]
    return out


def blocks(s):
    """The blocks of statements a statement opens, its `else`'s and its conditions' among them."""
    out = [s.get("body", [])]
    for c in s.get("conditions", []):
        out.append(c["body"])
    if s.get("else"):
        out.append(s["else"]["body"])
    for r in s.get("orelse", []):
        out.append(r["body"])
    return out


def statements(items):
    """Every statement of a block, and of the blocks inside it, each before the statements inside it."""
    for s in items:
        yield s
        for c in s.get("conditions", []):
            yield c
            yield from statements(c["body"])
        yield from statements(s.get("body", []))
        if s.get("else"):
            yield from statements(s["else"]["body"])
        for r in s.get("orelse", []):
            yield r
            yield from statements(r["body"])


def mutable_args(s):
    """The names a statement passes as `mutable`."""
    return {root(m["value"]) for e in own(s) for m in ytree.walk(e) if m["kind"] == "mutable" and root(m["value"])}


def changes(s):
    """The names a statement changes on its own line."""
    out = set()
    if s["kind"] == "assign" and plain(s["target"]):
        out.add(root(s["target"]))
    if not (s["kind"] == "let" and s["mutable"]):
        out |= mutable_args(s)
    if s["kind"] == "expr":
        obj = method(unwrap(s["value"]), "push", "extend", "set_number", "insert_first", "clear")
        if obj is not None and chain(obj):
            out.add(chain(obj).split(".")[0])
    if s["kind"] == "let":
        out.add(s["name"])
    if s["kind"] == "for":
        out.add(s["name"])
    return out


def mutated(items):
    """The names a block changes."""
    out = set()
    for s in statements(items):
        out |= changes(s)
    return out


def grown_only(items, names):
    """The names among `names` that a block changes only by adding to them, with `push` or `extend`."""
    other = set()
    for s in statements(items):
        for n in names:
            obj = method(unwrap(s["value"]), "push", "extend") if s["kind"] == "expr" else None
            if obj is not None and obj["kind"] == "name" and obj["name"] == n:
                continue
            if (s["kind"] == "let" and s["name"] == n) or (s["kind"] == "assign" and root(s["target"]) == n and plain(s["target"])) \
                    or n in mutable_args(s):
                other.add(n)
            obj = method(unwrap(s["value"]), "set_number", "insert_first", "clear") if s["kind"] == "expr" else None
            if obj is not None and chain(obj) and chain(obj).split(".")[0] == n:
                other.add(n)
    return {n for n in names if n not in other}


def widen(facts, names, grown):
    """Forgets what passes of a loop may have changed; a list that only grows keeps its least count."""
    for n in names:
        keep = facts.known.get(f"{n}.count()") if n in grown else None
        facts.forget(n)
        if keep is not None:
            facts.known[f"{n}.count()"] = (keep[0], None)


def last_expression(s):
    """The expression a statement's line ends with, before any `:`."""
    field = {"let": "value", "assign": "value", "expr": "value", "return": "value", "task": "value", "fail": "value",
             "for": "iterable", "conditional": "subject", "compare": "right"}.get(s["kind"])
    return s.get(field) if field else None


def failing_name(s):
    """The name of a statement's `or fail` site: the name it binds, or the method or function it calls; None when it
    has none."""
    e = last_expression(s)
    if e is None or e["kind"] != "or_fail":
        return None
    if s["kind"] == "let":
        return s["name"]
    if s["kind"] == "assign" and s["target"]["kind"] == "name":
        return s["target"]["name"]
    inner = unwrap(e)
    if s["kind"] in ("expr", "assign") and inner["kind"] == "call" and chain(inner["callee"]):
        if s["kind"] == "expr":
            return chain(inner["callee"]).split(".")[-1]
    return "call"


def site(s):
    """What identifies an `or fail` site: two that read the same are one."""
    t = s.get("name") if s["kind"] == "let" else text(s["target"]) if s["kind"] == "assign" else None
    return s["kind"], s.get("mutable"), t, text(last_expression(s))


class Function:
    def __init__(self, module, decl):
        self.module, self.name = module, decl["name"]
        self.env = {p: t for p, t in module.params.get(self.name, {}).items() if t}
        result, rtype = module.results.get(self.name, (None, None))
        if result and rtype:
            self.env[result] = rtype
        self.problems = []
        items = decl["body"]
        self.fails_total, self.fails_seen, self.order = {}, {}, {}
        for s in statements(items):
            n = failing_name(s)
            if n:
                self.fails_total[n] = self.fails_total.get(n, 0) + 1
        starts = [Facts(module, self.env)]
        for kind, pred in module.contracts.get(self.name, []):
            if kind == "where":
                starts = [f for s in starts for f in assume(module, self.env, pred, True, s)][:1]
        found = self.seq(items, starts[0]) if starts else []
        seen, self.paths = set(), []
        for kind, steps, _ in found:
            key = (kind, tuple(steps))
            if key not in seen:
                seen.add(key)
                self.paths.append((kind, steps))

    def problem(self, t):
        if t not in self.problems:
            self.problems.append(t)

    def note_type(self, s):
        """The type of a name a statement binds, or of a loop's variable, when it is known."""
        if s["kind"] == "let":
            t = self.module.type_of(unwrap(s["value"]), self.env)
            if type_text(t) == "int":
                t = ty("int32")
            if t and s["name"] not in self.env:
                self.env[s["name"]] = t
        if s["kind"] == "for":
            src = s["iterable"]
            el = element(self.module.type_of(src, self.env))
            if el:
                self.env[s["name"]] = el
            elif src["kind"] == "call" and chain(src["callee"]) == "range":
                t = self.module.type_of(arg(src, "from"), self.env) or ty("int32")
                self.env[s["name"]] = ty("int32") if type_text(t) == "int" else t

    def seq(self, items, facts):
        """[(kind, steps, facts)] for a block entered with `facts`."""
        states, done = [("next", [], facts)], []
        for s in items:
            self.note_type(s)
            nxt = []
            for kind, steps, f in states:
                for kind2, steps2, f2 in self.statement(s, f.copy()):
                    (nxt if kind2 == "next" else done).append((kind2, steps + steps2, f2))
            states = nxt
        return states + done

    def branch(self, n, pred, truth, body, facts):
        out = []
        for s, f in outcomes(self.module, self.env, pred, truth, facts):
            for kind, steps, f2 in self.seq(body, f):
                out.append((kind, [n + s] + steps, f2))
        return out

    def statement(self, s, facts):
        module, env, k = self.module, self.env, s["kind"]

        # `region a is …:` with `orelse region b is …:` after it: a decision whose outcomes are the regions
        if k == "region" and s["orelse"]:
            out = []
            for r in [s] + s["orelse"]:
                out += [(kind, [r["name"]] + steps, f2) for kind, steps, f2 in self.seq(r["body"], facts.copy())]
            return out

        if k == "if":
            if not s["else"]:
                self.problem(f"`if {s['name']}` has no named else block")
                return self.branch(s["name"], s["condition"], True, s["body"], facts)
            return self.branch(s["name"], s["condition"], True, s["body"], facts) + \
                self.branch(s["else"]["name"], s["condition"], False, s["else"]["body"], facts)

        if k == "conditional":
            label = s["label"]
            if s["subject"]:
                env.setdefault(label, module.type_of(s["subject"], env))
            out, rest = [], [facts]
            for c in s["conditions"]:
                for f in rest:
                    out += self.branch(c["name"], c["test"], True, c["body"], f)
                rest = [f2 for f in rest for f2 in assume(module, env, c["test"], False, f)]
            if s["else"]:
                for f in rest:
                    out += [(kind, [s["else"]["name"]] + steps, f2) for kind, steps, f2 in self.seq(s["else"]["body"], f)]
            return out

        if k == "compare":
            label, a, b = s["label"], s["left"], s["right"]
            possible = facts.possible_orders(a, b)
            if possible is None:
                self.problem(f"`compare {label}` compares values with no order")
                possible = {"lt", "eq", "gt"}
            names = {"less": "lt", "equal": "eq", "greater": "gt"}
            out, seen = [], set()
            for c in s["conditions"] + ([s["else"]] if s["else"] else []):
                if c["kind"] == "condition":
                    orders = [names[w["name"]] for w in operands(c["test"], "or")]
                    if seen & set(orders):
                        self.problem(f"`compare {label}`: an order is in two conditions")
                else:
                    orders = [o for o in ("lt", "eq", "gt") if o not in seen]
                seen |= set(orders)
                side = []
                for o in orders:
                    f = facts.copy()
                    if o in possible and f.constrain(a, b, o):
                        side.append((o, f))
                for o, f in side:
                    for kind, steps, f2 in self.seq(c["body"], f):
                        out.append((kind, [c["name"] + (f"'{o}" if len(side) > 1 else "")] + steps, f2))
            if seen != {"lt", "eq", "gt"}:
                self.problem(f"`compare {label}` leaves an order out")
            return out

        if k == "for":
            return self.loop(s["name"], None, s["iterable"], s, facts, f"for {s['name']} in {text(s['iterable'])}")

        if k == "while":
            return self.loop(s["name"], s["condition"], None, s, facts, f"while {s['name']}")

        if k == "return":
            return [("return", [], facts)]
        if k == "fail" and s["value"] and s["value"]["kind"] == "name" and s["value"]["name"] in EXEMPT:
            return [(s["value"]["name"], [], facts)]
        if k == "fail":
            return [("fail", [], facts)]
        if k in ("continue", "break"):
            return [(k, [], facts)]

        outs = [("next", [], facts)]
        if k in ("region", "arena", "inject"):
            outs = self.seq(s["body"], facts)
        n = failing_name(s)
        if n:
            key = (n, site(s))
            if key not in self.order:
                self.fails_seen[n] = self.fails_seen.get(n, 0) + 1
                self.order[key] = self.fails_seen[n]
            label = n if self.fails_total[n] == 1 else f"{n}'{self.order[key]}"
            outs = outs + [("fail", [f"{label}'fails"], facts.copy())]
        # what the statement changes, and what a new name stands for
        changed = changes(s)
        obj = method(unwrap(s["value"]), "push", "extend") if k == "expr" else None
        if obj is not None and obj["kind"] == "name":
            for kind, steps, f in outs:
                f.grow(obj["name"], 1 if unwrap(s["value"])["callee"]["name"] == "push" else None)
            changed.discard(obj["name"])
        bound = s["name"] if k == "let" else s["target"]["name"] if k == "assign" and s["target"]["kind"] == "name" else None
        if bound:
            changed.discard(bound)
        for kind, steps, f in outs:
            if kind != "next":
                continue
            for c in changed:
                f.forget(c)
            if bound:
                f.bind(bound, unwrap(s["value"]))
        return outs

    def count_of(self, src, facts):
        """The interval of the number of elements a `for` goes over."""
        if src["kind"] == "call" and chain(src["callee"]) == "range" and len(src["args"]) == 2 and src["args"][0]["name"] == "from" \
                and src["args"][1]["name"] in ("below", "through"):
            start, end = src["args"][0]["value"], src["args"][1]["value"]
            a, b = facts.interval(start), facts.interval(end)
            extra = 1 if src["args"][1]["name"] == "through" else 0
            if end["kind"] == "binary" and end["op"] == "+" and not end.get("parens") and text(end["left"], False) == text(start, False) \
                    and self.module.value(end["right"]) is not None:
                n = self.module.value(end["right"]) + extra
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
        return meet(meet(f.interval(count(name("loop'source"))), facts.interval(count(src))), (0, None))

    def loop(self, var, pred, src, s, facts, what):
        module, env = self.module, self.env
        out = []
        else_name = s["else"]["name"] if s["else"] else None
        else_body = s["else"]["body"] if s["else"] else []
        kids = s["body"]

        # the loop never runs
        if pred is None:
            n = self.count_of(src, facts)
            never = [] if (n[0] or 0) >= 1 else [("", self.with_count(src, facts, (0, 0)))]
        else:
            never = outcomes(module, env, pred, False, facts)
        if never and else_name is None:
            self.problem(f"`{what}` may never run, and has no named else")
        if not never and else_name is not None:
            self.problem(f"`{what}` runs for sure, and needs no else")
        if else_name is not None:
            for suffix, f in never:
                for kind, steps, f2 in self.seq(else_body, f):
                    out.append((kind, [else_name + suffix] + steps, f2))

        # the last pass: the first, or a later one, after which the facts on what the body changes no longer hold
        changed = mutated(kids)
        grown = grown_only(kids, changed)
        n = self.count_of(src, facts) if pred is None else None
        for many in (False, True):
            base = facts.copy()
            if many:
                widen(base, changed, grown)
            if pred is None:
                if empty(meet(n, (2, None) if many else (1, None))):
                    continue
                entries = [("", self.with_count(src, base, (2, None) if many else (1, None)))]
            else:
                entries = outcomes(module, env, pred, True, base)
            for suffix, f in entries:
                f.forget(var)
                for kind, steps, f2 in self.seq(kids, f):
                    goes_on = kind in ("next", "continue")
                    if pred is None and goes_on and empty(meet(n, (2, None) if many else (1, 1))):
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
                                    [var + ("'many" if many else "") + suffix] + steps, f3))
        return out

    def with_count(self, src, facts, interval):
        f = facts.copy()
        if not (src["kind"] == "call" and chain(src["callee"]) == "range"):
            f.add(count(src), interval)
        return f


CALLABLE = ("function", "transition", "handler", "workflow")


def callables(tree):
    """Every callable of a file, each with its own paths: its functions, transitions, handlers and workflows, the
    methods of its implementations, and the lambdas and nested functions in their bodies."""
    out = []
    def inner(d):
        for n in ytree.walk({"kind": "x", "body": d["body"]}):
            if n["kind"] == "lambda":
                out.append(n)
            if n["kind"] == "nested":
                out.append(n["function"])
    for _, d in ytree.declarations(tree):
        if d["kind"] in CALLABLE and not d.get("native"):
            out.append(d)
            inner(d)
        if d["kind"] == "implementation":
            for x in d["decls"]:
                if x["kind"] == "function":
                    out.append(x)
                    inner(x)
    # a lambda's or nested function's own lambdas
    return out


def functions(module):
    for d in callables(module.tree):
        yield Function(module, d)


def path_id(n, steps):
    return " > ".join([n] + steps)


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
