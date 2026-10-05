# Stage 2, part B: the library file for `std`, by hand

`libraries/std-1.97.0.constraints.json` has a subject for each of the 105 external items of strsim, fnv and the golden
crate (49 functions, 25 types, 19 traits, 11 macros, 1 const). `bare-rust libcheck` passes for all three: no item
missing, schema ok, 0 signatures mismatched. Strict library coverage is now the default of `bare-rust check` (opt out with
`--allow-missing-libraries`) and is asserted in `tests/corpora.rs`. The counts below are the output of
`bare-rust libstats libraries/std-1.97.0.constraints.json`, which reads the file and compiles its signatures.

## What the file holds

| Entries | Count | | Clauses | Count |
| --- | ---: | --- | --- | ---: |
| function (22 are trait methods) | 49 | | `ensure` | 47 |
| type | 25 | | `effects` | 26 |
| trait | 19 | | `result must be used` | 18 |
| macro | 11 | | `never panics` | 14 |
| const | 1 | | `law` | 6 |
| | | | `fails panic when` | 5 |
| | | | `callback` | 3 |
| | | | `unexpressed` | 184 |

303 clauses in all: 119 stated and 184 `unexpressed`. No `require` or `fails … when` was needed, because `std` documents
what panics, never a precondition. Stated clauses by status: documentation 51, observed in a body 45, `#[must_use]`
lifted 22, hand-written 1 (a clause of part A). Every subject has at least one `unexpressed` clause.

**Signatures**, compiled against rustc 1.97.0: 96 checked, 5 path-resolved only (`PartialEq`, `From`, `TryFrom`, `Index`,
`IndexMut`: a trait with generics), 4 unchecked (the bang macros `assert_eq!`, `panic!`, `vec!`, `write!`), 0 mismatched.
By kind: functions 49, types 25 and consts 1 checked; traits 14 checked and 5 path-resolved; macros 7 derives checked.
Of 137 type slots, 118 map to the type language (86%) and 19 are host text: lifetime argument 8, slice 5, associated type 5, never type 1.

## What the language lacked: the `unexpressed` promises (184)

Each reason in the file starts with its label, so these are counts of labels.

- **Types, traits and impls (84).** No impl or associated type 36; call resolves to an unnamed impl 9; must-use on a type 13;
  no quantifier over fields (derives) 7; promise conditional on a bound's impl 11; requirement on bound implementations 8.
- **Sequences and collections (24).** No sequence value 14; no identity of moved values 6; no frame condition on a
  collection 3; no tuple parts 1.
- **Hosts and effects (39).** No host state (capacity, hasher, formatter) 10; no order of calls 6; no value of a callback
  result 6; callback clause has no condition 2; no positive effect 3; no drop 3; no resource exhaustion 3; no unwinding
  behaviour 4; no build profile (overflow checks) 2.
- **Text, numbers and constants (30).** No const-ness 18; no formatted text 9; no sign of zero or NaN payload 1; no
  constant value 1; no expected or amortised cost 1.
- **Macros and the file format (7).** No macro arguments (per-arm signature) 4; no `budget` or `layout` clause kind 3.

Two findings reach beyond the language. The externals lists name a trait method, never the impl a call resolved to, so a
promise that depends on the impl (`try_from` on integers, `into_iter` on a `Vec`) has no subject. And the file's clause
kinds omit `budget` and `layout`, which language § 5 has.

## How the entries were written

By an agent, from the declarations and documentation in `core`, `alloc` and `std` at tag 1.97.0, fetched from
`raw.githubusercontent.com`. Signatures follow § 4.4. Every `source` is a line of that tag with a verbatim doc quote, a
code snippet, an attribute or a list of impls, and was checked against the fetched file as the entry was written (quotes
by exact match, line numbers from the match). Documented promises are `assumed … documentation`; guards and effects
found in a body are `observed` or `lifted`. Part A's four exemplars were re-cited the same way, their reasons relabelled,
and `result must be used` added to `cmp::max`. The second agent's check against the documentation (check 5, human half)
has not run.

Conventions the clauses assume, which language 15 does not state: a `ref` parameter names its referent (`x == old(y)`);
`count(it)` of an iterator is the number of items it yields from here; `self[k]` on a map is its optional lookup; laws on a
trait call its methods by path (`std::cmp::PartialEq::eq(a, b)`); `Option` and `Result` are matched as variants.

## Changes to part A's tooling

Two small fixes were needed to meet "libcheck passes": `#[derive(Eq)]` and `#[derive(Copy)]` are generated beside
`PartialEq` and `Clone`, which they need to compile (they were false mismatches); and each scratch crate now has a name of
its own, because cargo's fingerprint ignores the directory and parallel tests saw another test's build as fresh (false
clean). New: `bare-rust libstats`.
