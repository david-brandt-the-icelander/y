# What the constraint language cannot yet say: a plan

**Status:** Plan
**Date:** 2026-10-05
**Written for:** the team that owns the constraint language, who decide every change to it, and whoever builds the
changes into the bare Rust tool.
**Companion documents:** [`constraint-language.md`](constraint-language.md), cited as "language § n";
[`bare-rust.md`](bare-rust.md), cited as "trial § n". The evidence is in `tools/bare-rust/results/e1.md` and
`tools/bare-rust/results/libraries.md`, and in the `unexpressed` clauses of
`tools/bare-rust/libraries/std-1.97.0.constraints.json`.

---

## 0. The plan on one page

The bare Rust trial has found, with counts, what the language cannot say. Stage 2 wrote 303 promises for the 106 `std`
items the corpora use. The language could state 104 of them, and 199 are recorded as `unexpressed`, each with a
labelled reason. Stage 1 found Rust types the type language cannot hold. The trial's own list of gaps (trial § 12) adds
a few more.

- **Sort before extending.** Each gap falls into one of three classes:
  - **A, no language change (38 promises).** The language can already say it, through a view, a failure's payload, a
    refinement or a named function, or the gap is in the library file's format, the host profile or the tool. **Done**
    (§ 3): 24 closed, 8 turned out to be what a function computes and stay out, and 6 need a host function that does
    not exist. `unexpressed` fell from 199 to 175, and slices and arrays no longer reach the bare file as host text.
  - **B, an extension (158).** It needs a change to the language, in five groups.
  - **C, kept out (3).** The language rules it out on purpose.
- **Extend in five steps, largest first**, unless stage 3 says otherwise:
  1. Traits, implementations and associated types (52).
  2. Promises that hold unless an implementation does otherwise (30).
  3. Callbacks: their results, conditions and order (15).
  4. Eight small additions (47).
  5. Sequences (14).

  Each step is one kind or a few related kinds, as language principle 8 requires: a paragraph of meaning, a consumer, a
  check and a harvest.
- **Let stage 3 set the priority.** Experiment E5 gains a variant: library files without their `unexpressed` clauses
  (trial § 10). If agents do as well without that prose, the promises do not help an agent write code, and the
  extensions are worth building only where something checks or uses them. Then the order is set by which promises a
  checker or the compiler can use, not by count. If agents do worse, the order above stands.
- **Each step is measured.** It is done when the library file is rewritten under the extension, `unexpressed` falls by
  the expected count, and `libcheck` and the review pass.
- **It is cheap to build; the cost is people's review.** Class A cost about $3. The agent work for the five steps left
  is estimated at $40 to $80.
  Every step is a language change that the language's owners must accept (§ 7).

---

## 1. What is unexpressed

### 1.1 The library promises (stage 2)

Every `unexpressed` clause in the `std` file names what the language lacked. Grouped by class, with each label's count:

| Class | Group | Labels (count) | Total |
| --- | --- | --- | --- |
| A | No language change | host state 10; formatted text 9; quantifier over fields (derives) 7; macro arguments 4; unwinding behaviour 4; `budget` and `layout` missing from the file format 3; sign of zero and NaN payload 1 | 38 |
| B1 | Traits, implementations and associated types | no impl or associated type 38; call resolves to an unnamed impl 11; variants of a host enum 3 | 52 |
| B2 | Promises that hold unless an implementation does otherwise | conditional on a bound or on an overridable default body 22; requirement on a bound's implementations 6; build profile (overflow checks) 2 | 30 |
| B3 | Callbacks | value of a callback's result 7; order of calls 6; callback clause without a condition 2 | 15 |
| B4 | Small additions | const-ness 18; must-use on a type 14; identity of moved values 6; drop 4; resource exhaustion 2; expected or amortised cost 1; tuple parts 1; frame condition on a collection 1 | 47 |
| B5 | Sequences | no sequence value 14 | 14 |
| C | Kept out | positive effect 3 | 3 |
| | | | **199** |

### 1.2 The types and signatures (stage 1, and trial § 12)

