# Path tools

Scripts that keep the examples under [`../../examples`](../../examples/) in step with their execution paths (Y § 3.5,
§ 3.14). There is no Y compiler, so these stand in for the parts of one that the examples need: they enumerate the paths
of a Y module, regenerate the lowered `bare/` files and `bare/y-facts.json`, and run the tests to see which paths they
take. They need Python 3 and a Rust toolchain, and nothing else.

| File | What it does |
|---|---|
| [`rebare.sh`](rebare.sh) | `tools/paths/rebare.sh <example>`: remakes `examples/<example>/bare/` from the example's Rust and Y (using `regen.py` and `pathfacts.py`), then runs the example's checks in `tools/bare-rust/tests/y_examples.rs`. Run it after any change to an example's Y or Rust; never edit `bare/` by hand. It prints `uncovered: [] unknown: []` when every path that needs a test is claimed by one |
| [`regen.py`](regen.py) | Rebuilds the `.brs` bodies and constraint files from `bare-rust strip`, carrying each function's clauses over and re-pointing them at their lines in the Y. A function that is new has no clauses: pass them as JSON in the third argument. It prints the functions left without any |
| [`pathfacts.py`](pathfacts.py) | Rewrites `bare/y-facts.json`: the decisions (loops and `compare` among them), every path of the module and how it ends, and the path each test claims; the other facts carry over, each place moved to the line that now has its text |
| [`yp.py`](yp.py) | The path enumerator, with its prover of facts. `python3 tools/paths/yp.py examples/nim/y/src/nim.y all` lists the paths of a module; the module docstring states the rules. `tools/bare-rust/tests/y_paths/mod.rs` is a port of it to Rust, which the checks use: change both together, and keep their output the same |
| [`trace.py`](trace.py) | `python3 tools/paths/trace.py examples/<example> <module>`: runs the example's tests with every decision of the module instrumented (by [`ytrace.rs`](ytrace.rs)), and reports a test that does not take the path it claims, a path no test takes or claims, and a path taken that the enumerator does not know. The checks in `y_examples.rs` only see what the tests claim; this sees what they do |

`regen.py` and `pathfacts.py` find where a line has moved by comparing the files with git's `HEAD`, so run them on
uncommitted edits, as `rebare.sh` does, and commit the result with the edits.

`trace.py` works on a copy of the example in `$YTRACE_WORK` (by default `y-trace` in the system's temporary folder) and
keeps its cargo target folder there between runs. It exits with status 1 if it reports anything.
