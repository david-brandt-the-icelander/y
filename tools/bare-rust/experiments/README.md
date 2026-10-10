# Experiments E4 and E5: the agent harness

E4: 20 tasks x 2 arms (`rust`, `bare`) x 2 trials = 80 runs. E5: 10 of those tasks x 2 variants (`bare-nolib`,
`bare-nounexpressed`) x 2 trials = 40 runs; E5's third column is E4's `bare` arm on the same tasks. A run is one Claude
subagent given a workspace directory containing `TASK.md`. Everything is under `tools/bare-rust/experiments/`; the
binary is `$BARE_RUST` (default: the prebuilt one in the session scratchpad), nothing here builds `tools/bare-rust`.

## Layout

- `crates/strsim`, `crates/fnv`: the pinned corpora at HEAD with the in-source unit tests removed (`#[cfg(test)] mod`),
  `benches/`, CI files. These are the *snapshots* every task starts from. (In-source tests are removed so that the
  workspace is small and nothing but the hidden tests judges a change; `strsim`'s `tests/lib.rs` stays.)
- `tasks/<id>/`: `task.json` (crate, kind `body`|`signature`, `e5`), `STATEMENT.md` (the issue text the agent sees),
  `hidden.rs` (the hidden tests; graded as `tests/hidden.rs`), `base.patch` (snapshot -> base) and `ref.patch`
  (snapshot -> reference solution; empty = the snapshot itself). `tasks/_author.py` regenerates all of it from the specs
  in that file. Bases are mostly history-derived (a behaviour or feature of upstream's history taken out of HEAD);
  `generic-normalized-levenshtein` is an invented addition. Expected values in hidden tests are textbook values or
  upstream's own test values, and the reference passes them.
- `templates/`: `TASK.md` templates (`rust.md`, `bare.md`, `bare-nolib.md`, `bare-nounexpressed.md`, `bare-castlocal.md`, with
  `format-castlocal.md`) and the `weave.sh`
  given to bare workspaces.
- `prepare.sh`, `grade.sh` (-> `grade.py`), `record.py`, `summarize.py`, `make_matrix.py`, `verify_tasks.sh`,
  `smoke.sh`, `common.sh`; `matrix.tsv`; `results/`; `verify.txt` (last output of `verify_tasks.sh`).

## Tasks

| id | crate | kind | E5 | what |
|---|---|---|---|---|
| jw-low-similarity | strsim | body | | Jaro-Winkler boosts only when Jaro > 0.7 |
| jw-prefix-cap | strsim | body | | Jaro-Winkler prefix bonus capped at 4 |
| jaro-transpositions | strsim | body | | Jaro halves the transposition count |
| jaro-single-char | strsim | body | E5 | Jaro panics (overflow) on length-1 strings |
| osa-transposition | strsim | body | | OSA transposition costs 1 |
| ndl-empty | strsim | body | | normalized Damerau-Levenshtein of two empty strings is NaN |
| sorensen-whitespace | strsim | body | E5 | Sorensen-Dice ignores whitespace |
| sorensen-repeats | strsim | body | E5 | Sorensen-Dice counts repeated bigrams as a multiset |
| hamming-lengths | strsim | body | E5 | Hamming errors on unequal lengths |
| levenshtein-first-empty | strsim | body | E5 | Levenshtein with an empty first argument |
| nlev-multibyte | strsim | body | E5 | normalized Levenshtein divides by chars, not bytes |
| fnv-write-order | fnv | body | E5 | `FnvHasher::write` is FNV-1, must be FNV-1a |
| fnv-clone | fnv | signature | E5 | `FnvHasher: Clone` |
| fnv-const-hash | fnv | signature | | new `const fn fnv_hash` |
| fnv-aliases | fnv | signature | E5 | new `FnvHashMap`, `FnvHashSet`, `FnvBuildHasher` aliases |
| sorensen-dice | strsim | signature | E5 | new `sorensen_dice` (bigrams, `HashMap`) |
| ndl-add | strsim | signature | | new `normalized_damerau_levenshtein` |
| generic-hamming | strsim | signature | | new generic `generic_hamming` (hamming delegates to it) |
| error-traits | strsim | signature | | `Display` and `Error` for `StrSimError` |
| generic-normalized-levenshtein | strsim | signature | | new generic `generic_normalized_levenshtein` |

12 body-only, 8 signature-changing (all additions of a public item or trait impl, so the base fails to compile against
the hidden tests; none changes an existing item's signature). The E5 tasks are the ones that touch hashing, chars, iterators,
`HashMap`, `Result` and `Clone`, which the library file speaks about.

## Commands

    experiments/prepare.sh <task> <arm> <trial> <outdir>     # arms: rust bare bare-nolib bare-nounexpressed bare-castlocal; outdir must be empty
    experiments/grade.sh   <task> <arm> <workspace>          # one JSON line
    experiments/record.py  <run_id> <workspace> --agent-file <final-message.txt> --input N --output N [--total N] [--duration S]
    experiments/summarize.py                                 # Markdown tables from results/*.json
    experiments/verify_tasks.sh [task...]                    # base fails, reference passes, strip ok, reference weaves back
    experiments/smoke.sh [task...]                           # apply the reference through each arm's workspace and grade

Arms. `rust`: the base crate (`.rs` files, `Cargo.toml`, `tests/`), no hidden tests, `TASK.md`. `bare`: only `.brs` and
`.constraints.json` (from `bare-rust strip` of the base), `Cargo.toml` and other non-`.rs` files, `weave.sh`,
`libraries/std-1.97.0.constraints.json` (full, 264 KB), `TASK.md`. `bare-nolib`: as `bare` without `libraries/`, and a
`TASK.md` that says there is none. `bare-nounexpressed`: as `bare` but every clause of kind `unexpressed` removed from
the library file (jq `walk`), and a `TASK.md` that does not mention such clauses. `bare-castlocal`: the arm of the
cast comparison (the `cast-*` runs of `matrix.tsv`), which ran with `FORMAT.md`'s section on casts replaced by
`templates/format-castlocal.md` (casts written as `let`s whose entry is `{"type": T, "cast": true}`). That form is now the
default, in `strip` and in `FORMAT.md`, so the arm is now the same as `bare`. `LIBFILE=...` overrides the library file.
`weave.sh [check|test]` weaves into `.weave/` and runs cargo there; agents may add plain test files in `extra-tests/*.rs`.

## Grader output

`compiles` (the lib builds; bare: after `weave`), `weaves` (bare), `hidden_compiled`, `tests_passed`/`tests_failed`/
`tests_total`, `pass` (all hidden tests pass), `existing_tests_ok` (the crate's own `tests/lib.rs`), `json_entries_changed`
(constraint-file entries changed against the stripped base: subjects/top-level keys added, removed or modified, lists keyed by
`name` or `kind`), `json_leaf_edits`, `json_files_changed`, `lines_changed` (added+removed lines: of `.brs` files for bare
arms, `.rs` for the rust arm), `rs_files_in_bare` (any `.rs` an agent left in a bare workspace; ignored by the grade), and
`compile_error`/`hidden_error`/`weave_summary` text. Grading runs in a temp dir with its own target dir.

## Results

For each row of `matrix.tsv` (`run_id task arm trial`; run ids start `e4-` or `e5-`): `prepare.sh`, launch the agent in the
workspace with `TASK.md`, then `record.py <run_id> <workspace> --agent-file <final message> --input ... --output ...`. It
writes `results/<run_id>.json`: grader fields + `agent` (the self-report line: `done`, `attempts`, `json_edits`,
`library_promises_relied_on`, `notes`; null if absent) + `tokens` + `duration_s` + `model`. `summarize.py` prints: E4 by
arm, by arm and kind, per task; E5 variants with deltas against the full library, by kind and per task; library promises
cited (per arm, distinct ids, citations of entries that carry `unexpressed` clauses, most cited).
