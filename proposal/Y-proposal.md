# Y: a language for agents to write and people to review

**Status:** Proposal
**Date:** 2026-09-24
**Written for:** the language owner, deciding what Y becomes next; and, once decided, whoever writes the normative core.
**Relationship to other records:** Builds on [`../Y-current-language-design.md`](../Y-current-language-design.md) (2026-09-17)
and keeps all fifteen of its settled commitments. Where the two disagree, this document says so and says why. The ideas it
draws on, and where each came from, are catalogued in [`Y-idea-harvest.md`](Y-idea-harvest.md).

Syntax in this document is proposed, not illustrative: every example is written as the proposal intends Y to be spelled,
so that the examples can become the first conformance corpus.

---

## 0. The proposal on one page

The September design settled who is trusted: *AI proposes, deterministic tooling verifies.* It left implicit what the
human does. This proposal makes it explicit: **agents write Y, and people review samples of it.** A reviewer will not
read every line an agent emits. They will read what the toolchain puts in front of them, and they must be able to judge
each piece without reading anything else.

Three rules follow, and most of this document is their consequences.

1. **A sample must be judgeable on its own.** Everything a reviewer needs to know about a function — what it may touch,
   how it may fail, what it may wait on, what it promises, what it changes — is written in its declaration, and the
   compiler checks what is written against what the body does. Inference is how the agent finds the answer; it is not a
   reason to hide the answer from the reader. (*Inferred, then pinned* — § 1.2.)
2. **Every decision has a name.** Conditions, branches, guards, contracts, constraints, transactions and tasks carry
   names. A name is how a test declares what it covers, how a trace says what happened, how a diff stays stable across
   edits, and how a reviewer reads intent without decoding the expression. (§ 3.5)
3. **Anything that asks for trust is a claim.** Where the compiler cannot verify something, the source says so in a
   closed vocabulary, with a reason, and the toolchain routes every claim to a person. (§ 3.15)

A function, as a reviewer would be shown it:

```y
---
Holds one unit of an item for a buyer and asks for payment, exactly once.

why: Stock is never oversold, and a transaction that retries never charges twice.
item: The item being bought.
buyer: Who pays, and with what.
returns: The hold, waiting for the charge to be confirmed.
fails UnknownItem: The item is not stocked here.
fails TimedOut: No unit became free within five seconds.
---
public function hold_and_charge(item: ItemId, buyer: &Buyer) -> Hold ! HoldError
    with inventory: Shared<Inventory>, charges: Enqueue<ChargeCard>
{
    within 5s {
        atomic hold_one_unit {
            let stock = inventory.stock_of(item)?
            if sold_out = (stock.available == 0) {
                retry
            }
            stock.available -= 1
            let price = stock.price

            let hold = inventory.open_hold(item: item, buyer: buyer.id)
            enqueue ChargeCard { hold: hold.id, amount: price, method: buyer.payment_method }
            hold
        }
    }?
}
```

Without opening another file, a reviewer can see that this function touches exactly two things (one shared inventory,
one class of durable command), that it can fail in exactly two documented ways, that it waits at most five seconds,
that the transaction may re-run and the charge still happens once (because `enqueue` commits with the transaction), and
which branch a test must reach to cover the sold-out path (`hold_and_charge#sold_out`). None of those facts is a
convention; each one is checked, and the compiler rejects the program if any of them is false.

**Kept from September**, unchanged in meaning: deterministic semantics; Core IR as the definition of a program; ownership
and borrowing with no hidden clones; typed failure; software transactional memory with no external effect inside a
transaction; durable commands and workflows; bounded suspension without `async` colouring; structured concurrency;
contracts, typestate and units; versioned schemas; independent oracles.

**Brought back from earlier iterations:** named decisions and named results (YAPL, 2020–2024); mandatory, closed-grammar
documentation (spec blocks, April 2026); authority passed explicitly, never ambient (YAPL's OS context, 2024; y-string's
context object, March 2026); units on every length and index (July and March 2026); tests that declare what they cover
(toolchain, October 2025); fixtures generated from constraints (October 2025); *provable or explicitly missing*
(ai.lang, March 2026); gaps recorded where the code is (knowledge-driven development, August 2026).

**Dropped**, with reasons in § 7: significant indentation; lowercase type names; `x is integer` annotations; classes,
facets, factory methods and inheritance; exceptions; ambient `using` contexts; transactional memory for *all* state;
affinities and placements in source; comments with runtime meaning; the spec block as a replacement for the signature;
`afterCommit`.

## 1. The premise, and what it changes

### 1.1 Writing is cheap; reading is not

For an agent, writing a line costs almost nothing, and fixing a line the compiler rejected costs a round trip that ends
with the compiler's exact instruction. For a person, reading a line costs attention, which is the scarcest resource in
the loop. Two consequences:

- **Ceremony is only a cost where a machine cannot write it or a person cannot skip it.** A clause the compiler can
  infer, the fixer can write, and the reviewer can glance past (or read, when it matters) is nearly free. The September
  argument against writing effects in signatures — *an agent will get it wrong and a human will stop reading* — loses its
  first half when the agent is told exactly what to write, and its second half when the clause sits where a reviewer
  looks for it.
- **Surface novelty is a cost, not a feature, unless the compiler enforces it.** In the October 2025 toolchain, agents
  writing against a spec with idiosyncratic spelling (`row_count is integer`, lowercase type names, `--` comments)
  drifted into the mainstream forms they had been trained on — `let`, `name: type`, PascalCase, `$interp`, `#` comments
  — and nothing caught it. Y borrows most of its shape from Rust, Swift and Kotlin, which is what agents already write
  and reviewers already read. Where it departs, the departure is a rule the compiler checks and `y fix` applies:
  - Y spells its own words in full, as it requires of every name (`public function`, not `pub fn`).
  - Its comments are `--` and `---`, as they have been since YAPL.

  An agent that writes `fn` or `//` from habit gets a diagnostic naming the replacement. This costs one round trip,
  and nothing reaches a reviewer.

### 1.2 Inferred, then pinned

Y infers types inside bodies, ownership inside bodies, effects, suspension points and failure paths — and then **pins
the facts a reviewer needs into the declaration**, where they become checked claims:

| Fact | Where it is pinned | Written by |
| --- | --- | --- |
| Parameter and result types | Signature | Agent (public signatures are never inferred) |
| Ownership mode of each parameter and result | Signature (`T`, `&T`, `&mutable T`, `-> &T from x`) | Agent, corrected by fix-its |
| Ownership transfer at a call | Call site (`&x`, `&mutable x`, `move x`) | Agent, corrected by fix-its |
| Authority, and therefore effects | `with` clause | `y fix`, from inference |
| Failure set | `-> T ! E`, and `fails` doc roles | Agent; the compiler checks exhaustiveness of documentation |
| Preconditions and guarantees | `requires` / `ensures` | Agent |
| Wait bounds | `within` | Agent; an unbounded wait does not compile |

A pinned clause that no longer matches the body is a compile error, so a change in what a function may do cannot land
without the declaration changing — and the declaration is where the diff shows it. Facts that are *not* pinned
(suspension points, transaction read and write sets, inferred borrows inside a body) are printed on request by
`y explain` (§ 4.3).

### 1.3 Review is sampling

Nobody reviews everything. The toolchain decides what a person sees, and the language has to make that decision
computable. Three mechanisms, each described later:

- **Semantic diff** (§ 4.4). Every function has a *compatibility signature*: its types, authority, failure set,
  contracts, transactional writes and command classes. A change is classified by what it did to signatures, not by how
  many lines moved. Widened authority is always shown; a body-only change under an unchanged, fully tested signature is
  sampled.
- **Claims** (§ 3.15). Every construct that asks the reader to trust the author — `unsafe`, `forever`, `detach`,
  `assume`, a suppressed lint — carries a `because` and is always shown.
- **Gaps** (§ 3.16). Unfinished work is a typed construct the compiler knows about, not a comment it does not.

### 1.4 Names are identities

Stable identity is needed by coverage (which branch did this test reach?), by traces (which decision did production
take?), by review (was *this* function approved before?), by oracles (which construct disagrees?), and — later — by
hot swap (§ 5). Line numbers change on every edit; names do not. So Y names things aggressively, derives every
identifier from a name (`shop.checkout.hold_and_charge#sold_out`), and treats renaming as a refactoring the toolchain
records (`y rename`), not as a text edit.

### 1.5 Tie-breaks for open questions

The September principle (§ 32 of the working design), reordered and sharpened for this premise. Prefer the option that:

1. gives the compiler more semantic information;
2. makes invalid states unrepresentable;
3. stays deterministic;
4. lets a reviewer judge a sample without leaving it;
5. leaves an agent exactly one way to write it;
6. can be projected into an independent checker;
7. costs a person attention only where a machine could not have paid instead.

## 2. A tour: one change, as the agent and the reviewer see it

The September journey ([`../documentation/journeys/agent-writes-a-function.md`](../documentation/journeys/agent-writes-a-function.md))
told this story in prose. Here it is in the proposed language.

**The agent's first draft** puts the charge where it reads naturally:

```y
        atomic hold_one_unit {
            -- ...
            stock.available -= 1
            gateway.charge(method: buyer.payment_method, amount: stock.price)?
        }
```

**The compiler rejects it**, and the diagnostic names the replacement rather than only the prohibition — a rule this
strict gets worked around if it only says no:

```text
error[Y0412]: external effect inside a transaction
  --> src/shop/checkout.y:27:13
   |
27 |             gateway.charge(method: buyer.payment_method, amount: stock.price)?
   |             ^^^^^^^^^^^^^^ `Http<PaymentGateway>` performs network I/O
   |
   = why: `atomic hold_one_unit` may run more than once; this call cannot be undone, so it may happen more than once
   = fix: submit a durable command, which is persisted if and only if the transaction commits:
          enqueue ChargeCard { hold: ..., amount: ..., method: ... }
          (needs `Enqueue<ChargeCard>` in the `with` clause; `y fix` adds it)
   = spec: y:spec/shared-state#no-external-effect
```

