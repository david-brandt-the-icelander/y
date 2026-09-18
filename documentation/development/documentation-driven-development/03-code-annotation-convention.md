# 03 — Code annotation convention

> Legend: ✅ implemented and fully branch-covered · ⚠️ not implemented · ❌ known defect ·
> ⛔ blocked · 🙈 untested · 🙉 no direct unit test · 🙊 executed but unasserted.
> Prose rules in [02](02-annotation-convention.md); this document adds what only applies inside source files.

**A change that is only relevant in code does not get a document.** It gets a mark in the docstring of
the thing it is about. This is the same convention as [02](02-annotation-convention.md), scoped down to
where the audience is a maintainer reading the source.

---

## ✅ on a function means every branch is covered

**A `/// <summary>` carries ✅ if and only if every branch of execution in that member is covered by a
direct unit test.** Not "there is a test for this function" — every branch. One uncovered `else`, one
unexercised guard clause, one `catch` nothing throws into, and the ✅ is wrong and must not be written.

This is the strictest mark in the convention and it is deliberate. It is also the only one a machine
can contradict: branch coverage is already measured here (`coverlet.runsettings`, ≥95% line and branch
per [`AGENTS.md`](../../../AGENTS.md)), so a ✅ on a member below 100% branch coverage is a mechanical
finding, not a judgement call. → [07 § The coverage cross-check](07-enforcement.md#the-coverage-cross-check)

```csharp
/// <summary>
/// ✅ Returns the bay's current state, or <see cref="BayState.Unknown"/> when the last poll is
/// older than <paramref name="staleAfter"/>.
/// </summary>
public BayState GetBayState(int bayId, TimeSpan staleAfter)
```

The four member-level marks are mutually exclusive. A member is ✅, or it is one of ⚠️ / ❌ / 🙈 / 🙉 /
🙊 — never ✅ and something else. If part of a member is unbuilt or untested, the member is not ✅.

## The monkeys: three degrees of not-really-tested

Coverage percentages say how much ran. They cannot say what nobody checked. These three do.

| Mark | Means | Typical cause |
| --- | --- | --- |
| 🙈 **see no evil** | Nothing executes this path in any test suite. | A guard clause, a `catch`, a hardware error path that no fake reproduces |
| 🙉 **hear no evil** | The path executes, but only inside some *other* unit's test, or an integration/contract test. No test names this member. | A helper reached only through its caller's tests — the commonest case in this repository |
| 🙊 **speak no evil** | A test executes the path and asserts nothing about what it did. Coverage is green and the behaviour is unverified. | An assertion on the return value only, while the interesting effect was a write or a call |

🙈 is worse than 🙉 is worse than 🙊, and all three are worse than ✅. Where several apply to one member,
take the worst.

**Worked example, from this repository.** `SqlUserJourneyList`
(`N1CarWashesAPI.Admin/Databases/CarWashes/Admin/SQL/SqlUserJourneyList.cs`, namespace
`N1CarWashesAPI.Database.Databases.CarWashes.Admin.SQL`) has no test file bearing its name. It is
exercised only through `UnitTests.N1CarWashesAPI.Admin/DAL/Implementations/UserJourneysDb/UserJourneysDbTests.cs`.
That is 🙉 — real coverage, no direct test, and nobody would find the test from the class.

### Placement

- **Member-level** marks go at the front of the `<summary>`, before the first word.
- **Branch-level** marks go on a comment inside the body, on the line of the branch they describe, and
  say *what is unverified* — not merely that something is.

```csharp
public async Task<bool> ClaimAsync(int machineId)
{
    if (_clock.UtcNow < _leaseUntil)
        return false;            // 🙉 exercised only via WashQueueTests; no direct test names this guard

    try { return await _db.ClaimAsync(machineId); }
    catch (SqlException)
    {
        // 🙈 the executor is faked in every test, so no test reaches this handler; a real deadlock
        // would take the untested path and we do not know that it returns false rather than throwing
        return false;
    }
}
```

A branch-level 🙈 forces the member-level mark down: a member containing any 🙈 🙉 🙊 comment cannot be
✅. That is the invariant a conformance test can check without reading a coverage report.

## ⚠️ and ❌ in code

**⚠️ — a requirement that is not implemented.** Placed on the member or branch that ought to satisfy it,
describing the required behaviour in full. On a member that does not exist yet, ⚠️ goes on the type or
region where it belongs, so the reader finds the gap where they look for the capability.

```csharp
/// <summary>
/// Reverses a completed wash and refunds the sale.
/// ⚠️ Subscription washes are not reversed — the sale is refunded but the subscription's monthly
/// allowance is not credited back, so the customer loses the wash. Required: decrement
/// <c>SubscriptionUsage.Used</c> in the same transaction as the refund.
/// </summary>
```

**❌ — a known defect in code that exists.** Says what it does wrong and what that costs. Links its
`KNOWN_ISSUES/` entry where there is one; the link never replaces the sentence.

```csharp
// ❌ Rounds half-away-from-zero while the POS rounds half-to-even, so a 2.5 ISK line
// reconciles one króna high roughly one sale in two hundred. → KNOWN_ISSUES/rounding-mismatch.md
```

## Types, enums and enum members

- **A type's `<summary>`** takes ✅ only when every member of that type is ✅. It is the weakest link,
  not an average.
- **An enum takes ✅** when every one of its members is reachable in production and each is covered by a
  test that asserts behaviour specific to it. An enum member that no code path produces is ⚠️ (a
  placeholder for unbuilt behaviour) or ❌ (dead, and something is meant to produce it).
- **Mark individual enum members**, not the enum, when the states differ.

```csharp
public enum WashOutcome
{
    ✅ Completed,
    ✅ Cancelled,
    /// ⚠️ Never produced. Intended for a wash the machine reports as finished with faults;
    /// the FENiX driver currently maps that to Completed.
    PartiallyCompleted,
}
```

⚠️ **The example above does not compile.** A mark cannot precede an identifier in C#; it must be in a
comment or an XML doc. The rule as written is: enum members take their mark in a `///` summary or a
trailing `//` comment, never inline in the declaration. This document's example is retained as the
worked *counter*-example.

## The test-location rule

**A component is only "tested" if its test file can be *computed* from its identity, not searched
for.** The mapping below is total, bijective and mechanical: given a production file, exactly one test
path is correct, and given a test path, exactly one production file is under test. That is what makes
it enforceable by a conformance test rather than by reviewer attention.

### The mapping

Let a production type `T` be declared in repo-relative path
`<Project>/<rest…>/<File>.cs` with namespace `<Ns>`.

| | Rule | Determined by |
| --- | --- | --- |
| **1. Test project** | `UnitTests.<Project>` — the production file's **first path segment**, prefixed. | path |
| **2. Directory** | `<rest…>` reproduced verbatim under that project. | path |
| **3. File** | `<TypeName>Tests.cs`. | type name |
| **4. Namespace** | `<Ns>` — the production type's own namespace, unchanged. If the language or an existing symbol collision forbids it, `Tests.<Ns>` and nothing else. | namespace |

Rules 1–3 give the path; rule 4 gives the second lookup key. Both are needed because the two things a
reader has in hand are a file path (from a diff) and a fully-qualified name (from a stack trace), and
each must lead to the test without a search.

```
N1CarWashesAPI.Admin/Databases/CarWashes/Admin/SQL/SqlUserJourneyList.cs
  namespace N1CarWashesAPI.Database.Databases.CarWashes.Admin.SQL

→ UnitTests.N1CarWashesAPI.Admin/Databases/CarWashes/Admin/SQL/SqlUserJourneyListTests.cs
  namespace N1CarWashesAPI.Database.Databases.CarWashes.Admin.SQL
```

Note the divergence in that example: the file sits under `N1CarWashesAPI.Admin/` while its namespace
says `N1CarWashesAPI.Database`, because several projects compile into `N1CarWashesAPI.Core` through
`<Compile Include>` globs. **The path rule follows the path and the namespace rule follows the
namespace** — neither tries to reconcile them. Reconciling is a separate, larger question; the location
rule is deliberately indifferent to it.

For the abstract case the user's shorthand describes — production type `Foo.Bar` in `Foo/Bar.cs`:

```
Foo/Bar.cs, namespace Foo      →  UnitTests.Foo/BarTests.cs, namespace Foo (else Tests.Foo)
```

### Edge cases, all resolved deterministically

| Case | Rule |
| --- | --- |
| **Several test files for one type** | All tests for `T` live in the directory rule 2 gives, in files matching `<TypeName>*Tests.cs`. The suffix is a `#region` or method name per [`csharp_unit_tests.md`](../csharp_unit_tests.md). The *directory* stays exact; only the filename varies |
| **Several types in one file** | Each type gets its own `<TypeName>Tests.cs`. The rule keys on the type, not the file |
| **`partial` type across files** | One test directory, one or more `<TypeName>*Tests.cs`. Partiality is invisible to the rule |
| **Nested type `Outer.Inner`** | `<Outer><Inner>Tests.cs` in `Outer`'s directory. Never a nested test class |
| **Generic type `Foo<T>`** | `FooTests.cs`. Type parameters and CLR arity suffixes are dropped |
| **Type renamed** | The test file is renamed in the same commit. A rename that leaves the test behind breaks the rule and the gate catches it |
| **Type moved between projects** | The test moves with it, to the new project's `UnitTests.` twin |
| **No `UnitTests.<Project>` exists yet** | Create it. A production project with no test twin is itself a finding |
| **Interface or abstract type** | Tested through its implementations; the interface file needs no test. Rule 5 below exempts it |

### Rule 5 — what the rule does not require

Not every production file owes a test file. These are exempt, and a conformance test must encode the
exemptions rather than accumulate a suppression list:

- Interfaces, and types with no executable member (records, DTOs, marker attributes, pure enums —
  enums are annotated per [§ Types, enums](#types-enums-and-enum-members) but need no file).
- Generated code, `Migrations/`, and `N1CarWashesAPI.Database/SQL/**`.
- `Program.cs` and DI composition roots.
- Test infrastructure itself (`N1CarWashesAPI.TestLibraries`).

### What the other suites mean for the rule

`ContractTests.*`, `FailureModelTests.*` and `IntegrationTests.*` are **not** part of the mapping and
never satisfy it. They are cross-cutting by design — a contract test deliberately sweeps types it does
not name. A member covered only by one of them is 🙉: genuinely exercised, not locatable from its own
name. That is not a criticism of those suites; it is exactly what 🙉 was introduced to say.

### Status

⚠️ **The rule holds nowhere near universally today, and no gate checks it.** `SqlUserJourneyList` above
has no file at its computed path — it is reached only through
`UnitTests.N1CarWashesAPI.Admin/DAL/Implementations/UserJourneysDb/UserJourneysDbTests.cs`, which the
rule cannot derive from anything. The count of types in that position is unknown; producing it is
[wave 0](06-rollout-plan.md#wave-0), and the gate that would hold the line afterwards is
[07 § The location gate](07-enforcement.md#the-location-gate).

⚠️ **Existing tests are not moved wholesale.** A file is relocated when it is next edited, the same
containment strategy the `Test_` prefix removal uses. A mass move would be a large mechanical diff
across ~10,900 test methods with no behavioural benefit, and would destroy `git blame` on the test
corpus at the moment the convention most needs it readable.

A member whose test exists but is not at its computed location is 🙉, not ✅ — being tested somewhere is
not the same as being verifiable by the next reader.

## Graduation

Same rule as prose, with the monkeys added to the blocking set: **when a source file contains no ⚠️,
no ❌ and no 🙈 🙉 🙊, delete every ✅ from it.** A fully covered, defect-free file carries no marks at
all — which is what the vast majority of files should eventually look like, and why a marked file is
worth stopping on.

## Interaction with the existing rules

- Coverage remains gated at ≥95% line and branch on changed code by [`AGENTS.md`](../../../AGENTS.md).
  This convention does not change the gate; it makes the residual 5% *named*.
- The unit-test style guide ([`csharp_unit_tests.md`](../csharp_unit_tests.md)) governs test shape,
  naming and `[Theory]` use. Nothing here overrides it. The one addition is the location rule
  above, which constrains *placement* rather than form.
- ⚠️ Neither guide references this document yet.
