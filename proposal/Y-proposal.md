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
-- Taking stock for a buyer.
public facet holds:
    ----------------------------------------------------------------------------------------------------
    Holds one unit of an item for a buyer and asks for payment, exactly once.

    why: Stock is never oversold, and a transaction that retries never charges twice.

    item: The item being bought, as item_id.
    buyer: Who pays, and with what, as buyer.

    hold: The hold, waiting for the charge to be confirmed, as hold.

    inventory: The stock and the holds on it, as shared<inventory>.
    charges: Where charges are queued, as enqueue<charge_card>.

    fails hold_error.unknown_item: The item is not stocked here.
    fails hold_error.timed_out: No unit became free within five seconds.
    ----------------------------------------------------------------------------------------------------
    function hold_and_charge(item, buyer) returns hold fails hold_error:
        dependencies:
                inventory
                charges

        within 5s?:
            atomic hold_one_unit:
                stock = inventory.stock_of(item)?

                if sold_out = (stock.available == 0):
                    retry
                else in_stock:
                    stock.available := stock.available - 1
                    price = stock.price
                    opened = inventory.open_hold(item: item, buyer: buyer.id)
                    enqueue charge_card(hold: opened.id, amount: price, method: buyer.payment_method)
                    return hold = opened
```

Without opening another file, a reviewer can see that this function touches exactly two things (one shared inventory,
one class of durable command), that it can fail in exactly two documented ways, that it waits at most five seconds,
that the transaction may re-run and the charge still happens once (because `enqueue` commits with the transaction), and
which branch a test must reach to cover the sold-out path (`hold_and_charge#sold_out`). None of those facts is a
convention; each one is checked, and the compiler rejects the program if any of them is false.

**Kept from September**, unchanged in meaning: deterministic semantics; Core IR as the definition of a program;
deterministic destruction; typed failure; software transactional memory with no external effect inside a
transaction; durable commands and workflows; bounded suspension without `async` colouring; structured concurrency;
contracts, typestate and units; versioned schemas; independent oracles.

**Brought back from earlier iterations:** named decisions and named results (YAPL, 2020–2024); mandatory, closed-grammar
documentation (spec blocks, April 2026); authority passed explicitly, never ambient (YAPL's OS context, 2024; y-string's
context object, March 2026); units on every length and index (July and March 2026); tests that declare what they cover
(toolchain, October 2025); fixtures generated from constraints (October 2025); *provable or explicitly missing*
(ai.lang, March 2026); gaps recorded where the code is (knowledge-driven development, August 2026); facets as the one
place visibility is written (toolchain, 2025); `name is type` for fields (YAPL).

**Dropped**, with reasons in § 7: braces; classes,
factory methods and inheritance; exceptions; ambient `using` contexts; transactional memory for *all* state;
affinities and placements in source; comments with runtime meaning; the spec block as a replacement for the signature;
`afterCommit`; references, borrows and moves in source, which become the compiler's choices under value semantics
(§ 3.6).

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
  — and nothing caught it, because nothing checked the spelling. Y borrows most of its shape from Rust, Swift and Kotlin, which is what agents already write
  and reviewers already read. Where it departs, the departure is a rule the compiler checks and `y fix` applies:
  - Y spells its own words in full, as it requires of every name (`public facet` and `function`, not `pub fn`).
  - Every identifier is `lower_snake_case`, types included.
  - Its comments are `--` and `---`, as they have been since YAPL.
  - Signatures name their result and carry no types and no `->`: the types are in the documentation above.

  An agent that writes `fn`, `//`, `PascalCase` or `->` from habit gets a diagnostic naming the replacement. This costs
  one round trip,
  and nothing reaches a reviewer.

### 1.2 Inferred, then pinned

Y infers types inside bodies, effects, suspension points and failure paths — and then **pins
the facts a reviewer needs into the declaration**, where they become checked claims:

| Fact | Where it is pinned | Written by |
| --- | --- | --- |
| Parameter, result and capability types | Documentation above the signature (`line: ..., as string.`) | Agent (public signatures are never inferred) |
| Which parameters the function may change | Signature (`x` or `mutable x`) | Agent, corrected by fix-its |
| Which arguments a call lets the callee change | Call site (`mutable x`) | Agent, corrected by fix-its |
| Authority, and therefore effects | `dependencies:` block | `y fix`, from inference |
| Failure set | `fails e` in the signature, and `fails` doc roles | Agent; the compiler checks exhaustiveness of documentation |
| Preconditions and guarantees | `where` / `ensures` | Agent |
| Wait bounds | `within` | Agent; an unbounded wait does not compile |

A pinned clause that no longer matches the body is a compile error, so a change in what a function may do cannot land
without the declaration changing — and the declaration is where the diff shows it. Facts that are *not* pinned
(suspension points, transaction read and write sets, the copies and borrows the compiler chose) are printed on request by
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
identifier from a name (`shop.checkout.holds.hold_and_charge#sold_out`), and treats renaming as a refactoring the toolchain
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
        atomic hold_one_unit:
            -- ...
            stock.available := stock.available - 1
            gateway.charge(method: buyer.payment_method, amount: stock.price)?
```

**The compiler rejects it**, and the diagnostic names the replacement rather than only the prohibition — a rule this
strict gets worked around if it only says no:

```text
error[Y0412]: external effect inside a transaction
  --> src/shop/checkout.y:27:13
   |
27 |             gateway.charge(method: buyer.payment_method, amount: stock.price)?
   |             ^^^^^^^^^^^^^^ `http<payment_gateway>` performs network I/O
   |
   = why: `atomic hold_one_unit` may run more than once; this call cannot be undone, so it may happen more than once
   = fix: submit a durable command, which is persisted if and only if the transaction commits:
          enqueue charge_card { hold: ..., amount: ..., method: ... }
          (needs a capability of type `enqueue<charge_card>` in the `dependencies:` block; `y fix` adds it)
   = spec: y:spec/shared-state#no-external-effect
```

The agent switches to `enqueue`. The next build fails again — `retry` waits for the inventory to change, and nothing
bounds that wait:

```text
error[Y0301]: unbounded wait
  --> src/shop/checkout.y:22:17
   |
22 |                 retry
   |                 ^^^^^ waits until `inventory.stocks` changes, with no deadline above it
   |
   = fix: bound the enclosing scope, e.g. `within 5s?: ...`; the expiry becomes a typed failure
```

After `within 5s?:`, the compiler reports that the failure set now includes `expired` and that `hold_error` has
no case carrying it. The agent adds `case timed_out(deadline: expired)`, and `y fix` pins the authority it inferred:

```text
$ y fix src/shop/checkout.y
  pinned  with inventory, charges   (hold_and_charge)
  added   inventory: ..., as shared<inventory>.   charges: ..., as enqueue<charge_card>.
  added   fails hold_error.timed_out: ...   (documentation for the new capabilities and failure case; prose left for the author)
```

**The test** lives at the path computed from the module (`src/shop/checkout.test.y`), declares which outcome it covers,
supplies capabilities the same way production code receives them, and forces the retry path instead of hoping for it:

```y
-- A conflicting commit forces the transaction to run three times; the customer is still charged once.
test charges_once_when_the_transaction_retries
        covers hold_and_charge#in_stock:
    given:
        widget = item_id.parse("widget")?
        buyer = any<buyer>(seed: 11)
    inject:
            inventory = shared.new(inventory.stocked(item: widget, available: 1, price: 1_500isk))
            charges = fake enqueue<charge_card>
    faults:
        conflict at hold_and_charge#hold_one_unit, times: 2
    when:
        hold = hold_and_charge(widget, buyer)?
    then:
        charged_once = (charges.committed().count() == 1)
        held_the_widget = (hold.item == widget)
        no_stock_left = (inventory.snapshot().stock_of(widget)?.available == 0)
```

**The reviewer** does not open the diff first. They open the review packet, which is ordered by risk:

```text
$ y review main..hold-and-charge

Signatures
  + shop.checkout.holds.hold_and_charge                   new, public
      authority    shared<inventory>, enqueue<charge_card>
      fails        hold_error.unknown_item, hold_error.timed_out
      waits        at most 5s
      transacts    hold_one_unit — writes inventory.stocks, inventory.holds; enqueues charge_card
Commands
  charge_card                                             new submitter: hold_and_charge
