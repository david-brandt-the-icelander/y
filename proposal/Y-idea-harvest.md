# Y idea harvest, 2019–2026

**Status:** Research record
**Date:** 2026-09-24
**Written for:** the language owner, and anyone who wants to know where an idea in [`Y-proposal.md`](Y-proposal.md) came
from, or what was left behind and why.

**How this was gathered.**

- Ten repositories and working folders under `~/dev` hold attempts at this language. They were read in full where
  they are design documents, and sampled where they are implementation.
- Chronology comes from git commit timestamps. Where history was lost in a move (the 2024 YAPL design discussions),
  file dates and dates inside the text were used instead.
- The May 2026 salvage in `~/dev/padresmurfa@gmail.com/y26/y/` served as an index to the y26 material, but every date
  below comes from the original repository.
- Other folders under `~/dev` that turned out not to be language work are listed at the end.

Verdicts in the ledger:

- **Adopted:** in the proposal essentially as it was.
- **Adapted:** in the proposal, changed.
- **Deferred:** kept for after v1.
- **Dropped:** left out, with the reason.
- **Superseded:** overtaken by a later iteration's decision, which the proposal keeps.

Section numbers in the ledger's last column (`§ 3.5`, …) point into [`Y-proposal.md`](Y-proposal.md). References to this
document's own sections say "above" or "below".

---

## 1. Timeline

| When | Where (under `~/dev/`) | What it was |
| --- | --- | --- |
| 2019-09 → 2021-04 | `padresmurfa@gmail.com/y26/yapl` — README, aspirational chapters, `v0`, `v1` | *Yet Another Programming Language*: "a language that I have been dreaming of creating for quite some time". A transpiler to "all machines", written in Python |
| 2021-04 → 2022-08 | `yapl` — `v2`, `v3`, `v4` | The "hungry rabbit" walkthrough; semantic comments; refinement types; indentation-only blocks; then colon blocks, `--` comments and named conditions |
| 2022-08 | `yapl` — `v5` (Rust), `v6` (Python) | Parsers organised around modules, classes and facets; a "Why?" attached to every rule; effects declared in the signature |
| 2023-11 → 2023-12 | `yapl` — `v7` (JavaScript), `v8` (C++) | A grammar in a day, then a C++ lexer and parser with call-site error handling |
| 2024-09 → 2024-10 | `yapl/.old/design_discussions` (152 ChatGPT transcripts) | Every construct designed chapter by chapter. The language is renamed **Y** in discussion 77 ("allows for a very nice tool command (y build)") |
| 2024-10 → 2024-11 | `yapl` — `lexical_analyzer` (v9) | C++20 lexer in namespace `y::`, the first code to use the name; its reserved-word list is the consolidated 2024 vocabulary. `v10` is empty |
| 2024-11 → 2025-03 | `y26/string`, `y26/older_string_versions` | A C++ Unicode string library, `ystring`/`yrune`/…: the would-be runtime of Y |
| 2025-10 → 2025-12 | `y26/y-language-toolchain` (1,502 commits, about half merged from Codex) | The YAPL reference imported, split into chapters and hardened in October. November and December went into runtime platform work: MVCC, determinism, observability, security, builds |
| 2025-10 | `padresmurfa@gmail.com/style_guides/language_reference.md` | A fragment: variables carry affinity and placement |
| 2025-10 → 2026-03 | `y26/string` ("v1", then "v2") | C and C++ runtime foundations, a lexer built through test-driven agent prompts, coreutils-style tools. Both versions were abandoned (January, March) and deleted in April |
| 2026-01 | `y26/ypm` | A package-manager repository with agent rules and empty vision documents. No language content |
| 2026-03-05 → 03-10 | `padresmurfa@gmail.com/ai.lang` | An AI-native *specification* language: a five-layer organisational graph carried in Markdown, plus a deterministic plugin pipeline |
| 2026-03-21 → 03-24 | `y26/yx/y-string` | A C++ core library (text, memory, context, collections) whose agent directives reversed several spec positions |
| 2026-04-21 → 04-26 | `y26/string/claude.ai` | The language reference, lifted out of v2 and reworked "for first-class AI participation": spec blocks, contracts, a Go lexer after a five-language bake-off |
| 2026-05 | `y26/y` | A salvage: dated snapshots of all y26 language material, deliberately not reconciled |
| 2026-07-05 → 07-13 | `kisi@zazzcat.com/yet-another-programming-language-2026.07.05` | A C spike: MVCC transactional memory, and a transactional blob, packed array and string built on it |
| 2026-09-17 → 09-18 | `kisi@zazzcat.com/y` | The current design: a working design document, the conversation it came from (`riff.md`), and a documentation corpus under the knowledge-driven-development convention |

## 2. What every iteration believed

Some ideas recur so consistently that they are better read as the author's convictions than as design choices. A
proposal that dropped any of them would not be Y.

- **Name everything.** Conditions, else-branches, switch cases, loops, blocks, return values, assertions, constraints,
  lambdas and transactions.
  - YAPL, 2024: "part of yapl's philosophy of naming everything".
  - Toolchain, 2025: "the using block gains a name, which is always a priority in Y".
  - Held until April 2026. Absent from the September design, which came out of a conversation that started from Swift
    and Rust syntax.
  - Restored in the proposal (§ 3.5), where names also become stable identities.
- **Explicit over implicit, even at the cost of words.** No truthiness, no shadowing, no reliance on precedence, no
  implicit conversion, no abbreviations, mutation marked at both ends of a call.
  - YAPL: "We don't want to make the code reader have to ponder the meaning of an expression."
  - The toolchain era's idiosyncrasies appendix listed the cost of each rule honestly.
