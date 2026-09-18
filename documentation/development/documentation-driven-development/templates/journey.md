# Journey: <name, matching its UserJourney row>

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it.
> Marks are per-step. → [convention](../README.md)

**Actor:** <who>  **`UserJourney` row:** <id / admin link>  **Starts when:** <trigger>
**Ends when:** <terminal state>

> A step marked ⚠️ or ❌ is QA-visible. Set `RequiresRetest` in the same PR that adds it, or a tester
> will certify a journey this document says is broken. → [`AGENTS.md`](../../../../AGENTS.md)

## Steps

| # | The actor does | The system does | Status |
| --- | --- | --- | --- |
| 1 | | | ✅ |
| 2 | | | ⚠️ <what should happen, and what happens instead> |

## Where it can stop

| Step | Why it stops | What the actor sees | Recovery | Status |
| --- | --- | --- | --- | --- |

## Variants

<Subscription vs. pay-per-wash, app vs. terminal, etc. A variant that differs in more than two steps is
its own journey and its own `UserJourney` row.>

## Capabilities this traverses

<Links to feature documents. Do not re-describe them here — a journey that restates a capability drifts
from it within two changes.>
