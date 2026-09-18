# 04 — API contract annotation (OpenAPI / Swagger)

> Legend for *this* document: ✅ implemented and verified · ⚠️ not implemented · ❌ implemented but
> broken · ⛔ blocked.
> The rules below govern what appears in the **generated OpenAPI document**, which uses a deliberately
> different, smaller vocabulary.

## The consumer is not one of us

Everyone else reading a mark in this repository can follow the link to this convention. A consumer of
the OpenAPI document is an integrator at Django, in the N1 app, or at a third party. They see a
rendered Swagger UI page with no link, no `README.md` beside it, and no way to ask. Three consequences
follow.

## ✅ never appears in an OpenAPI description ✅

For an external consumer, "this endpoint works" is not information — it is the default assumption, and
the reason they are reading the page at all. A tick on 200 operations and not on the 201st communicates
nothing except by omission, and omission is exactly what a partially-migrated document will produce by
accident.

So: **no ✅ in any `[SwaggerOperation]`, `[SwaggerResponse]`, `<summary>` that flows into the OpenAPI
document, schema description, or example.** An unmarked operation is an operation that works. That is
the whole vocabulary the happy path needs.

(The ✅ in this section's heading is the marker for the rule itself, which *is* implemented in this
document. It is the sort of thing that has to be stated once and then never repeated.)

## ⚠️ and ❌ do appear, and must explain themselves inline

A consumer needs to know when an endpoint is a stub or is lying. They cannot look the symbol up, so
every occurrence carries its own explanation, in full, every time. Repetition across operations is
correct here — the reader sees one operation, not the document.

**Required phrasing.** The mark, then a bracketed gloss naming the mark's meaning, then the specifics
and what the consumer should do.

```csharp
[SwaggerOperation(
    Summary = "Reverses a completed wash and refunds the sale.",
    Description =
        "⚠️ NOT IMPLEMENTED — this operation is defined but not built. It currently returns " +
        "501 Not Implemented on every call. Do not depend on it; the request and response shapes " +
        "below are the intended contract and may still change before it ships.")]
```

```csharp
[SwaggerOperation(
    Summary = "Lists sales for a station.",
    Description =
        "❌ KNOWN DEFECT — this operation is implemented and returns data, but `totalAmount` " +
        "excludes subscription washes, so it under-reports for stations with subscribers. " +
        "Sum the `lines` array instead until this is fixed.")]
```

The rules those examples encode:

- **Spell the meaning in words on every occurrence** — `⚠️ NOT IMPLEMENTED` and `❌ KNOWN DEFECT`. The
  glyph is a scanning aid for a human eye; the words are what a translation, a screen reader, a
  terminal without emoji fonts, or an integrator's own doc pipeline will actually carry.
- **Lead the `Description` with it.** Swagger UI truncates and collapses; a mark in the last paragraph
  is a mark nobody sees.
- **Say what the consumer should do instead.** A workaround, a fallback field, or an explicit "do not
  call this yet". A defect an integrator cannot route around is a defect they will ship on top of.
- **⛔ never appears here either.** Who we are waiting on is our problem, not the integrator's. A
  blocked operation is ⚠️ or ❌ to a consumer and nothing more; the blocker lives in the internal
  document that tracks it.
- **Never mark the `Summary`.** The summary is the collapsed one-line title in Swagger UI and in
  generated clients; keep it a clean description of intent so the generated method name and tooltip
  stay usable.
- **Field-level marks go on the schema property description**, in the same form, when the endpoint
  works but one field does not.
- ⚠️ **A deprecated operation is not a marked one.** `deprecated: true` is native OpenAPI and says
  something different — it works, and it is going away. Use the real mechanism, not a glyph.
  `UX700PetrolFacadeV1Controller` already does this correctly.

## Where the text lives

Descriptions reach the OpenAPI document through `Swashbuckle.AspNetCore.Annotations` attributes on
controller actions across `N1CarWashesAPI.Pin`, `.Subscriptions`, `.Sms`, `.UX700PetrolFacade`,
`.Simulators` and others, and through `Swashbuckle.AspNetCore.Filters` example providers. The
annotation goes wherever the consumer-visible text is authored.

⚠️ **XML documentation comments are not currently included in the generated document**
(no `IncludeXmlComments` registration, no `GenerateDocumentationFile` in `Directory.Build.props`), so a
`///` summary on a controller action is *internal* text today and follows [03](03-code-annotation-convention.md),
including ✅. If XML comments are ever wired into Swashbuckle, every controller-action ✅ becomes
consumer-visible and this rule breaks silently. The gate that would prevent it is in
[07 § The Swagger scan](07-enforcement.md#the-swagger-scan) and does not exist.

## Contract snapshots

`documentation/Contracts/wire-contracts.json` and its `.sha256` are machine-generated shape snapshots.
❌ **Do not annotate them** — a mark inside a hashed artefact changes the hash and breaks the
comparison it exists to perform. Statements about contract *state* belong in the prose document that
describes the contract, not in the snapshot.

## Status

- ⚠️ No operation in the repository carries a mark under this convention.
- ⚠️ Nothing prevents a ✅ from being added to a `[SwaggerOperation]` today.
- ⚠️ `.github/instructions/facades.instructions.md` — the file an agent reads before touching a
  facade controller — does not mention this rule.