| Gap | Count in E1 | Class |
| --- | --- | --- |
| Slices | 32 type slots | A: `list<T> as 'std::primitive::slice'`, as the stage 2 library file writes them; `strip` now does too |
| Types with lifetime arguments (`Chars<'a>`, `Entry<'a, K, V>`) | 25 | B1 |
| Arrays `[T; N]` | 10 | A: `list<T> where size(value) == N`, with the host type named by `as` |
| `impl Trait`, `dyn Trait`, associated types | 5, 2 and 1 | B1 |
| Generic host subjects: `for T where …` has no place on a host type, function or `impl` | 33 type and 27 lifetime parameters, 34 bounds, 9 `where` clauses | B1 |
| `usize` and `isize`, whose width is the target's | 108 | A: `u64 as 'usize'`, with `fits usize` from the host profile |
| `const fn` | 1 in the corpora, 18 in `std` | B4 |
| `unsafe fn` | 0 in the corpora | B4 |
| Body hints, which language principle 5 forbids | 93 hints | Not here: decided by E2 (trial § 13, question 2) |

---

## 2. How each gap is decided

These are the language's own rules, applied to gaps:

- **A kind is added only when something consumes it** (language principle 8). A gap's count is evidence of demand,
  not a consumer. The consumers here are the agent writing bare code (E4 and E5 measure it), the checks a woven host
  can run (the compiler, `libcheck`, generated checks), and the discharge of generated checks by library promises
  (language § 11.1).
- **Reuse before adding.** Views (`view(e)`, language § 9), failure payloads (§ 6.2), refinements (§ 3.4), interfaces
  (§ 9), protocols (§ 6.5), lifetime predicates (§ 7.3) and named functions (§ 4) already cover more than the
  library's writer used. Class A is the gaps these close.
- **The boundary, never the body** (principle 5). A promise about what happens inside a call is in scope only if it is
  observable at the boundary: arguments, results, failures, callbacks, and what happens to the entities that cross it.
- **Bounds limit; they never describe** (language § 6.3). What a call does is the effect manifest's job.
- **What the language says it cannot say stays out** (language § 16): what a function computes in general, how
  anything is done, style.
- **Each change is one paragraph of meaning,** with its syntax, how it fails, how it is checked and how it is
  harvested, in the section of the language it extends.

---

## 3. Class A: no language change (38)

| Gap | Count | How it is said | Where the change is |
| --- | --- | --- | --- |
| Host state: capacity, hasher, formatter state | 10 | A view: `view capacity(m) = 'std::collections::HashMap::capacity'`, declared beside the type and usable in any clause (`ensure : capacity(result) >= capacity`) | Library file format: views bound to host functions that are pure |
| Formatted text | 9 | A panic or error message is the failure's payload: `fails panic { message: msg }`. The text a `Debug` or `Display` produces is what a function computes, and stays out (language § 16) | Library file format: payloads on declared failures; about 4 of the 9 move |
| Quantifier over fields, for derives | 7 | A derive's promise is a template, expanded per type that uses it: `Clone!` on `Cache` becomes `ensure : result.entries == self.entries` for each field. This is language § 17, question 15: the generator's input is the subject | `bare-rust`: derive templates, expanded by `libcheck` and `weave` |
| Macro arguments | 4 | A macro's forms belong in the host profile, one signature per arm | Host profile |
| Unwinding or abort, `#[track_caller]` | 4 | How a panic is carried out is the build's panic strategy, a property of the host profile, not of a subject | Host profile |
| `budget`, `layout` | 3 | Both are in the language (§ 6.7, § 6.1) | Library file format: add the two clause kinds |
| Sign of zero, NaN payload | 1 | Named functions: `sign_bit(x)`, `total_order(a, b)` (already in § 4) | Catalog of named functions |

Slices, arrays and `usize` (§ 1.2) belong here too: each is a list or an integer, refined where needed, with the host
type named by `as`. Of the 9 formatted-text promises, about 5 were expected to be what a function computes;
the build found 8 (below).

All of class A is one release of the library file format and the tool, and needs no language review.

**Done.** Two short agents built it and rewrote the entries, for about $3 (6M tokens, 0.6% of a Max 5x week):

- **Views** bound to pure host functions: a `views` list (`{name, of, host}`) at the top of the file or on a subject.
  `libcheck` compiles each host path; 3 resolve (`HashMap` and `Vec` capacity, `Hasher::finish` as `hash`).