Claims            none
Gaps              none
Coverage          4 of 4 outcomes covered by 4 tests (#sold_out, #in_stock, !hold_error.unknown_item, !hold_error.timed_out)
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
- Imports name what they bring in, by their path inside the module, facet included:
  `from shop.inventory import stock.inventory, stock.item_id`. There are no wildcard imports, no
  re-exports and no relative paths. An import cycle is an error that lists the cycle.
- The formatter sorts imports (standard library, then dependencies, then this package). Each name means the same thing
  everywhere in a package, so an alias (`as`) is allowed only to resolve a collision.
- **Everything is private to its module by default.** A declaration is visible to other modules only as a member of a
  `public facet`, and `public` applies to nothing else (§ 3.2).
- A module contains declarations only. There is no executable code at module scope, no module initialiser, and no
  mutable global state of any kind. State that outlives a call is passed in (§ 3.7).

**Names.**

- **Every identifier is `lower_snake_case`:** values, functions, fields, modules, types, cases, traits, states,
  commands and constants alike. Any other casing is a compile error, as are leading, trailing and doubled underscores.
- This is the long-standing lowercase-everything rule. A reviewer tells a type from a value by where it stands, not by
  its case: types appear in the documentation of a function, in a field, after `type`, and before `.` in a
  constructor or a qualified case (`word.new(spelling)`, `payment_result.approved(...)`).
- **Types and values have separate names, but one spelling.** A field may be `word is word`, and a capability
  `console` may have the type `console`. In an expression, a value hides a type of the same name; the type is then
  reached through its module (`bank.receipt(...)`), which the formatter writes only where it is needed.
- Identifiers are ASCII. Bidirectional-control and invisible characters are rejected everywhere in source except as
  escapes inside text literals. A reviewer must be able to trust that what they see is what compiles.
- **No abbreviations,** except a short list that every Y programmer learns once:
  - the project glossary (`id`, `url`, `http`, …);
  - the sized numeric types (`int8` … `int64`, `uint8` … `uint64`, `float32`, `float64`);
  - standard unit symbols (`s`, `ms`, `min`, `h`, `km`, `kb`), and currency codes (`isk`, `eur`).

  The linter checks this.
  - The rule binds the language as much as its programs. Keywords are words: `public`, `function`, `constant`,
    `implementation`, `mutable`.
  - So are the standard library's names (`boolean`, `divide_floor`, `remainder_euclidean`) and the toolchain's
    commands (`y format`).
- Lengths, sizes and durations do not need units in their names, because units are in their types (§ 3.3).

**Bindings.**

- **`name = value` binds a name.** There is no `let`: the first `=` for a name in a scope declares it, and every
  binding is initialised where it is declared.
- **Every change to a name is written `:=`:** `low := middle.next()`, `account.balance := account.balance - amount`.
  `=` never changes a name, and there are no compound assignments (`-=`, `+=`), so a reader finds every change of a
  value by searching for `:=`. YAPL had `=` to bind and `:=` to rebind (§ 7).
- **No keyword marks a name that changes.** `found = list.new()` declares `found`, and `found.push(parsed)` or
  `low := middle.next()` later changes it. Each change is visible where it happens, as `:=` or as a `mutable` argument,
  so a marker at the declaration would say nothing new. There is no `let`, `var` or `variable`.
- **No shadowing:** a name visible in a scope may not be declared again inside it, including parameter names. Within a
  function, a name means one thing.

**Expressions.**

- **Y keeps symbols few.** Boolean operators are words (`and`, `or`, `not`), a result is named with `returns`, and a
  failure type with `fails` (§ 3.4). There is no `->`, no `&&`, `||` or prefix `!`, and no `&`.
- Mixing operator families requires parentheses. Write `a + (b * c)` and `(a < b) and (c < d)`. Chains of one
  operator need none (`a + b + c`). Chained comparisons (`a < b < c`) are rejected. A reader should never have to
  recall a precedence table.
- **Parentheses around a single element are a lint error,** which formatting fixes. A name, a literal, a field path,
  an index or a call needs none: `if blank = piece.is_empty():`, not `if blank = (piece.is_empty()):`. A comparison,
  an arithmetic or boolean expression, and `not` keep theirs where a named condition binds them:
  `if reached = (misses >= needed):`. Parentheses are there to show grouping, and where nothing is grouped they
  only add noise.
- `/` is defined only on fractional types.
  - Integer division is spelled with the rounding it performs: `divide_floor`, `divide_truncated`, `divide_euclidean`,
    or `divide_exact`, which fails with `inexact` if there would be a remainder.
  - There is no `%`; use `remainder_euclidean` or `remainder_truncated`.
  - Arithmetic is checked, and overflow is a fault (§ 3.4).
- Conversions that can lose information say so in their names (`to_integer_truncated`,
  `to_integer_rounded(rounding.half_even)`). No conversion is implicit.
- Interpolation accepts a name or a field path and nothing else: `"hold {hold.id} for {buyer.name}"`. Calls,
  indexing and arithmetic are computed into a named local first.
- Literals that are small languages are checked at compile time: `pattern"^[a-z]+$"` (a regular expression),
  `url"https://example.com"`, and format strings. Y has no query strings (§ 3.13).
- **Blocks are statements.** `if`, `switch`, loops, `within`, `atomic`, `orelse`, `inject`, `arena`, `uncancellable` and
  `supervise` produce no value, and Y has no conditional expression and no ternary operator.
  - A value chosen by a condition is assigned in each branch. The decision then has a name and a place, like every
    other: `if has_head = (misses >= 1):` assigns `head = "O"`, and `else no_head:` assigns `head = " "`.
  - A value made inside a transaction or a deadline is returned from inside it, or assigned with `:=` to a name declared
    outside it.
  - A `switch` arm is `case`, the arm's name, `=` and a pattern, ending in a colon, with its statements indented
    beneath it, as every block is: `case valid = result.ok(parsed):` then `found.push(parsed)`. There is no `=>`, and
    every arm is named (§ 3.5).

**Functions and their results.**

```y
-- answer: The meaning of life, as int32.
function meaning_of_life() returns answer:
    return answer = 42
```

- **A signature names its parameters and its result, and nothing else.** It has no types and no `->`:
  - `function guess(mutable game, line) returns verdict fails guess_error`;
  - then the colon, and as the body's first block its capabilities, `dependencies:` with `console` and `random`
    beneath it;
  - then its contracts.
- **Types live in the documentation directly above** (§ 3.2). Each parameter, the result and each capability has a
  role that ends with `as` and its type: `line: What the player typed, as string.` A signature whose documentation lacks a
  type does not compile, so every function with parameters or a result is documented, private ones included.
- **The result is a name.** `returns answer` names the result, and only the `return` that leaves the function gives
  it a value: `return answer = verdict.hit`. Assigning to the result anywhere else is an error (the toolchain's rule,
  language reference § 8).
  - Every path ends in `return` or `fail`.
  - A `return` inside `atomic` commits the transaction and then returns; inside `within`, it leaves the deadline's
    block. So a value made inside a transaction is returned from inside it (§ 0's `hold`).
  - A function without `returns` has no result, and its `return` takes no name.
- **A function that can fail says so in its signature:** `fails guess_error`. Its documentation then has a
  `fails <type>.<case>:` role for every case: `fails guess_error.empty:`.

**Calls.**

- Arguments are positional only when the callee has one parameter, or two parameters of different types. Otherwise
  every argument is named: `transfer(from: savings, to: checking, amount: 50eur)`.
- The formatter removes names where positions are unambiguous and adds them where they are required, so there is one
  canonical spelling of every call.

**Lambdas and nested functions.** Y has no inline lambda expression and no `=>` closure. Every callable has a name,
documentation and a signature, as the toolchain's language reference (§ 8, "Lambda declarations") and YAPL's explicit
closures (`closure … closes … over …`) both required:

```y
----------------------------------------------------
Whether someone has signed in lately.

user: The person, as user.

active: Whether they have, as boolean.
----------------------------------------------------
is_active = lambda is_active(user) returns active
        captures cutoff:
    return active = (user.last_seen > cutoff)


active_users = users.filter(is_active)
```

- A lambda is bound by assignment, `name = lambda name(…)`, and can be stored and passed like any value. A helper that
  is only called is a `nested function` in the body.
- **What a callable captures is listed:** `captures cutoff`, `captures mutable total`. A capture that is not listed is
  an error, so the signature shows everything the helper can reach.
- The name appears in traces and fault reports, as every function's does.
- A named predicate over data that more than one function uses is a `rule` (§ 3.13), which a pipeline takes by name.

**Comments and layout.**

- **`--` starts a comment** that runs to the end of its line.
- **A line of three or more dashes, alone, opens a block comment,** and the next such line closes it.
  - Any number of dashes from three up is a rule. Its job is to show plainly that a block begins and ends, and to look
    tidy doing it.
  - An author types `---`. `y format`, and every editor and agent that writes Y, widens every rule in a file to end in
    one column: the first multiple of four past the right-most character of code in the file, comments not counted. A
    rule starts at its indentation, so the rules of a file share their right edge whatever their depth, and they
    reach a little past the code beside them.
  - Two lines of one block that differ in width are a lint error, which formatting fixes.
- **A comment directly above a declaration, with no blank line between, is its documentation** (§ 3.2). Every other
  comment is free text.
- `//` and `/* */` are not comments in Y. The diagnostic names `--`, and `y fix` rewrites them.
- Commented-out code is a lint error. Delete it; history keeps it.
- **Blocks are a colon and indentation, as in Python.** A line that opens a block ends with `:`, and the block is the
  lines indented four spaces beneath it. A header that does not fit on one line continues on lines indented eight
  spaces, and its last line ends with the colon. There are no braces.
- **One statement per line.** A block's body never shares its header's line, and two statements never share one line.
- **Two blank lines separate one declaration from the next,** at module level and inside a facet alike; fewer or more is
  a lint error, which formatting fixes. Inside a body, at most one blank line separates statements. Members of a
  structure, a type, a constant group or an enumeration are not declarations in this sense, and follow one another
  directly.
- **A blank line comes before every flow-control statement** (`if`, `while`, `for`, `switch`, `break`, `continue`,
  `retry`, and the blocks `atomic`, `within`, `inject`, `arena`, `uncancellable` and `supervise`), unless it is the first
  statement of its block. Comments about the statement may sit between the blank line and the statement. `fail` and
  `return` need no blank line, and neither do `else`, `orelse` and a `switch` arm's `case`, which continue the
  statement above them. The rule holds in every block, not only in functions: a test's sections, a lambda, a handler,
  a workflow. A missing blank line is a lint error, which formatting fixes.
- Y sets no line length. `y format` has no options and preserves every comment. It produces one layout for any
  program, so every diff is about content.
- A function body longer than 60 lines is a lint warning, and longer than 120 is an error. A reviewable sample fits on
  a screen.

### 3.2 Declarations and documentation

**Declaration kinds.** Y has few: `function`, `structure`, `type`, `enumeration`, `state`, `transition`, `trait`,
`implementation`, `constant`, `constants`, `unit`, `schema`, `migrate`, `command`, `handler`, `workflow`, `derived`,
`rule`, `test`, and `facet`, which groups the others.

- There are no classes, singletons, factory methods or inheritance.
- A constructor is an ordinary function returning the type (`new`, `parse`, `from_…`).
- Behaviour is shared through traits with explicit `implementation`s. Every iteration that had more kinds of type than
  this spent its time reconciling them.

**Constants, constant groups and enumerations** keep the toolchain's syntax (`y-language-toolchain`, language reference
§ 3, "Constant, Constants, and Enumeration Declarations"):

```y
-- The number of seconds in a minute.
seconds_per_minute is constant int64 = 60


-- The rules every game is played by.
constants game_rules:
    -- How many wrong guesses a player may make.
    allowed_misses is count<element> = 6
    constants scoring:
        points_per_hit = 10


-- Where a game stands.
constants outcome:
    playing = "playing"
    won = "won"
    lost = "lost"


-- How serious a log entry is, from least to most.
enumeration severity is string:
    debug = "DEBUG"
    warning = "WARNING"
    error = "ERROR"
```

- A single constant is `name is constant type = value`, and its type may be left to the value: `answer is constant = 42`.
  Inside a `constants` group the word `constant` is not repeated, and a group may hold groups. A member is reached
  through its group: `game_rules.allowed_misses`.
- Every value is computed while the program is built (§ 3.17) and holds nothing mutable.
- **A constant group whose members share one type is also a closed type:** a value of `outcome` is one of its members.
  A member is always written with its group, `outcome.won` (§ 3.3), and `switch` over it must cover every member. This
  is the type for a set of named values with no order: where a game stands, what a guess did, and the cases of a
  failure that carry nothing.
- **An enumeration is for a set that has an order and is iterated:** `for level in severity`, `severity.warning <
  severity.error`. Its values are numbered from zero in the order written unless given values, and have one type, the
  smallest unsigned integer that holds them unless the enumeration names one (`is string`). Without an order to use or
  a reason to iterate, a constant group is the right declaration.
- A `type` with `case`s is for cases that carry data (§ 3.3).
- **`is` joins a name to its type wherever a declaration states one:** a field (`word is word`), a case's field
  (`case console(error is console_error)`), a constant (`is constant`), a typed member or enumeration, and a refined
  type (`type word is string where …`, § 3.3). A function's parameters and result take their types from the
  documentation instead. `is` has no other meaning: it is never a predicate, a capture or a mutability marker (§ 7).

**Facets** keep the toolchain's syntax and its rule that a facet's header sets the visibility of everything in it
(`y-language-toolchain`, language reference § 10, "Module Bodies and Facets"):

```y
-- One game, and what a guess does to it.
public facet rounds:
    -- Where a game stands.
    constants outcome:
        playing = "playing"
        won = "won"
        lost = "lost"


    ----------------------------------------------------------------------------
    Starts a game.

    word: The word the player must find, as word.

    started: A game with nothing guessed, as game.
    ----------------------------------------------------------------------------
    function new_game(word) returns started:
        return started = game(word: word, hits: set.new(), misses: list.new())


-- Private: nothing outside this module can call it.
function part(misses, needed, mark) returns drawn:
    ...
```

A structure may hold facets of its own, and so facets nest, one structure apart:

```y
-- Accounts, as other modules see them.
public facet ledgers:
    -- An account and its balance.
    structure account:
        -- What every holder of an account may read.
        public facet statement:
            number is account_number
            balance is money<isk>

        -- Kept to this module: changed only through `withdraw` and `deposit`.
        facet bookkeeping:
            entries is list<entry>
```

- **A facet is a named group of declarations,** in a module or in a structure. It holds any declaration except a
  facet or a test, and a declaration belongs to at most one facet.
- **Facets nest, though never directly.** A facet holds no facet, but it may hold a structure, the structure may hold
  facets, and those may hold structures in turn: module, facet, structure, facet, and so on, as deep as the design
  needs.
- **`public` is written only on a facet.** `public facet rounds:` makes every member visible to modules that import
  them; `facet drawing:` groups members that stay private. A declaration outside every facet is private to its
  module. No function, type or constant is ever marked `public` itself.
- **Private is the default everywhere.** The cases of a type and the members of a constant group or an enumeration
  are visible wherever the type or group is. A structure's fields are private to its module; a structure whose fields
  other modules read or build declares them inside a `public facet` of the structure, as `account` does above.
- **A facet's name is part of each member's path.** `new_game` in the facet `rounds` of the module `hangman` is
  `hangman.rounds.new_game`, and an importer writes `from hangman import rounds.new_game`. The path goes on through
  each structure and facet that holds a member: `bank.ledgers.account.statement.balance`. The address of a decision
  includes it too: `hangman.rounds.guess#repeated`.
- The import binds the last segment of the path, so the importer calls `new_game(word)`. Inside its own module, a
  declaration names the others by their own names, whichever facet holds them.
- A field is still read from a value by its own name, `account.balance`: the path names the declaration, and a
  value already knows its type.
- The review packet (§ 4.5) groups a change to a module's surface by facet.
- Every facet and every member of a public facet carries documentation: together they are the module's surface.
- The toolchain's facets also carried override, abstraction and telemetry rules for classes. Y has no classes, so a
  facet carries visibility and a name, and nothing else.
- In this proposal, a *public* declaration means a member of a public facet.

**Documentation is grammar, written as a comment.** The April 2026 spec block was right that every declaration a reader
relies on needs structured documentation in a closed grammar. It went further and made the block *replace* the
signature, which the examples themselves then abandoned for small helpers. Y keeps the signature in code.

Its documentation is the comment directly above it: a `--` line, or a `---` block. That comment is read in a closed
set of roles, and it is where a function's types are written:

```y
--------------------------------------------------------------------------------
Takes money out of an account.

account: The account to debit, as account.
amount: How much to take, as money<isk>.

debit: A receipt for the debit, as receipt.

fails withdraw_error.insufficient_funds: The balance is lower than the amount.
--------------------------------------------------------------------------------
function withdraw(mutable account, amount) returns debit fails withdraw_error
```


| Role | Meaning | Required on |
| --- | --- | --- |
| *(summary)* | First paragraph; one sentence saying what the declaration is for | Every facet, every public declaration, every `test`, every `command` |
| `why:` | Why it exists or is shaped this way, where the name does not say | Optional; required on claims (§ 3.15) |
| `<parameter>: …, as <type>.` | One per parameter, by name: what it means, and its type | Every parameter of every function |
| `<result>: …, as <type>.` | What the result named after `returns` means, and its type | Every function with a result |
| `<capability>: …, as <type>.` | One per capability in `dependencies:` | Every capability |
| `fails <type>.<case>:` | When and why each failure case occurs, the case written with its type (§ 3.3) | Every case of a public function's failure type |
| `satisfies:` | A `y:` link to the requirement or journey this implements | Optional |
| `see:` | A `y:` link to a related declaration or document | Optional |
| `gap:` / `defect:` / `blocked:` | Unfinished or broken behaviour (§ 3.16) | Where it applies |

**Roles of one kind sit together, and one blank line separates the kinds.** The parameters form one group, the
result another, then the capabilities, the failures, and each other role (`why:`, `see:`, …). Inside a group the
lines follow one another directly; between two groups exactly one blank line is required. A missing or extra blank
line is a lint error, which formatting fixes. A reader sees at a glance what goes in, what comes out, what is held
and what can go wrong:

```y
--------------------------------------------------------------------
Reads a word list.

text: The words, separated by spaces, as string.

words: Every word of the list, in order, as list<word>.

fails word_list_error.empty: The list has no words.
fails word_list_error.not_a_word: A piece of the list is not a `word`.
--------------------------------------------------------------------
function parse_word_list(text) returns words fails word_list_error:
```

The compiler checks the documentation against the declaration:

- A parameter, result or capability documented but not declared, or declared but not documented, is an error, and so
  is one whose role has no type.
- A `fails` role for a case the failure type does not have is an error.
- Any prose line of the shape `word:` must be a valid role (`Y0102`). An agent cannot confuse a stray colon with a
  structured field, and a reader cannot either.

Prose is constrained CommonMark:

- No HTML and no images.
- Links go only to `https:`, `mailto:` or `y:`.
- `y:` links are resolved by the compiler: `y:shop.checkout.holds.hold_and_charge#sold_out`,
  `y:requirements/checkout.md#no-oversell`, `y:spec/shared-state#no-external-effect`. A broken link is a build error.

**Traceability is one role, not an ontology.**

- `satisfies:` is the whole of what survives from ai.lang's five-layer graph inside the language.
- From it the toolchain can answer the two questions that graph existed for: *which code implements this requirement*,
  and *which requirements have no code*.
- The organisational layers (visions, personas, owners, on-call) belong in documents the links point to, not in the
  language.

### 3.3 Types

**Scalars.**

- `boolean`, and `int64`, which is 64-bit and checked.
- `int8` … `int64` and `uint8` … `uint64`; `float64`; `decimal`.
- `void` is an ordinary type with one value.

There is no truthiness, no null, and no implicit numeric widening.

**Structures** have named, typed fields, each `name is type`, and may declare named invariants:

```y
structure stay:
    guest is guest_id
    arrives is date
    departs is date

    invariant departs_after_arrival = (departs > arrives)
```

**Sum types** are closed. Every case payload has named fields, because Y has no tuples anywhere:

```y
type payment_result:
    case approved(authorization is authorization)
    case declined(reason is decline_reason)
    case unavailable
```

Referring to a case:

- **A case is always written with its type:** `payment_result.approved(...)`, `outcome.won`, in an expression, a
  pattern, a `return` and a `fail` alike. There is no `.approved` shorthand: a reader never has to work out which type
  a case belongs to, and an agent never has to guess.
- The built-in sums are named like any other: a call's outcome is `result.ok(value)` or `result.err(failure)`, and an
  optional value is `optional.some(value)` or `optional.none`.
- A pattern binds a payload field under its own name (`payment_result.declined(reason)`), or under a new one
  (`payment_result.declined(reason: why_declined)`).

**Absence.**

- `t?` is a distinct type from `t`. Neither converts to the other implicitly.
- `??` supplies a default, `?.` reaches through absence, and `switch` covers `optional.some(value)` and `optional.none`.
- **A condition narrows.** In the branch of `if found = (user != optional.none):`, and in the `else present:` branch
  of `if missing = (user == optional.none):`, `user` is a `user`, no longer a `user?`. Nothing is written for
  this: every condition adds its predicate as a fact at the points it guards (constraint language § 11.1.1), and the type checker reads the fact.
  - The fact ends where the value may change: an assignment with `:=`, or a call that takes it `mutable`. A field path
    such as `order.customer` narrows the same way, until something may write to that path.
  - The same facts discharge a callee's `where` preconditions (§ 3.11) when they imply it.

**Several results.** A function that produces several results returns one structure. For a one-off result, its type is
an anonymous structure with named fields: `parts: The quotient and the remainder, as structure(quotient: int64, remainder: int64).`

**Refined types.** A type may be a named restriction of another. Each restriction has a name, and construction checks
all of them:

```y
type quantity is int64 where positive = (value > 0)
type email_address is string where has_one_at_sign = (value.count_of('@') == 1)
type word is string:
    where letters_only = value.matches(pattern"^[a-z]*$") and
          long_enough = (value.count() >= 4)
```

- A refined type is `type name is base where …` on one line. Several restrictions are joined with `and`, each with
  its own name. When they do not fit on one line, the header ends with a colon, `type word is string:`, and the
  `where` clause is its block, indented beneath it, with each further restriction aligned under the first.

- `quantity.new(n)` returns a `quantity`, and fails with `constraint_violation`.
- A literal is checked at compile time: `ship(quantity: 0)` does not compile when the parameter is a `quantity`.
- A violation names the constraint that failed and the values of its subexpressions. Constraints never carry
  hand-written messages; the name *is* the message.
- The same declaration drives test data. `any<quantity>(seed: 7)` produces values that satisfy every constraint, and
  `any_violating<quantity>(seed: 7)` produces values that break exactly one.

**Units.** A quantity's unit is part of its type:

- `5s`, `12.4km` and `1_500isk` are typed literals. Arithmetic checks dimensions, and conversions are explicit, with the
  factor declared once on the unit.
- A currency is a dimension, not a scale factor, so ISK and EUR never convert at an ambient rate.
- Units are erased before code generation.

The same mechanism gives every length and index its unit:

- `string.count()` gives a `count<rune>`, `bytes.count()` a `count<byte>`, and `list.count()` a `count<element>`.
- `index<rune>` does not index `bytes`.
- An integer *literal* takes its count or index unit from context, so `list.count() == 1` needs no ceremony. An `int64`
  *value* never converts implicitly. Physical units get no such allowance: `within 5` is an error, because `5` could
  mean seconds or milliseconds, but a count of runes can only mean runes.
- This is the July 2026 rule (*always name the unit: `length_in_bytes`, never `size`*) and the March 2026 typed lengths,
  moved from naming convention into the type checker.

**Strings.**

- `string` is a sequence of `rune`s. A `rune` is exactly one extended grapheme cluster, and a string is NFC-normalised
  when it is constructed, so canonically equivalent strings are equal.
- `bytes` is a separate type. Conversion between the two names its encoding and what to do with invalid input
  (`string.decode_utf8(bytes, on_invalid: invalid_input.reject)`), and rejecting is the default.
- Nothing is locale-sensitive by default. Case folding, collation and formatting take a `locale` argument, and no
  locale is ever inferred from the process.

**Equality, hashing and ordering** are derived structurally by the compiler, and cannot be written by hand. A hand-written
`equals` is a place for an agent to be subtly wrong and for a reviewer to miss it.

- A field marked `transient` is excluded from equality, hashing and serialisation.
- Comparison returns an `ordering` (`ordering.less`, `ordering.equal`, `ordering.greater`), never an integer.

**Sensitive data.** `sensitive<t>` wraps values such as credentials and personal data.

- It cannot be interpolated, logged, traced or serialised outside a schema field declared sensitive.
- `reveal(value)` requires the `reveal` capability (§ 3.7).
- Fault reports and traces mask it.

### 3.4 Failure

A function that can fail names its failure type in its signature: `returns debit fails withdraw_error`.

- **A call's outcome is a built-in two-case sum,** `result.ok(value)` or `result.err(failure)`. This settles the September open
  question: one `switch` ranges over success and failure together, and an optional result that can fail (`as
  receipt?` with `fails e`) composes without a special rule. Core IR still records every propagation edge explicitly,
  so *which failures can escape, and from where* is always answerable.
- Inside the function, success is `return debit = value`, never `result.ok(value)`, and failure is written
  `fail withdraw_error.insufficient_funds(shortfall: gap)`. Every failure exit is spelled `fail` or `?`, so every one of them can be
  found with a search, and there is exactly one way to write each.
- `?` propagates a failure unchanged, or converts it through the single case of the enclosing failure type that
  carries it. If two cases could carry it, the program is rejected and the error names both candidates.
- A failure leaving an `atomic` block aborts the transaction, which leaves no trace (§ 3.9). This is the July 2026
  rollback-on-failure rule, carried into the language.

```y
switch load_customer(id):
    case known = result.ok(customer):
        greet(customer)
    case new_visitor = result.err(customer_error.not_found(id: missing_id)):
        offer_signup(missing_id)
    case unreachable = result.err(customer_error.unavailable(retry_after)):
        try_later(retry_after)
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
| Condition | `if found = (items[middle] == target):` |
| Alternative | `else if go_right = (items[middle] < target): ...` |
| Fallback | `else go_left: ...` |
| Loop | `while searching = (low <= high):`, then `else exhausted:` |
| Switch arm | `case known = result.ok(customer):`, and `else other:` for the rest |
| Guard | `case active = optional.some(user) when (user.last_seen > cutoff):` |
| Precondition, guarantee | `where positive = (amount > 0isk)`, `ensures debited = (...)` |
| Invariant, constraint | `invariant departs_after_arrival = (...)`, `where positive = (value > 0)` |
| Assertion | `assert charged_once = (charges.committed().count() == 1)` |
| Transaction, task | `atomic hold_one_unit: ...`, `task profile = load_profile(id)` |

**Exemptions and defaults.**

- A predicate that is already a single boolean name, or `not` of one, needs no second name: `if is_empty: ...`.
- **`else` always has a name of its own:** `else go_left:`. The name is bound to `true` when the fall-through branch
  runs, so tests, traces, debuggers and logs can say which branch ran, as for every other branch. An `else` without a
  name does not compile (the toolchain's rule, language reference § 7).
- **Every outcome of every decision is a named branch.** An `if` always ends in a named `else` block, and a `while`
  has one too, which runs when the loop ends, as Python's does: `while searching = (low <= high):`, then
  `else exhausted:`. A decision with an unnamed outcome does not compile.
  - Each branch is a line of its own: Y never puts two statements on one line, so the name of the other outcome cannot
    ride on the header.
  - Code that follows a decision usually belongs in its `else` block (`else in_stock:` in § 0), so the block is never
    empty. A decision with nothing to do on one side is better written as a function with a named result.
- **Every `switch` arm is named:** `case known = result.ok(customer):`. An arm that catches what the others leave is
  `else name:`, as in an `if`. The arm's name covers its guard: `case active = optional.some(user) when (…):` needs no
  second name. A test covers an arm as it covers a branch, `load#new_visitor` (§ 3.14).
- A `for` loop is named by its variable. Inside nested loops, `break` and `continue` must name their loop:
  `break line`.
- There is no `loop` and no `while true`.

**What the names are for.** Each name becomes part of a stable address: `shop.checkout.holds.hold_and_charge#sold_out`,
`#in_stock`, `#hold_one_unit`. Those addresses are what:

- tests declare they cover (§ 3.14);
- production traces record, when sampled;
- fault reports cite (§ 3.4);
- the review outline shows. `y review --outline` renders a function as its signature, its documentation and its tree
  of named decisions with bodies collapsed, so a reviewer can read at the level of intent and open only what looks
  wrong. This is ai.lang's "zoom", as a view rather than a data model.

```y
--------------------------------------------------------
Finds where a value sits in a sorted list.

target: The value to look for, as int64.
items: The list to search, as sorted_list<int64>.

position: The target's position, or nothing when it is absent, as index<element>?.
--------------------------------------------------------
function position_of(target, items) returns position:
    low = items.first_index()?
    high = items.last_index()?

    while searching = (low <= high):
        middle = low.midpoint(high)

        if found = (items[middle] == target):
            return position = optional.some(middle)
        else if go_right = (items[middle] < target):
            low := middle.next()
        else go_left:
            high := middle.previous()?
    else exhausted:
        return position = optional.none
```

In a function whose result is optional, `?` on an absent value returns with the result absent.

The parameter is a `sorted_list`, not a `list` with a `where sorted` precondition. Where a property can be carried by
a type, Y prefers the type (§ 1.5, rule 2).

### 3.6 Values and ownership

**A program reads as if every value were garbage-collected.** Y source has no references, no borrows, no lifetimes
and no moves. A value is passed, stored and returned as freely as in a garbage-collected language. What it never does
is change under someone who holds it: Y values have value semantics.

- **Parameters are immutable by default.** A parameter `x` is read and never changed, so the caller's value is the
  same after the call.
- **`mutable` marks the exception, at both ends of the call.** A parameter `mutable x` lets the callee change the
  caller's value, and the call says so: `guess(mutable game, line)`. While the call runs, nothing else can reach that
  value. This is YAPL's two-sided `mutable` (2024): a reviewer sees at the call which arguments the callee may change.
- **Bindings:** a local name may be changed, with `:=` or by a call that takes it `mutable`, and nothing marks it at its
  declaration. A parameter may be changed only when it is declared `mutable`.
- **Method receivers** are declared `self` or `mutable self`. They are not marked at the call (`order.cancel()`),
  because receiver syntax is where agents' habits are strongest. The elaborated view shows the mode.
- **Keeping a value keeps it as it is.** When `new_game(word)` stores `word` in the game, nothing done to the game's
  word changes the caller's, and the reverse.

**The compiler chooses the representation.** How a value is passed and kept is a physical decision of the backend:

- An immutable parameter that the callee only reads is passed as a shared borrow.
- A value that the callee keeps is moved when the caller no longer uses it, and copied when it does.
- A large value that would be copied often may be counted and copied on write instead.
- A `mutable` parameter is an exclusive borrow.

`y explain` lists every copy the compiler made and why it made it, and the review packet counts them. A copy is
visible where it costs something, without being written where it does not.

**Why.** Ownership syntax is where agents writing Rust spend most of their failed attempts. It tells a reviewer about
memory, not about intent. Value semantics keep what the borrow rules were for: nothing changes behind a reader's back,
and two tasks never share a value that either may change (§ 3.8, § 3.9). The cost is the copies the compiler cannot
avoid, and it reports each one.

**No lifetimes.** Source has no references, so it has no lifetime parameters and no results that borrow. A structure
holds values, never references to them. Borrows exist only in the generated code, where `rustc` and Miri check them
(§ 4.6).

**Sharing:**

- Mutable state reachable from more than one task is `shared<t>` and nothing else (§ 3.9). There is no interior
  mutability outside transactions.
- Graphs live in an arena or collection and refer to each other by typed index (`index<node>`).

**Cleanup.**

- A type that declares release behaviour is a **resource**: a file, a socket, a lock. A resource is never copied.
  Giving it to a callee that keeps it ends the caller's use of it, and a later use is an error that names where it was
  given away.
- A resource is released exactly once, at the end of the scope of its last holder, on normal exit and on failure
  alike. Resources in one scope are released in reverse order.
- Release that can fail declares its failure type, and a scope that holds such a resource includes that failure in its
  own signature. Release failures are never discarded.

**Arenas.** An arena (`arena parse_scratch: ...`) releases everything allocated inside it at once, and nothing
allocated there may escape. Arena ownership says *when a resource is destroyed*, never *when an operation may run*.
A socket owned by an arena is still an external effect (§ 3.9). This is the September distinction, unchanged.

### 3.7 Authority and effects

**A function can affect nothing outside itself except through a capability it holds.** Capabilities are values, and
effects are what capabilities do. This merges September's two open questions, effect syntax and capability syntax,
into one clause:

```y
------------------------------------------------------------------------
Exports one month's invoices as files.

month: The month to export, as month.

exported: How many invoices were written, as count<element>.

invoices: The stored invoices, as store<invoice>.
uploads: Where the files go, as file_system.
clock: The time the export is stamped with, as clock.

fails export_error.write_failed: A file could not be written.
------------------------------------------------------------------------
function export_invoices(month) returns exported fails export_error:
    dependencies:
            invoices
            uploads
            clock
```

**The capability kinds**, each declaring the effects it carries:

| Capability | Effects it carries |
| --- | --- |
| `clock`, `random` | Reads time, draws randomness. Replay records them; tests fake them |
| `http<service>`, `file_system`, `process`, `secrets<name>` | External I/O, may suspend |
| `store<schema>` | Persistent reads and writes (§ 3.12) |
| `shared<t>` | Transactional access to shared state, may suspend on `retry` (§ 3.9) |
| `enqueue<command>`, `submit<command>` | Durable command submission (§ 3.10) |
| `detach`, `forever`, `reveal`, `native` | Trust-requiring operations, each also a claim (§ 3.15) |

**A function with no `dependencies:` block is pure.** It may allocate, compute and fault, but it touches nothing else. Purity
needs no annotation because the missing clause is the annotation. Contract expressions, constraints, `derived`,
`rule` and migrations must be pure, and it is visible that they are.

**Passing capabilities is dependency injection,** with two keywords, one at each end (YAPL v0's `dependency` and
`inject`, 2020, here `dependencies` and `inject`):

- **`dependencies:`, at the receiving site,** declares what a function needs. It is the first block of the body, and
  its names sit one per line beneath it, indented eight spaces, as `inject:`'s bindings do. There is no one-line
  form, and no singular `dependency`, even for one capability:

  ```y
  function play(words) fails play_error
          where has_words = (not words.is_empty()):
      dependencies:
              console
              random
              forever

      chosen = random.index_below(words.count())
  ```

  The block is part of the signature, though it sits beneath the header: `y explain`, the review packet and the
  compatibility signature read it with the parameters.
- **`inject`, at the sending site,** supplies it. `inject:` opens a block. Its bindings come first, one per line,
  indented eight spaces as a header's continuation is, and the statements follow, indented four. Every call in the
  block that needs a `console` or a `random` receives these:

  ```y
  inject:
          console = system.console()
          random = system.random()
      play(words)?
  ```

  A test's `inject:` section has the same bindings, usually fakes (`console = fake console(input: ["r"])`), and no
  statements.
- A callee's dependencies are satisfied, by type, from the capabilities in the caller's scope: the caller's own
  `dependencies:` block and any enclosing `inject:` block. Call sites do not spell them out, so adding a capability to a
  deep function does not edit every call.
- Every function between the entry point and that deep function must still *declare* the capability, because a
  declaration is where a reviewer looks. `y fix` maintains those clauses.
- When two capabilities in scope could satisfy a requirement, the call must choose:
  an `inject:` block around the call, `mailer = receipts_mailer`.

**Deriving capabilities.**

- The entry point receives the root, `system`, and every other capability derives from it.
- There is no way to construct a capability from nothing, read one from a global, or recover one from a type.
- Attenuation only narrows: an `inject:` block binding `uploads = uploads.subtree(path"/var/app/exports")`. This is the single place
  where a name may be rebound in an inner scope, and it is allowed because it can only reduce authority.
- A package manifest lists the root capabilities a program may request. A package's public functions declare what
  they require, so a dependency upgrade that widens required authority is a review item, not a surprise. This is
  YAPL's OS-category approval (2024), made exact.

**Checking.**

- A `dependencies:` block that declares less than the body uses is an error naming the capability and the call path that needs
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
- `sensitive` values are masked.

Friction on observability produces programs nobody can observe, and every iteration of Y has treated observability as
the language's job.

### 3.8 Time and concurrency

**Every wait is bounded, and the bound is checked.**

- `within 30s?: ...` sets an absolute deadline shared by everything inside it. `queue.receive() within 5s` bounds a
  single operation.
- A nested `within` can only shorten the deadline, never extend it.
- Expiry is a typed failure (`expired`). `within 5s?:` propagates it, as `?` after a call propagates a failure, so the
  exit is spelled where the reader looks.
- A suspension with no finite bound above it does not compile. A wait that is genuinely indefinite is written
  `forever because "..."`, which requires the `forever` capability and is a claim (§ 3.15).

**Tasks are structured.**

- `task profile = load_profile(id)` starts concurrent work owned by the enclosing scope. Reading `profile` waits for
  it, within the scope's deadline.
- A scope does not exit until its tasks finish.
- A failing task cancels its siblings, and the scope's failure type includes the failures of its tasks.
- Two tasks may not hold the same value `mutable`, nor one `mutable` while another reads it.
- Work that outlives its scope is written `detach because "..." within 1h: ...`. It requires the `detach`
  capability and carries an explicit lifetime.

**Cancellation.**

- Cancellation arrives only at suspension points, as a failure.
- Cleanup during cancellation has its own bound.
- A short critical region may be protected from cancellation (`uncancellable within 50ms: ...`), and only with a
  bound.
- Cancellation never undoes a committed transaction or withdraws a command that committed with it.

**Supervision is a policy on a scope, not a new kind of thing.** A plain scope is already a supervisor whose policy is
"fail on the first failure". Other policies are written on the scope:

```y
within 24h supervise(restart: restart_policy.each_failed, at_most: 5, per: 10min):
    task ingest = run_ingest()
    task compaction = run_compaction()
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

**Shared mutable state exists only as `shared<t>`**, a capability created with `shared.new(initial)`, normally by the
entry point, and passed down. September's module-level `shared inventory: inventory` becomes a value someone had to
hand you. There is no global state to reach.

- A `shared<t>` is read and written only inside `atomic`, where it behaves as a `mutable` value and the runtime tracks what
  was read and written.
- `snapshot()` outside a transaction returns an immutable, consistent copy. It is a read-only transaction, and exists
  so that tests and reports need not open one.
- State that is not shared is owned by one task, is not transactional, and costs nothing.

**Transactions are named blocks**, because the name shows up in conflict metrics, in fault injection
(`conflict at hold_and_charge#hold_one_unit`), and in the review packet ("transacts hold_one_unit — writes …"):

```y
------------------------------------------------------------
Takes the next job, from the pending queue first and from the retried queue when it is empty.

job: The job to run, as job.

jobs: The queues, as shared<job_queues>.
------------------------------------------------------------
function take_job() returns job:
    dependencies:
            jobs

    atomic take_next_job:
        pending = jobs.pending.pop_front()

        if nothing_pending = (pending == optional.none):
            retry
        else got_pending:
            return job = pending
    orelse atomic take_retried_job:
        retried = jobs.retried.pop_front()

        if nothing_retried = (retried == optional.none):
            retry
        else got_retried:
            return job = retried
```

**Semantics:**

- An `atomic` block has no value. What it makes is assigned to names declared outside it.
- Its reads observe one snapshot, and its writes become visible together at commit.
- A conflicting commit re-runs it from the start.
- A failure leaving it aborts it without trace.
- A nested `atomic` joins the enclosing one.
- `retry` abandons the attempt and waits until something it read changes. That wait is bounded like any other.
- `orelse` tries alternatives left to right and waits on the union of their read sets.

**The load-bearing rule is unchanged: no external effect inside a transaction.** The body of an `atomic` block may
contain only:

- pure computation;
- `shared` access;
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
command charge_card:
    hold is hold_id
    amount is money<isk>
    method is payment_method_id

    policy:
        deduplicate_by: hold
        idempotent: true
        retry: backoff(first: 1s, factor: 2, attempts: 8)
        deadline: 24h
        on_abandon: compensate release_hold(hold: hold)


------------------------------------------------------------------------------------------------------------------------
Performs the charge; runs outside any transaction.

command: The charge to make, as charge_card.

charged: The payment gateway's receipt, as charge_receipt.

gateway: The payment gateway, as http<payment_gateway>.

fails charge_error.declined: The gateway refused the charge.
------------------------------------------------------------------------------------------------------------------------
handler charge_card(command) returns charged fails charge_error:
    dependencies:
            gateway

    within 30s?:
        return charged = gateway.charge(idempotency_key: command.id, method: command.method, amount: command.amount)?
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
------------------------------------------------------------------------------------------------
Takes a held item from payment to shipment.

hold: The hold to fulfil, as hold_id.

shipped: The shipment, as shipment.

orders: The stored orders, as store<order>.
charges: Where charges are submitted, as submit<charge_card>.
shipments: Where shipments are submitted, as submit<create_shipment>.

fails fulfilment_error.payment_declined: The charge was refused.
fails fulfilment_error.payment_unreachable: The charge could not be delivered.
------------------------------------------------------------------------------------------------
workflow fulfil(hold) returns shipped fails fulfilment_error:
    dependencies:
            orders
            charges
            shipments

    order = orders.for_hold(hold)?

    switch submit charge_card(hold: hold, amount: order.total, method: order.payment_method):
        case charged = result.ok(receipt):
            return shipped = submit create_shipment(order: order.id, receipt: receipt.id)?
        case declined = result.err(undelivered.failed(reason)):
            fail fulfilment_error.payment_declined(reason)
        case abandoned = result.err(undelivered.abandoned(abandonment)):
            fail fulfilment_error.payment_unreachable(abandonment)
```

- `submit` suspends the workflow durably until the command's outcome is recorded.
- It succeeds with the handler's result, or fails with `undelivered<e>`, whose two failure cases are `undelivered.failed(e)` and `undelivered.abandoned(...)`. Abandonment is
  therefore an ordinary failure that must be matched or propagated, and cannot be ignored.
- A workflow may open `atomic` blocks.
- A workflow's determinism is enforced by its capability set. It may hold `store`, `shared`, `submit` and `enqueue`,
  and not `clock`, `random` or any I/O capability. Time inside a workflow is the recorded time of its last durable step.
- A workflow persists its logical identity and continuation version, never an address. It resumes under a version whose
  compatibility signature matches (§ 4.4).
- **An external query from transactional logic is a workflow.** Submit the query, wait durably, then open `atomic` with
  the answer. September's four-step request pattern (§ 10 of the working design) becomes this one construct.

### 3.11 Specification in the program

**Contracts** are part of the declaration and, like every predicate, are named:

```y
----------------------------------------------------------------------------------------
Takes money out of an account.

account: The account to debit, as account.
amount: How much to take, as money<isk>.

debit: A receipt for the debit, as receipt.

fails withdraw_error.insufficient_funds: The balance is lower than the amount.
----------------------------------------------------------------------------------------
function withdraw(mutable account, amount) returns debit fails withdraw_error
        where positive_amount = (amount > 0isk)
        ensures debited_exactly = (account.balance == old(account.balance) - amount):
    if short = (account.balance < amount):
        fail withdraw_error.insufficient_funds(shortfall: amount - account.balance)
    else covered:
        account.balance := account.balance - amount
        return debit = receipt(account: account.id, amount: amount)
```

**A precondition and a failure are not the same thing.** A zero amount is the caller's bug, so it is a `where` precondition. An
insufficient balance is a condition the caller must handle, so it is a failure case. Agents conflate the two, and the
documentation rules make the choice visible: a `where` precondition has no `fails` role, and a failure has one.

**Semantics.**

- `ensures` holds when the function succeeds. `ensures on failure name = (...)` states what holds when it fails.
- Contract expressions are pure, since they can have no `dependencies:` block.
- A structure's `invariant` holds whenever the structure is not being mutated. A function taking it `mutable` may break the
  invariant inside its body, and must restore it before returning.
- An invariant on a structure held in `shared<t>` is checked at the commit of every transaction that wrote the structure.
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
state order:
    id is order_id
    lines is list<line>

    case created
    case payment_pending(charge is command_id)
    case paid(receipt is receipt_id)
    case cancelled(reason is cancel_reason)


----------------------------------------------------------------------------
Records that payment was requested for a new order.

created: The new order, as order.created.
charge: The charge command that was submitted, as command_id.

pending: The order, waiting for payment, as order.payment_pending.
----------------------------------------------------------------------------
transition request_payment(created, charge) returns pending:
    return pending = created.becomes(order.payment_pending(charge: charge))
```

- `order.created` is a type, and a transition takes the value it starts from: the name passed to it cannot be used
  afterwards. A stale copy of the old state is therefore a compile error, not a logic error.
- A transition that is not declared does not exist.
- A field that means something only after payment exists only in `paid`.
- A value whose state is unknown, because it came from storage, is matched to recover it.
- Transitions are projected to TLA+ and checked with Apalache for reachability and invariants.

In v1, typestate applies to owned values. Shared or stored state machines are advanced inside `atomic`: switch on the
current state, call the transition, and let conflicting commits retry. This answers the September open question
without new machinery.

### 3.12 Persistent data

A record that outlives its process has a second type: the one on disk. Y makes that type a declaration:

```y
-- A customer as stored, version 3.
schema customer v3:
    key id is customer_id
    name is string
    email is email_address?
    marketing_consent is consent


-- Version 3 adds explicit marketing consent; nobody is assumed to have given it.
migrate customer v2 to v3:
    marketing_consent := consent.not_given
```

**Schemas.**

- Every version stays in source, so the compiler compares versions rather than guessing from a diff. It classifies each
  change as additive or breaking, and names the field that made it breaking.
- Codecs are generated, identity is declared (`key`), and constraints and invariants are checked on read as well as on
  write.
- A field whose type holds a capability or a task does not compile.
- Persisted states are `state` declarations, so the legal transitions of stored data are those of the type.

**Migrations.**

- A migration gives one expression per new or changed target field.
- It is pure, deterministic and idempotent. It may declare other schemas it `reads`.
- It fails per record, not wholesale, and each failure is recorded against that record's key.
- It is reversible only if a `reverse` block is declared, and the compiler reports which version pairs can be rolled
  back.
- An additive change with a total default needs no migration, and the compiler says so.

`store<schema>` is the capability for persistent data, and its reads and writes inside `atomic` join the transaction.

Live migration (`converge`), and the choice of physical representation, belong to the runtime and come later (§ 5). v1
must not prevent them, and does not need them.

### 3.13 Collections and queries

**Collections.**

- `list<t>`, `map<k, v>` and `set<t>`.
- **Iteration order is always deterministic.** A `list` iterates by position; a `map` or `set` by insertion. No
  collection exposes hash order, because hash order breaks replay (§ 4.7), and an agent will eventually rely on it.

**Pipelines** say what the answer is, which leaves the compiler free to fuse, parallelise or index:

```y
active_names = users.filter(active).map(name_of).sorted()
```

- Each stage takes a named callable: a `rule` (`active`), a function, or a lambda bound above (§ 3.1). There is no
  inline lambda.
- **A pipeline reads its source and produces a new value.** It never changes the source: changing the elements of a
  collection is a `for` loop whose body says so, with `:=` or a `mutable` call.
- A pipeline over transactional state reads inside the enclosing transaction.
- Unordered results make no promise about order beyond their source's. Order is asked for with `sorted`.

**Derived values and rules.**

- `derived` declares a value maintained from other state, and is pure. Whether it is stored, cached or recomputed is a
  physical decision, not a keyword.
- `rule` declares a named predicate over data, usable in pipelines, contracts and other rules. **In v1 rules are not
  recursive.** That keeps the declarative subset a vocabulary of named conditions rather than a second evaluator.
  Recursion, with the stratification it requires, can be added later.

```y
-- user: The person to judge, as user.
rule active(user) = (user.last_seen > 30d.ago())


derived active_user_count: count<element> = users.filter(active).count()


----------------------------------------------------------------------------------------------------
A customer whose payments have failed often and who owes a lot.

customer: The customer to judge, as customer.
----------------------------------------------------------------------------------------------------
rule high_risk(customer) = (customer.failed_payments > 3) and (customer.outstanding > 100_000isk)
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
| `given` | Plain values, including generated ones: `any<buyer>(seed: 11)` |
| `inject` | The capabilities the code under test receives: real, attenuated, or `fake` |
| `faults` | What to force: `conflict at f#txn, times: 2`, `crash after commit f#txn`, `expire at f#txn`, `cancel at f#task` |
| `when` | The single action under test |
| `then` | Named checks only, one per line: `charged_once = (charges.committed().count() == 1)` |

**Checks.** Each line of `then` is a named boolean expression, written as a named condition is: `name = expression`.
No `assert` keyword is needed there, because the section holds nothing else.

- The test fails when a check is false. The report names the check (`charges_once_when_the_transaction_retries#
  held_the_widget`) and shows the values of its subexpressions.
- A comment directly above a check is its failure message, and is optional:

  ```y
  then:
      -- Expected the player to be told to type a letter.
      told_to_type = console.written().contains("Type a letter.")
  ```

- `assert name = …` stays the statement for a check inside ordinary code, where its failure is a fault (§ 3.4).

**Coverage is declared, then verified.**

- A function's *outcomes* are the named branches of every decision (`f#sold_out` and `f#in_stock`; in an `if` with an
  `else` block, the block's own name, `f#go_left`), every named `switch` arm (`f#new_visitor`), and every case of its
  failure type, written with its type (`f!failure_type.case`).
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
  generates inputs that satisfy the `where` preconditions and constraints, checks the `ensures` clauses, shrinks any
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
| `unsafe because "...": ...` | This native or foreign code upholds Y's rules | `native` |
| `forever because "..."` | This wait is meant to be unbounded | `forever` |
| `detach because "..." within 1h: ...` | This work may outlive its scope | `detach` |
| `reveal(secret) because "..."` | This sensitive value may be seen here | `reveal` |
| `assume name = predicate because "..."` | The prover may take this as true without proof | — |
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
--------------------------------------------------------
Computes the CRC-32 of some bytes.

bytes: The bytes to check, as bytes.

crc: Their CRC-32, as uint32.
--------------------------------------------------------
function checksum(bytes) returns crc:
    table = compile crc_table(polynomial: 0xEDB8_8320)
    ...
```

**What can be compiled.**

- The operand is one call to a function with no `dependencies:` block, which makes it pure (§ 3.7).
- The function carries no mark. The same function runs while building and while running, as suspension needs no
  `async`. Unlike C++'s `constexpr` and Rust's `const fn`, there is no second kind of function to keep in step with the
  first.
- Every argument is known when the program is built: a literal, a `constant`, or a name bound by another `compile`. Any
  other argument is an error that names it.
- The result holds no capability. It cannot hold a reference either, because Y has none (§ 3.6).
- A constant is evaluated the same way, so `standard_crc_table is constant list<uint32> =
  crc_table(polynomial: 0xEDB8_8320)` needs no keyword. `compile` is the same evaluation inside a body, for a value that one function uses.

**What evaluation produces.**

- **The value is the one the call would return at run time.** Pure code in Y depends on nothing that differs between
  the build machine and the target: integers have fixed sizes and are checked, and there is no locale or hash order
  (§ 4.7).
- **A fault is a build error.** A failed `assert`, an overflow, a violated `where` precondition or a reached `todo` produces the
  fault report of § 3.4, placed at the `compile` and at the construct that faulted.
- **A failure is a build error too.** A call to a function that `fails` must succeed. A failure is reported with its
  case and the case's `fails` role, and the value is the function's result. Literal prefixes (`pattern"…"`, `url"…"`) are this rule with a shorter
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
- A value Rust cannot hold as a constant, such as a `map`, is built once, on first use, from static data.
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
- **`y fix`** applies the machine-applicable part of every diagnostic. It pins inferred authority, adds `mutable` at call
  sites, names or unnames arguments, sorts imports, and scaffolds missing documentation roles. It leaves
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

- the borrows, moves and copies the compiler chose, and why it made each copy;
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

- its parameter and result types, and which parameters are `mutable`;
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

- multi-version transactional memory for `shared` state (the July 2026 spike, re-hosted);
- a durable command log and outcome store;
- a workflow engine;
- `store` over one database projection;
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
| Failure: distinct channel or sum type | `fails e` in the signature; a call's outcome is a built-in two-case sum; Core IR records propagation edges | § 3.4 |
| Degree of ownership inference | All of it: source has value semantics and `mutable` at both ends of a call; borrows, moves and copies are the compiler's, and it reports copies | § 3.6 |
| Types that opt out of single ownership | None needed: values are shared freely; `shared<t>` for mutable state between tasks; arenas and typed indices for graphs | § 3.6 |
| Effect annotation syntax | The `dependencies:` block: effects are what capabilities carry | § 3.7 |
| Capability declaration and passing | Declared per function, threaded by type from the caller's scope, overridden explicitly | § 3.7 |
| Where handler failure escalates | Mandatory `on_abandon`: compensate or park; in workflows, `undelivered.abandoned` is a failure to handle | § 3.10 |
| `afterCommit` durability | Dropped; `enqueue` and buffered telemetry cover every case | § 3.10 |
| Workflow syntax and body boundary | `workflow` may open `atomic`; determinism enforced by its permitted capabilities | § 3.10 |
| Supervision source model | A policy on a task scope | § 3.8 |
| Refinement reasoning | Named `where` constraints on types, contracts on functions, conditions narrow where they guard; no other flow-sensitive inference in v1 | § 3.3, § 3.11 |
| Typestate over shared state | Owned values in v1; shared state machines advanced inside `atomic` | § 3.11 |
| Schema projection onto storage | One projection in v1; projection choice belongs to the physical optimiser later | § 3.12, § 5 |
| What triggers `converge` | A deployment action, post-v1 | § 5 |
| Recursive rules | Not in v1 | § 3.13 |
| Whether runtime generations are in Y | Post-v1; foundations in v1 | § 5 |
| Native backend, ABI and FFI | Rust first; foreign calls are `unsafe` claims needing `native` | § 3.15, § 4.6 |

## 7. What this proposal drops, and why

Everything here was a considered idea in some iteration. Several were held for years. They are dropped because the
premise changed, or because the iteration that tried them showed their cost. The provenance of each is in
[`Y-idea-harvest.md`](Y-idea-harvest.md).

**Syntax and spelling**

| Dropped | Held in | Replaced by | Why |
| --- | --- | --- | --- |
| Braces for blocks | Earlier drafts of this proposal | A colon and indentation, as in Python and YAPL v4 | Indentation is what a reader follows anyway; the formatter and the parser agree on it, and an agent that sends code through a tool that eats whitespace is told so by the parser |
| `is` in five roles | YAPL → April 2026 | `is` in one role, a declared name and its type: `word is word` in fields, `is constant`, `is <type>`, `type name is base` | `is` meant annotation, predicate sugar, capture, mutability and optionality; the March 2026 review called it the most ambiguous token in the language |
| `name: type` for fields | Earlier drafts of this proposal | `name is type`, as YAPL wrote it | One form joins a name to its type; the colon is left to documentation roles and named arguments |
| Hand-matched `---` / `===` / `~~~` delimiter widths | YAPL → April 2026 | `--` lines, and `---` rules that tooling widens to one right edge per file, the first multiple of four past the widest code | Nobody counts dashes; the tool sets a width a reader finds tidy |
| A fixed line length (120 columns) | Earlier drafts of this proposal | No limit; the formatter's layout | The limit served the comment rules, which now take their width from the code |
| `//` and `///` comments | Earlier drafts of this proposal | `--` and `---`, Y's own since YAPL; documentation is the comment above a declaration | Spelling borrowed for agents' habits, which a diagnostic and `y fix` correct instead |
| Abbreviated keywords and names: `pub`, `fn`, `const`, `impl`, `mut`, `Int`, `Bool` | Earlier drafts of this proposal | `public`, `function`, `constant`, `implementation`, `mutable`, `int64`, `boolean` | The no-abbreviation rule binds the language's own words too; the sized numeric types and standard units are the few abbreviations kept |
| `PascalCase` for types, cases, traits, states and commands | Earlier drafts of this proposal | `lower_snake_case` for every identifier, as in YAPL; any other casing is a compile error | One casing rule with no exceptions; a type is told from a value by where it stands |
| Types in signatures, `->` and `T ! E` | Earlier drafts of this proposal | `function f(a, mutable b) returns result fails e`; each type in the role that documents its name (`a: ..., as string.`) | Few symbols; a signature reads as words, and every name is documented with its type in one place |
| Unnamed results and an implicit last-expression result | Earlier drafts of this proposal | `returns name`, and `return name = value` | A result has a name, which its documentation explains, and every exit is a visible `return` or `fail` |
| References in source: `&t`, `&mutable t`, `move`, borrowing results, `copy` types, `reference_counted<t>`, `weak<t>`; "no clone the programmer did not write" | September → earlier drafts of this proposal | Value semantics, `mutable` at both ends of a call; the compiler chooses borrows, moves and copies, and reports each copy | Ownership syntax is about memory, not intent, and is where agents fail most; value semantics keep what borrowing was for |
| `match` with `pattern => statement` arms | Earlier drafts of this proposal | `switch`, each arm `case name = pattern:` with its block beneath, as in the toolchain's language reference § 7 | One way to write a block; no `=>`; every outcome named |
| `with` for capabilities, at both ends | Earlier drafts of this proposal | `dependencies:` where a function declares what it needs, `inject:` where a caller supplies it, each a block with one name per line, after YAPL v0's `dependency` and `inject` | One keyword per side of the injection; a reader sees which side a line is on |
| `public` on any declaration | Earlier drafts of this proposal | `public facet`, as in the toolchain; private by default | Visibility is written in one place per group, and a module's surface reads as a list of facets |
| A case written `.case` where the type is known | September → earlier drafts of this proposal | `type_name.case` everywhere, as YAPL had it (`color.red`) | The reader never infers which type a case belongs to |
| `let` | Earlier drafts of this proposal | `name = value` declares; `name := value` changes, as in YAPL | Every change to a value is spelled one way and found by one search |
| `variable` (and `var` before it), marking a name that may change | Earlier drafts of this proposal | Nothing at the declaration; each change is `:=` or a `mutable` argument | The change is already marked where it happens, so the marker repeated it |
| `if` and `switch` as expressions; a ternary operator | Earlier drafts of this proposal | Statements; a value is assigned in each branch | A decision is a statement with a name and a place, never a value hidden inside an expression |
| Spec block in place of the signature | April 2026 | A signature naming parameters and result; documentation above it with closed roles and the types | The examples abandoned it for helpers; a signature is where a reader looks for names and modes |
| Two spellings of one keyword, method synonyms | YAPL, toolchain | One spelling | Contradicts "one way to write it" |
| `loop` / `leave` / `next` | YAPL → toolchain | `break` / `continue`, named when nested; no unbounded `loop` | Keeps YAPL's rule, not its words |

**Semantics**

| Dropped | Held in | Replaced by | Why |
| --- | --- | --- | --- |
| Classes, factory methods, singletons, properties, inheritance; facets' override and telemetry rules | YAPL → toolchain | Structures, sum types, traits, `implementation`; facets that carry visibility only | Every iteration with more kinds spent its effort reconciling them; facets carried visibility, telemetry and override rules at once |
| Exceptions, `throws`, wrapping in `unexpected_exception` | YAPL → April 2026 | `fails e`, and a two-case sum at the call | Values can be matched exhaustively; wrapping hid the failure set |
| Ambient `using` contexts, context stacks, affinity-based name lookup | YAPL → toolchain | Capabilities in `dependencies:` and `inject:` | Action at a distance; March 2026 had already reversed it ("no global or thread-local state") |
| Transactional memory for every variable; reference tracing; one OS thread per process as a guarantee | YAPL → July 2026 | `shared<t>` only; ownership; runtime's choice | Costs paid by code that shares nothing; semantics should not depend on the scheduler |
| Affinities and placements in types | Toolchain | Physical parameters behind Core IR | Runtime detail in source, which agents misuse |
| Comments with runtime meaning (interpolated, traced) | YAPL → toolchain | `log` statements | A comment that runs surprises the reviewer |
| Two kinds of absence (`nada`/`nothing` and `undefined`) | Toolchain → April 2026 | `t?` and definite initialisation | Definite initialisation makes "not yet provided" unrepresentable |
| `afterCommit` | September 2026 | `enqueue`; buffered telemetry | Redundant if durable, a bug if not |
| Module-level `shared` declarations | September 2026 | `shared<t>` values passed down | Global state by another name |
| Tests reading a function's locals | YAPL (2024) | Named outcomes and fault injection | Couples tests to implementation |

**Syntax for policy, and scope beyond a language**

| Dropped | Held in | Replaced by | Why |
| --- | --- | --- | --- |
| Encrypted-log syntax; `throw ... capturing snapshot ... trace retry sandbox` | Toolchain (Nov 2025) | `sensitive<t>`, fault reports, replay | Policy and debugging configuration do not belong in grammar |
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
   - YAPL's translatable literals (`t"Hello {name}"`, with plural completeness checked per locale) were a good idea.
   - Leaning: a `user_text` type in the standard library, with a compile-time catalogue check, not new syntax.
6. **Thresholds.** The named-argument rule and the 60/120-line body limits should be calibrated on a corpus of
   agent-written Y, not fixed by taste.
7. **Arenas in v1.** An arena is visible in source here (§ 3.6). It could instead be a physical decision the optimiser
   makes.
   - Leaning: keep the syntax out of v1 until a program needs it.
8. **Transactions spanning `shared` and `store`.** One runtime can commit both atomically, but a remote database cannot
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
| M2 | Value semantics and `mutable`; the choice of borrows, moves and copies; Rust backend for the pure subset | `rustc` projection agrees on the corpus |
| M3 | Capabilities, effects, `y fix` pinning; tests, `covers`, the coverage gate | The § 2 tour runs end to end without `shared` |
| M4 | `shared`, `atomic`, `retry`, `orelse`; `within`, `task`, supervision; controllable scheduler, fault injection | The § 2 tour runs end to end |
| M5 | Commands, handlers, workflows; schemas, migrations, `store` | Crash-injection suite |
| M6 | Semantic diff, review packet, claims, approvals ledger | A week of agent-written changes reviewed only through packets |
| Later | Contract and typestate provers, more backends, runtime generations (§ 5) | Oracles (§ 4.6) |

M6 is where the premise is tested. If a reviewer working only from review packets misses defects that reading diffs
would have caught, the language has not yet done its job, and the packet or the language changes, not the reviewer.

## Appendix: reserved words

The complete list, so that the size of the surface is visible:

```text
declarations   module from import public facet function returns fails structure type case enumeration state transition trait
               implementation constant constants is unit schema migrate to command handler workflow test derived rule
               transient self
contracts      where ensures invariant old assert
control        if else switch when for in while break continue return fail
bindings       mutable lambda nested captures dependencies inject arena compile
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