The agent switches to `enqueue`. The next build fails again — `retry` waits for the inventory to change, and nothing
bounds that wait:

```text
error[Y0301]: unbounded wait
  --> src/shop/checkout.y:22:17
   |
22 |                 retry
   |                 ^^^^^ waits until `Inventory.stocks` changes, with no deadline above it
   |
   = fix: bound the enclosing scope, e.g. `within 5s { ... }`; the expiry becomes a typed failure
```

After `within 5s { ... }?`, the compiler reports that the failure set now includes `Expired` and that `HoldError` has
no case carrying it. The agent adds `case TimedOut(deadline: Expired)`, and `y fix` pins the authority it inferred:

```text
$ y fix src/shop/checkout.y
  pinned  with inventory: Shared<Inventory>, charges: Enqueue<ChargeCard>   (hold_and_charge)
  added   fails TimedOut: ...   (documentation for the new failure case; prose left for the author)
```

**The test** lives at the path computed from the module (`src/shop/checkout.test.y`), declares which outcome it covers,
supplies capabilities the same way production code receives them, and forces the retry path instead of hoping for it:

```y
-- A conflicting commit forces the transaction to run three times; the customer is still charged once.
test charges_once_when_the_transaction_retries
    covers hold_and_charge#sold_out.else
{
    given {
        let widget = ItemId.parse("widget")?
        let buyer = any<Buyer>(seed: 11)
    }
    with {
        inventory = Shared.new(Inventory.stocked(item: widget, available: 1, price: 1_500isk))
        charges = fake Enqueue<ChargeCard>
    }
    faults {
        conflict at hold_and_charge#hold_one_unit, times: 2
    }
    when {
        let hold = hold_and_charge(widget, &buyer)?
    }
    then {
        assert charged_once = (charges.committed().count() == 1)
        assert held_the_widget = (hold.item == widget)
        assert no_stock_left = (inventory.snapshot().stock_of(widget)?.available == 0)
    }
}
```

**The reviewer** does not open the diff first. They open the review packet, which is ordered by risk:

```text
$ y review main..hold-and-charge

Signatures
  + shop.checkout.hold_and_charge                         new, public
      authority    Shared<Inventory>, Enqueue<ChargeCard>
      fails        UnknownItem, TimedOut
      waits        at most 5s
      transacts    hold_one_unit — writes Inventory.stocks, Inventory.holds; enqueues ChargeCard
Commands
  ChargeCard                                              new submitter: hold_and_charge
Claims            none
Gaps              none
Coverage          4 of 4 outcomes covered by 4 tests (#sold_out, #sold_out.else, !UnknownItem, !TimedOut)
                  1 fault schedule recorded (seed 0x5e1d)
Contracts         none declared
Oracles           rustc ownership projection agrees
Bodies shown      1 of 1 (new public functions are always shown)
```

Everything the September journey needed a reviewer to *know* — never call a payment provider inside a transaction,
always pass a timeout — became a compile error with the remedy attached. What is left for the person is what only a
person can judge: is five seconds right, and is this the behaviour the business wanted.

## 3. The language

### 3.1 Source form

**Files and modules.**

- A module is one file. Its first line is `module shop.checkout`, and it must match the path `src/shop/checkout.y`.
  The redundancy is deliberate: a sample shown out of context still says where it lives.
- Imports name what they bring in: `use shop.inventory.{Inventory, ItemId}`. There are no wildcard imports, no
  re-exports and no relative paths. An import cycle is an error that lists the cycle.
- The formatter sorts imports (standard library, then dependencies, then this package). Each name means the same thing
  everywhere in a package, so an alias (`as`) is allowed only to resolve a collision.
- Visibility is `public` or private to the module.
- A module contains declarations only. There is no executable code at module scope, no module initialiser, and no
  mutable global state of any kind. State that outlives a call is passed in (§ 3.7).

**Names.**

- `snake_case` for values, functions, fields and modules. `PascalCase` for types, cases, traits, states and commands.
  The compiler enforces both, with no leading, trailing or doubled underscores.
- This departs from the long-standing lowercase-everything rule. The departure is deliberate: it matches what agents
  write unprompted, and it lets a reviewer tell a type from a value at a glance.
- Identifiers are ASCII. Bidirectional-control and invisible characters are rejected everywhere in source except as
  escapes inside text literals. A reviewer must be able to trust that what they see is what compiles.
- **No abbreviations,** except those listed in the project glossary (`id`, `url`, `http`, …). The linter checks this.
  - The rule binds the language as much as its programs. Keywords are words: `public`, `function`, `variable`,
    `constant`, `implementation`, `&mutable`.
  - So are the standard library's names (`Integer`, `Boolean`, `ReferenceCounted<T>`, `divide_floor`) and the
    toolchain's commands (`y format`).
  - Unit symbols declared with `unit` (`s`, `km`, `isk`) are symbols, not abbreviations.
- Lengths, sizes and durations do not need units in their names, because units are in their types (§ 3.3).

**Bindings.**

- `let` binds once and `variable` may be reassigned. Every binding is initialised where it is declared.
- **No shadowing:** a name visible in a scope may not be declared again inside it, including parameter names. Within a
  function, a name means one thing.

**Expressions.**

- Boolean operators are words: `and`, `or`, `not`. This keeps `!` for failure types (§ 3.4).
- Mixing operator families requires parentheses. Write `a + (b * c)` and `(a < b) and (c < d)`. Chains of one
  operator need none (`a + b + c`). Chained comparisons (`a < b < c`) are rejected. A reader should never have to
  recall a precedence table.
- `/` is defined only on fractional types.
  - Integer division is spelled with the rounding it performs: `divide_floor`, `divide_truncated`, `divide_euclidean`,
    or `divide_exact`, which fails with `Inexact` if there would be a remainder.
  - There is no `%`; use `remainder_euclidean` or `remainder_truncated`.
  - Arithmetic is checked, and overflow is a fault (§ 3.4).
- Conversions that can lose information say so in their names (`to_integer_truncated`,
  `to_integer_rounded(.HalfEven)`). No conversion is implicit.
- Interpolation accepts a name or a field path and nothing else: `"hold {hold.id} for {buyer.name}"`. Calls,
  indexing and arithmetic are computed into a named local first.
- Literals that are small languages are checked at compile time: `pattern"^[a-z]+$"` (a regular expression),
  `url"https://example.com"`, and format strings. Y has no query strings (§ 3.13).
- **`if`, `match` and loops are statements.** They produce no value, and Y has no conditional expression and no ternary
  operator.
  - A value chosen by a condition is assigned in each branch. The decision then has a name and a place, like every
    other: `variable head = " "`, then `if has_head = (misses >= 1) { head = "O" }`.
  - A `match` arm runs a statement: a call, an assignment, `return`, `fail`, `break` or `continue`.

**Calls.**

- Arguments are positional only when the callee has one parameter, or two parameters of different types. Otherwise
  every argument is named: `transfer(from: savings, to: checking, amount: 50eur)`.
- The formatter removes names where positions are unambiguous and adds them where they are required, so there is one
  canonical spelling of every call.

**Closures.**

- A closure is a single expression: `users.filter(user => user.active)`.
- Anything longer is a named local function that lists what it captures (`function add_line(line: &Line)
  captures &mutable total { ... }`). The name appears in traces, and the capture list shows what the helper can reach.

**Comments and layout.**

- **`--` starts a comment** that runs to the end of its line.
- **A line of three or more dashes, alone, opens a block comment.** The next line of exactly the same dashes closes it,
  so a block that must quote a `---` line opens with `----`.
  - The width belongs to the block, not to the file: editing one block never changes another.
- **A comment directly above a declaration, with no blank line between, is its documentation** (§ 3.2). Every other
  comment is free text.
- `//` and `/* */` are not comments in Y. The diagnostic names `--`, and `y fix` rewrites them.
- Commented-out code is a lint error. Delete it; history keeps it.
- Blocks are braced. `y format` has no options and preserves every comment. It produces one layout for any program, so
  every diff is about content.
- A function body longer than 60 lines is a lint warning, and longer than 120 is an error. A reviewable sample fits on
  a screen.

### 3.2 Declarations and documentation

**Declaration kinds.** Y has few: `function`, `record`, `type`, `state`, `transition`, `trait`, `implementation`,
`constant`, `unit`, `schema`, `migrate`, `command`, `handler`, `workflow`, `derived`, `rule`, `test`.

- There are no classes, facets, singletons, factory methods or inheritance.
- A constructor is an ordinary function returning the type (`new`, `parse`, `from_…`).
- Behaviour is shared through traits with explicit `implementation`s. Every iteration that had more kinds of type than
  this spent its time reconciling them.

**Documentation is grammar, written as a comment.** The April 2026 spec block was right that every declaration a reader
relies on needs structured documentation in a closed grammar. It went further and made the block *replace* the
signature, which the examples themselves then abandoned for small helpers. Y keeps the signature in code.

Its documentation is the comment directly above it: a `--` line for a summary alone, or a `---` block. That comment is
read in a closed set of roles:

```y
---
Takes money out of an account.

account: The account to debit.
amount: How much to take.
returns: A receipt for the debit.
fails InsufficientFunds: The balance is lower than the amount.
---
public function withdraw(account: &mutable Account, amount: Money<Isk>) -> Receipt ! WithdrawError
```


| Role | Meaning | Required on |
| --- | --- | --- |
| *(summary)* | First paragraph; one sentence saying what the declaration is for | Every `public` declaration, every `test`, every `command` |
| `why:` | Why it exists or is shaped this way, where the name does not say | Optional; required on claims (§ 3.15) |
| `<parameter>:` | One per parameter, by name | Every parameter of a `public function` |
| `returns:` | What the result means | Every `public function` not returning `Void` |
| `fails <Case>:` | When and why each failure case occurs | Every case of a `public function`'s failure type |
| `satisfies:` | A `y:` link to the requirement or journey this implements | Optional |
| `see:` | A `y:` link to a related declaration or document | Optional |
| `gap:` / `defect:` / `blocked:` | Unfinished or broken behaviour (§ 3.16) | Where it applies |

