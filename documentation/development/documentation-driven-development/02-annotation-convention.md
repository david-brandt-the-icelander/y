# 02 — Annotation convention (prose documents)

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it.
> Canonical definitions in [README.md](README.md). Code comments add 🙈 🙉 🙊 → [03](03-code-annotation-convention.md).
> This document is the **canonical** statement of the convention. Nothing else restates it; everything
> else links here.

## No page carries the legend

**A page links to this convention; it never restates it.** One line, at the top:

```markdown
> Status marks: [`<relative path to>/Development/documentation-driven-development/README.md`](<relative path to>/Development/documentation-driven-development/README.md)
```

The reason is the grep, and it is the whole point of having a fixed vocabulary:

```bash
grep -rn '❌' documentation/    # every known bug in the system
grep -rn '⚠️' documentation/    # every capability that must exist and does not
```

Those only work if **every** occurrence is a claim about the system. A legend is a *mention* — four
glyphs per page that are not defects — so a corpus where each page explains the marks is one where
both commands return mostly noise, and a reader learns not to run them. The definition lives in exactly
one place so that everywhere else the glyph means what it says.

This is the same rule as [07 § Converting a corpus](07-enforcement.md#converting-a-corpus-from-one-vocabulary-to-another)
reached from the other end: the legend was the one line a by-meaning sweep was guaranteed to corrupt,
and it is also the one line that breaks the search. Deleting it fixes both.

Enforced by `MarkVocabularyConformanceTests`, which fails the build on a page that defines a mark
rather than linking to this one. → [07 § The mark-vocabulary gate](07-enforcement.md#the-mark-vocabulary-gate)

An unmarked document is one that has not adopted the convention. That is a legitimate state, and not a
claim that everything on it works.

## Placement and granularity

**The mark goes on the smallest statement that is wholly true or wholly false.** A section header
carrying one ⚠️ for eleven paragraphs, nine of which work, is worse than no mark: it makes the two
broken paragraphs unfindable.

- A bullet that describes one capability takes the mark at the start of the bullet.
- A table row describing one capability takes the mark in its own column, headed `Status`.
- A paragraph takes the mark at the start of the sentence that is untrue, not at the start of the
  paragraph.
- A whole section may take a mark on its heading **only** when every statement under it shares that
  state. `### Refund reversal ⚠️` is correct when nothing about refund reversal exists.
- **Never mark a heading and its contents both.** The reader cannot tell whether the inner marks
  refine the outer one or contradict it.

## What each mark obliges you to write

The mark is not the content. A document whose gaps are marked but not described has recorded that it
has gaps, which the reader already suspected.

**The mark is the whole commentary.** Write the sentence as the system *should* be — present tense,
in the detail a person would need to use it — and mark it. Do not then explain the mark. No "what it
does instead", no "the customer sees", no "this was ruled on because", no account of the failure. The
glyph already says *this sentence is not true yet* or *this sentence is not true*; a paragraph
restating that in prose turns a specification back into a defect report, which is the thing the page
replaced.

**⚠️ — not implemented.** The sentence describes behaviour that does not exist. It is finished when
someone could build from it without asking a question, which is the only quality bar it has to clear.

**❌ — implemented but broken.** The sentence describes behaviour that exists and does not hold.

The discipline this asks for is real and it is worth naming, because the instinct runs the other way:
a page that has just cost somebody a day of debugging wants to *say so*. It must not. The incident
belongs in the changelog entry that introduced the mark, and the diagnosis belongs in the PR that
fixes it. What the page owes a reader is the sentence they can build against, and one glyph telling
them whether to trust it.

**⛔ — blocked.** Written straight after the ⚠️ or ❌ it qualifies, never on its own. It says that the
gap cannot be closed by whoever reads it: the decision is not ours, the hardware is not here, the
third party has not shipped, another initiative owns the file. It carries two further facts:
**what would unblock it, and who can do that.**

⛔ is the only mark that can become false without anybody touching the code. Re-read the blockers when
an initiative is picked up; an unblocked ⛔ is a stale claim exactly like a stale ✅.

**✅ — implemented and verified.** May be written only by someone who has read the code and seen the
tests pass. Not "a plan said so", not "an agent reported success", not "the PR is merged". Where no CI
gates the area, say who ran what and when, or the reader will assume a machine did it. → repo rule in
the workspace `CLAUDE.md` (`~/dev/festi/CLAUDE.md`).

## Ownership

**The mark moves in the same commit as the code that changes its truth.** Shipping a capability turns
its ⚠️ into ✅ *in the same PR*. Finding a defect adds the ❌ in the commit that found it, or in the
`KNOWN_ISSUES/` entry's own commit if the finding lands separately.

A PR that changes behaviour and moves no mark is either not user-visible or has left the documentation
lying. Reviewers should ask which.

## Graduation

**When a document contains no ⚠️ and no ❌, delete every ✅ in it and replace the status-marks link
with:**

```markdown
> This component is fully implemented, tested and free of known defects.
```

The ticks are scaffolding for the gaps. With no gaps left they carry no information, cost a line of
noise per statement, and — worse — decay into decoration that a later reader stops believing. Removing
them is what makes the remaining marked documents worth reading.

Two clarifications, because the rule as originally stated mentions only ⚠️:

- **❌ blocks graduation too.** "Fully implemented" over a page with a live defect on it is false.
- **⛔ never blocks on its own**, because it never appears on its own — the ⚠️ or ❌ it qualifies is
  what graduation waits on.
- **Reintroduction is normal and cheap.** The first new ⚠️ or ❌ restores the link and marks only the
  statements that need it — a graduated document is not re-ticked wholesale.
  ✅ marks come back only where they neighbour a gap and the contrast is informative.

For code files the same rule applies with 🙈 🙉 🙊 added to the blocking set → [03 § Graduation](03-code-annotation-convention.md#graduation).

## The collision, resolved

The vocabulary used to be inverted at workspace level: ⚠️ meant *implemented, with a limitation* and
❌ meant *not implemented*. That was settled on 2026-08-28 in favour of the definitions above, and
`~/dev/festi/CLAUDE.md` now carries them. Two consequences that outlive the decision:

- **A document written before that date may mean the opposite of what it says.** A page linking to
  this convention is under this vocabulary; one linking to nothing is under none.
- ⚠️⛔ **The SqlFortress SDK documentation still uses the superseded vocabulary**, so every mark in it
  reads backwards. It is not converted here: it lives on another initiative's branch, which this one
  must not mutate. **Unblocked by** whoever next picks up SqlFortress, or by David reassigning it.

Beyond the inversion, the marks were also doing several unrelated jobs in *prose and comments* across
this repository — good and bad code examples, yes/no cells in tables, done-ticks on checklists,
findings columns in review ledgers. All of those were removed on the same date and replaced by prose
that says the thing in words.

## The marks are for readers of the source, not users of the product

**A glyph in a string a person reads at runtime is not a status mark and this convention says nothing
about it.** An admin page rendering ✅ beside an enabled feature flag, a status pill showing ❌ for a
failed sale, a startup log line badged ⚠️ — those are product. They are chosen by whoever designs that
screen or that log, they are read by an operator who has never seen this document, and changing them
is a change to what a user sees. The convention has no standing there.

The boundary is not the file type, it is **who reads the string**:

| | Governed by this convention | Not governed |
| --- | --- | --- |
| Markdown documentation | ✅ every mark | — |
| `///` and `//` comments, doc comments | ✅ every mark | — |
| String literals rendered to a screen, a log, a report, an email | — | ❌ never touch these |
| OpenAPI descriptions | ✅ but with a smaller vocabulary → [04](04-api-contract-annotation.md) | — |

The one thing the convention *does* ask of product strings: **do not reason about them from a grep.**
A repository-wide search for ✅ will hit both, so any gate that scans source for a mark must exclude
string literals, or it will fail the build over a feature-flag icon.
→ [07 § Scanning source for a mark](07-enforcement.md#scanning-source-for-a-mark)

## There is no `TODO`

**A `TODO`, `FIXME`, `HACK` or `XXX` marker is a mark that failed to say anything.** It records that
somebody was unhappy at that line, and nothing else: not whether the code works, not whether it is
broken, not what it should do instead, not who could fix it. Everything a reader needs it withholds.

So the vocabulary above replaces it entirely, and the replacement is enforced rather than requested:

- **No `TODO/` directory.** A directory of deferred work is a second inventory — the exact thing this
  method exists to abolish — and it drifts from the code the moment it exists. Each item goes into the
  document that owns the capability, as a marked section.
- **No `TODO`/`FIXME`/`HACK`/`XXX` opening a comment**, in any source file. Write ⚠️, ❌ or ⛔ and the
  paragraph that says what the behaviour should be.
- **Both are build failures**, checked by `DeferredWorkMarkerConformanceTests` in
  `ContractTests.TestIsolation`. → [07 § The deferred-work gate](07-enforcement.md#the-deferred-work-gate)

The gate catches a marker that *opens* a comment, not every occurrence of the four words. Prose that
discusses the convention is not a marker, and a gate that fires on any appearance of four common
English strings trains people to suppress it.

Two things survive alongside the marks and are not `TODO`s:

- **`KNOWN_ISSUES/`** records a deferred *defect* — something found wrong that a review chose not to
  block a merge on. Each entry is also ❌ at the site of the defect; the folder is the review trail,
  not the inventory. ⚠️ Whether it should be folded into the documents entirely is undecided.
- **Cross-cutting engineering work** that belongs to no capability — a framework upgrade, a tooling
  gap. [01](01-methodology.md#what-it-costs) says plainly that this method is bad at that; the honest
  answer is a document about the thing itself, such as
  [`agent-tooling-friction.md`](../agent-tooling-friction.md), not a list.

## What does not get a mark

- **Facts that are not capabilities.** "All timestamps are UTC" is a property of the system, not a
  feature with a build state. Marking it invites a ✅ that means nothing.
- **Rules and instructions.** An `AGENTS.md` directive is obeyed or not; it is not implemented or not.
- **Historical records.** `CHANGELOG/` entries and ADRs describe a past state and are immutable. Never
  annotate them; a ✅ there would be a claim about the present in a document about the past.
- **Anything a test can assert instead.** If a statement is mechanically checkable, write the test and
  let the test carry the truth. → [07](07-enforcement.md)
