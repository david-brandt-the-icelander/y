# Stage 3: results and recommendation

Stage 3 of [`bare-rust.md`](../../../proposal/bare-rust.md) § 11 called for five things: errors mapped back to the
bare body and the JSON, then E2, E3, E4 and E5. All five are done, together with the small tool fixes that were still
open. The detail is in [`e2.md`](e2.md), [`e3.md`](e3.md) and [`e4-e5.md`](e4-e5.md).

## What was built

- **Error mapping** (`src/errmap.rs`).
  - Every diagnostic from `cargo check` on the woven crate is mapped to its `.brs` line and column. Where the span is
    woven-in signature text, it is also mapped to the JSON pointer of the slot that produced it
    (`/subjects/3/params/1/type`).
  - Secondary spans and undetermined holes are mapped the same way.
  - `weave <dir> --out` prints the mapped form, and `--json` gives the same information for tools. Tests inject one
    error into a constraint file and one into a body.
- **`bare-rust e2` and `bare-rust e3`,** with their results.
- **The harness for E4 and E5** in `experiments/`: 20 verified tasks, workspaces for four arms, a grader, a recorder
  and a summary.
- **Small fixes.**
  - A named lifetime in a trait argument (`impl<'a> From<&'a str> for Ref<'a>`) now round-trips.
  - Const generics are carried.
  - Item-level macro invocations are kept verbatim and counted as host text.
  - `weave` of a single-file crate now reports its count correctly.
  - `weave` now refuses a type alias that has no entry, where it used to print `type Foo;`.
  - `weave` now fails when `cargo` itself cannot start, where it used to report success. E4's first two pilot runs
    found this.
  - The three examples that had been trimmed to avoid these problems were restored.

## Results

| Experiment | Question | Answer |
| --- | --- | --- |
| E2 | Do bodies stay bare? | Yes, mostly. With every local and hint dropped, 17% of all functions need one (121 of 705). Excluding the tool's own source, it is 7% of strsim, 1 of fnv's 10 functions and 1 of the 55 example functions. Nine in ten of the holes `rustc` cannot infer are locals (89) or closure parameters (24), mostly in the tool's own code. No hole is ever filled by a guess. |
| E3 | Do constraint files survive ordinary edits? | Yes. 39 of 76 changed files changed only bodies. Each of those weaves back exactly with the old file, with no unbound entries. Most signature changes add an item, which the weaver refuses until the item has an entry, as designed. Of 13 commits compiled with the old file, 2 fail, both reported as an undetermined hole that names the clause to add. None fails with a type error. |
| E4 | Do agents do as well in bare Rust? | On these tasks, yes: 40 of 40 passed in each arm. Body tasks cost the same in both arms. Signature tasks cost about 2× the attempts and 1.5× the tokens in bare Rust, all of it spent learning how a new item is split. |
| E5 | Are the library files worth it to an agent? | Not measurably here. Only 1 of 80 runs cited a library promise. Removing the file cost 16% more tokens, and removing only its `unexpressed` clauses cost 8%, both within noise. The tasks did not turn on `std` behaviour Sonnet does not already know. |

## Recommendation: complete the tool and build a second host

Language § 11.2 holds on every count the trial could measure:

- strip and weave round-trip;
- bodies need few hints;
- constraint files survive edits;
- agents complete real changes in bare form as reliably as in Rust, at the same cost when only a body changes.

Nothing found calls for a change to the design, and nothing argues for stopping.

Three things come first, all small and cheap:

1. **Make adding an item cheap for an agent.** This is the one place where bare Rust cost agents more.
   - A parse error for `let x: T` that names the fix.
   - A `scaffold` command that prints a new item's bare stub and JSON entry.
   - A one-page format reference with an example of each kind of subject.
2. **Rerun E4 at a size that can tell the arms apart:**
   - tasks in multi-file crates;
   - tasks that change existing signatures;
   - E4 on the TypeScript host, as planned in [`second-hosts.md`](../../../proposal/second-hosts.md).

   The current task set hit a ceiling, so it cannot rank the arms by success.
3. **Do not order the language-gap work by E5.** E5 at this size gave the 52 `unexpressed` promises no priority. If
   they are reconsidered, test them on the studio's own libraries, which agents know less well than `std`, not on
   `std`.

The release order of § 11 then follows from these results:

1. The whole signature, which removes refusals.
2. The holes the trial left closed.
3. Disguised constraints.
4. The text form with E6.

Library tooling stays last. E5 gives it no case for being built sooner.

### Answers to § 13's open questions

1. **Bare extraction:** E4 gives no evidence. No task touched an `.unwrap()` on an input, so the current decision
   stands.
2. **Body hints:** they stay as positional records. E2 shows they are needed but rare outside closure-heavy code. E3
   shows that hints bound by ordinal are the one fragile binding: 2 unbound hints in 76 files.
3. **How far to look into library bodies:** not yet. E5 gives no case for the library tooling.
4. **Where library files live:** the proof of concept's answer stands.
5. **The second host:** TypeScript first, then C#, then Python, as planned in
   [`second-hosts.md`](../../../proposal/second-hosts.md).

## Cost of stage 3

| Part | Calls | Cache writes | Cache reads | Output recorded | Cost |
| --- | --- | --- | --- | --- | --- |
| Build: fixes, error mapping, E2, E3 (Sonnet) | 78 | 0.45M | 10.7M | under 0.1M | $3.3 to $4 |
| Build: E4 and E5 harness, 20 tasks (Sonnet) | 29 | 0.15M | 3.0M | under 0.1M | $1.0 to $1.3 |
| 124 experiment runs, including 4 pilots (Sonnet) | about 700 | 1.75M | 31.6M | under 0.1M | $11 to $13 |
| Orchestration (Opus) | about 120 | | | | about $5, estimated |
| **Stage 3** | | | | | **about $20 to $23** |

That is against the estimate of § 14.2: about $103, or 20% of a Max 5x week.

- **Experiment runs:** each was a seventh of the unit, because the tasks were small.
- **Build runs:** two build runs and no repair round did the work of the four build and four check runs planned.
- **No separate check run:** the orchestrating session checked the work instead. It ran the targeted tests, rebuilt
  the tool and graded every run itself. The full `cargo test` (about 13 minutes) was run by the agent
  that changed `src/`.

Output is undercounted in the transcripts, so each range allows for unseen thinking as in § 14.1. Orchestration is an
estimate from the length of this session, not a measured figure.