The compiler checks the documentation against the declaration:

- A parameter documented but not declared, or declared but not documented, is an error.
- A `fails` role for a case the failure type does not have is an error.
- Any prose line of the shape `word:` must be a valid role (`Y0102`). An agent cannot confuse a stray colon with a
  structured field, and a reader cannot either.

Prose is constrained CommonMark:

- No HTML and no images.
- Links go only to `https:`, `mailto:` or `y:`.
- `y:` links are resolved by the compiler: `y:shop.checkout.hold_and_charge#sold_out`,
  `y:requirements/checkout.md#no-oversell`, `y:spec/shared-state#no-external-effect`. A broken link is a build error.

**Traceability is one role, not an ontology.**

- `satisfies:` is the whole of what survives from ai.lang's five-layer graph inside the language.
- From it the toolchain can answer the two questions that graph existed for: *which code implements this requirement*,
  and *which requirements have no code*.
- The organisational layers (visions, personas, owners, on-call) belong in documents the links point to, not in the
  language.

### 3.3 Types

**Scalars.**

- `Boolean`, and `Integer`, which is 64-bit and checked.
- `Integer8` … `Integer64` and `UnsignedInteger8` … `UnsignedInteger64`; `Float64`; `Decimal`.
- `Void` is an ordinary type with one value.

There is no truthiness, no null, and no implicit numeric widening.

**Records** have named, typed fields and may declare named invariants:

```y
record Stay {
    guest: GuestId,
    arrives: Date,
    departs: Date,

    invariant departs_after_arrival = (departs > arrives)
}
```

**Sum types** are closed. Every case payload has named fields, because Y has no tuples anywhere:

```y
type PaymentResult {
    case Approved(authorization: Authorization)
    case Declined(reason: DeclineReason)
    case Unavailable
}
```

Referring to a case:

- A case is written `.Approved(...)` where the expected type decides it.
- Where the type is ambiguous, write it qualified, `PaymentResult.Approved(...)`. An unqualified case the compiler
  cannot place is an error, never a guess.
- A pattern binds a payload field under its own name (`.Declined(reason)`), or under a new one
  (`.Declined(reason: why_declined)`).

**Absence.**

- `T?` is a distinct type from `T`. Neither converts to the other implicitly.
- `??` supplies a default, `?.` reaches through absence, and `match` covers `.Some(value)` and `.None`.
- **A condition narrows.** Inside `if found = (user != .None) { ... }`, and after `if missing = (user == .None) {
  return ... }`, `user` is a `User`. Nothing is written for this: every condition adds its predicate as a fact at the
  points it guards (constraint language § 11.1.1), and the type checker reads the fact.
  - The fact ends where the value may change: an assignment to a `variable`, or a call that borrows it `&mutable`. A field path
    such as `order.customer` narrows the same way, until something may write to that path.
  - The same facts discharge a callee's `requires` (§ 3.11) when they imply it.

**Several results.** A function that produces several results returns a record. For a one-off result, write an
anonymous record type with named fields: `-> { quotient: Integer, remainder: Integer }`.

**Refined types.** A type may be a named restriction of another. Each restriction has a name, and construction checks
all of them:

```y
type Quantity = Integer where positive = (value > 0)
type EmailAddress = Text where has_one_at_sign = (value.count_of('@') == 1)
```

- `Quantity.new(n)` returns `Quantity ! ConstraintViolation`.
- A literal is checked at compile time: `let q: Quantity = 0` does not compile.
- A violation names the constraint that failed and the values of its subexpressions. Constraints never carry
  hand-written messages; the name *is* the message.
- The same declaration drives test data. `any<Quantity>(seed: 7)` produces values that satisfy every constraint, and
  `any_violating<Quantity>(seed: 7)` produces values that break exactly one.

**Units.** A quantity's unit is part of its type:

- `5s`, `12.4km` and `1_500isk` are typed literals. Arithmetic checks dimensions, and conversions are explicit, with the
  factor declared once on the unit.
- A currency is a dimension, not a scale factor, so ISK and EUR never convert at an ambient rate.
- Units are erased before code generation.

The same mechanism gives every length and index its unit:

- `text.count() -> Count<Rune>`, `bytes.count() -> Count<Byte>`, `list.count() -> Count<Element>`.
- `Index<Rune>` does not index `Bytes`.
- An integer *literal* takes its count or index unit from context, so `list.count() == 1` needs no ceremony. An `Integer`
  *value* never converts implicitly. Physical units get no such allowance: `within 5` is an error, because `5` could
  mean seconds or milliseconds, but a count of runes can only mean runes.
- This is the July 2026 rule (*always name the unit: `length_in_bytes`, never `size`*) and the March 2026 typed lengths,
  moved from naming convention into the type checker.

**Text.**

- `Text` is a sequence of `Rune`s. A `Rune` is exactly one extended grapheme cluster, and text is NFC-normalised when
  it is constructed, so canonically equivalent text is equal.
- `Bytes` is a separate type. Conversion between the two names its encoding and what to do with invalid input
  (`Text.decode_utf8(bytes, on_invalid: .Reject)`), and rejecting is the default.
- Nothing is locale-sensitive by default. Case folding, collation and formatting take a `Locale` argument, and no
  locale is ever inferred from the process.

**Equality, hashing and ordering** are derived structurally by the compiler, and cannot be written by hand. A hand-written
`equals` is a place for an agent to be subtly wrong and for a reviewer to miss it.

- A field marked `transient` is excluded from equality, hashing and serialisation.
- Comparison returns `Ordering` (`.Less`, `.Equal`, `.Greater`), never an integer.

**Sensitive data.** `Sensitive<T>` wraps values such as credentials and personal data.

- It cannot be interpolated, logged, traced or serialised outside a schema field declared sensitive.
- `reveal(value)` requires the `Reveal` capability (§ 3.7).
- Fault reports and traces mask it.

### 3.4 Failure

`T ! E` is the result of a computation that produces a `T` or fails with an `E`.

- It is a built-in two-case sum type. This settles the September open question: one `match` ranges over success and
  failure together, and `T? ! E` composes without a special rule. Core IR still records every propagation edge
  explicitly, so *which failures can escape, and from where* is always answerable.
- Inside a function returning `T ! E`, success is the plain value (`hold`, not `.Ok(hold)`) and failure is written
  `fail .InsufficientFunds(shortfall: gap)`. Every failure exit is spelled `fail` or `?`, so every one of them can be
  found with a search, and there is exactly one way to write each.
- `?` propagates a failure unchanged, or converts it through the single case of the enclosing failure type that
  carries it. If two cases could carry it, the program is rejected and the error names both candidates.
- A failure leaving an `atomic` block aborts the transaction, which leaves no trace (§ 3.9). This is the July 2026
  rollback-on-failure rule, carried into the language.

```y
match load_customer(id) {
    .Ok(customer) => greet(&customer),
    .Err(.NotFound(id: missing_id)) => offer_signup(missing_id),
    .Err(.Unavailable(retry_after)) => try_later(retry_after),
}
```

**Faults are not failures.** A violated contract, arithmetic overflow, an exhausted allocator, a reached `todo`, or a
failed `assert` is a *fault*, which means a bug. Faults:

- do not appear in signatures;
- cannot be caught by ordinary code;
- end the enclosing task scope, which its supervision policy then handles (§ 3.8).

A fault report carries:

- the stable identity of the construct that faulted;
- the name of the violated predicate, with the values of its subexpressions;
- the non-sensitive locals of each frame, which is YAPL's traceback rule;
- the code version;
- a report identifier short enough to read aloud.

### 3.5 Every decision has a name

Every iteration from YAPL v4 (2022) to April 2026 required conditions to be named. The September design did not.
This proposal restores the rule and extends it to every predicate in the language:

| Construct | Form |
| --- | --- |
| Condition | `if found = (items[middle] == target) { ... }` |
| Alternative | `else if go_right = (items[middle] < target) { ... }` |
| Loop | `while searching = (low <= high) { ... }` |
| Guard | `.Some(user) when active = (user.last_seen > cutoff) => ...` |
| Precondition, guarantee | `requires positive = (amount > 0isk)`, `ensures debited = (...)` |
| Invariant, constraint | `invariant departs_after_arrival = (...)`, `where positive = (value > 0)` |
| Assertion | `assert charged_once = (charges.committed().count() == 1)` |
| Transaction, task | `atomic hold_one_unit { ... }`, `task profile = load_profile(id)` |

**Exemptions and defaults.**

- A predicate that is already a single boolean name, or `not` of one, needs no second name: `if is_empty { ... }`.
- `else` may be named (`else in_range { ... }`). An unnamed `else` is identified as the last condition's name plus
  `.else`.
- A `for` loop is named by its variable. Inside nested loops, `break` and `continue` must name their loop:
  `break line`.
- There is no `loop` and no `while true`.

**What the names are for.** Each name becomes part of a stable address: `shop.checkout.hold_and_charge#sold_out`,
`#sold_out.else`, `#hold_one_unit`. Those addresses are what:

- tests declare they cover (§ 3.14);
- production traces record, when sampled;
- fault reports cite (§ 3.4);
- the review outline shows. `y review --outline` renders a function as its signature, its documentation and its tree
  of named decisions with bodies collapsed, so a reviewer can read at the level of intent and open only what looks
  wrong. This is ai.lang's "zoom", as a view rather than a data model.

