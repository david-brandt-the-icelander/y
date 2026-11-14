"""The trees of Y source files, as `y-parse` (`tools/y-parse`) makes them: each node a dict with its `kind`, the
`line` it starts on, and its fields. `trees` builds `y-parse` with cargo when it needs to.

    import ytree
    t = ytree.tree("examples/nim/y/src/nim.y")
    for facet, f in ytree.declarations(t): ..."""
import json, os, subprocess, sys

MANIFEST = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "y-parse", "Cargo.toml")
_trees = {}


def trees(paths):
    """Each file's tree, by its path as given. A file that does not parse ends the program with the parser's error,
    `file:line: message`."""
    want = [p for p in paths if os.path.abspath(p) not in _trees]
    if want:
        run = subprocess.run(["cargo", "run", "-q", "--release", "--manifest-path", MANIFEST, "--", *want], capture_output=True, text=True)
        if run.returncode != 0:
            sys.exit(run.stderr.strip())
        for p, t in json.loads(run.stdout).items():
            _trees[os.path.abspath(p)] = t
    return {p: _trees[os.path.abspath(p)] for p in paths}


def tree(path):
    return trees([path])[path]


def parse_text(text):
    """The tree of a Y source given as text, such as a file as git's `HEAD` has it; None when it does not parse."""
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "source.y")
        with open(p, "w") as fh:
            fh.write(text)
        run = subprocess.run(["cargo", "run", "-q", "--release", "--manifest-path", MANIFEST, "--", p], capture_output=True, text=True)
        return json.loads(run.stdout)[p] if run.returncode == 0 else None


def nodes(node, key):
    """The nodes of a field: those of a list, or the one node it holds; none when it is missing or null."""
    v = node.get(key)
    if isinstance(v, dict):
        return [v]
    return [x for x in v if isinstance(x, dict)] if isinstance(v, list) else []


def walk(node):
    """This node and every node inside it, each before the nodes inside it, in source order."""
    yield node
    for v in node.values():
        for x in (v if isinstance(v, list) else [v]):
            if isinstance(x, dict) and "kind" in x:
                yield from walk(x)


def declarations(t):
    """Each declaration of a file, with the facet it is in or None: those in a facet in place of the facet."""
    for d in nodes(t, "decls"):
        if d["kind"] == "facet":
            for x in nodes(d, "decls"):
                yield d["name"], x
        else:
            yield None, d