- **Friction by name.** Operations that might surprise get long names that make a reader stop:
  - `contains_reference_to`, `to_integer_truncated`, `is_standard_ascii`;
  - `divided_by_rounded_half_even` instead of `/`;
  - `get`/`set` keywords "so that the caller realises that this is not necessarily a simple atomic operation".
- **Transactions are the concurrency model.**
  - YAPL, 2024: fibers inside transactions; "logs and metrics emitted only on commit".
  - Toolchain, 2025: MVCC for all mutation.
  - July 2026: an MVCC implementation in C.
  - September 2026: STM with no external effects inside.
  - The scope narrowed each time; the conviction did not.
- **Authority comes from a context, never from a global.**
  - YAPL v4 notes (2022): "yapl has no globals. it has context and capabilities and sandboxes".
  - YAPL discussion 56 (2024): "There will be no mechanism in YAPL to allow code to access any such operating system
    concepts except … through these context variables."
  - The toolchain added package capability manifests.
  - March 2026's directives banned global and thread-local state outright.
  - September made capabilities first-class values.
- **Tests are part of the language.**
  - YAPL's 2020 testing chapter already required mocks to be declared on the function, dependencies to come with
    capability contracts, untested code to be kept out of production builds, and a new version to reproduce the
    recorded behaviour of the old one.
  - 2024 had `test` blocks and the rule that "if a callable is not tested … this is considered an error by default in
    production builds".
  - October 2025 added a sectioned test DSL and `exercised` branch assertions.
  - April 2026 added "a test contributes to the coverage of only what it declares it covers".
- **Observability is the language's job.**
  - Tracebacks with every non-sensitive local.
  - Facet names as metric namespaces.
  - `log` and `metrics` as statements.
  - Telemetry buffered until commit.
- **Determinism.** Time and randomness only from explicit sources.
  - YAPL MVCC used sequence numbers, not wall clock.
  - Toolchain, November 2025: "Predictable Worlds".
  - ai.lang: plugins receive `entry_time` and `random_seed`.
  - September: replay.
- **One canonical form.**
  - Formatter-enforced layout and sorted imports.
  - One public construct per module.
  - One construction path.
  - "Tools should normalize metadata to avoid unnecessary diffs" (ai.lang).
- **Agents do the work; people set the rules.** From October 2025 onward almost every iteration was built by agents:
  - Codex PRs from step-wise prompts ("In THIS run, perform ONLY Step K");
  - scoped `AGENTS.md` files;
  - per-change changelogs naming the human author and the AI assistant;
  - directives such as "Agents must never ignore an error code".

  The language itself was pitched at human teams until April 2026, when the thesis became "Y was conceived in an era
  when AI assistants are first-class participants in the coding process." September made agents the primary author.

**One recurring weakness.** Scope outran consistency in every iteration:

- 152 transcripts produced a specification with unreconciled contradictions.
- The toolchain reference grew past a megabyte while agent-written examples ignored it.
- ai.lang's specification and its tooling diverged within a week.

§ 4 below and § 9 of the proposal are about not doing that again.

## 3. The ledger, iteration by iteration

### 3.1 YAPL (2019 – 2024)

**Where it started.** YAPL began in 2020 as a language to transpile to "*all machines*" (`aspirational/00`), and its
introduction states the thesis that survives every later version: "YAPL is not intended to be brief, but rather
readable and maintainable", with a grammar that is "explicit and regular, allowing for easy analysis by automatic
tools".

**How it reached 2024.** Ten numbered versions tried a Python transpiler, Rust, Python again, JavaScript and C++. The
author changed his mind in both directions on several basic questions:

- **Blocks:** braces *and* indentation (2020), then indentation only (v3), then colon blocks (v4 onward).
- **Comments:** `//`, then semantic `//` beside plain `#`, then `--`, chosen because it is "rarely used by other
  languages".