```y
---
Finds where a value sits in a sorted list.

target: The value to look for.
items: The list to search.
returns: The target's position, or nothing when it is absent.
---
public function position_of(target: Integer, items: &SortedList<Integer>) -> Index<Element>? {
    variable low = items.first_index()?
    variable high = items.last_index()?
    while searching = (low <= high) {
        let middle = low.midpoint(high)
        if found = (items[middle] == target) {
            return .Some(middle)
        } else if go_right = (items[middle] < target) {
            low = middle.next()
        } else {
            high = middle.previous()?
        }
    }
    .None
}
```

The parameter is a `SortedList`, not a `List` with a `requires sorted` precondition. Where a property can be carried by
a type, Y prefers the type (§ 1.5, rule 2).

### 3.6 Ownership

Y keeps the September model: one owner, shared borrows that exclude mutation, exclusive borrows that permit it,
deterministic destruction, **and no clone the programmer did not write**. What changes is where ownership is written
down.

- **Signatures are explicit.** A parameter is `x: T` (ownership moves in), `x: &T` (shared borrow) or `x: &mutable T`
  (exclusive borrow). A result that borrows names what it borrows from: `fn display_name(user: &User) -> &Text from
  user`.
- **Call sites are explicit.** Passing a non-copy value is written `move order`, `&order` or `&mutable order`. A bare name
  is allowed only for copy types and temporaries. This is YAPL's two-sided `mutable` (2024), generalised. A reviewer
  sees at the call which arguments the callee may change or keep.
- **Bodies are inferred.** Borrows of locals, reborrows and the end of a borrow's life are the compiler's job, and
  `y explain` prints what it concluded.
- **Method receivers borrow as their signature says** (`order.total()` borrows, `order.cancel()` may take `&mutable`), as
  in Rust, because receiver syntax is where agents' habits are strongest. The elaborated view shows the mode.
- **Copy is declared.** A record or sum type marked `copy` (identifiers, small values) is duplicated rather than moved.
  The set of copy types is never inferred from size.

**No type contains a borrow in v1.** Records, sum types and collections own what they hold. Borrows live in
parameters, locals and results only. This one restriction is what lets Y have no lifetime parameters, which is where
agents writing Rust spend most of their failed attempts. Pipelines and iterators are built in, so the compiler handles
their borrows. The restriction is a bet: relaxing it later is additive, but imposing it later would not be.

**Sharing without a single owner:**

- `ReferenceCounted<T>` shares immutable data, and `Weak<T>` observes it without keeping it alive.
- Mutable state reachable from more than one task is `Shared<T>` and nothing else (§ 3.9). There is no interior
  mutability outside transactions.
- Graphs live in an arena or collection and refer to each other by typed index (`Index<Node>`), not by pointer.

**Cleanup.**

- Values are destroyed at the end of their owning scope, in reverse order.
- A type may declare release behaviour that runs exactly once, on normal exit and on failure alike.
- Release that can fail declares its failure type, and a scope that owns such a value includes that failure in its own
  signature. Release failures are never discarded.

**Arenas.** An arena (`arena parse_scratch { ... }`) releases everything allocated inside it at once, and nothing
allocated there may escape. Arena ownership says *when a resource is destroyed*, never *when an operation may run*.
A socket owned by an arena is still an external effect (§ 3.9). This is the September distinction, unchanged.

### 3.7 Authority and effects

**A function can affect nothing outside itself except through a capability it holds.** Capabilities are values, and
effects are what capabilities do. This merges September's two open questions, effect syntax and capability syntax,
into one clause:

```y
public function export_invoices(month: Month) -> Count<Element> ! ExportError
    with invoices: Store<Invoice>, uploads: FileSystem, clock: Clock
```

**The capability kinds**, each declaring the effects it carries:

| Capability | Effects it carries |
| --- | --- |
| `Clock`, `Random` | Reads time, draws randomness. Replay records them; tests fake them |
| `Http<Service>`, `FileSystem`, `Process`, `Secrets<Name>` | External I/O, may suspend |
| `Store<Schema>` | Persistent reads and writes (§ 3.12) |
| `Shared<T>` | Transactional access to shared state, may suspend on `retry` (§ 3.9) |
| `Enqueue<Command>`, `Submit<Command>` | Durable command submission (§ 3.10) |
| `Detach`, `Forever`, `Reveal`, `Native` | Trust-requiring operations, each also a claim (§ 3.15) |

**A function with no `with` clause is pure.** It may allocate, compute and fault, but it touches nothing else. Purity
needs no annotation because the missing clause is the annotation. Contract expressions, constraints, `derived`,
`rule` and migrations must be pure, and it is visible that they are.

**Passing capabilities.**

- A callee's `with` requirements are satisfied, by type, from the capabilities in the caller's scope: the caller's own
  `with` clause and any enclosing `with` block. Call sites do not spell them out, so adding a capability to a deep
  function does not edit every call.
- Every function between the entry point and that deep function must still *declare* the capability, because a
  declaration is where a reviewer looks. `y fix` maintains those clauses.
- When two capabilities in scope could satisfy a requirement, the call must choose:
  `notify(order) with mailer = receipts_mailer`.

**Deriving capabilities.**

- The entry point receives the root, `System`, and every other capability derives from it.
- There is no way to construct a capability from nothing, read one from a global, or recover one from a type.
- Attenuation only narrows: `with uploads = uploads.subtree(path"/var/app/exports") { ... }`. This is the single place
  where a name may be rebound in an inner scope, and it is allowed because it can only reduce authority.
- A package manifest lists the root capabilities a program may request. A dependency's public functions declare what
  they require, so a dependency upgrade that widens required authority is a review item, not a surprise. This is
  YAPL's OS-category approval (2024), made exact.

**Checking.**

- A `with` clause that declares less than the body uses is an error naming the capability and the call path that needs
  it.
- One that declares more is a fixable lint on a private function. On a public function or trait method it is allowed,
  because a public signature may reserve room, and the review packet shows it.

**Suspension is an effect, not a colour.**

- There is no `async` and no `await`.
- A function may suspend if a capability it holds may suspend, so suspension is visible in the declaration without
  colouring every caller.
- `y explain` lists the exact suspension points.

**Observability is the one ambient exception.** `log` and `metric` statements need no capability:

- They are write-only: nothing in a program can read them back, so they cannot change results.
- They carry the current stable identity and trace context automatically.
- Inside a transaction they are buffered, emitted if it commits and discarded if it does not. YAPL had this in 2024
  ("logs and metrics are emitted only on commit").
- `Sensitive` values are masked.

Friction on observability produces programs nobody can observe, and every iteration of Y has treated observability as
the language's job.

### 3.8 Time and concurrency

**Every wait is bounded, and the bound is checked.**

- `within 30s { ... }` sets an absolute deadline shared by everything inside it. `queue.receive() within 5s` bounds a
  single operation.
- A nested `within` can only shorten the deadline, never extend it.
- Expiry is a typed failure (`Expired`), propagated like any other.
- A suspension with no finite bound above it does not compile. A wait that is genuinely indefinite is written
  `forever because "..."`, which requires the `Forever` capability and is a claim (§ 3.15).

**Tasks are structured.**

- `task profile = load_profile(id)` starts concurrent work owned by the enclosing scope. Reading `profile` waits for
  it, within the scope's deadline.
- A scope does not exit until its tasks finish.
- A failing task cancels its siblings, and the scope's failure type includes the failures of its tasks.
- Two tasks may not hold conflicting borrows.
- Work that outlives its scope is written `detach because "..." within 1h { ... }`. It requires the `Detach`
  capability and carries an explicit lifetime.

**Cancellation.**

- Cancellation arrives only at suspension points, as a failure.
- Cleanup during cancellation has its own bound.
- A short critical region may be protected from cancellation (`uncancellable within 50ms { ... }`), and only with a
  bound.
- Cancellation never undoes a committed transaction or withdraws a command that committed with it.

**Supervision is a policy on a scope, not a new kind of thing.** A plain scope is already a supervisor whose policy is
"fail on the first failure". Other policies are written on the scope:

```y
within 24h supervise(restart: .EachFailed, at_most: 5, per: 10min) {
    task ingest = run_ingest()
    task compaction = run_compaction()
}
```

- A restarted task starts from its declared initial state. Shared state it committed survives, so restart is never a
  recovery mechanism for transactional data.
- Exceeding the policy fails the scope, and the failure escalates to the enclosing scope.

This settles September's overlap between task scopes and supervisors by making them one construct.

**What the language does not have.**

- No threads, futures, promises or `await`.
- Where a task runs is the runtime's decision. A v1 runtime may be single-threaded per instance and cooperative, as
  the July 2026 spike was, because nothing in the semantics depends on parallelism.
- Deterministic testing (§ 3.14) depends on the scheduler being controllable, and on a schedule that failed once being
  replayable from a recorded seed.

### 3.9 Shared state

**Shared mutable state exists only as `Shared<T>`**, a capability created with `Shared.new(initial)`, normally by the
entry point, and passed down. September's module-level `shared inventory: Inventory` becomes a value someone had to
hand you. There is no global state to reach.

- A `Shared<T>` is read and written only inside `atomic`, where it behaves as `&mutable T` and the runtime tracks what
  was read and written.
- `snapshot()` outside a transaction returns an immutable, consistent copy. It is a read-only transaction, and exists
  so that tests and reports need not open one.
- State that is not shared is owned by one task, is not transactional, and costs nothing.

**Transactions are named blocks**, because the name shows up in conflict metrics, in fault injection
(`conflict at hold_and_charge#hold_one_unit`), and in the review packet ("transacts hold_one_unit — writes …"):

```y
atomic take_next_job {
    jobs.pending.pop_front() ?? retry
}
orelse atomic take_retried_job {
    jobs.retried.pop_front() ?? retry
}
```

**Semantics:**

- An `atomic` block's value is its result.
- Its reads observe one snapshot, and its writes become visible together at commit.
- A conflicting commit re-runs it from the start.
- A failure leaving it aborts it without trace.
- A nested `atomic` joins the enclosing one.
- `retry` abandons the attempt and waits until something it read changes. That wait is bounded like any other.
- `orelse` tries alternatives left to right and waits on the union of their read sets.

