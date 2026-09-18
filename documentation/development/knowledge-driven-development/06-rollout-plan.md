# 06 — Rollout plan

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it. → [README](README.md)
> Every wave below is ⚠️. Nothing has been done. The marks record which parts are specified precisely
> enough to start and which are still hand-waving.

## The size of the problem

Counted on `base--documentation-driven-development` at `74a5b35fe`, 2026-08-28:

| | Count | Implication |
| --- | --- | --- |
| Production `.cs` files | 3,396 | Component-level annotation cannot be a project. It is a rule applied to files as they are touched |
| Test `.cs` files | 2,782 | The test corpus is nearly as large as the code; the [location rule](03-code-annotation-convention.md#the-test-location-rule) has a lot of existing placement to reconcile |
| Controllers | 39 | The contract surface is small enough to annotate exhaustively — this is the one tractable sweep |
| Existing documents under `documentation/` | 212 | Most predate the convention. Converting all of them is neither possible nor desirable |
| `TODO/` items | 16 | Candidates for conversion into ⚠️ paragraphs in the document that owns them |
| `KNOWN_ISSUES/` entries | 1 | Candidates for ❌ marks at the site of the defect |

**The plan is therefore not "annotate everything".** Three-quarters of the value is in four small,
finishable sweeps; the rest arrives file by file, carried by a rule rather than a project.

## Sequencing principle

**Enforcement before adoption, contracts before internals, and no wave starts before the one that
makes it checkable.** A corpus annotated by hand with no gate rots faster than it grows — the marks go
stale, readers learn to distrust them, and the convention is dead with the cost already sunk. Every
wave below therefore ships its gate with it.

---

## Wave 0 — make it real, and measure

*Nothing is annotated. This wave makes annotation possible and produces the numbers every later wave is
sized from.*

- ⚠️ **Route to the convention from the agent instructions.** One routing-table row in
  [`AGENTS.md`](../../../AGENTS.md) — *Documenting a capability, journey, class or endpoint →
  `documentation/Development/knowledge-driven-development/`* — and, per the repository's own rule,
  a line deleted to pay for it. Without this row no agent applies the convention and every later wave
  is manual.
- ✅ **The mark collision is resolved** (2026-08-28) in favour of this vocabulary, and every
  non-convention use of the glyphs in this repository has been converted to prose.
  → [02 § The collision, resolved](02-annotation-convention.md#the-collision-resolved)
- ⚠️⛔ The SqlFortress SDK documentation is the one corpus left reading backwards. It lives on another
  initiative's branch, which this one must not mutate. **Unblocked by** whoever next picks up
  SqlFortress, or by David reassigning it.
- ⚠️ **Measure test locatability.** A script that computes the expected path for every production type
  and reports the misses. Output: a count, a per-project breakdown, and the exemption list that
  [rule 5](03-code-annotation-convention.md#rule-5--what-the-rule-does-not-require) has to encode. This
  is a read-only sweep and is the cheapest useful thing in the whole plan.
- ⚠️ **Measure branch coverage per member** from the existing `coverlet.runsettings` run, so ✅ can be
  contradicted mechanically rather than argued about.
- ✅ **`TODO/` is abolished and gated** (2026-08-28). Its 22 items moved into nine documents that own
  the capabilities they describe, every source marker became a mark, and
  `DeferredWorkMarkerConformanceTests` fails the build on a return.
- ⚠️ **Roughly 300 citations of retired `TODO/<file>.md` paths remain in the documentation** as
  dangling pointers. The 22 that moved were repointed; the rest name notes that were already deleted
  before today. Sweeping them is a mechanical pass nobody has run.
- ⚠️ **The gate does not sweep Markdown**, so a `TODO` written into a document is not caught.
- ⚠️ **Write the templates' first real user** — convert exactly one document of each kind, by hand, and
  see what the templates got wrong before anything is scaled.

**Done when:** an agent reading `AGENTS.md` finds the convention, the vocabulary is settled, and two
numbers exist that nobody currently knows.

## Wave 1 — the contract surface

*39 controllers. Bounded, externally visible, and the audience least able to ask us a question.*

- ⚠️ Annotate every operation that is a stub or has a known defect per [04](04-api-contract-annotation.md).
  Operations that work stay unmarked, which is most of them — the sweep is a filter, not a rewrite.
- ⚠️ Add the Swagger scan gate → [07 § The Swagger scan](07-enforcement.md#the-swagger-scan). It fails
  the build on a ✅ reaching consumer-visible text, and on an ⚠️/❌ without its spelled-out gloss.
- ⚠️ Add the routing line to `.github/instructions/facades.instructions.md`, the file an agent actually
  reads before touching a facade.

**Delegable.** Reading 39 controllers and their existing annotations is wide, shallow and fully
specified — Codex work, reviewed here.

**Done when:** the generated OpenAPI document tells an integrator the truth about every operation, and
cannot silently regain a ✅.

## Wave 2 — features and journeys

*The documents a human reads. The highest-value and least mechanical wave.*

- ⚠️ **Derive the capability inventory.** From the controllers, the admin pages and the `UserJourney`
  rows — not from `documentation/Features/`, whose three receipt-shaped files are evidence of how the
  current inventory was assembled. Expect 15–30 features.
- ⚠️ **Create `documentation/Journeys/`, one document per `UserJourney` row**, linked to the row.
  → [05 § User journey documents](05-document-taxonomy.md#user-journey-documents).
- ⚠️ **Fold `TODO/`'s 16 items and `KNOWN_ISSUES/`'s single entry into the documents that own them**,
  as ⚠️ and ❌ paragraphs. Keep in `TODO/` only what belongs to no capability — cross-cutting work,
  which [01](01-methodology.md#what-it-costs) says this method is bad at and does not pretend otherwise.
- ⚠️ **The `RequiresRetest` interaction.** A step marked ❌ or ⚠️ in a journey document is a QA-visible
  fact; the rule that a journey change ships its `RequiresRetest` migration in the same PR extends to
  documentation changes, or QA certifies journeys the documentation says are broken.

**Not delegable below this tier.** Deciding what the system's capabilities *are*, and which gaps
matter, is the judgement half. Drafting each document once the inventory exists is delegable.

**Done when:** every `UserJourney` row has a document, and every open `TODO/`/`KNOWN_ISSUES/` item is
either an annotated paragraph or explicitly cross-cutting.

## Wave 3 — subsystems

*One document per independently-failing component: FENiX, Milestone ANPR, doors, e-stop, Verifone,
progress displays, the UX700 facades, the Django facade, SMS, blobs, subscriptions, simulators.*

- ⚠️ Each links *to* `Architecture/architecture.md` rather than annotating it — the standing
  ask-before-changing rule on that file is unaffected, and annotating is changing.
- ⚠️ Each carries a **"faked in tests, unverified against the device"** section. For hardware this is
  the most valuable paragraph in the corpus: it is where 🙈 becomes visible to someone who is not
  reading source, and it is exactly the class of gap `AGENTS.md` rule 4 exists for.

**Sequenced after wave 2** because a subsystem document's audience arrives from a feature document.

## Wave 4 — components, forever

*Not a wave. A rule.*

- ⚠️ **A file gets annotated when it is edited, not before.** 3,396 files cannot be swept, and a swept
  file's marks would be stale before the sweep finished.
- ⚠️ **The ≥95% coverage gate already fires on changed code.** This convention adds one obligation: the
  residual uncovered branches are *named* with 🙈 🙉 🙊 rather than left anonymous.
- ⚠️ **A test file moves to its computed location when it is next edited**, matching the containment
  strategy already used for the `Test_` prefix. No mass move.

Coverage of the component level is therefore proportional to churn, which is the correct
distribution: the files nobody touches are the files nobody is misled by.

---

## What stops it rotting

Four things, in descending order of how much they actually matter:

1. **Gates, not diligence.** Everything mechanically checkable is a build failure → [07](07-enforcement.md).
2. **The mark moves in the same commit.** Enforced by review and by the changelog requirement that
   already exists on every PR.
3. **Graduation.** Documents that clean up shed their marks entirely, so the marked corpus stays small
   enough to stay true. A convention that only ever adds marks becomes wallpaper.
4. **A generated index, never a written one.** If a status overview is wanted, generate it from the
   marks. A hand-written one is the second inventory this whole method exists to abolish.

## What this plan deliberately does not do

- **Convert all 212 existing documents.** Most describe the past, or are design records, or are fine.
  Conversion happens when a document is next substantively edited.
- **Annotate `CHANGELOG/`.** 1,987 entries, ~1.7M tokens, immutable history. Reading it is against
  repository rules and annotating it would be meaningless.
- **Replace Asana.** The document owns *what and whether*; the tracker owns *when and who*.
  → [01](01-methodology.md#what-it-costs).
- **Touch `documentation/WWB/`.** 35 versioned design documents, a design record rather than a
  description of the present. ⚠️ Whether the V2.1 journeys should seed `documentation/Journeys/` is
  undecided and is a wave-2 question.
