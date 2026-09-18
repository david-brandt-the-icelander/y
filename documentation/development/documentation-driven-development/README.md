# Documentation-Driven Development

The backlog for this system is written into the documentation that describes it, not kept beside it.
Every requirement, defect and unbuilt capability appears as an annotated line in the document a reader
would open anyway — the operator manual, the journey walkthrough, the class summary, the function's
own docstring.

## Legend

| Mark | Meaning | Where it may appear |
| --- | --- | --- |
| ✅ | **Implemented and verified.** For prose: the behaviour exists and its tests pass. For a function, type or enum: *every branch of execution is covered by a direct unit test*. | Prose documents, code comments. **Never** in an OpenAPI/Swagger description. |
| ⚠️ | **Not implemented.** The surrounding text describes the intended shape; nothing behind it works yet. This is the backlog item. | Prose documents, code comments, OpenAPI descriptions |
| ❌ | **Implemented but broken.** A known defect. The text says what it does wrong, not only that it is wrong. | Prose documents, code comments, OpenAPI descriptions |
| ⛔ | **Blocked.** The gap marked immediately before it cannot be closed from here — it waits on a decision, a third party, hardware, another initiative, or an environment nobody here controls. Never stands alone; always written straight after the ⚠️ or ❌ it qualifies, and always says what would unblock it and who can. | Prose documents, code comments. **Never** in an OpenAPI description |
| 🙈 | **No test coverage at all.** No test executes this path. | Code comments only |
| 🙉 | **No *direct* unit test.** The path executes only inside some other unit's test, an integration test or a contract test. Nothing names it. | Code comments only |
| 🙊 | **Executed but unasserted.** A test runs this path and asserts nothing about what it did. | Code comments only |

**Where the marks apply.** Documentation prose and source comments. **Never a string a user reads** —
an admin page's status icon, a log line's severity badge and a report's tick are product, chosen by
whoever designed them, and this convention has no standing over them.
→ [`02-annotation-convention.md`](02-annotation-convention.md#the-marks-are-for-readers-of-the-source-not-users-of-the-product)

**The graduation rule.** When a document contains no ⚠️, no ❌ and (for code) no 🙈 🙉 🙊, every ✅ in
it is deleted and replaced by one line: *this component is fully implemented, tested and free of known
defects; the ✅ mark is retired here.* A page covered in green ticks is a page that has
stopped carrying information — the ticks exist only to make the gaps legible, so they go when the gaps
do. → [`02-annotation-convention.md`](02-annotation-convention.md#graduation)

## The documents

| # | Document | What it is for |
| --- | --- | --- |
| 01 | [Methodology](01-methodology.md) | What documentation-driven development is here, what it costs, what it buys, and how it compares with a ticket tracker, README-driven development, spec-driven development, ADRs and literate programming |
| 02 | [Annotation convention](02-annotation-convention.md) | The canonical rules for ✅ ⚠️ ❌ in prose. Placement, granularity, ownership, graduation |
| 03 | [Code annotation convention](03-code-annotation-convention.md) | The same marks plus 🙈 🙉 🙊 in C# XML docs and inline comments; the branch-coverage meaning of ✅; the test-locatability rule |
| 04 | [API contract annotation](04-api-contract-annotation.md) | What a consumer of the OpenAPI document sees, and why ✅ never appears there |
| 05 | [Document taxonomy](05-document-taxonomy.md) | The five kinds of document — vertical feature, user journey, subsystem, class, small component — and which one a given fact belongs in |
| 06 | [Rollout plan](06-rollout-plan.md) | How the whole system gets covered: inventory, waves, sequencing, effort, and what stops the corpus rotting |
| 07 | [Enforcement](07-enforcement.md) | The seven gates — one built, six specified. Conformance tests, the coverage cross-check, the CI wiring |
| — | [templates/](templates/) | One starting file per document type |

## Status of this initiative

Everything in this folder is a specification of a convention that is, at the time of writing,
adopted nowhere in the codebase.

- ✅ The convention itself is written down, and this folder demonstrates it on itself.
- ⚠️ No production document has been converted. The inventory in
  [`06-rollout-plan.md`](06-rollout-plan.md) is derived from a directory listing, not from reading
  the 206 existing documents.
- ✅ **`TODO` is abolished and gated.** There is no `TODO/` directory and no `TODO`/`FIXME`/`HACK`/`XXX`
  marker in any source file; `DeferredWorkMarkerConformanceTests` fails the build on either. The 22
  items that were in `TODO/` are now marked sections in the nine documents that own the capabilities
  they describe.
- ⚠️ The other six gates in [`07-enforcement.md`](07-enforcement.md) are described in the present tense
  and none of them exist.
- ⚠️ Roughly 300 citations of retired `TODO/<file>.md` paths remain in the documentation as dangling
  pointers, and the gate does not sweep Markdown.
- ⚠️ The repository's own agent instructions do not mention the convention. Until
  [`AGENTS.md`](../../../AGENTS.md) routes to it, agents will not apply it.
- ✅ The vocabulary is settled. The older workspace convention inverted ⚠️ and ❌; it was replaced by
  this one on 2026-08-28 and `~/dev/festi/CLAUDE.md` now carries these definitions. Every other use of
  these glyphs in this repository — good/bad code examples, yes/no table cells, log-line severity
  badges, checklist ticks — was removed on the same date and replaced by prose.
  → [`02 § The collision, resolved`](02-annotation-convention.md#the-collision-resolved)
- ⚠️⛔ The SqlFortress SDK documentation still reads backwards, on another initiative's branch which
  this one must not mutate. **Unblocked by** whoever next picks up SqlFortress.

Worked examples elsewhere in this repository, all created on 2026-08-28 when `TODO/` was abolished:
[`Reference/tammermatic-fenix-subsystem.md`](../../Reference/tammermatic-fenix-subsystem.md) (a
subsystem document, three ❌ and one ⛔),
[`Features/cancelled-wash-refunds.md`](../../Features/cancelled-wash-refunds.md) (a feature document),
and [`Development/agent-tooling-friction.md`](../agent-tooling-friction.md) (the residue this method is
honest about being bad at). The full list is in
[`INDEX.md` § Where the gaps are recorded](../../INDEX.md#where-the-gaps-are-recorded).

There is no separate backlog file for this initiative. The lines above are the backlog.