**The load-bearing rule is unchanged: no external effect inside a transaction.** The body of an `atomic` block may
contain only:

- pure computation;
- `Shared` access;
- `enqueue`;
- buffered `log` and `metric` statements;
- `retry`.

Anything else is rejected, and the diagnostic names the capability, the call path, and the durable mechanism to use
instead. Values from the clock or randomness are drawn before the transaction and passed in.

The implementation is multi-version concurrency control with first-committer-wins, as the July 2026 C spike built it.
The difference is scope: the YAPL-era runtime put *every* variable under MVCC, and Y puts only what is shared there.

### 3.10 External effects

Y promises exactly-once *intent*, at-least-once *delivery*, and the identity an external system needs to deduplicate.
It does not promise exactly-once effects, which no language can.

**Commands** are declared with their delivery policy. A policy is not optional, because every field of it is a
decision someone has to make:

```y
-- Charges a buyer's payment method for a hold.
command ChargeCard {
    hold: HoldId,
    amount: Money<Isk>,
    method: PaymentMethodId,

    policy {
        deduplicate_by: hold
        idempotent: true
        retry: backoff(first: 1s, factor: 2, attempts: 8)
        deadline: 24h
        on_abandon: compensate ReleaseHold { hold: hold }
    }
}

-- Performs the charge; runs outside any transaction.
handler ChargeCard(command: ChargeCard) -> ChargeReceipt ! ChargeError
    with gateway: Http<PaymentGateway>
{
    within 30s {
        gateway.charge(idempotency_key: command.id, method: command.method, amount: command.amount)?
    }?
}
```

- `enqueue` submits a command from inside a transaction, which is the only place it may appear. The command is durable
  if and only if the transaction commits. Duplicates collapse on `dedupe_by`.
- A command carries a stable identity and a correlation back to the transaction that submitted it.
- A handler runs outside transactions. Its outcome — completed, failed, or abandoned after exhausting its policy — is
  recorded durably against the command's identity.
- **`on_abandon` answers September's largest open question.** A command that exhausts its policy either:
  - enqueues a compensating command, built purely from its own fields; or
  - is `park`ed as a durable, queryable dead letter.

  Either way, it never disappears.
- **`afterCommit` is dropped.** `enqueue` is already commit-conditional and durable, and the only other thing people
  reach for after a commit is logging, which is already buffered until commit. A second mechanism would be either
  redundant (if durable) or a lost-email bug with syntax (if not).

**Workflows** are durable, long-running functions:

```y
-- Takes a held item from payment to shipment.
workflow fulfil(hold: HoldId) -> Shipment ! FulfilmentError
    with orders: Store<Order>, charges: Submit<ChargeCard>, shipments: Submit<CreateShipment>
{
    let order = orders.for_hold(hold)?
    match submit ChargeCard { hold: hold, amount: order.total, method: order.payment_method } {
        .Ok(receipt) => return submit CreateShipment { order: order.id, receipt: receipt.id }?,
        .Err(.Failed(reason)) => fail .PaymentDeclined(reason),
        .Err(.Abandoned(abandonment)) => fail .PaymentUnreachable(abandonment),
    }
}
```

- `submit` suspends the workflow durably until the command's outcome is recorded.
- Its result is `R ! Undelivered<E>`, whose two failure cases are `.Failed(E)` and `.Abandoned(...)`. Abandonment is
  therefore an ordinary failure that must be matched or propagated, and cannot be ignored.
- A workflow may open `atomic` blocks.
- A workflow's determinism is enforced by its capability set. It may hold `Store`, `Shared`, `Submit` and `Enqueue`,
  and not `Clock`, `Random` or any I/O capability. Time inside a workflow is the recorded time of its last durable step.
- A workflow persists its logical identity and continuation version, never an address. It resumes under a version whose
  compatibility signature matches (§ 4.4).
- **An external query from transactional logic is a workflow.** Submit the query, wait durably, then open `atomic` with
  the answer. September's four-step request pattern (§ 10 of the working design) becomes this one construct.

### 3.11 Specification in the program

**Contracts** are part of the declaration and, like every predicate, are named:

```y
---
Takes money out of an account.

account: The account to debit.
amount: How much to take.
returns: A receipt for the debit.
fails InsufficientFunds: The balance is lower than the amount.
---
public function withdraw(account: &mutable Account, amount: Money<Isk>) -> Receipt ! WithdrawError
    requires positive_amount = (amount > 0isk)
    ensures debited_exactly = (account.balance == old(account.balance) - amount)
{
    if short = (account.balance < amount) {
        fail .InsufficientFunds(shortfall: amount - account.balance)
    }
    account.balance -= amount
    Receipt { account: account.id, amount: amount }
}
```

**A precondition and a failure are not the same thing.** A zero amount is the caller's bug, so it is `requires`. An
insufficient balance is a condition the caller must handle, so it is a failure case. Agents conflate the two, and the
documentation rules make the choice visible: a `requires` has no `fails` role, and a failure has one.

**Semantics.**

- `ensures` holds when the function succeeds. `ensures on failure name = (...)` states what holds when it fails.
- Contract expressions are pure, since they can have no `with` clause.
- A record's `invariant` holds whenever the record is not being mutated. A function taking it as `&mutable` may break the
  invariant inside its body, and must restore it before returning.
- An invariant on a record held in `Shared<T>` is checked at the commit of every transaction that wrote the record.
  This is the April 2026 "watch list" idea, with the watch list taken from the fields.

**Checking.**

- A contract is proven statically where the compiler can prove it, and the program is rejected where the compiler can
  refute it. Otherwise it becomes a runtime check whose failure is a fault.
- **The build reports every contract it demoted to a runtime check, and the review packet lists them.** A guarantee
  that was never proven is not allowed to look like one that was.
- Contracts generate property tests (§ 3.14) and are projected to Why3 or Dafny (§ 4.6).
- v1 has no flow-sensitive refinement inference beyond the facts conditions add (§ 3.3): a condition's own predicate
  holds where it guards, and nothing else is derived from it. Properties that must travel with a value belong in a
  refined type (§ 3.3). Properties of one call belong in its contract. No ordinary build needs a theorem prover.

**Typestate.** A `state` declaration lists the fields every state shares, then the fields only some states have:

```y
-- An order's life from creation to payment.
state Order {
    id: OrderId
    lines: List<Line>

    case Created
    case PaymentPending { charge: CommandId }
    case Paid { receipt: ReceiptId }
    case Cancelled { reason: CancelReason }
}

-- Records that payment was requested for a new order.
transition request_payment(order: Order.Created, charge: CommandId) -> Order.PaymentPending {
    order.becomes(.PaymentPending { charge: charge })
}
```

- `Order.Created` is a type, and a transition consumes the value it starts from. A stale reference to the old state is
  therefore an ownership error, not a logic error.
- A transition that is not declared does not exist.
- A field that means something only after payment exists only in `Paid`.
- A value whose state is unknown, because it came from storage, is matched to recover it.
- Transitions are projected to TLA+ and checked with Apalache for reachability and invariants.

In v1, typestate applies to owned values. Shared or stored state machines are advanced inside `atomic`: match the
current state, call the transition, and let conflicting commits retry. This answers the September open question
without new machinery.

### 3.12 Persistent data

A record that outlives its process has a second type: the one on disk. Y makes that type a declaration:

```y
-- A customer as stored, version 3.
schema Customer v3 {
    key id: CustomerId
    name: Text
    email: EmailAddress?
    marketing_consent: Consent
}

-- Version 3 adds explicit marketing consent; nobody is assumed to have given it.
migrate Customer v2 -> v3 {
    marketing_consent = .NotGiven
}
```

**Schemas.**

- Every version stays in source, so the compiler compares versions rather than guessing from a diff. It classifies each
  change as additive or breaking, and names the field that made it breaking.
- Codecs are generated, identity is declared (`key`), and constraints and invariants are checked on read as well as on
  write.
- A field whose type holds a borrow, a capability or a task does not compile.
- Persisted states are `state` declarations, so the legal transitions of stored data are those of the type.

**Migrations.**

- A migration gives one expression per new or changed target field.
- It is pure, deterministic and idempotent. It may declare other schemas it `reads`.
- It fails per record, not wholesale, and each failure is recorded against that record's key.
- It is reversible only if a `reverse` block is declared, and the compiler reports which version pairs can be rolled
  back.
- An additive change with a total default needs no migration, and the compiler says so.

`Store<Schema>` is the capability for persistent data, and its reads and writes inside `atomic` join the transaction.

Live migration (`converge`), and the choice of physical representation, belong to the runtime and come later (§ 5). v1
must not prevent them, and does not need them.

### 3.13 Collections and queries

**Collections.**

- `List<T>`, `Map<K, V>` and `Set<T>`.
- **Iteration order is always deterministic.** A `List` iterates by position; a `Map` or `Set` by insertion. No
  collection exposes hash order, because hash order breaks replay (§ 4.7), and an agent will eventually rely on it.

**Pipelines** say what the answer is, which leaves the compiler free to fuse, parallelise or index:

```y
let active_names = &users.filter(user => user.active).map(user => user.name).sorted()
```

- **The receiver's mode decides ownership**, and stage names do not change to say so. `&users.filter(...)` borrows,
  `users.map(...)` consumes, and `&mutable users.for_each(...)` mutates. This comes from the September riff, and it
  matches the call-site rule of § 3.6.
- A pipeline over transactional state reads inside the enclosing transaction.
- Unordered results make no promise about order beyond their source's. Order is asked for with `sorted`.

**Derived values and rules.**

- `derived` declares a value maintained from other state, and is pure. Whether it is stored, cached or recomputed is a
  physical decision, not a keyword.
