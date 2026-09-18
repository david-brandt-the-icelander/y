# <Subsystem or integration>

> Legend: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but broken ·
> ⛔ blocked, qualifying the mark before it.
> → [convention](../README.md)

**Written for:** the engineer on call, and whoever integrates with this.
**Canonical architecture:** [Architecture/architecture.md](../../../Architecture/architecture.md) —
this document links to it and does not annotate it.

## What it is

<One paragraph. Where it sits, what it owns.>

## The boundary

<What is on the other side, who built it, and what protocol. If Festi did not build the far side,
assume its documentation and its behaviour disagree and that the device wins.
→ [`external_system_integration.instructions.md`](../../../../.github/instructions/external_system_integration.instructions.md)>

## Behaviours

- ✅ / ⚠️ / ❌ <one marked statement per behaviour>

## Failure modes seen in production

| Symptom | Cause | What it looks like from outside | Handled? |
| --- | --- | --- | --- |
| | | | ✅ / ⚠️ / ❌ |

## Faked in tests, unverified against the real thing

<The most valuable section here. Every behaviour whose only evidence is a fake we wrote — where 🙈
becomes visible to a reader who is not in the source. For hardware this is where the next production
surprise is already written down.>

- 🙈 <path that no test reaches, and what would break if it fired>
- 🙉 <path exercised only through something else's test>
- 🙊 <path executed by a test that asserts nothing about it>

## Operating it

<Restart, reset, diagnose. Links to the operator tooling and the MCP tools that touch it.>