- **Failure payloads**: `failures.panic` declares `{message: text, location: location}`, and a clause writes
  `fails panic { message: msg, location: caller } when …`. `#[track_caller]` is `location: caller`.
- **`budget` and `layout`** are clause kinds of the file.
- **Derive templates** for Clone, Copy, PartialEq, Eq, Hash and Default, in `src/derive.rs`; `libfmt --expand` shows
  the per-field clauses. `weave` does not consume them yet.
- **A host profile**, `libraries/host-rust-1.97.0.json`: the panic strategy, the caller location, and one signature
  per arm for `assert_eq!`, `panic!`, `vec!` and `write!`.
- **Named functions** `sign_bit` and `is_nan` join `total_order`.
- **Slices and arrays** strip to `list<T> as 'std::primitive::slice'` and `list<T> where size(value) == N as
  'std::primitive::array'`; all 42 such slots of E1 are now lists. `usize` was already `u64 as 'usize'`.

Of the 38, 24 are closed. 8 are what a function computes (Debug and Display text, and the Debug rendering inside
`Result::expect`), relabelled and moved to class C. 6 stay as `no host state`: `Entry::or_insert`, `Formatter`,
`TryFromIntError`, `Display::fmt`, `Hasher::write` and `Hash` need state that no pure host function reads; they wait
for B1, where an implementation can declare its own views.

---

## 4. Class B: extensions, in proposed order

### 4.1 Traits, implementations and associated types (52, plus most of E1's host-text types)

**What is missing.** The language has `interface` subjects, whose clauses bind every implementation (language § 9), and
`for T where T: Bound` generics. It has no way to say that a type implements an interface, no subject for one
implementation, no associated types, no "some type that implements I" (`impl Trait`) or "any type that implements I"
(`dyn Trait`), and no generics on a host subject (trial § 12). A type that holds a borrow, such as `Chars<'a>`, has no
type at all.

**Proposed meaning.** An implementation is a subject: the clauses an interface states for every implementation, plus
what this implementation promises beyond them, which behavioural subtyping already permits (§ 6.10). A type declares
the interfaces it implements, with their associated types. An existential type `some I` is a value of one type, fixed
by the subject, that implements `I`; `any I` is a reference to a value of any type that implements `I`.

```text
type rust 'std::str::Chars'
    for 'a
    hold : borrows for 'a                         -- the type holds a borrow; a use says from what
    implements 'std::iter::Iterator' { Item = char }, 'std::clone::Clone'

impl rust 'std::iter::Iterator' for 'std::str::Chars'
    function next(self: ref<Chars>) -> char?
        access self : write exclusive

function rust 'eve_cache::Cache::keys'(self: ref<Cache>) -> some 'std::iter::Iterator' { Item = ref<Key> }
    hold result : borrows from self
```

- **What it closes.** "No impl or associated type" (38), "call resolves to an unnamed impl" (11): an entry can now name
  `impl Index<Range<usize>> for String`. Host enum variants (3), once `Entry<'a, K, V>` has a type. E1's lifetime
  arguments (26), `impl Trait` (6), `dyn Trait` (3) and associated types (1), and the generic host subjects of trial
  § 12.
- **The tool.** `bare-rust externals` records the impl each call resolves to; it is in the MIR it already reads. `strip`
  maps `impl Trait`, `dyn Trait`, lifetime arguments and generics on impls into the new forms. The JSON gains an `impl`
  subject kind, an `implements` slot and existential types.
- **Checked by** `rustc` after weaving, as the rest of a signature is; by `libcheck` for library files, through the
  same signature coercion, extended to impls.
- **Consumer.** The weaver, which carries 33 of stage 1's 78 host-text types only as Rust text. Agents, who are
  told what an iterator yields. Every host with interfaces: C#, Java, Kotlin, Swift and TypeScript all need it.