- `rule` declares a named predicate over data, usable in pipelines, contracts and other rules. **In v1 rules are not
  recursive.** That keeps the declarative subset a vocabulary of named conditions rather than a second evaluator.
  Recursion, with the stratification it requires, can be added later.

```y
derived active_user_count: Count<Element> = users.filter(user => user.active).count()

rule high_risk(customer: &Customer) = (customer.failed_payments > 3) and (customer.outstanding > 100_000isk)
```

Y has no query strings, and no user-visible index management in v1.

### 3.14 Tests

Tests are part of the language, because the compiler must know what a test claims to cover.

**Location.**

- The tests for `src/shop/checkout.y` live in `src/shop/checkout.test.y`. That path is computed from the module, never
  searched for.
- A test file belongs to its module, so it may use private declarations. It may not reach into a function's locals, as
  YAPL's tests could. Tests address a function through its named outcomes and its faults, so they survive refactoring
  of its body.

**Shape.** A test has a documented purpose, a `covers` list, and fixed sections in a fixed order. Only `when` and
`then` are mandatory:

| Section | Holds |
| --- | --- |
| `given` | Plain values, including generated ones: `any<Buyer>(seed: 11)` |
| `with` | The capabilities the code under test receives: real, attenuated, or `fake` |
| `faults` | What to force: `conflict at f#txn, times: 2`, `crash after commit f#txn`, `expire at f#txn`, `cancel at f#task` |
| `when` | The single action under test |
| `then` | Named assertions only |

**Coverage is declared, then verified.**

- A function's *outcomes* are both sides of every named decision (`f#name`, `f#name.else`) and every case of its
  failure type (`f!Case`).
- A test lists the outcomes it covers. The runner fails the test if it did not actually reach every one of them.
- A test contributes coverage only for what it declares. This is the April 2026 rule, and it keeps coverage an auditable
  claim rather than an accident.
- **A release build requires every outcome of every function to be covered by at least one test that declares it.**
  The toolchain reports outcomes that nothing reaches, and outcomes reached only incidentally by tests that do not
  declare them. These are knowledge-driven development's 🙈 and 🙉, computed rather than hand-written.

**Test data.**

- Fakes record what happened (`charges.committed()`) and can be driven (`clock.advance(5s)`).
- Every test gets fresh capabilities, so shared state and stores never leak between tests.
- **Property tests come from declarations.** For every function with contracts or refined parameter types, the runner
  generates inputs that satisfy the `requires` clauses and constraints, checks the `ensures` clauses, shrinks any
  counterexample, and offers it as an ordinary test to commit.
- **Concurrency is explored, not hoped for.** `y test --explore` enumerates the interleavings that matter, using the
  read and write sets in Core IR to skip equivalent ones. A failing schedule is recorded as a seed and committed with
  the test that found it.

### 3.15 Claims: the trust surface

Some things the compiler cannot check. Y does not pretend otherwise; it makes each one a **claim**:

- written in a closed vocabulary;
- carrying a `because` that says why the author believes it;
- where it touches authority, requiring a capability, which keeps it visible in the signature as well.

| Claim | What it asserts | Also requires |
| --- | --- | --- |
| `unsafe because "..." { ... }` | This native or foreign code upholds Y's rules | `Native` |
| `forever because "..."` | This wait is meant to be unbounded | `Forever` |
| `detach because "..." within 1h { ... }` | This work may outlive its scope | `Detach` |
| `reveal(secret) because "..."` | This sensitive value may be seen here | `Reveal` |
| `assume name = (predicate) because "..."` | The prover may take this as true without proof | — |
| `allow(Y2031) because "..."` | This lint does not apply here | — |

**How claims are handled.**

- `y claims` lists every claim in a package by stable identity.
- The review packet always shows claims that are new or changed. Claims are never sampled.
- Compiler errors cannot be suppressed at all. A small set of lints cannot be suppressed either (YAPL's "official lint
  rules, which cannot be suppressed"), including shadowing, abbreviations and unnamed decisions.
- Accepted oracle disagreements are claims too, recorded in the project with their construct and reason (§ 4.6).

The number of claims in a module is a number worth watching. It should be small and should not drift upward unnoticed.

### 3.16 Gaps: unfinished work, in the language

Knowledge-driven development (August 2026) moved the backlog out of trackers and into the documents a reader opens
anyway. Y moves the part of it that concerns code into the code, where the compiler can see it.

- **`todo("what is missing")`** is a typed hole.
  - It type-checks as whatever the context needs.
  - It faults if reached.
  - It is allowed in development and test builds, and rejected in a release build if reachable from an entry point.
  - YAPL's `...` (2024) and the toolchain's `unimplemented` (October 2025) were earlier versions of it.
- **`gap:`, `defect:` and `blocked:`** are documentation roles (§ 3.2), matching ⚠️, ❌ and ⛔.
  - `blocked:` must follow a `gap:` or `defect:`, and must say what decision unblocks it and who makes it.
  - `y status` collects them all, by stable identity. That list *is* the backlog for code.
  - `TODO` and `FIXME` comments are a lint error.
- **Coverage marks are computed, never written.** The one part of the convention a machine can do, the machine does.

Together with claims and the coverage gate, this gives Y ai.lang's fifth invariant as a property of every build:
*everything claimed is provable or explicitly missing*.

- A contract is either proven, or demoted and reported.
- An outcome is either covered, or reported.
- A requirement linked by `satisfies` either has code, or is listed without it.
- An unfinished piece of work either compiles as a `todo` or a `gap`, or it does not compile.

### 3.17 Evaluation at compile time

`compile` runs a call while the program is built, and the program holds the result as a value:

```y
function checksum(bytes: &Bytes) -> UnsignedInteger32 {
    let table = compile crc_table(polynomial: 0xEDB8_8320)
    ...
}
```

**What can be compiled.**

- The operand is one call to a function with no `with` clause, which makes it pure (§ 3.7).
- The function carries no mark. The same function runs while building and while running, as suspension needs no
  `async`. Unlike C++'s `constexpr` and Rust's `const fn`, there is no second kind of function to keep in step with the
  first.
- Every argument is known when the program is built: a literal, a `const`, or a name bound by another `compile`. Any
  other argument is an error that names it.
- The result holds no capability. A result cannot hold a borrow either, because no type does (§ 3.6).
- A `const` declaration is evaluated the same way, so `const CRC_TABLE = crc_table(polynomial: 0xEDB8_8320)` needs no
  keyword. `compile` is the same evaluation inside a body, for a value that one function uses.

**What evaluation produces.**

- **The value is the one the call would return at run time.** Pure code in Y depends on nothing that differs between
  the build machine and the target: integers have fixed sizes and are checked, and there is no locale or hash order
  (§ 4.7).
- **A fault is a build error.** A failed `assert`, an overflow, a violated `requires` or a reached `todo` produces the
  fault report of § 3.4, placed at the `compile` and at the construct that faulted.
- **A failure is a build error too.** A call returning `T ! E` must succeed. A failure is reported with its case and the
  case's `fails` role, and the result has type `T`. Literal prefixes (`pattern"…"`, `url"…"`) are this rule with a shorter
  spelling: a literal that would not parse never builds.
- **Evaluation has no limit.** A call that runs long keeps running. The toolchain reports where evaluation is (its call
  stack) while it runs, so whoever is waiting can see what is slow and stop the build if they choose. The report never
  changes whether a program is accepted.

**What a reviewer sees.**

- `y explain` shows the value at each `compile`. A large value is shown as its size and fingerprint.
- When an edit to the function or its arguments changes a computed value, the semantic diff (§ 4.4) shows the old and
  new values, so a reviewer sees what the edit did without running anything.

**Lowering.**

- The compiler evaluates the call on Core IR. The backend writes the result as data. In bare Rust it is a `static` or
  `const` item holding the value, not a call, so it does not depend on what Rust's `const fn` allows (which excludes
  allocating).
- A value Rust cannot hold as a constant, such as a `Map`, is built once, on first use, from static data.
- The constraint file records where the value came from. A generated test makes the same call at run time and compares
  the results, which checks the compiler's evaluator against the backend as an independent oracle (§ 4.6).

**Not in v1.**

- Parameters that must be known when the program is built, such as Zig's `comptime` parameters and C++ templates over
  values. They would let a function specialise on a value (a buffer of fixed size, say). Generics (§ 8, question 2)
  decide whether they come.
- Generating types or declarations. Y has no macros, and `compile` produces values only.

## 4. The toolchain is the reviewer's instrument

In this design the language and its toolchain are one product. Every rule in § 3 exists so that a tool in this section
can compute something a reviewer would otherwise have to work out by reading.

### 4.1 One canonical form

- **`y format`** has no options. It is lossless: it preserves every comment, and never rewrites prose or `y:` links. It
  is idempotent. Two programs that mean the same thing and differ only in layout format to the same text, so every diff
  line is a real change.
- **`y fix`** applies the machine-applicable part of every diagnostic. It pins inferred authority, adds `&`, `&mutable` and
  `move` at call sites, names or unnames arguments, sorts imports, and scaffolds missing documentation roles. It leaves
  the prose to the author.
- The parser recovers from errors and reports every error in a file in one run. Its syntax tree keeps spans and
  comments, so a mechanical rewrite touches only the lines it means to. Agents depend on this more than people do.

### 4.2 Diagnostics are an interface

Diagnostics are the channel through which the compiler teaches the agent, so they are designed like an API.

- **Codes are stable.** `Y0412` means *external effect inside a transaction* forever. A retired code is never reused,
  and codes are never renumbered.
- Every diagnostic carries four things:
  - the span;
  - **why** the rule exists, in one sentence;
  - the **fix**, as an edit where one is mechanical;
  - a `y:spec/...` link to the normative statement it enforces.
- **A diagnostic for a prohibition names the alternative.** The September journey identified the failure mode: an
  agent told only "no" routes around the rule (moving state out of `shared` to dodge the transaction rule). An agent
  told "no, do this instead" does the right thing.
