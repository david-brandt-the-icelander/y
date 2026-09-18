# 01 — The methodology

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it.
> See [README.md](README.md). This document describes a way of working; its marks refer to whether
> *this repository* works that way yet.

## The claim

A backlog is a second description of the system, written for a different audience, kept in a different
tool, and updated by a different act than the one that changes the code. It therefore drifts. The drift
is not a discipline problem — it is structural, and every team that has ever run a ticket tracker has
paid for it.

Knowledge-driven development (KDD) removes the second description. There is one artefact per audience —
the operator's manual, the integrator's contract, the maintainer's class summary — written **in the
present tense as the system ought to be**, with each statement marked for whether it is true yet.

- An ⚠️ paragraph is a feature request, written in the voice of the person who will use the feature,
  in the place they will look for it.
- A ❌ paragraph is a bug report, in the place the bug will be found.
- A ✅ paragraph is a claim someone verified, and a claim you may build on.

The document is the specification, the backlog, the release note and the onboarding material at once,
because those four things were always the same set of facts filtered for different readers.

## Why it works here specifically

Three properties of this repository make the trade favourable, and they are worth stating because they
are also the conditions under which it stops being favourable.

1. **Agents write nearly all the code and nearly all the tests.** An agent's context is loaded from
   files. A backlog item in Asana is invisible at the moment of editing; an ⚠️ four lines above the
   method being changed is not. Putting the requirement where the work happens is, for this workforce,
   the difference between a rule that applies and a rule that does not.
2. **The repository already pays for context on every turn.** `AGENTS.md` is deliberately 194 lines
   because everything in it is re-read constantly. Annotations are the opposite kind of cost: they are
   read only by whoever opens that file, and they replace prose that was going to be there anyway.
3. **The gaps are already the interesting part.** The car-wash system talks to hardware nobody here
   built, so "documented behaviour" and "actual behaviour" diverge routinely. A convention whose whole
   purpose is to mark that divergence in the place it bites is a fit for the domain, not an overhead
   on it.

## What it costs

Honestly stated, because a methodology document that lists only benefits is marketing.

- **A mark is a claim, and a wrong claim is worse than no claim.** ✅ next to something nobody ran is
  a lie that a later reader will build on. This is the failure mode that kills the method, and the
  only defence is that ✅ is *expensive to write*: it requires running the tests. → [07](07-enforcement.md)
- **Marks rot silently.** Nothing in a text file notices that the code beneath it changed. Rot is
  contained only by moving the mark in the same commit as the code, and by gates that can mechanically
  contradict a mark (a ✅ on a function whose branch coverage is 40% is machine-detectable; a ✅ on a
  paragraph about operator workflow is not).
- **It is bad at cross-cutting work.** "Upgrade to .NET 10" belongs to no document. Work that is not
  *about* a capability has no natural home and will either be scattered or omitted. This method needs
  a small residual place for it — here that is `TODO/` and `KNOWN_ISSUES/`, which remain.
- **It is bad at scheduling.** A document says what is missing, never when it will be done, who owns
  it, or what it blocks. Sequencing, estimates and dependencies genuinely belong in a tracker outside
  the repository — **the document owns *what and whether*; a tracker owns *when and who***. This
  repository's own mandatory tracker integration (Asana) was removed 2026-08-29 as a failed experiment
  (`documentation/Development/agent-tooling-friction.md`); the division of labour stands as a principle
  even with no tracker currently enforcing the "when and who" half.
- **Diff noise.** Every shipped capability produces a documentation hunk. This is a feature disguised
  as a cost — a PR with no mark moved is a PR that changed nothing a reader cares about — but it is
  real friction, and reviewers will feel it.
- **Emoji are a lossy encoding.** They break `grep -w`, sort unpredictably, render inconsistently in
  terminals, and are invisible to a screen reader that announces "warning sign". They were chosen for
  scannability by humans and greppability by agents; they are not a good data structure and should
  never become the *only* record of a defect that matters.

## Compared with the alternatives

| Approach | Where the intent lives | Drifts because | Why not that, here |
| --- | --- | --- | --- |
| **Ticket tracker** (Jira, Asana, GitHub Issues) | Outside the repository | Closing a ticket and changing the code are two acts, and only one is enforced | Invisible to an agent editing the file. Retained for scheduling and human coordination, which it is genuinely good at |
| **`TODO:` comments** | In the code, unstructured | No taxonomy, no audience, no lifecycle; nothing distinguishes "not built" from "built wrong" | This is KDD's degenerate ancestor. The convention here is essentially `TODO:` with a type system and a reader in mind |
| **README-driven development** (Preston-Werner) | One README, written before the code | Only covers the front door; says nothing about internals, and has no vocabulary for partial truth | Adopted wholesale, then generalised: KDD is RDD applied to every audience at once, with marks so the document survives past version 0.1 |
| **Spec-driven development** (a spec file per feature, then generate) | A separate spec artefact | The spec is written once and abandoned after implementation; nobody reads it again | The spec here is not separate. It *is* the manual, so it keeps being read, which is the only thing that keeps it current |
| **ADRs** | `decisions/NNNN-*.md`, immutable | Does not drift — by design it records a decision at a point in time | Complementary, not competing. ADRs answer *why we chose this*; KDD documents answer *what is true now*. Both are kept |
| **Literate programming** (Knuth) | Prose and code interleaved in one source | Requires a tangling toolchain and a whole-file rewrite; nobody maintains the prose half | Too heavy. KDD keeps prose and code in separate files with a shared vocabulary, which is 80% of the benefit at 5% of the cost |
| **Doc tests / executable specs** (Gherkin, doctest) | Executable prose | Does not drift at all — it is a test | Strictly better *where it applies*. KDD's job is precisely the residue: things not yet executable, things broken, things about people rather than code. Where a statement can be a test, [make it a test](07-enforcement.md) and drop the mark |
| **Coverage reports** | Generated, outside the source | Accurate but anonymous — tells you a percentage, not which behaviour nobody checked | 🙈 🙉 🙊 exist to say the thing a percentage cannot: *this branch, deliberately, is unverified, and here is what would break* |

The honest summary: **KDD is not better than an executable specification, and does not try to be.** It
is better than a tracker at the things a tracker is bad at, and it degrades gracefully — a document
that nobody maintains is still a document, whereas a spec that nobody maintains is a lie with a
schema.

## The three rules that make or break it

Everything else in this folder is detail. These are the load-bearing rules.

1. **The mark moves in the same commit as the code.** ✅ is not a status field updated later; it is
   part of the change. → [02 § Ownership](02-annotation-convention.md#ownership)
2. **✅ means someone ran it.** Not that a plan said so, not that an agent reported success, not that
   the code looks right. → [07](07-enforcement.md)
3. **Describe the unbuilt thing properly anyway.** An ⚠️ that says "not implemented" is worthless; an
   ⚠️ that describes the intended behaviour in full is a specification a cheaper agent can implement
   from. The mark is metadata — the paragraph is the deliverable.

## Adoption status

- ⚠️ Nothing in this repository is written this way yet. The convention has one demonstration — this
  folder — and 206 existing documents that predate it.
- ⚠️ No agent instruction routes to this convention, so no agent applies it. The `AGENTS.md` routing
  table entry is drafted in [`06-rollout-plan.md`](06-rollout-plan.md#wave-0) and not yet added.
- ✅ The division of labour with Asana no longer has a conflicting skill to contradict it — the
  mandatory `carwash-asana-tracking` skill (which expected tasks for work this convention would keep in
  a document) was removed 2026-08-29.
