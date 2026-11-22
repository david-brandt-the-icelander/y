# Path tools

Scripts that keep the examples under [`../../examples`](../../examples/) in step with their execution paths (Y § 3.5,
§ 3.14). They were written before the Y compiler, [`../y-compile`](../y-compile/), and stand in for the parts of one
that the examples need: they enumerate the paths of a Y module, regenerate the lowered `bare/` files and
`bare/y-facts.json` from the hand-written Rust, and run the tests to see which paths they take. They need Python 3 and a Rust toolchain, and nothing else. Each reads the Y source through the parser in
[`../y-parse`](../y-parse/), by way of [`ytree.py`](ytree.py).

| File | What it does |
|---|---|
| [`rebare.sh`](rebare.sh) | `tools/paths/rebare.sh <example>`: remakes `examples/<example>/bare/` from the example's Rust and Y (using `regen.py` and `pathfacts.py`), then runs the example's checks in `tools/bare-rust/tests/y_examples.rs`. Run it after any change to an example's Y or Rust; never edit `bare/` by hand. It prints `uncovered: [] unknown: []` when every path that needs a test is claimed by one. `rebare.sh --from-y <example>` takes `bare/` from the Y compiler instead, and `src/` from its weave, then runs the same checks; `git checkout examples/<example>/src` brings the hand-written Rust back |
| [`regen.py`](regen.py) | Rebuilds the `.brs` bodies and constraint files from `bare-rust strip`, carrying each function's clauses over and re-pointing them at their lines in the Y: a `require` at its `where name = `, an `ensure` at its `ensures name = `, in the function the subject names, and any other clause at the function's head. It makes from the Y source's tree what strip cannot see: each `injected` parameter's mark, and each region's subject with its clauses. A function that is new has no clauses: pass them as JSON in the third argument. It prints the functions left without any |
| [`ytree.py`](ytree.py) | The trees of Y files, from [`../y-parse`](../y-parse/), as dicts: `ytree.tree(path)`, `ytree.walk(node)`, `ytree.nodes(node, field)`, and `ytree.declarations(tree)`, each declaration with the facet it is in. It builds the parser with cargo when it needs to |
| [`pathfacts.py`](pathfacts.py) | Rewrites `bare/y-facts.json`: the decisions (loops and `compare` among them), every path of the module and how it ends, and the path each test claims, the decisions and the tests read from the Y source's tree; the other facts carry over, each place found again by what the fact says (see `places.py`) |
| [`places.py`](places.py) | How `pathfacts.py` finds a fact's lines after an edit: by the fact's own words, not by its old line's text. A place in the Y source is a node of its tree: a claim's is its `forever because "…"` or its region's constraint with that reason; a facet's is its `facet`; a capability's is its function's head; a suspension's is the call. A place in a bare body is found by its line's text: a claim's `// y:` marker gives its reason; a copy's bare line is marked `// y: a copy` and clones what it copies. Facts that share such a key take its lines in order |
| [`yp.py`](yp.py) | The path enumerator, with its prover of facts. `python3 tools/paths/yp.py examples/nim/y/src/nim.y all` lists the paths of a module; the module docstring states the rules. `tools/bare-rust/tests/y_paths/mod.rs` is a port of it to Rust, which the checks use: change both together, and keep their output the same |
| [`ylint.py`](ylint.py) | `python3 tools/paths/ylint.py <file.y>…`: the lint `type_argument_name` (Y § 3.1), which `rebare.sh` runs: an argument that takes a type is named `…_type`, and one that takes a value is not, unless `-- lint: allow type_argument_name because "…"` suppresses it. `tools/bare-rust/tests/y_lint/mod.rs` is the same lint in Rust, which the example checks run: change both together |
| [`trace.py`](trace.py) | `python3 tools/paths/trace.py examples/<example> <module>`: runs the example's tests with every decision of the module instrumented (by [`ytrace.rs`](ytrace.rs)), and reports a test that does not take the path it claims, a path no test takes or claims, and a path taken that the enumerator does not know. The checks in `y_examples.rs` only see what the tests claim; this sees what they do |

So an edit may change or move the lines a fact is about, and the fact follows. Only a fact no key finds (a copy's Y
line when its function was renamed, an invariant's clause) falls back to where its old line went: in the Y source, the
node that started on that line in git's `HEAD`, found again in the tree by its kind, its name or what it writes, and the
declarations it is in (`places.same_node`); in a bare body, the line with the text it had. So run the scripts on
uncommitted edits, as `rebare.sh` does, and commit the result with the edits.

`trace.py` works on a copy of the example in `$YTRACE_WORK` (by default `y-trace` in the system's temporary folder) and
keeps its cargo target folder there between runs. It exits with status 1 if it reports anything.