- Output is available as JSON for agents and as text for people. Both carry the same content.

### 4.3 `y explain`: what the compiler concluded

For any declaration, `y explain` prints the elaborated program *as Y*, not as an IR dump. It includes:

- inferred borrows and moves;
- every capability used, and the call path that needed it;
- suspension points;
- each transaction's read and write sets;
- every failure propagation path;
- each contract's status: proven, tested, or demoted.

Every inferred fact carries its provenance, so "why does the compiler think this can suspend?" is answered with a call
path, not a shrug. The same view, rendered to Rust, is September's "show generated Rust": output a competent Rust
programmer might have written.

### 4.4 Compatibility signatures and semantic diff

Every function has a **compatibility signature**, computed from its elaborated form. It contains:

- its parameter and result types, with their ownership modes;
- its authority;
- its failure set;
- its contracts;
- its transactional writes;
- the command classes it may submit;
- the schemas it touches;
- its wait bounds.

September introduced this for hot swap (§ 5). This proposal uses it first for review:

- **`y diff --semantic base..head`** reports changes to compatibility signatures, not to lines. Adding a capability, a
  failure case, a command class or a transactional write shows up as exactly that, whatever the textual diff looks like.
- Every declaration has a **semantic hash**, taken over its elaborated Core IR. Formatting, comments and renames
  recorded by `y rename` do not change it. Any change in meaning does.

### 4.5 The review packet

`y review base..head` produces what a person reads, in this order:

1. **Claims** added or changed (§ 3.15). Always shown.
2. **Authority widened** anywhere. Always shown.
3. **Public signature changes**: types, failure sets, contracts. Always shown.
4. **Transactions, commands, schemas and migrations** added or changed. Always shown.
5. **Contracts demoted** to runtime checks, **gaps** added, **outcomes** left uncovered. Always shown.
6. **New public declarations**, bodies included. Always shown.
7. **Body-only changes** under unchanged compatibility signatures, fully covered and agreed by the oracles. **Sampled.**
   The probability of a body being shown rises with its size, with the authority reachable from it, and with the
   defect history of its module.
8. **Generated tests.** Sampled. ai.lang's rule stands: "always review generated tests carefully". Here "carefully"
   means a random sample, so that the agent cannot predict which tests will be read.

**Recording approvals.**

- An approval is recorded in a ledger against a declaration's stable identity and semantic hash. It survives
  reformatting and recorded renames, and is voided by any change of meaning.
- The ledger tells the sampler what has already been looked at, and tells an auditor what never has.
- `y review --outline` shows any function at the altitude of its signature, documentation and named decisions (§ 3.5),
  with bodies folded.

Each change's record follows the practice the ai.lang and `string` repositories already use. It names:

- the human who asked;
- the agent that wrote the change;
- the base commit;
- the verification that ran, with its results.

The packet's summary is that record.

### 4.6 Core IR, backends and oracles

These are kept from September, whose documents in [`../documentation/toolchain/`](../documentation/toolchain/) remain
the right description.

- **Core IR** is the definition of a Y program. It is backend-neutral, deterministic to the byte, versioned,
  self-contained and explicit rather than compact. A consumer that needs a fact Core IR does not carry has found a
  defect in Core IR.
- **Rust** is the first backend and the first oracle. Zig or C comes next, then a native backend.
  [`../examples/hangman`](../examples/hangman/) is a whole program taken through it, with the lowering done by hand.
- **Independent oracles:**
  - `rustc` and Miri check ownership.
  - CBMC and Frama-C check generated C.
  - Why3 and Dafny check contracts.
  - TLA+ and Apalache check typestate.
  - Loom, Shuttle and Y's own explorer check concurrency.
  - Running several backends and comparing them checks equivalence.
- **Disagreement is a bug signal, never a vote.** A disagreement is minimised automatically and committed as a
  reproducer.
- **Conformance** is the oracle for the specification itself. Every example in the normative text compiles in CI, and
  produces the diagnostics it claims to (§ 9).

### 4.7 Determinism and replay

- The same source and dependencies produce byte-identical Core IR on any machine, and builds carry a fingerprint.
- Every non-deterministic input reaches a program through a capability: time, randomness, I/O, command outcomes and
  commit order. Each can therefore be recorded, and a recorded execution replays to the same results, committed state
  and emitted commands.
- Iteration order, scheduling in tests and physical representation are never observable to a program. A physical
  decision a program can observe is a runtime bug.

This is what makes a failure an agent introduced reproducible, and it is also the foundation § 5 builds on.

## 5. Runtime: foundations now, generations later

The second half of the September riff describes a running system:

- deterministic replay;
- a *semantic* optimiser that must be correct, and a *physical* optimiser that may experiment, because "a bad
  candidate is slow, not wrong";
- function-level hot swap with shadowing and canaries;
- live state migration by convergent copying (`converge`);
- system generations that switch code, schema, physical plan and state root atomically.

The September corpus left open whether any of this is in Y
([`../documentation/toolchain/runtime-generations.md`](../documentation/toolchain/runtime-generations.md)).

**Proposal: a post-v1 capability whose foundations are in v1.** The review design in § 4 already needs most of those
foundations, which makes the decision cheap:

| Foundation | Needed in v1 by | Later used by |
| --- | --- | --- |
| Stable logical identity from names (§ 1.4) | Coverage, traces, approvals ledger | Versioned dispatch, hot swap |
| Compatibility signatures (§ 4.4) | Semantic diff, review packet | Deciding whether a swap is transparent |
| Every non-deterministic input behind a capability (§ 3.7) | Fakes in tests, reproducible failures | Deterministic replay of production |
| Versioned schemas with pure migrations (§ 3.12) | Deploying schema changes safely | `converge`, per-record invalidation |
| Workflows persisting identity and continuation version (§ 3.10) | Surviving a deploy | Resuming under a compatible version |
| Code version on every invocation's telemetry | Diagnosing production faults | Workload corpus for the optimiser |
| Physical choices recorded as parameters on Core IR | Nothing yet | Physical search space |

**The v1 runtime is deliberately small:**

- multi-version transactional memory for `Shared` state (the July 2026 spike, re-hosted);
- a durable command log and outcome store;
- a workflow engine;
- `Store` over one database projection;
- a cooperative scheduler, which can be single-threaded per instance.

Hot swap, the physical optimiser, `converge` and generations come after v1. Because the foundations are already in
place, they arrive as extensions rather than as a runtime redesign.

Other runtime ideas are left out of the language: affinities, placements, primary–secondary VMs, garbage-collection
tickets and entropy budgets (§ 7). If any of them turns out to be worth having, it belongs behind Core IR as a physical
decision, not in source.

## 6. The September open questions, answered

The September corpus carries 23 blocked marks (⛔). Four of them duplicate another mark or concern the corpus itself,
which leaves nineteen language decisions waiting on the language owner. This proposal recommends an answer to each. The
owner still decides; the recommendation is here so that deciding is quick.

| Blocked in September | Recommendation | Where |
| --- | --- | --- |
| Grammar: layout or braces | Braces, with one canonical formatter | § 3.1 |
| Macro or metaprogramming model | None in v1. The compiler derives equality, hashing, ordering and codecs; agents write the rest | § 3.3, § 7 |
| `T ! E`: distinct channel or sum type | A built-in sum type; Core IR records propagation edges | § 3.4 |
| Degree of ownership inference | Signatures and call sites explicit, bodies inferred; no type contains a borrow in v1 | § 3.6 |
| Types that opt out of single ownership | `ReferenceCounted<T>` for immutable sharing; `Shared<T>` for mutable; arenas and typed indices for graphs | § 3.6 |
| Effect annotation syntax | The `with` clause: effects are what capabilities carry | § 3.7 |
| Capability declaration and passing | Declared per function, threaded by type from the caller's scope, overridden explicitly | § 3.7 |
| Where handler failure escalates | Mandatory `on_abandon`: compensate or park; in workflows, `.Abandoned` is a failure to handle | § 3.10 |
| `afterCommit` durability | Dropped; `enqueue` and buffered telemetry cover every case | § 3.10 |
| Workflow syntax and body boundary | `workflow` may open `atomic`; determinism enforced by its permitted capabilities | § 3.10 |
| Supervision source model | A policy on a task scope | § 3.8 |
| Refinement reasoning | Named `where` constraints on types, contracts on functions, conditions narrow where they guard; no other flow-sensitive inference in v1 | § 3.3, § 3.11 |
| Typestate over shared state | Owned values in v1; shared state machines advanced inside `atomic` | § 3.11 |
| Schema projection onto storage | One projection in v1; projection choice belongs to the physical optimiser later | § 3.12, § 5 |
| What triggers `converge` | A deployment action, post-v1 | § 5 |
| Recursive rules | Not in v1 | § 3.13 |
| Whether runtime generations are in Y | Post-v1; foundations in v1 | § 5 |
| Native backend, ABI and FFI | Rust first; foreign calls are `unsafe` claims needing `Native` | § 3.15, § 4.6 |

## 7. What this proposal drops, and why

Everything here was a considered idea in some iteration. Several were held for years. They are dropped because the
premise changed, or because the iteration that tried them showed their cost. The provenance of each is in
[`Y-idea-harvest.md`](Y-idea-harvest.md).

**Syntax and spelling**