- **Mutability:** immutable by default, then mutable by default ("mutability leaks through a code-base like async
  keywords", v5), then immutable again.
- **Memory:** single ownership with explicit moves (2020), then garbage collection, then full reference tracking (2024).
- **Errors:** checked exceptions (v0), then "how about the rust model" (v4, 2022), then declared `raises` with call-site
  handling (v6, v8), then `try`/`catch`.

The September design's ownership and `T ! E` are therefore a return to YAPL's own early instincts, not an import from
outside.

**2024.** The 152 design discussions of September–October 2024 are the richest single source of ideas in the whole
history, and the origin of most of the convictions in § 2 above. The v9 lexer's reserved-word list is their consolidated
vocabulary.

| Idea | First seen | Verdict | Where / why |
| --- | --- | --- | --- |
| **Conditions are named**: `if validation_successful = is_valid and is_complete:`; `else validation_failed:`; named `case`s | 2022 (v4 examples); 2024 (discussion 32) | Adopted | § 3.5 — extended to every predicate; `else` naming optional |
| **Named return values**; "There is no tuple in Y"; the only return form is `return c = a + b` | 2020 (v0); 2024 (32, 109) | Adapted | § 3.3 — no tuples; several results are a record |
| Named arguments required beyond two parameters | 2024 (32) | Adapted | § 3.1 — also required when two parameters share a type |
| A smart editor may *display* positional arguments while "the underlying file will still maintain the named parameter form" | 2024 (94) | Adapted | § 4.1, § 4.5 — one canonical file, many views |
| Lambdas must be named — "anonymous lambdas are a royal pain when debugging or for observability" | 2020 (99xx); 2024 (77) | Adapted | § 3.1 — single-expression closures stay anonymous; anything longer is a named local function |
| Explicit captures (`x is captured y`); a grandchild captures only what its parent captured | 2020 (v0 `closure … over (…)`); 2024 (77, 80) | Adapted | § 3.1 `captures` on local functions |
| No shadowing; mandatory initialisation "regardless of whether the initial value is considered obvious" | 2024 (81, 106) | Adopted | § 3.1 |
| Type inference stops at the file boundary | 2024 (81, 106) | Adapted | § 1.2 — public signatures are never inferred |
| "Y deliberately avoids relying on operator precedence"; parentheses when mixing operator kinds; no chained comparisons | 2020 (`aspirational/02`); 2024 (70, 89, 146) | Adopted | § 3.1 |
| Integer `/` undefined; a division method per rounding mode; `_exact` fails on loss | 2024 (20, 21, 89) | Adopted | § 3.1 |
| Lossy conversions named as lossy ("`to_integer` is a lossy operation, so lets not have it have the innocent looking name") | 2024 (20) | Adopted | § 3.1 |
| `nada` has no truth value | 2024 (13) | Adopted | § 3.3 — no truthiness |
| Interpolation accepts only a variable or a property access | 2024 (22, 87) | Adopted | § 3.1 |
| Constraints create subtypes; named `where` constraints; violation reports "generated programmatically from the rule", with subexpression values | 2021 (v2); 2024 (20, 57, 71) | Adopted | § 3.3 refined types |
| Named assertions: `assert bonus_cannot_exceed_total_salary = …` | 2024 (126) | Adopted | § 3.5 |
| Parameters read-only unless `mutable`, which is required at the call site too: `foo(mutable o)` | 2024 (149) | Adapted | § 3.6 — `&mut x`, `move x` at every call |
| `clone()` is always deep and always explicit; "YAPL should favour correctness over performance" | 2024 (14) | Adopted | § 3.6 — no hidden clone |
| **OS access only through context properties** (`file_system_library`, `socket_library`, …); packages declare the categories they need and the developer approves them | 2022 (v4: "yapl has no globals. it has context and capabilities and sandboxes"); 2024 (56, 146) | Adopted | § 3.7 capabilities and package manifests |
| `context.drop(…)` disables a capability even for third-party code below | 2024 (55) | Adapted | § 3.7 attenuation |
| Context values injected by declaring them in the signature, so "the compiler … track[s] dependencies" and tests can mock them | 2024 (54, 77) | Adapted | § 3.7 — the `with` clause |
| `using` contexts as a statically checked dynamic scope; a per-fiber context stack | 2024 (22, 79, 99) | Dropped | Action at a distance; replaced by capabilities (§ 7) |
| MVCC timestamps from a sequence counter, not the wall clock | 2024 (50) | Adopted | § 4.7 determinism |
| Transactions as named blocks with an operator message; hierarchical transaction IDs; fibers run in child transactions; "logs and metrics emitted only on commit" | 2024 (51, 52, 149) | Adapted | § 3.9 named `atomic`; § 3.7 buffered telemetry |
| "I'd rather put the load of 'coloring' the functions on the caller than the callee" | 2024 (63) | Adapted | § 3.7 — no colour on either side |
| Fibers awaited automatically when their parent transaction ends | 2024 (63) | Adapted | § 3.8 structured tasks |
| `test` blocks with `given` / `then`, mocks, each test in a rolled-back transaction | 2024 (151) | Adapted | § 3.14 |
| "If a callable is not tested … this is considered an error by default in production builds"; an unreached branch is a test failure | 2020 (98xx: "should not link code that isn't tested into a production app"); 2024 (151) | Adopted | § 3.14 release coverage gate |
| Test file path mirrors source path (`src/foo/bar/blat.y` → `tests/foo/bar/blat.yt`) | 2021 (v3); 2024 (151) | Adopted | § 3.14 |
| Tests assert on any local variable of the function under test | 2024 (151) | Dropped | Couples tests to implementation (§ 7) |
| Tracebacks with every non-sensitive local, the context, transactions begun and committed, the build's git hash and a unique ID; a 12-word encoding of that ID for users to read out | 2024 (49) | Adapted | § 3.4 fault reports |
| Catching an exception that no reachable code can raise is a compile error | 2024 (49) | Adapted | § 3.5 — unreachable `match` arms are errors |
| Module path = directory path; no `..` imports; no wildcards; cycles reported with the whole cycle; import order enforced | 2024 (100, 124, 148) | Adopted | § 3.1 |
| No free code at module level; deterministic static initialisation order | 2024 (22, 72) | Adapted | § 3.1 — no module initialisation at all |
| Callable length: lint warning at 30 lines, error at 60 | 2024 (80, 113) | Adapted | § 3.1 — 60 and 120 lines, to be calibrated (§ 8) |
| "Official YAPL linting rules, which cannot be suppressed" | 2024 (11) | Adopted | § 3.15 — for a small set of lints |
| Dependency contracts, including capability contracts ("a dependency cannot e.g. access the file system or the network, if your contract doesn't allow it"); consumer-driven tests run on every update | 2020 (98xx); 2024 (11) | Adapted | § 3.7 — dependencies declare their authority; contract tests deferred to package tooling |
| Literals that are small languages (format strings, resource paths, translation IDs, plural forms) checked at compile time | 2024 (23–28) | Adapted | § 3.1 checked literals; § 8 user-facing text |
| Translatable literals `T"Hello $name!"` with plural completeness per locale | 2024 (23, 28) | Deferred | § 8 |
| A rune is a grapheme; strings immutable; substrings share storage; bytes become text only through an explicit encoding | 2024 (17, 19) | Adopted | § 3.3 |
| `typed_decimal` for units — "way to much work … we're removing typed_decimal entirely"; currency kept, with explicit exchange rates | 2024 (26, 28) | Adapted | § 3.3 — units return through the type system; currency is a dimension |
| Separate methods instead of boolean flags (`date_to_long_string`, not `long_format: true`) | 2024 (26) | Adopted | Standard-library style |
| Enumeration keys always qualified (`color.red`) | 2024 (30) | Adapted | § 3.3 — `.Case` where the type is certain, qualified where not, never a guess |
| Classes with facets; factory methods instead of constructors; `get`/`set` properties; singletons; opt-in overriding | 2024 (12, 33, 103, 147, 150) | Dropped | § 7 — too many kinds |
| `=` to bind and `:=` to rebind, "so the operator's visual weight discourages mutation" | 2024 (32) | Dropped | § 7 — `let` / `var` |
| Deterministic reference tracking; "any object that contains a resource is itself a resource" | 2024 (22, 149) | Adapted | § 3.6 — ownership; a scope owning something with fallible release carries that failure |
| Mocks must be declared on the function they replace (`mockable(os.path.exists)`) | 2020 (98xx) | Adapted | § 3.14 — the only things a test can replace are capabilities, and those are declared in `with` |
| A new version must reproduce the recorded inputs and outputs of the live one before it ships | 2020 (98xx) | Deferred | § 5 — shadow execution and replay; six years before the September riff |
| Functions carrying performance targets that emit metrics and alarms | 2020 (99xx) | Deferred | A natural addition to compatibility signatures |
| Returning a coroutine without awaiting it is illegal; chained promise pipelines banned | 2020 (99xx) | Superseded | § 3.8 — no futures or promises at all |
| Exactly one owner, traceable to a frame; resources split into handle and open state (`File` / `OpenFile`); mutable objects confined to the narrowest execution context at compile time | 2020 (`aspirational/04`) | Adopted | § 3.6, § 3.9 |
| Leaving a resource undisposed is a compile error | 2024 (02) | Adopted | § 3.6 — ownership guarantees release |
| Declared dependencies with `dependency (user: …)` and `inject … into { }` | 2020 (v0) | Adapted | § 3.7 — the `with` clause, resolved lexically |
| Deployables as constructs: `process { }`, `service { }` | 2020 (v0) | Dropped | An entry point with supervised scopes (§ 3.8) covers it |
| Method sections `input:` / `code:` / `output:` / `postcondition:` | 2022 (v4) | Adapted | § 3.11 `requires` / `ensures` |
| Effects in the signature: `emits`, `raises`, `returns`, handled at the call site | 2022 (v6, v8) | Adapted | § 3.4, § 3.7 |
| A "Why?" attached to every rule | 2022 (v5) | Adopted | § 4.2 — every diagnostic says why its rule exists |
| Semantic comments and `##` trace directives that produce readable execution traces | 2021 (v2) | Dropped | § 7 — comments with runtime meaning; `log` does the job |
| Prepositional parameter names (`in_garden`, `using_initial_state`) so calls read as English | 2021 (v3) | Dropped | Named arguments carry the meaning |
| Comments the compiler may reject "if [they] do not make sense to a LLM"; names checked against a `.dictionary` glossary | 2024 (01) | Adapted | § 3.1 glossary lint. An LLM consistency check can be an advisory review signal only, never a build gate, because AI stays outside the trusted core |
| `sensitive` values never logged; `.redacted` message lists; separate user and operator messages | 2020 (02); 2024 (01) | Adapted | § 3.3 `Sensitive<T>` |
| No macros; templates expand into readable concrete source | 2024 (01) | Adopted | § 6 — no macros in v1 |
| Pattern matching rejected because it "basically requires unpacking of tuples … they can get ugly quite fast" | 2024 (01, 41) | Superseded | § 3.3 — `match` over sum types whose payloads have named fields; the objection was to tuples, and Y has none |
| Union types rejected in favour of overloading | 2024 (01) | Dropped | Y has closed sum types and no overloading |
| Importing a package no trusted auditor has reviewed is a compile error; namespace authority proven by DNS; checksum locks | 2024 (05–10) | Deferred | Package tooling |
| Each construct specified as tokens → syntax tree → annotated tree → exact error messages; sugar documented with its expansion | 2024 (134–143) | Adopted | § 4.3 `y explain`; § 9 examples with exact diagnostics |
| `otherwise:` on loops, a runtime error when missing and the loop never runs | 2024 (45, 145) | Dropped | A runtime surprise for a static concern |
| "Cancel the idea entirely that YAPL may one day support traditional multi-threading" | 2024 (02) | Superseded | § 3.8 — where tasks run is the runtime's choice |
| "As we are no longer aiming for transpilation" | 2024 (79) | Superseded | September made Rust a backend and oracle, not a transpilation target |

### 3.2 The string library (November 2024 – March 2025, October 2025)

`y26/string` began as a Bazel workspace of C++ libraries (`ystring`, `yrune`, `ynumbers`, `ymemory`, `yalloc`,
`yobservability`, `ylocale`), the runtime Y would stand on. Its language-relevant contributions were small but durable:

| Idea | First seen | Verdict | Where / why |
| --- | --- | --- | --- |
| Iterate text by rune; no code-unit access for library users | 2024-12-17 | Adopted | § 3.3 `Text` |
| Strongly typed lengths (`length_in_bytes_t`) | 2024-12 | Adapted | § 3.3 `Count<Byte>` — moved from naming into units |
| Tests written with `given_`, `actual_`, `expected_` prefixes | 2024-12 | Adapted | § 3.14 sections replace prefixes |
| Integer overflow raises instead of wrapping | 2025 | Adopted | § 3.1 checked arithmetic |
| Errors split into `logic_error` ("should have been prevented at coding time") and `runtime_error` ("could not have been prevented") | 2025 | Adopted | § 3.4 — faults versus failures |
| "Readable for Humans and AIs": the first explicit AI-reader principle in any iteration (C++ style guide) | 2025-10-26 | Adopted | The proposal's premise |

### 3.3 The toolchain era (October – December 2025)

`y-language-toolchain` imported the 2024 reference (16,000 lines, still saying "YAPL") on 2025-10-12, split it into
chapters, and hardened it through October. After that the work was runtime platform. About 750 of the 1,502 commits
were Codex pull requests executing step-wise briefs. `ypm` (January 2026) added no language content.

| Idea | First seen | Verdict | Where / why |
| --- | --- | --- | --- |
| Named conditions become addressable branches (`payments.invoice.calc_tax.branch.base_rate`) for tests and coverage | 2025-10-22 | Adopted | § 3.5 stable identities (`f#name`) |
| `exercised <branch>` asserted by tests | 2025-10-22 | Adapted | § 3.14 `covers`, verified by the runner |
| Sectioned test DSL: `given` … `system_under_test` … `when` … `then` … `behavior` … `observability`, with namespaced data | 2025-10-18 | Adapted | § 3.14 — five sections instead of ten |
| Fixtures generated from `where` constraints (`fixture Order(seed = 7)`) | 2025-10-22 | Adopted | § 3.3 `any<T>(seed:)`, `any_violating` |
| `unimplemented` as a scaffold-ownership handshake | 2025-10-22 | Adapted | § 3.16 `todo(...)`; ownership moves to the approvals ledger (§ 4.5) |
| `where` refinements with named constraints; structured `violations[]` | inherited, 2025-10-31 | Adopted | § 3.3 refined types |
| At most two positional arguments | 2025-10-19 | Adapted | § 3.1 — also named when two parameters share a type |
| Module = file = import path; no re-exports (`NS005`), no wildcards, no cycles | 2025-10-21 | Adopted | § 3.1 |
| One public symbol per module | 2025-10-21 | Dropped | Too strict for a type and its failure type; small files and body limits do the job |
| A missing visibility modifier is an error | 2025-10-21 | Dropped | `pub` or private is simpler; nothing is silently exported |
| Package files hold metadata only: licence, platforms, `y.security.capabilities` | 2025-10-23 | Adapted | § 3.7 package manifest lists root capabilities |
| Compiler-synthesised `Equatable` / `Hashable` / `Ordered`; user-written equality forbidden | 2025-11-14 | Adopted | § 3.3 |
| `sensitive` data: `requires sensitive_access` propagates to callers; masked in logs | 2025-10-19 | Adapted | § 3.3 `Sensitive<T>`, § 3.7 `Reveal` capability |
| Units of measure with dimensional analysis and stable diagnostic codes | 2025-10-20 | Adopted | § 3.3 |
| Telemetry buffered until commit, dropped on rollback | 2025-10-19 | Adopted | § 3.7 |
| Deterministic time and randomness from the transaction (`transaction.time`, `transaction.random`); a pass rejecting wall-clock reads | 2025-11 | Adapted | § 3.7 — through `Clock` / `Random` capabilities |
| Stable diagnostic codes; lossless trivia; canonical tree-sitter grammar driving the formatter | 2025-10 | Adopted | § 4.1, § 4.2 |
| Typed collection literals ("generics are not optional") | 2025-10-18 | Adopted | Implied by § 3.3; no untyped literals |
| Deterministic builds with fingerprints | 2025-11-26 | Adopted | § 4.7 |
| `TODO.md` backlog: preconditions, postconditions, invariants, "testing in prod by shadowing the old version" | 2025-11 | Adopted | § 3.11; shadowing in § 5 |
| Type specializers `<affinity=…, placement=…, variance=…>` | 2025-10-18 | Dropped | Runtime detail in source (§ 7) |
| Visibility sprawl: six modifiers, facets, `restricted` with allow/disallow lists | 2025-10-21 | Dropped | Too many ways to say who can see what |
| Primary–secondary VMs, GC release tickets, entropy budgets, encrypted-log grammar, `throw … capturing snapshot` | 2025-10 → 11 | Dropped | Platform and policy in grammar (§ 7) |
| "Y deliberately ships without a standard library" | 2025-10-18 | Dropped | Contradicted by the 500 KB standard library specification written alongside it |
| Single OS thread per process as a language guarantee | 2025-10-19 | Dropped | A runtime choice, not a semantic (§ 3.8) |

### 3.4 ai.lang (5–10 March 2026)

Not a programming language. It describes a whole software organisation as one typed graph:

- five layers (intent, human value, system architecture, runtime and verification, responsibility);
- about seventy node types;
- a whitelist of allowed edges.

The graph is carried in Markdown files with metadata in trailing HTML comments. A deterministic plugin pipeline
processes the files. The graph semantics were never implemented; the tooling that was implemented was syntax
validation and templating.

| Idea | Verdict | Where / why |
| --- | --- | --- |
| "Everything claimed must be provable or explicitly missing" | Adopted | § 3.16 — as a property of every build |
| Everything connects upward to purpose and downward to implementation | Adapted | § 3.2 `satisfies:` role; `y:` links resolved by the compiler |
| Every acceptance criterion has a test or an explicit gap marker | Adapted | § 3.14 coverage gate; § 3.16 `gap:` |
| Three tiers of validation: invalid, incomplete, heuristic warning | Adapted | Errors, release gates, lints |
| Provenance on inferred facts (asserted / inferred / observed, with confidence) | Adapted | § 4.3 provenance in `y explain` |
| Side effects, transactions and external calls as first-class | Adopted | Already in September; § 3.7, § 3.9, § 3.10 |
| "Every invariant must be enforced" by something | Adopted | § 3.11 invariants are checked, or demoted and reported |
| Deterministic plugins: clock and seed injected, determinism tested by re-running | Adopted | § 3.7, § 4.7 |
| "Natural language … is an authoring convenience … not the execution truth" | Adopted | Proposal premise |
| No hidden semantics: adapters visible, templates add nothing an explicit form could not | Adopted | § 3.1, no macros (§ 6) |
| Callable bodies as blocks, each with a one-line natural-language summary; zooming from intent to code | Adapted | § 3.5 named decisions; `y review --outline` |
| Test grammar for generated tests (GIVEN / MOCKING / … / BEHAVIOR; one branch per test) | Adapted | § 3.14 |
| Per-change changelog naming human author and AI assistant, base commit and evidence | Adopted | § 4.5, § 9 |
| "Tools must avoid rewriting human prose; inventing uncertain metadata" | Adopted | § 4.1 `y fix` leaves prose to the author |
| Self-describing registry: validators derive rules from the language's own documents | Adapted | § 9 every example is a conformance case |
| The five-layer ontology, seventy node types, org and on-call modelling | Dropped | Not a programming-language concern (§ 7) |
| Markdown with metadata in HTML comments as the source carrier | Dropped | Metadata far from what it annotates; a real grammar serves agents better |
| "Natural language remains the primary source of meaning" | Dropped | Contradicts the compiler being the authority |
| Twelve-level zoom ladder and "roughness" | Dropped as a model | Kept only as a review view |

### 3.5 y-string (21–24 March 2026)

The language reference in this repository was an unchanged copy of November 2025. What was new was a C++ core library
written by agents under strict `AGENTS.md` directives, and those directives reversed several positions the
specification still held.

| Idea | Verdict | Where / why |
| --- | --- | --- |
| **No global or thread-local state; everything ambient lives on a context object passed explicitly** | Adopted | § 3.7 capabilities; § 3.9 `Shared<T>` replaces module-level shared state |
| Locale is never implicit | Adopted | § 3.3 `Locale` argument |
| A rune is exactly one extended grapheme cluster, NFC-normalised; equality by canonical equivalence | Adopted | § 3.3 — this settled the specification's own contradiction (scalar value or grapheme) |
| Every length and index typed by its unit (`length_in_runes`, `index_of_rune`, …) | Adopted | § 3.3 `Count<Rune>`, `Index<Rune>` |
| Lossy conversion policy is explicit, rejecting by default | Adopted | § 3.3 `on_invalid: .Reject` |
| Ownership is a static property of a type: owned, borrowed or view | Adopted | § 3.6 |
| `borrow_*` versus `get_*` naming for views versus values | Adapted | § 3.6 — the type (`&T` result with `from`) says it instead of the name |
| `comparison_result` enum, never an integer | Adopted | § 3.3 `Ordering` |
| Canonical vocabulary: `clear` not `zero`, `clone` not `copy`, `as_` for representation changes | Adopted | Standard-library naming |
| "One canonical construction path" | Adopted | § 3.2 constructors are ordinary functions |
| Directive precedence: direct instruction, then nearest `AGENTS.md`, then parent; on conflict "follow the strictest safe rule" | Adopted | Process (§ 9) |
| Hungarian prefixes (`i_`, `o_`, `m_`, `s_`) | Dropped | C++ workarounds (§ 7) |

### 3.6 string/claude.ai (21–26 April 2026)

The language reference was lifted out of the abandoned v2 and reworked on a branch named `improve-ai-readability`. Its
thesis was the first to put AI at the centre: "To make every declaration legible to both humans and AI agents, the
language uses a single declarative form — the spec block — as the canonical source of truth for every nameable
construct … in a closed grammar."

| Idea | Verdict | Where / why |
| --- | --- | --- |
| Spec block: structured, closed-grammar documentation of every declaration | Adapted | § 3.2 — `///` with closed roles |
| The spec block *replaces* the signature | Dropped | The April examples fell back to inline signatures for helpers; agents' priors; types are read in the signature |
| Closed role vocabulary; a prose line shaped like `word:` must be a valid role | Adopted | § 3.2 (`Y0102`) |
| Imports typed and formatter-expanded ("the committed source is always precise"); per-import comments | Adapted | § 3.1 — imports name what they bring; the formatter sorts |
| Constrained CommonMark in comments; `y:` links resolved by the compiler | Adopted | § 3.2 |
| `exports:` as the only visibility lever | Dropped | `pub` is simpler |
| First-class `invariant` / `precondition` / `postcondition`; `snapshot` for pre-state; invariants checked at commit of transactions that touch their fields | Adapted | § 3.11 — `requires`, `ensures`, `old()`; record invariants checked at commit |
| Properties must be non-blocking and non-throwing | Dropped with properties | § 7 |
| `covers:` — "a test contributes to the coverage of only what it declares it covers" | Adopted | § 3.14 |
| Named conditions with named loops and blocks | Adopted | § 3.5 |
| Every `catch` ends in `rethrow`, `suppress` or `return` | Dropped with exceptions | § 3.4 |
| At most two positional arguments | Adapted | § 3.1 |
| Quoted identifiers normalised to `snake_case` for prose test names | Dropped | Test names are ordinary identifiers; the summary line carries the prose |
| Stable diagnostic codes that "must not change once committed" | Adopted | § 4.2 |
| Spec changes shipped with whole-corpus rewriting scripts | Adopted | § 9 `y migrate` |
| DIRECTIVES: 100% branch coverage with exactly one dedicated test per branch, proven by a trace log per branch | Adapted | § 3.14 — at least one declaring test per outcome, verified by the runner |
| DIRECTIVES: comments explain *why*; never reference the current task; no deprecation shims — delete | Adopted | § 3.2 `why:`; § 3.16 |
| File-wide uniform comment-delimiter width | Dropped | Diff noise (§ 7) |
| Too many declaration kinds (class, structure, interface, trait, singleton, namespace, exception *and* error, application) | Dropped | § 3.2 |

### 3.7 The July spike (5–13 July 2026) and an October 2025 fragment

`yet-another-programming-language-2026.07.05` is C. It builds the runtime substrate the language keeps asking for:

- an MVCC software transactional memory with first-committer-wins conflict detection;
- a transactional byte blob, packed array and string on top of it.

Its `CLAUDE.md` is a compact statement of the author's conventions.

| Idea | Verdict | Where / why |
| --- | --- | --- |
| MVCC STM with first-committer-wins | Adopted | § 3.9 implementation; § 5 v1 runtime |
| **Rollback on failure**: a failed operation records why, rolls the transaction back, and makes later operations safe no-ops; check once at the end | Adapted | § 3.4 — a failure leaving `atomic` aborts it |
| Out-of-memory is fatal, never a null to check | Adopted | § 3.4 faults |
| A blob grows only when its owner says so | Adopted in spirit | § 3.6 — no hidden cost, no hidden clone |
| Distinct typed handles, never `void *` | Adopted | Typed identifiers throughout (`HoldId`, `Index<Node>`) |
| Namespace = folder path; `include.h` / `implementation.c` when a name would repeat its directory | Adapted | § 3.1 module = file = path |
| Naming: `constructor`/`destructor`, `is_` for booleans, `as_` for representation changes, `get_`/`set_`; containers use `count` and `contains`, never `size`; **always name the unit** (`length_in_bytes`) | Adapted | § 3.3 — units move into types; naming rules go into the standard library |
| Single OS thread, cooperative; interleaving handled by versions, not locks | Adapted | § 3.8 — allowed for a v1 runtime, not promised by the language |
| "Agents must never ignore an error code" | Adopted | § 3.4 — failure is a value that must be matched or propagated |
| Never call a libc function that has a `y_` wrapper; add wrappers only as needed | Adopted in spirit | § 3.7 — the world is reached only through capabilities |

`style_guides/language_reference.md` (October 2025) says every variable has an affinity and a placement, with
defaults filled in by the compiler. Dropped with affinities (§ 7).

### 3.8 The September design (17–18 September 2026)

`kisi@zazzcat.com/y` has three records:

- the working design, `Y-current-language-design.md`;
- the conversation it grew from, `riff.md`, which calls the language "Rill" and has it transpile to Rust;
- a documentation corpus that restates the design under the knowledge-driven-development (KDD) convention, marks every
  statement as not yet built, and marks 23 places blocked on a decision.

The proposal keeps all fifteen settled commitments of the working design. What follows is how each of its larger ideas
fared.

| Idea | Verdict | Where / why |
| --- | --- | --- |
| "AI proposes. Deterministic tooling verifies." The compiler is the authority | Adopted | Premise |
| Two layers: pleasant source, explicit semantic layer; Core IR is the definition | Adopted | § 4.6 |
| Conventional surface syntax (Swift / Kotlin / Rust / F#); "no syntactic novelty" | Adopted | § 1.1, § 3.1 — with evidence from the 2025 drift |
| `T ! E`, `?`, derived conversion only when unambiguous | Adopted | § 3.4 — resolved as a built-in sum type |
| `T?`, `??`, `?.`; exhaustive `match`; guarded arms do not count; subsumed arms are errors | Adopted | § 3.3, § 3.5 |
| Ownership without hidden clones; `&`, `&mut`, `move`; arenas; "arena-owned is not transactional" | Adopted | § 3.6 — plus explicit call sites, `from` on returned borrows, no borrows in types |
| Effects inferred and mostly unwritten | Adapted | § 1.2 — inferred, then pinned in `with` |
| Capabilities as ordinary, attenuable, unforgeable values | Adopted | § 3.7 |
| No `async` colouring; suspension is an effect | Adopted | § 3.7 |
| Every suspension bounded; `within`; "forever" explicit and greppable | Adopted | § 3.8; `forever` is a claim |
| Structured tasks, cancellation at suspension points, detach only deliberately | Adopted | § 3.8 |
| Erlang-style supervision | Adapted | § 3.8 — a policy on a scope |
| STM: `atomic`, `retry`, `orElse`; no external effect inside a transaction; the diagnostic names the alternative | Adopted | § 3.9 — transactions named; `orelse` |
| Module-level `shared x: T` | Adapted | § 3.9 — `Shared<T>` passed as a capability |
| Durable commands; outbox in the language; idempotence, deduplication, correlation | Adopted | § 3.10 — policy made mandatory |
| `afterCommit` | Dropped | § 3.10 |
| External queries as persisted request, response and resumption | Adapted | § 3.10 — expressed as a workflow |
| Durable workflows resuming under a compatible version | Adopted | § 3.10 |
| Contracts; `old()`; the build reports contracts demoted to runtime checks | Adopted | § 3.11 — contracts named; demotions are review items |
| Typestate: `state`, `transition` consuming the source state | Adopted | § 3.11 — shared fields declared once |
| Units of measure; currency as a dimension | Adopted | § 3.3 — extended to counts and indices |
| Schemas with retained versions, migrations that are pure, reversible where declared | Adopted | § 3.12 |
| `derived`, `rule`, pipelines, receiver mode decides ownership | Adopted | § 3.13 — rules non-recursive in v1 |
| Elided-parameter closures (`.where { .age >= 18 }`) | Dropped | § 3.1 — one closure form |
| Oracles: rustc, Miri, CBMC, Frama-C, Why3, Dafny, TLA+, Apalache, Loom, Shuttle, Alive2, backend equivalence; disagreement is never a vote | Adopted | § 4.6 |
| Deterministic scheduler; injected cancellation, deadline expiry, conflicts and crashes; recorded seeds | Adopted | § 3.14 `faults`, `--explore` |
| Readable generated Rust ("Show generated Rust") | Adopted | § 4.3 |
| Compatibility signature per function, for hot swap | Adapted | § 4.4 — used first for review |
| Riff: semantic optimiser must be correct, physical optimiser may experiment | Deferred | § 5 |
| Riff: function-level hot swap, shadowing, canaries | Deferred | § 5 — foundations in v1 |
| Riff: `converge`, live migration by convergent MVCC copying; system generations | Deferred | § 5 |
| Riff: "what v1 must not foreclose" list | Adopted | § 5 table |
| KDD: the backlog lives in the documents, as ✅ ⚠️ ❌ ⛔ marks | Adapted | § 3.16 `gap:` / `defect:` / `blocked:`; § 9 marks on the specification |
| KDD: 🙈 🙉 🙊 coverage marks | Adapted | § 3.14 — computed, never written |
| KDD: a test file's path is computable from the production file's identity | Adopted | § 3.14 |
| KDD: `TODO` abolished and gated | Adopted | § 3.16 |
| KDD: every document written for one reader; a fact lives in one document | Adopted | This proposal's own header |
| The journey "an agent writes a function": if the reviewer must know something the compiler could have known, that is a defect in Y | Adopted | § 2 — the proposal's test |

## 4. How the iterations were built, and what that teaches

The ideas are only half the history. How they were produced explains why so few reached a compiler.

**Design in conversation, pasted in.**

- YAPL's 2024 specification is 152 ChatGPT transcripts.
- ai.lang's core specification arrived as one 6,691-line commit, "as discussed in the conversation", still containing
  stray glyphs from the chat.
- The September design came out of `riff.md`.

Conversation is a good way to explore. It is a poor way to keep a specification consistent. Every one of these
iterations left contradictions that nobody reconciled:

- rune as scalar value or as grapheme;
- `export` legal and illegal;
- weak references present and absent;
- `throws` checked at compile time and wrapped at runtime.

**Agents as the workforce, from late 2025.**

- The toolchain: about 750 of 1,502 commits were merged Codex PRs.
- ai.lang: 76 of 114 commits.
- The `string` v1 and v2 work ran on step-wise, test-first prompts ("In THIS run, perform ONLY Step K").

The directives that governed these agents are some of the best material in the corpus:

- the precedence of `AGENTS.md` files;
- "follow the strictest safe rule";
- per-change changelogs naming the human author and the AI assistant;
- "never ignore an error code";
- "delete, don't deprecate".

They are also the direct ancestors of this proposal's premise.

**What went wrong, repeatedly.**

- **Specifications larger than any context that had to hold them.** The toolchain reference passed a megabyte, and
  merged single-file snapshots were produced to feed it to models.
- **Drift with nothing to catch it.**
  - Agent-written examples used PascalCase, `name: type`, `let`, `$interpolation` and `#` comments, all of which the
    specification forbade.
  - The grammar's test corpus accepted parse-error nodes as expected output.
- **Scope added, never subtracted.** Each month of the toolchain added platform surface: encrypted logging, garbage
  collection tickets, entropy ledgers.
- **Churn without record.** Many commits in ai.lang and y-string are titled "foo".
- **Implementation language churn.** Python, Rust, JavaScript, C++, C-subset C++, a five-language bake-off won by Go,
  then C. The language was re-founded more often than it was implemented.
- **Abandonment by quarantine.** `string` v1 and v2 were moved to `deprecated/` with "Agents must not read or write
  anything in this directory", then deleted. The May 2026 salvage exists because of this.

**What worked, and is kept.**

- Mechanical migrations of the whole corpus when the specification changed (April 2026).
- Stable diagnostic codes.
- Marks on every statement, so that a reader knows what is real (September 2026).
- A salvage that preserved disagreements instead of flattening them (May 2026).
- Writing big documents skeleton-first, section by section, a rule in the `string` repository's `CLAUDE.md` that this
  harvest and the proposal followed.

These lessons are the basis of § 9 of the proposal: a small normative core, every example a conformance test,
migrations shipped with changes, and a record per change.

## 5. Folders surveyed and set aside

These live under `~/dev` alongside the language work, and were checked and found not to be attempts at the language:

- `skald` (a BIP39 replacement that encodes keys as poems; YAPL's 2024 idea of a 12-word error report identifier is a
  cousin);
- `meldanor`, `fortura`, `aeron`, `aeron-using-saemis-approach`;
- `the-phase-wave-hypothesis` and its worktrees;
- `festi/*`, `mtg-commander-deck-generator`, `snekkjo`, `carpooling`;
- `prompts`, `asana*`, `jemalloc`, `forfuckssake.today`, `probe_dns`.

`style_guides` contributed the one fragment noted in § 3.7 above. Its per-language unit-test guides share lineage with
the test grammars in § 3.3 and § 3.4 above.