**Done: the paragraph, the format and the tool; the library file is not rewritten yet.** Language § 6.11 and § 15 state
the meaning and the grammar. The JSON gains an `implements` slot on a type (`{trait, assoc, impl}`), `assoc` and
`trait`/`self` on an `impl` subject, `impl` on a method of one, `hold.for` on a type, and, in type strings, `some I {..}`,
`any I`, lifetime arguments (`Chars<'a>`) and `Self::Item`. `strip` maps `impl Trait`, `dyn Trait`, lifetime arguments,
associated types and the self types of impls into them, and `weave` prints them back; `libcheck` lints the new clause forms
and compiles impls, `implements` entries, impl methods and existential types against the real `std`; `externals` records, for
each function, the impls its calls resolve to (read from the `<T as Trait>::f` paths of the MIR it already reads; a call
through a type parameter says so, by a heuristic on the name). On the corpora, host-text types fall from 39 to 3 (strsim 5 to
0; the tool's own source 34 to 3: a pointer and two references with a lifetime in a hole). What is left as host text is in
§ 6.11. Step 4, the rewrite of the 52 promises, and step 5's measure wait for it.

### 4.2 Promises that hold unless an implementation does otherwise (30)

**What is missing.** `never panics`, `terminates` and the bounds on effects are unconditional. Most of what `std`
promises holds unless a caller-supplied implementation misbehaves: `HashMap::get` never panics, unless `K`'s `Hash` or
`Eq` panics. A default method body may be overridden. Some promises hold only under a build setting, such as overflow
checks.

**Proposed meaning.** A property may be qualified by `given`, naming what it relies on: properties of a bound's
implementations, the default body, or a host build setting. The promise holds whenever what it names holds, and says
nothing otherwise. A requirement on a bound's implementations is a law stated over the generic parameter, which a
caller's choice of type must satisfy.

```text
function rust 'std::collections::HashMap::get'(self: ref<HashMap<K, V>>, k: ref<Q>) -> ref<V>?
    for K, V, Q where K: 'Borrow<Q>', Q: 'Hash' + 'Eq'
    never panics   given Q.hash, Q.eq never panic
    effects none   given Q.hash, Q.eq effects none
    law borrow_agrees : all k : K : hash(k) == hash(borrow(k))     -- the caller's types must satisfy it

function rust 'std::iter::Iterator::count'(self: Self) -> u64 as 'usize'
    ensure counted : old(count(self)) <= 18446744073709551615 implies result == old(count(self))
    fails panic when old(count(self)) > 18446744073709551615   given build overflow_checks
```

- **Closes** the 22 conditional promises, the 6 requirements on bound implementations, and the 2 that depend on the
  build profile.
- **Checked:** `given` adds the named promises to what a checker may assume, and makes the caller responsible for them.
  A law on a generic parameter is checked in CI for each type the code instantiates it with, as laws are now (§ 6.2).
- **Consumer.** Any discharge of a generated check by a library promise (trial § 7.3): almost no `std` promise is
  unconditional, so without `given` nothing can be discharged.

**Done: the paragraph, the format and the lint; the library file is not rewritten yet.** Language § 6.12 and § 15 state
the meaning and the grammar. A clause of the library file gains an optional `given`, a list of conditions joined by `and`
(`"Q.hash, Q.eq never panic"`, `"default"`, `"build overflow_checks"`); `text` stays the unconditional clause, and the
language form is `text given …`. It is allowed on `never panics`, `effects`, `ensure`, `fails` and `panics` clauses, not on
`unexpressed` or `law` (a law over a generic parameter is a requirement on the caller's type, and stays a `law`). Files
without it stay valid. `libcheck` lints each condition: the parameter is a generic of the subject, the method exists on one
of its bounds when the file knows the bound's methods (it knows a bound only through trait subjects and `Trait::m`
functions, so a bound it has none of passes unchecked), `default` needs a trait method, and a build setting is one the host
profile `libraries/host-rust-1.97.0.json` lists in its new `build` (`overflow_checks`, `debug_assertions`, `panic=unwind`,
`panic=abort`; the fixed list is in `src/host.rs`). `libstats` reports conditional clauses as a count of their own, by
what they are given, and `libfmt` prints `given` after `text`. No compile check of a condition against `std` yet. The
rewrite of the 30 promises, and step 5's measure, wait for it.

### 4.3 Callbacks: their results, conditions and order (15)

**What is missing.** `callback` says how often a function argument is called, but not when, not in what order relative
to other calls, and not what its result means for the caller's result. A clause cannot call a function argument,
because it need not be pure.

**Proposed meaning.** A callback clause may name the arguments and the result of its call. The result is a value that
crossed the boundary, as principle 5 allows. It may carry a condition (`when`), and a subject may state a `protocol`
(§ 6.5) over its callbacks' calls.

```text
function rust 'std::option::Option::map_or_else'(self: T?, default: ref<FnOnce>, f: ref<FnOnce>) -> U
    callback default : called exactly once when self is none,  returning d
    callback f       : called exactly once when exists self,   with self, returning r
    ensure : result == (if exists self then r else d)

function rust 'std::iter::Iterator::count'(self: Self) -> u64 as 'usize'
    protocol (next)* next                -- next is called until it returns none, at least once
```

- **Closes** the callback-result promises (7), the conditional callbacks (2), and the order of calls (6).
- **Checked** by generated wrappers around the argument, where the host allows one, and in CI otherwise.

**Done: the paragraph, the format, the lint and the rewrite; 7 of the 15 are closed.** Language § 6.13 and § 15 state the
meaning and the grammar: `callback c : called exactly once when p, with a, b, returning r`, where `when` is a condition
on the subject's own names, `with` lists the call's arguments (an expression, or a new name for that argument) and
`returning` binds the result; `with` and `returning` need `exactly once` or `at most once`; `protocol` is § 6.5's
regular expression over the subject's callbacks, its parameters and the methods of a parameter's bounds (`self.next`).
The library file keeps the clause in `text` (no new slot), and gains the clause kind `protocol`. `libcheck` lints a
callback's `when` and `with` against the subject's names, `returning` and the new `with` names are in scope in the
subject's other clauses and in no callback's `when`, a protocol names callbacks, parameters and methods of known
bounds, and `libstats` counts callback clauses by what they name. Old files stay valid. Rewrite: `unexpressed` 121 to
114. Closed: `Option::map_or_else` (2: a conditional callback each, and `ensure` on the result), `Entry::and_modify` (2:
the condition, and the identity of the value `f` receives and the entry holds), `Ord` (1: `max` and `min` as laws over
`cmp`, which unfolds the `max_by` and `min_by` that the contract names), `Iterator::count` (1: `protocol self.next+`)
and `assert_eq!` (1: `protocol left right`). Not closed, and relabelled (8): the callback of `map`, `filter` and
`take_while` is called by `next` on the returned adapter, a call of another subject, so none of what the proposal
expected for them can be said of the function itself (*no promise about a result's later calls* 3, with `enumerate`'s
4), nor what the adapter yields (*no sequence value*, +4 with `flat_map`, which needs B5); `Hasher::write` promises
nothing about its order (*no promise*). Still open: `count`'s protocol says `next` is called and at least once, not
that the run ends at the first none, which depends on a value the calls returned; a callback on a result's `next`
would need a field of the result to be a callback of its own subject. No compile check of the clauses against `std`.

### 4.4 Eight small additions (47)

Each is a clause or an expression, small enough to add on its own:

| Addition | Count | Sketch | Extends |
| --- | --- | --- | --- |
| `const` as a bound | 18 | `evaluable at compile time`: the body may run in a constant context. It is a bound on effects, as trial § 12 says of `const fn` | § 6.3 |
| Must-use on a type | 14 | `type … must be used`: every subject that returns it gets `result must be used` | § 6.2 |
| Moves | 6 | `hold result : moved from v2`. A value moved into or out of a call keeps its provenance where `==` cannot tell two equal values apart | § 7.2 |
| Ends and drops | 4 | `lifetime v1 ends in call`; `lifetime a, b survive call`, for `mem::swap` | § 7.3 |
| Resource exhaustion | 2 | `never panics except exhausted`: allocation failure is the one runtime failure left open | § 6.2 |
| Expected and amortised cost | 1 | `budget work expected O(1)`, `budget work amortised O(1)` | § 6.7 |
| Tuple parts | 1 | `result.0`, and patterns `(a, b)` in `let` | § 4 |
| Element frame condition | 1 | `mutates only self[key]`: every other element of the collection is unchanged | § 8.2 |
| Obligations on callers (`unsafe fn`) | 0 in the corpora | `obliges caller : p because "…"`: a precondition the callee does not check, which is `assume` seen from the caller | § 10.2 |

The first two close 32 promises with almost no new meaning: `const` restricts effects, which bounds already do, and
must-use extends an existing clause from a subject to a type.

**Done: the paragraphs, the format, the lint, strip and weave for two, and the rewrite; 37 of the 47 are closed.**
Language § 6.14 states the nine additions in the form of § 6 (one subsection each), extends § 4, § 6.2, § 6.3, § 6.7, § 7.2,
§ 7.3, § 8.2 and § 10.2 with a pointer, and § 15 with the grammar: `evaluable at compile time`; `must be used` on a type or
trait; `hold result : moved from v` and the expression `result moved from v`; `lifetime v ends in call [when p]` and
`lifetime a, b survive call`; `never panics except exhausted`; `budget work expected amortised O(1)`; `result.0`, and the
patterns `let (a, b)` and `e is (a, b)`; `mutates only self[key]`; and `obliges caller : p because "…"`. The library file
keeps each in the kind it extends (`effects`, `result must be used`, `hold`, `never panics`, `budget`, `ensure`,
`access`), with one new kind, `obliges`. `libcheck` lints the forms (`moved from` names one path, `ends in call` and
`survive call` have their shape, the `because` of an `obliges` is a quoted reason, `is (a, b)` binds its names, and
`must be used` is for a type or trait, `evaluable at compile time` and `obliges` for a function), and compiles each
`const` claim by calling the item from a `const fn`. `strip` turns `const fn` and a plain `#[must_use]` into clauses and
`weave` writes them back (`#[must_use]` before the visibility, `const` after it); a `#[must_use = "…"]` keeps its message
as host text. `libstats` counts the nine forms. Old files stay valid. Rewrite: `unexpressed` 114 to 77.

- **Closed (37).** `const` 14 (the items with `rustc_const_stable`); must-use 14 (11 iterator types, `Result`, the
  `Iterator` trait, `Chars`); moves 3 (`cmp::max`, `cmp::min`, `Ord::max`: on a tie the second or first argument comes
  back); ends 2 (`mem::swap`: `lifetime x, y survive call`; `mem::replace`: `lifetime dest, src survive call`, with the two
  `hold … moved from` its documentation states); exhaustion 2 (`HashMap::insert` and `entry`: `never panics except
  exhausted given K.hash, K.eq, S.hash_one never panic`); cost 1 (`HashMap::get`: `budget work expected O(1)`, from std's
  table of collection costs, which the entry had called absent; `insert` gains `expected amortised`); tuple parts 1
  (`split_at_mut`: the sizes of `result.0` and `result.1`, and `disjoint`).
- **Stay under corrected labels (10 of the 47, and 3 more).** *No stable const-ness* 4 (`cmp::max`, `cmp::min`,
  `Option::map_or_else`, `slice::iter` are `rustc_const_unstable`: no release is bound to keep them const). *No identity
  of cloned values* 2 (`Clone::clone` and `vec![elem; n]`: `moved from` says a value moved, not what a clone shares) and *no
  identity of a stored key* 1 (`HashMap::insert` keeps the old key: `self[k]` looks up a value, and no expression names the
  key the map holds). `Entry::or_insert`: the drop of `default` in the occupied arm is stated (`lifetime default ends in
  call when self is Occupied(e)`), and the rest is *no effects of a destructor* (V's `Drop` is arbitrary code, so no
  `never panics`), and the frame condition is *no host state*: the map the entry borrows is not a name, so `mutates only
  self[key]` has no collection to be about. `vec![expr; 0]` is *no arm of a macro in a clause* (`elem` and `n` are names of
  one arm of the host profile, not of the subject). `vec::IntoIter`'s drop of the elements not yielded is *no sequence
  value*. `insert` and `entry`'s *no sufficient condition* (2) is now *no condition on an associated type's
  implementation*, the hasher's own `write` and `finish`, as `get`'s.
- **Still open or unsure.** The tie rules of `max`, `min` and `Ord::max` are stated as moves; that the loser is dropped
  follows from ownership and is not stated (the `ends` of a generic value that a provided method may override has no `given`).
  `never panics except exhausted` for `insert` leaves out the destructor of the replaced key, as every `never panics`
  of the file leaves out the drop of a type parameter. No compile check of moves, drops, exhaustion or must-use, and `strip`
  reads only functions: `#[must_use]` on a type, `unsafe fn` as `obliges`, and `let (a, b)` have no library use yet (the
  corpora have no `unsafe fn`, and a library `let` has no clause kind).

### 4.5 Sequences (14)

**What is missing.** The language has `list<T>` and facts about collections (`size`, `in`, `count`, `all`), but no
sequence value: no order of elements, no indexing, no concatenation, nothing to say "an iterator yields the characters
of `s` in order".

**Proposed meaning.** A view `yields(it)` gives the sequence an iterator-like value produces, as a `list<T>`. `list`
gains order: indexing `xs[i]`, slices `xs[i..j]`, `++`, `reverse`, and `map` and `filter` by a named pure function.

```text
function rust 'std::primitive::str::chars'(self: ref<text>) -> some 'std::iter::Iterator' { Item = char }
    ensure in_order : yields(result) == scalars(self)
```

- **Closes** the 14 sequence promises, and is what most iterator adapters need for more than `count`.
- **The largest semantic addition.** Every place that compiles the language must implement the new operators,
  identically (principle 2).
- **It waits for evidence.** It is worth building if E4 or E5 shows agents getting iterator code wrong in ways these
  promises would have prevented.

**Done: the paragraph, the format, the lint and the rewrite; 12 of the 20 are closed.** Language § 6.15 defines each
operator by its elements: `xs[i]` (optional: `none` beyond the ends, so it never fails), `xs[i..j]` (never fails; it is
`take(j - i, skip(i, xs))`), `++`, `reverse`, `take`, `skip`, `zip`, `enumerate`, `map` and `filter` by a named pure
function, `scalars` and `deref`; the view `yields(it)` for a finite or infinite iterator (an infinite equality is of
every prefix, and `size`, `count` and `reverse` are decided where the sequence is finite). § 4, § 5, § 6.13's pointer, § 15's
grammar (`seq_expr`) and § 16 are extended. `libcheck` lints the operators' arguments and that `map` and `filter` are
given a view of the file or `scalars` or `deref`, never a parameter or callback; `libstats` counts them. The file's
kinds are unchanged (the clauses are `ensure`). Old files stay valid. Rewrite: `unexpressed` 77 to 65.

- **Closed (12).** `str::chars` (`yields(result) == scalars(self)`), `slice::iter` (`map(deref, yields(result)) ==
  self`), `split_at_mut` (`result.0 == old(self)[..mid]`, `result.1 == old(self)[mid..]`), `Iterator::next` (`head` and
  `advances`), `enumerate`, `skip`, `take`, `chain` (`++`), `zip`, `copied` (`map(deref, …)`), `IntoIterator::into_iter`
  and `Vec`'s `into_iter` (which takes the claim from `vec::IntoIter`'s "yields the elements of the vector it was made
  from").
- **Stay under corrected labels (8 of the 20, and 4 later calls).** *No callback as a function* 4 (`map`, `filter`,
  `take_while`, `flat_map`: the argument is a closure, and `map` takes a named function, which is what keeps expressions
  pure); *no promise of disorder* 1 (`HashMap`'s iteration order: nothing is promised); *no lifetime for the elements of a
  sequence* 1 (`IntoIter`'s drop); *no successor of a type parameter* 1 (`Range<Idx>` for a generic `Step`); *no from_iter
  subject* 1 (`collect`: Result's and String's `from_iter` have no subject in the file). The four *no promise about a
  result's later calls* (`enumerate`'s overflow, `map`'s and `filter`'s laziness, `take_while`'s consumed item) are the
  calls of a returned adapter, which sequences do not name.
- **Still open or unsure.** `IntoIterator::into_iter`'s `yields(result) == yields(self)` is the definition of the order a
  collection has, so it states little beyond what it names; `next`'s `head` and `advances` hold of the state and say nothing
  of an iterator that resumes after a `None`, and `Option<Option<T>>` items make `xs[0]` ambiguous (language § 6.15 names
  it). No compile check of sequence clauses, and no conformance suite exists yet: the equations are the definition.

---

## 5. Class C: kept out (3)

- **Positive effects** (3): "each `HashMap` draws its own random seed". Bounds limit; they never describe (§ 6.3). The
  effect manifest records what a call does. The one useful consequence, that iteration order is not fixed, is stated
  as a bound on callers' assumptions, not as an effect.
- **What a `Display` or `Debug` implementation produces** (about 5 of § 3's 9 formatted-text promises): this is what
  a function computes (language § 16).

---

## 6. Order, and how each step lands

| Step | Closes | Unexpressed after | First consumer | Wait for |
| --- | --- | --- | --- | --- |
| A, format and profile (done) | 24 | 175 | Agents, through better library files | — |
| B1, traits and implementations | 52 | about 123 | `weave`, which carries `impl Trait`, `dyn Trait` and lifetime arguments only as Rust text | Nothing |
| B4, the first two (`const`, must-use) | 32 | about 91 | `weave` (`const fn`); the compiler and lints | Nothing |
| B2, `given` | 30 | about 61 | Discharge of generated checks | Trial stage 4 or later |
| B4, the rest | 15 | about 46 | The memory clauses of language § 7 and § 8 | — |
| B3, callbacks | 15 | about 31 | Agents writing iterator and option code | E5 |
| B5, sequences | 14 | about 17 | Agents writing iterator code | E4 and E5 |

The 17 left are 11 of class C (3 positive effects, 8 computed texts) and the 6 host-state promises, unless B1's views
cover them.

The counts are estimates: an extension may close fewer promises than its labels suggest, and a rewrite may find new
ones. Each step lands in the same way:

1. **The paragraph.** A section of the language, in the form of language § 6: meaning, syntax, how it fails, how it is
   checked, how it is harvested. The grammar of language § 15 is extended.
2. **Review by the language's owners.** No step is merged without it (language principle 9).
3. **The format.** The JSON schema, `libcheck`'s clause lint, and, where needed, `strip`, `weave` and `externals`.
4. **The rewrite.** Agents rewrite the affected entries of the library file, in batches of about 20, each in a fresh
   agent. A second agent reviews them.
5. **The measure.** `bare-rust libstats` shows `unexpressed` falling by the expected count, and checks 1 to 5 pass.

The B1 step comes first among the extensions because two consumers already need it: `weave` refuses or carries as host
text every `impl Trait`, `dyn Trait` and type with a lifetime argument, and every host with interfaces will hit the
same gap. The rest of the order is set by stage 3:

- **If E5's variant without `unexpressed` prose shows no loss,** the prose was not helping agents. Then build only
  what a checker or the compiler consumes: B1, `const` and must-use, then `given` when discharge is built. Leave B3 and
  B5 until a consumer appears.
- **If it shows a loss,** agents need these promises, and the order above stands, with B3 and B5 promoted to follow
  B1 if the tasks that lost were iterator or callback code.

---

## 7. Cost

The language changes are design work: an agent can draft each paragraph, but the language's owners must accept it.
That review is the real cost, and it is not counted here. The agent work uses the units of trial § 14.2:

| Step | Build and check runs | Entries rewritten | Estimated cost |
| --- | --- | --- | --- |
| A (measured) | 2 agents | 38 | about $3 |
| B1 | 3 and 2 | about 50 | $15 to $30 |
| B2 | 1 and 1 | about 30 | $5 to $10 |
| B3 | 1 and 1 | about 15 | $4 to $8 |
| B4 | 2 and 1 | about 45 | $8 to $15 |
| B5 | 2 and 1 | about 15 | $8 to $15 |
| **B1 to B5, left** | | **about 155** | **$40 to $80** |

Each range assumes short agents, as stage 1 ran; stage 2's single long agent cost twice the unit (trial § 14.1). At the
Max 5x proxy of trial § 14.3, what is left is about 8 to 15% of a week.

---

## 8. Open questions

1. **`impl` as a subject, or implementations as clauses of a type?** A subject per implementation matches Rust and
   gives each impl its own clauses and status; clauses on the type are smaller and match C# and Java, where a class
   lists what it implements. Leaning: a subject, with `implements` on the type as its summary.
2. **`given` on predicates too?** It is proposed for properties only. A predicate on the request path must be total
   and decided (principle 3), so it cannot depend on what an implementation might do. Leaning: properties only.
3. **How much of sequences?** Indexing and `yields` may be enough for agents; `map` and `filter` by named function
   make every place that compiles the language larger. Leaning: the smallest set that closes the 14, and nothing more
   until a consumer asks.
4. **Who keeps derive templates and macro signatures?** They are per host, not per library. Leaning: in the host
   profile, beside the type mapping, owned by the team that owns the profile (language § 3.3).