| Dropped | Held in | Replaced by | Why |
| --- | --- | --- | --- |
| Significant indentation | YAPL → April 2026 | Braces and one formatter | Whitespace does not survive every tool, chat window and review UI an agent's output passes through; the formatter gives the same visual structure |
| Lowercase type names | YAPL → April 2026 | `PascalCase` types, `snake_case` values | Agents drifted to PascalCase unprompted; readers lose the type/value distinction |
| `name is type`, and `is` in five roles | YAPL → April 2026 | `name: Type` | `is` meant annotation, predicate sugar, capture, mutability and optionality; the March 2026 review called it the most ambiguous token in the language |
| File-wide width-matched `---` / `===` / `~~~` comment blocks | YAPL → April 2026 | `--` lines, and `---` blocks whose width belongs to each block | File-wide delimiter width makes one edit rewrite many lines of diff |
| `//` and `///` comments | Earlier drafts of this proposal | `--` and `---`, Y's own since YAPL; documentation is the comment above a declaration | Spelling borrowed for agents' habits, which a diagnostic and `y fix` correct instead |
| Abbreviated keywords and names: `pub`, `fn`, `var`, `const`, `impl`, `mut`, `Int`, `Bool`, `Rc` | Earlier drafts of this proposal | `public`, `function`, `variable`, `constant`, `implementation`, `mutable`, `Integer`, `Boolean`, `ReferenceCounted` | The no-abbreviation rule binds the language's own words too |
| `if` and `match` as expressions; a ternary operator | Earlier drafts of this proposal | Statements; a value is assigned in each branch | A decision is a statement with a name and a place, never a value hidden inside an expression |
| Spec block in place of the signature | April 2026 | Signature in code, documentation with closed roles above it | The examples abandoned it for helpers; types are read in the signature; agents' priors |
| `=` to bind, `:=` to rebind | YAPL → April 2026 | `let` / `variable` | Mutability is marked where the name is declared; `:=` fought every agent's habits |
| Two spellings of one keyword, method synonyms | YAPL, toolchain | One spelling | Contradicts "one way to write it" |
| Mandatory name on `else` | YAPL → toolchain | Optional name | A name bound to `true` carries no information |
| `loop` / `leave` / `next` | YAPL → toolchain | `break` / `continue`, named when nested; no unbounded `loop` | Keeps YAPL's rule, not its words |

**Semantics**

| Dropped | Held in | Replaced by | Why |
| --- | --- | --- | --- |
| Classes, facets, factory methods, singletons, structures, properties, inheritance | YAPL → toolchain | Records, sum types, traits, `implementation` | Every iteration with more kinds spent its effort reconciling them; facets carried visibility, telemetry and override rules at once |
| Exceptions, `throws`, wrapping in `unexpected_exception` | YAPL → April 2026 | `T ! E` | Values can be matched exhaustively; wrapping hid the failure set |
| Ambient `using` contexts, context stacks, affinity-based name lookup | YAPL → toolchain | Capabilities in `with` | Action at a distance; March 2026 had already reversed it ("no global or thread-local state") |
| Transactional memory for every variable; reference tracing; one OS thread per process as a guarantee | YAPL → July 2026 | `Shared<T>` only; ownership; runtime's choice | Costs paid by code that shares nothing; semantics should not depend on the scheduler |
| Affinities and placements in types | Toolchain | Physical parameters behind Core IR | Runtime detail in source, which agents misuse |
| Comments with runtime meaning (interpolated, traced) | YAPL → toolchain | `log` statements | A comment that runs surprises the reviewer |
| Two kinds of absence (`nada`/`nothing` and `undefined`) | Toolchain → April 2026 | `T?` and definite initialisation | Definite initialisation makes "not yet provided" unrepresentable |
| `afterCommit` | September 2026 | `enqueue`; buffered telemetry | Redundant if durable, a bug if not |
| Module-level `shared` declarations | September 2026 | `Shared<T>` values passed down | Global state by another name |
| Tests reading a function's locals | YAPL (2024) | Named outcomes and fault injection | Couples tests to implementation |

**Syntax for policy, and scope beyond a language**

| Dropped | Held in | Replaced by | Why |
| --- | --- | --- | --- |
| Encrypted-log syntax; `throw ... capturing snapshot ... trace retry sandbox` | Toolchain (Nov 2025) | `Sensitive<T>`, fault reports, replay | Policy and debugging configuration do not belong in grammar |
| The five-layer organisational ontology; Markdown with metadata in HTML comments; natural language as primary meaning | ai.lang | `satisfies:` and `y:` links | Not a programming-language concern; the spec and its tooling drifted apart within a week |
| "No standard library" | Toolchain | A small standard library: capabilities, collections, text, units | Contradicted by every iteration's practice |
| Hungarian-style prefixes (`i_`, `o_`, `m_`) | C++ directives, 2026 | Types and keywords | They were C++ workarounds for missing language features |
| Transpiling to Rust as the definition of Y | September riff | Rust as backend and oracle | Already settled in September; restated because the riff says otherwise |

## 8. Questions this proposal leaves open

These are genuinely the owner's to decide. Each has a leaning, not a decision.

1. **The compiler's implementation language.**
   - The April 2026 bake-off chose Go, for bootstrap speed.
   - This proposal leans to Rust, because the first backend, the first oracle and the transactional runtime would then
     share one ecosystem, and the generated code could link against the runtime directly.
2. **Generics.**
   - Leaning: invariant generics with trait bounds; no variance annotations, specialisation or higher-kinded types in
     v1.
   - YAPL settled on invariant generics in 2024. The toolchain's `allow_subtypes` vocabulary is the fallback if
     variance proves necessary.
3. **Coverage granularity for failures.** Is covering every failure *case* enough, or must every `?` *site* be reached?
   Leaning: cases, because sites multiply without adding much a reviewer would read.
4. **How far implicit capability threading can go.**
   - By-type resolution from lexical scope keeps call sites clean and signatures honest.
   - If real programs show frequent ambiguity, fall back to naming capabilities at call sites.
5. **User-facing text.**
   - YAPL's translatable literals (`T"Hello {name}"`, with plural completeness checked per locale) were a good idea.
   - Leaning: a `UserText` type in the standard library, with a compile-time catalogue check, not new syntax.
6. **Thresholds.** The named-argument rule and the 60/120-line body limits should be calibrated on a corpus of
   agent-written Y, not fixed by taste.
7. **Arenas in v1.** An arena is visible in source here (§ 3.6). It could instead be a physical decision the optimiser
   makes.
   - Leaning: keep the syntax out of v1 until a program needs it.
8. **Transactions spanning `Shared` and `Store`.** One runtime can commit both atomically, but a remote database cannot
   without a protocol.
   - Leaning: v1 supports one store co-located with the runtime, and says so.

## 9. From here to a compiler

The iterations of Y did not fail for lack of ideas. They ran aground in four ways:

- **Spec sprawl.** The toolchain's reference reached about 1.2 MB, larger than any context that had to hold it.
- **Drift.** Agents wrote examples in syntax the spec forbade, and the grammar's own test corpus accepted parse errors
  as expected output.
- **Contradiction.** Chapters disagreed on rune semantics, re-exports, weak references and the standard library.
- **Churn without record.** Many commits were titled "foo".

The process has to prevent all four.

- **A small normative core.**
  - The specification that follows this proposal should fit in one agent's context with room to work, meaning tens of
    pages, not hundreds.
  - Each statement carries a knowledge-driven-development mark. Knowledge-driven development already worked well in the
    September corpus.
  - Rationale lives in documents like this one, not in the normative text.
- **Every example is a test.**
  - An example in the specification is a conformance case. It either compiles or produces exactly the diagnostics it
    says it does, and CI runs all of them.
  - A statement without an example is not yet normative.
- **Changes to the language ship with the migration.** April 2026 already did this with three one-shot scripts that
  rewrote the corpus when the spec changed. Y makes it the rule: a spec change that alters valid programs lands together
  with a `y migrate` rule and the rewritten corpus.
- **Every change has a record.** The record names who asked, which agent wrote it, the base commit, and the evidence it
  passed, as ai.lang and the `string` repository already require.

**Milestones.** Each milestone is useful on its own, and each one's output is checked by the next.

| # | Delivers | Checked by |
| --- | --- | --- |
| M0 | Grammar, error-recovering parser, lossless tree, `y format`, stable diagnostic codes | Every example in this document formats to itself |
| M1 | Names, types, sum types, named decisions, documentation roles, no-shadowing, refined types | Conformance corpus; `y explain` for types |
| M2 | Ownership (explicit signatures and calls, inferred bodies); Rust backend for the pure subset | `rustc` projection agrees on the corpus |
| M3 | Capabilities, effects, `y fix` pinning; tests, `covers`, the coverage gate | The § 2 tour runs end to end without `Shared` |
| M4 | `Shared`, `atomic`, `retry`, `orelse`; `within`, `task`, supervision; controllable scheduler, fault injection | The § 2 tour runs end to end |
| M5 | Commands, handlers, workflows; schemas, migrations, `Store` | Crash-injection suite |
| M6 | Semantic diff, review packet, claims, approvals ledger | A week of agent-written changes reviewed only through packets |
| Later | Contract and typestate provers, more backends, runtime generations (§ 5) | Oracles (§ 4.6) |

M6 is where the premise is tested. If a reviewer working only from review packets misses defects that reading diffs
would have caught, the language has not yet done its job, and the packet or the language changes, not the reviewer.

## Appendix: reserved words

The complete list, so that the size of the surface is visible:

```text
declarations   module use public function record type case state transition trait implementation constant unit
               schema migrate command handler workflow test derived rule copy transient self Self
contracts      requires ensures invariant where old assert
control        if else match when for in while break continue return fail
bindings       let variable mutable move from captures with arena compile
concurrency    within task supervise detach forever uncancellable
transactions   atomic retry orelse enqueue submit
claims, gaps   unsafe assume allow reveal because todo
tests          covers given faults then fake
observability  log metric
operators      and or not
values         true false
contextual     as key reads reverse policy compensate park on, and the field names of
               `policy`, `supervise` and `faults` clauses (reserved only inside those clauses)
```

The word list is closed, and every word is spelled in full (§ 3.1). Comments are `--` and `---` (§ 3.1), so `//`
and `/*` mean nothing. A new keyword is a change to the language and ships with a migration (§ 9). Test-data
generators (`any`, `any_violating`), `snapshot`, `becomes` and literal prefixes (`pattern"…"`, `url"…"`, `path"…"`) are
standard-library names, not keywords.
