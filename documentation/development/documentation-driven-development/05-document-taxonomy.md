# 05 — Document taxonomy

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it. → [README](README.md)

A fact belongs in exactly one document. Where two could hold it, the more specific one holds it and the
other links. Duplication is how a corpus starts contradicting itself, and a contradiction between two
marked documents is worse than an unmarked one — both look authoritative.

**Every document is written from one named perspective**, and the perspective decides what counts as a
fact. The operator does not care that a poll is debounced; the maintainer does not care which button is
on the left. A document that serves two perspectives serves neither and is the commonest way these
corpora become unreadable.

## The five kinds

| Kind | Perspective | Answers | Lives in | Granularity of marks |
| --- | --- | --- | --- | --- |
| **Vertical feature** | Whoever *uses* the capability — operator, customer, back-office | "What can this system do about washes/subscriptions/refunds, and what can it not?" | `documentation/Features/<feature>.md` | Per capability |
| **User journey** | One actor, start to finish | "What happens, step by step, when a customer buys a wash at the pump?" | `documentation/Journeys/<journey>.md`, one per `UserJourney` row | Per step |
| **Subsystem / integration** | The engineer on call, and the integrator | "What is this component, what does it talk to, how does it fail?" | `documentation/Architecture/`, `documentation/Reference/` | Per behaviour or failure mode |
| **Contract** | A consumer outside this repository | "What may I call, what comes back, what must I not rely on?" | The OpenAPI document itself → [04](04-api-contract-annotation.md) | Per operation and per field |
| **Component** | The maintainer with the file open | "Does this member work, and does anything check that?" | The source file → [03](03-code-annotation-convention.md) | Per member and per branch |

## Choosing between them

The test is **who is inconvenienced when it is wrong.**

- An operator stands at a bay and the machine does not respond → *vertical feature* or *journey*.
- Django's integration breaks after a deploy → *contract*.
- A maintainer changes a method and does not know what it is allowed to return → *component*.
- An on-call engineer needs to know why the FENiX driver retries three times → *subsystem*.

Two rules that resolve most of the remaining arguments:

- **A capability is described once, in the feature document, and referenced from the journeys that
  traverse it.** A journey that re-describes the capability will drift from the feature document within
  two changes.
- **A journey never contains a mark that no step owns.** "Refunds are unreliable" belongs to the
  feature; "step 6, the refund confirmation SMS, ⚠️ is never sent" belongs to the journey.

## Vertical feature documents

The unit is a capability someone would name, not a project or a directory. `documentation/Features/`
holds three today (`sending_sms_receipts.md`, `sending_pdf_receipts.md`,
`fuel_terminal_sending_pdf_receipts.md`) — all narrow and all receipt-shaped, which is a fair sign the
inventory was written by whoever last touched receipts rather than derived from the system.

Structure: what the capability is for · who invokes it · what it does today, marked statement by
statement · the failure modes and what an operator does about each · what it deliberately does not do
(unmarked — a non-goal is not a gap) · links to the journeys, the contract operations and the
subsystems that implement it.

⚠️ **The feature inventory does not exist.** Deriving the real list of capabilities is
[wave 1](06-rollout-plan.md#wave-1).

## User journey documents

**These are not free-form.** `UserJourney` is a first-class row in the admin database, with test
records, setup progress, and a `RequiresRetest` flag that is the only signal reaching a QA tester
(`SqlUserJourneyList`, `SqlUserJourneySetRequiresRetest`, and the `RequiresRetest` migration rule in
[`AGENTS.md`](../../../AGENTS.md)). A journey document is the prose face of a row that already exists,
and the two must not diverge.

- **One document per `UserJourney` row**, named for the row, linking to it.
- **A step's ⚠️ or ❌ is a QA-visible fact.** Marking a step broken and not setting `RequiresRetest` in
  the same PR leaves a tester certifying a journey the document says is broken.
- The existing `documentation/WWB/*/journeys.md` files are versioned *design* journeys for the
  wash-with-benefits work, not these. They describe intent at a point in time; a journey document
  describes the present. ⚠️ Whether they should be folded in or left as design history is undecided.

⚠️ **`documentation/Journeys/` does not exist**, and no document corresponds to any `UserJourney` row.

## Subsystem documents

`documentation/Architecture/architecture.md` is canonical and large, and `AGENTS.md` requires asking
before changing it. That standing rule is unaffected: **annotating is changing.** A subsystem document
under this convention is a new, smaller, per-component document that links *to* the architecture rather
than annotating it. → [06 § Wave 2](06-rollout-plan.md#wave-2)

One per component that fails independently: FENiX, Milestone ANPR, doors, e-stop, Verifone, progress
displays, the UX700 facades, the Django facade, SMS, blobs, subscriptions, simulators. What it is · the
contract with the thing outside · every failure mode observed in production and what it looks like from
the outside · what is faked in tests and therefore unverified against the real device.

That last section is where 🙈 becomes visible above the source level, and it matters most for hardware,
where `AGENTS.md` rule 4 already says the device wins any disagreement with its documentation.

## Component documentation

In the source, per [03](03-code-annotation-convention.md). **No separate file.** A component that
needs a prose document has outgrown component scope and is a subsystem.

## Anti-patterns

- **A document per project directory.** Directory structure is a build artefact here — several projects
  compile into one assembly — and a reader looking for "how do refunds work" will not guess a folder.
- **A status page listing every mark in the corpus.** That is a second inventory, it will drift, and
  drift in the *index* discredits the documents it points at. Generate it if it is wanted; never write
  it by hand. → [07](07-enforcement.md)
- **Marks in `CHANGELOG/`, ADRs, or `TODO/`.** Records of the past and of out-of-scope work. → [02](02-annotation-convention.md#what-does-not-get-a-mark)
- **A journey document for a journey with no `UserJourney` row.** Create the row, or it is a feature
  document wearing a journey's clothes.
