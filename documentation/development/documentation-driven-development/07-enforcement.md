# 07 — Enforcement

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it. → [README](README.md)
> **One gate on this page exists** — [the deferred-work gate](#the-deferred-work-gate). The other six
> are ⚠️, and this page is the specification they are built from.

A convention maintained by attention decays at the rate attention decays. The rules worth having are
the ones a build can fail on, and the point of the vocabulary in [02](02-annotation-convention.md) and
[03](03-code-annotation-convention.md) is that most of it is mechanically checkable.

These belong in `ContractTests.*` — the repository already puts cross-cutting gates in projects
unrelated to the code they judge, and `AGENTS.md` requires every `UnitTests.*` and `ContractTests.*`
project to run on every change, so a gate placed there cannot be skipped by touching only one project.

## The coverage cross-check

⚠️ **Fails when a member's `<summary>` carries ✅ and its branch coverage is below 100%.**

The single most valuable gate, because ✅ is the mark a reader builds on and the only one that is a
claim about verification rather than about intent. Input is the Cobertura output the existing
`coverlet.runsettings` run already produces; the check is a join between that and the doc comments.

- Also fails the inverse: a member at 100% branch coverage carrying 🙈, 🙉 or 🙊. A stale monkey is a
  false alarm, and false alarms are how a reader learns to ignore marks.
- Also fails a member whose `<summary>` is ✅ while its body contains any 🙈 🙉 🙊 comment — checkable
  from source alone, with no coverage run, so it can also live in a fast pre-commit lane.
- ⚠️ Open question: whether ✅ can be *auto-applied* from a coverage report. Machine-written ✅ removes
  the "someone ran it" meaning that makes the mark worth anything, and this document's position is that
  it should not. Recorded because it will be proposed.

## The location gate

⚠️ **Fails when a production type has no test file at the path computed by
[the location rule](03-code-annotation-convention.md#the-test-location-rule), and is not exempt under
rule 5.**

Cannot be switched on repository-wide — the current corpus would fail comprehensively, and wave 0 has
not yet measured how comprehensively. It ships in two stages:

1. ⚠️ **Report-only.** Emits the count and the list; fails nothing. Gives wave 0 its number.
2. ⚠️ **Ratchet on changed files.** Fails only for types touched by the PR, the same containment
   strategy the `Test_` prefix removal uses. The baseline count may never increase.

## The Swagger scan

⚠️ **Fails when consumer-visible OpenAPI text contains ✅, or contains ⚠️/❌ without its spelled-out
gloss** (`NOT IMPLEMENTED` / `KNOWN DEFECT`) → [04](04-api-contract-annotation.md).

Scans `[SwaggerOperation]`, `[SwaggerResponse]`, schema descriptions and example providers. Cheap: a
source scan over 39 controllers, no OpenAPI generation needed.

- ⚠️ It must also fail if XML documentation ever starts flowing into the generated document
  (`IncludeXmlComments`, or `GenerateDocumentationFile` in `Directory.Build.props`) while any
  controller-action `<summary>` still carries ✅. That configuration change would turn hundreds of
  internal marks consumer-visible in one commit, silently, and it is the most likely way this rule
  breaks.
- ⚠️ It must not scan `documentation/Contracts/*.json` — hashed snapshots, never annotated
  → [04 § Contract snapshots](04-api-contract-annotation.md#contract-snapshots).

## The mark-vocabulary gate

✅ **Fails when any Markdown file under `documentation/` defines a status mark instead of linking to
this convention.** `MarkVocabularyConformanceTests` in `ContractTests.TestIsolation`. It catches the two
shapes a definition takes here — a `> Legend:` blockquote, and a table row whose first cell is a bare
glyph — and exempts this folder, where the glyphs are the subject.

**It exists because the prose did not work.** This document already said no page carries the legend, and
already predicted the exact failure below under
[§ Converting a corpus](#converting-a-corpus-from-one-vocabulary-to-another) — the legend is "the one
line a by-meaning sweep was guaranteed to corrupt". Two independent sessions then converted pages to the
vocabulary on 2026-08-28 and 2026-08-29, and **both rewrote the legend into the new meanings rather than
deleting it**: each sweep converted every real mark correctly and corrupted the only line that was never
a claim. A third instance was this repository's own `documentation/AGENTS.md`, which carried a summary
table directly beneath a sentence saying the convention "is not restated here".

Three careful readers breaking one rule the same way is not a discipline problem. The rule needed a
build failure, and the gate is nine lines of regex.

**The graduation check** rides along with it. ⚠️ Fails a document containing ✅ but no ⚠️, ❌, 🙈, 🙉
  or 🙊. That is exactly the state [the graduation rule](02-annotation-convention.md#graduation)
  says must be resolved by deleting the ✅ marks — and it is completely mechanical, which makes it the
  cheapest gate on this page and the one most likely to be got right.
- ⚠️ Exempt: `CHANGELOG/`, ADRs, `TODO/`, `KNOWN_ISSUES/` and this folder's own worked
  counter-examples, which contain marks *as examples* rather than as claims. The exemption for
  example-bearing text is the fiddly part and the most likely source of false failures.

## The deferred-work gate

✅ **Fails on a `TODO`/`FIXME`/`HACK`/`XXX` marker opening a comment in any tracked source file, and on
any tracked path under a `TODO/` directory.** `DeferredWorkMarkerConformanceTests` in
`ContractTests.TestIsolation`; green as of 2026-08-28, when the repository's 22 `TODO/` items were moved
into the documents that own them and its source markers were rewritten as marks.

The only gate on this page that exists. It is here first because it is the one with teeth: the others
protect the *quality* of a mark, this one protects the fact that a mark is where the work is recorded
at all.

- A marker is the comment opener, optional whitespace, then the word. Prose discussing the words is
  deliberately not caught → [02 § There is no `TODO`](02-annotation-convention.md#there-is-no-todo).
- `CHANGELOG/` is exempt: append-only history that must not be rewritten.
- ⚠️ It does not sweep Markdown. A `TODO` written into a document is not caught, and roughly 300
  citations of retired `TODO/<file>.md` paths remain in the documentation as dangling pointers.

## Converting a corpus from one vocabulary to another

⚠️ **A find-and-replace over the vocabulary cannot distinguish a mark from a mention of a mark**, and
the legend is the one line in an annotated page where *every* glyph is a mention rather than a claim
about code. A by-meaning conversion is therefore guaranteed to corrupt it.

Learnt the expensive way on 2026-08-28: the sweep that unified the vocabulary rewrote ⚠️ and ❌ by
meaning everywhere they appeared, including inside the legends that define them. Four pages shipped to
`staging` defining ⚠️ twice and ❌ never, three of which use ❌ in the body — so a reader meeting one
had no definition for it, and the mark they *could* look up was ambiguous as well. Caught by another
session and repaired in #1807.

**The procedure, for the next sweep:**

1. Convert **bodies** by meaning, statement by statement. The mapping is not one-for-one between
   vocabularies and cannot be done by substitution alone → [02 § The collision, resolved](02-annotation-convention.md#the-collision-resolved).
2. Then **restore every legend verbatim** from this convention, overwriting whatever the body pass did
   to it.
3. Then assert it: no page may define one mark twice, and no page may use a mark its legend omits.
   ⚠️ That check is mechanical and does not exist → [the legend gate](#the-legend-gate).

The same trap has a second form worth naming: **a mark inside an example, a template or a quoted
legend is a mention too.** This folder's own templates and worked counter-examples are exempt from the
gates for exactly that reason.

## Preserve the triage when work moves

⚠️ **A finding carries a severity, and the severity is the prioritisation — when the finding moves into
a page, the severity moves with it.**

The `TODO/` corpus graded every item `S1`–`S4` under its title. When those items became marked sections
the grade had to travel, or a money-path gap becomes an unmarked ⚠️ among fifty others and the triage is
lost even though the content is not. All 44 graded items that this initiative rehomed kept their grade;
the fourteen absorbed by #1795 did not, and four of those were S1.

**A section that came from a graded finding carries `**Severity: Sn**` immediately under its heading.**
⚠️ Nothing enforces this.

## Scanning source for a mark

Two rules for any gate that reads source looking for a status mark. Both were learnt on 2026-08-28,
one of them by getting it wrong.

⚠️ **Exclude string literals.** The marks govern comments and documentation, never a string a user
reads → [02 § The marks are for readers of the source](02-annotation-convention.md#the-marks-are-for-readers-of-the-source-not-users-of-the-product).
The admin pages render ✅ ❌ ⚠️ 🚫 as status icons and the startup log lines carry them as severity
badges; those are product, chosen by whoever designed that screen, and a gate that fails the build over
a feature-flag icon is a gate that gets deleted. Scan comment text, not the whole file.

⚠️ **Decode `\uXXXX` escapes in whatever you do scan.** `TammermaticEvents/Index.cshtml.cs` writes its
icons as `"✅"` and `"⛔️"`, so a repository-wide grep for the literal characters reports
the file clean. A grep for the character is not a check for the character — in either direction.

## The orphan-⛔ check

⚠️ **Fails when a ⛔ is not immediately preceded, on the same line, by the ⚠️ or ❌ it qualifies.**

Trivially mechanical, and it protects the one rule that keeps ⛔ from decaying into a general
severity badge. It rides along with the legend gate.

- ⚠️ It cannot check the two facts a ⛔ owes — what would unblock it and who can. That is a review
  comment.

## Marks that quietly stop being true

⚠️ **Nothing detects a mark that was correct when written and has since been falsified by code landing
elsewhere.** This is the convention's weakest point, and it fails in the worst direction: a ⚠️ against a
capability that now exists sends the next person to build it twice.

Found on `Design/paid-wash-liveness.md`, 2026-08-29: four marks wrong, of which only one was a merge
artefact. The other three went false when unrelated branches shipped — a ❌ on a defect #1780 had fixed,
a ❌ on an escape hatch #1821 had deleted, and a ✅ on a sentence asserting a gap that #1816 and #1819
had closed, written that same morning by the session that then falsified it. No conflict, no failing
build. A green run and a clean merge both say nothing about whether a mark is still true.

**The cause is structural.** "The mark moves in the same commit as the code" works when the author knows
which pages describe what they touched, and fails exactly when they do not — which is most of the time
across parallel branches.

### Citation-resolves is the wrong gate, and this is why

The obvious proposal is to fail a mark whose paragraph cites `File.cs:NNN` where the citation no longer
resolves. It was measured before being specified, and it does not work:

- `documentation/` carries **409 distinct `path.cs:line` citations**. A sample of 40 found 39 whose file
  exists with the line in range.
- The citation that motivated the proposal — `CarWashesService.Booking.cs:746`, for an escape hatch
  #1821 deleted — **passes that check**. Line 746 exists; it now reads
  `span.Exercised("leaving", ...)` and has nothing to do with the claim citing it.

So the gate would be ~97% green and would have been green on the one defect it was designed to catch. A
line number survives the edit that invalidates it, which is exactly what makes it useless as an anchor.

### The rule that follows

⚠️ **Cite a symbol, not a line.** A symbol is stable under unrelated edits, and its existence is
mechanically checkable where a line's *meaning* is not. This is the same rule as "a number that cannot
be re-derived by a command printed next to it does not belong in a page", applied to the 409 citations
that are exactly such numbers.

⚠️ **Then the gate that pays: a citation index, not a truth check.** Build `path → citing pages` from
the corpus, and on a change touching those paths, list the pages whose marks may be affected. It judges
nothing and cannot be wrong; it removes the one excuse the structural cause rests on — *the author did
not know which pages described what they touched.*

⚠️ **Symbol-keyed truth checking is rejected.** A ⚠️ paragraph names the symbols the intended shape
*would* use, so "⚠️ names a symbol that exists" fires on every well-written gap. No allow-list rescues
a signal that is wrong by construction.

⛔ Any gate keyed on a mark's *meaning* rather than its glyph must handle the SqlFortress SDK pages,
which still carry the pre-2026-08-28 reversed sense. **Unblocked by** that estate being converted.

## What is deliberately not gated

- **Whether a ✅ paragraph is true.** Not checkable. Contained by making ✅ expensive to write and by
  the requirement that whoever writes it ran something.
- **Whether an ⚠️ paragraph is a usable specification.** A judgement, and a review comment.
- **Whether a mark was moved in the same commit as the code.** Approximable — a PR that changes a
  controller and no documentation is suspicious — but the false-positive rate on a heuristic like that
  would train people to bypass it. Left to review.
- **Mark placement granularity.** A section-level mark hiding nine working paragraphs is a real defect
  and a machine cannot see it.

## Status

- ⚠️ Six of the seven gates do not exist.
- ⚠️ No `ContractTests.*` project has been chosen or created to hold them.
- ⚠️ The coverage cross-check depends on parsing Cobertura output per member; whether the existing
  `generate_coverage_reports.py` already produces something usable has not been checked.
