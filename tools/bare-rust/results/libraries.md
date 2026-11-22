# Stage 2, part B: the library file for `std`, by hand

`libraries/std-1.97.0.constraints.json` has a subject for each of the 105 external items of strsim, fnv and the golden
crate (49 functions, 25 types, 19 traits, 11 macros, 1 const), and one more, `std::cmp::Ordering`, that the laws need
(263 subjects: 106 entries for the corpora's items, `OccupiedEntry` for a view, 150 impls and impl methods of B1 and B6 (B6 adds the `FromIterator` impl of `String` and the `from_iter` methods of it and of `Result`), and 5 trait methods and one trait that B2's `given` items name: `Hash::hash`, `PartialEq::eq`, `PartialOrd` with `lt`, `Ord::cmp`, `Ord::min`). `bare-rust libcheck` passes for all three: no item missing, schema ok, every clause's text well formed,
0 signatures mismatched. Strict library coverage is the default of `bare-rust check` (opt out with
`--allow-missing-libraries`) and is asserted in `tests/corpora.rs`. The counts below are the output of
`bare-rust libstats libraries/std-1.97.0.constraints.json`, which reads the file and compiles its signatures.

## What the file holds

| Entries | Count | | Clauses | Stated | `unexpressed` |
| --- | ---: | --- | --- | ---: | ---: |
| function (27 are trait methods) | 74 | | `ensure` | 99 | |
| type | 27 | | `implements` | 141 | |
| trait | 20 | | `effects` | 45 | |
| macro | 11 | | `result must be used` | 32 | |
| const | 1 | | `never panics` | 20 | |
| impl | 130 | | `law` | 16 | |
| | | | `fails panic`, `fails` | 11, 1 | |
| | | | `hold`, `derive`, `callback`, `protocol`, `let` | 11, 6, 7, 2, 1 | |
| | | | `budget`, `layout`, `invariant`, `on failure`, `access` | 4, 1, 1, 1, 2 | |
| | | | `unexpressed` | | 52 |

453 clauses in all: 401 stated and 52 `unexpressed`, on 243 subjects. B1 (step 4 of `proposal/language-gaps.md` § 6)
took `unexpressed` from 175 to 138 and added 129 impl subjects, each compiled against the real std; B2 took it from 138 to 121; B3 (callbacks) from 121 to 114; B4 (eight small additions) from 114 to 77; B5 (sequences) from 77 to 65; B6 (the residuals of B3 to B5, below) from 65 to 52.
27 of the stated clauses are conditional, qualified by `given` (on 18 subjects): 17 `given` items on a bound's implementations, 16 on the
default body, 2 on a build setting. Stated clauses by
status: documentation 115, observed 217, lifted 54 (the attributes `#[must_use]` and `rustc_const_stable`), hand-written 15.

**Signatures**, compiled against rustc 1.97.0: 253 checked, 6 path-resolved only (`PartialEq`, `PartialOrd`, `From`, `TryFrom`,
`Index`, `IndexMut`: a trait with generics), 4 unchecked (the bang macros `assert_eq!`, `panic!`, `vec!`, `write!`),
0 mismatched. Of 192 type slots, 185 map to the type language (96%) and 7 are host text. Slices are no longer host text (below).

## What the language lacked: the `unexpressed` promises (52)

Each reason in the file starts with its label. Class A took the file from 199 to 175; B1 (traits, implementations and
associated types) from 175 to 138. Of B1's 52, 38 are closed outright and 14 stay under corrected labels; 6 of the
host-state ones stay (the vacant half of `Entry::or_insert` is all that moved).

- **Left from B1, relabelled (16).** No trait items 3 (what a trait's associated types and provided methods are); no
  negative impl 2 (`Eq` and `Copy`); no elided lifetime in a trait argument 3 (`From<&str>`, `Index<&Q>`,
  `Extend<&T>`); no deref coercion 1; no mutable reference as an impl's self type 1; no unsized text as an associated
  type 1 (`Deref<Target = str>`); no unstable or negative bound 1 (`Step` for `Range`); no view of two arguments 1
  (`is_char_boundary`); no sequence value +1 (`collect`'s Result and String impls); no drop +1 (`vec::IntoIter`). B5 relabelled the last two (below), and B6 closed them.
- **Host state (5)**: `Formatter`, `TryFromIntError`, `Display::fmt`, `Hasher::write`, `Hash`. `Entry::or_insert`'s two
  (the map after a vacant insert, and the other keys) moved in B6, below.
- **Others.** What a function computes 8; no positive effect 3; and the smaller labels. B3 changed no sequence value (15
  to 19), no promise (5 to 6) and no promise about a result's later calls (1 to 4), and removed *no value of a callback
  result* (7), *no order of calls* (6) and *callback clause has no condition* (2). B4 removed *no const-ness* (18),
  *must-use on a type* (14), *no identity of moved values* (6), *no drop* (5), *no resource exhaustion* (2), *no
  expected or amortised cost* (1), *no tuple parts* (1), *no frame condition on a collection* (1) and *no sufficient
  condition* (2). B5 removed *no sequence value* (20); see below.

## Class B6: the residuals of B3 to B5

B6 took 16 `unexpressed` clauses that B3 to B5 left on the language side: 13 are stated, and 3 stay, 2 of them under a
label that now names the true blocker. The new forms are language § 6.13.1 (`callback c : called by P.m, at most once per
call, with x, returning y`, whose `x` and `y` are the lists of the arguments and results of the calls in the run;
`fails F in P.m when p`; `final(e)`), § 6.15 (`present`, `select`, `while_true`, `flatten`, and `yields` as a named
function), § 4 (the handle: `current(h)`, `owner(h)`; `stored_key(m, k)`), § 7.3 (`lifetime every item of e ends with p`)
and the clause kind `let` (`let (a, b) = e`). The file's other kinds are unchanged. `libcheck` lints each form (the
later call is `result` or a parameter with a method its bounds have; `with` names only; no `when`; the count `per call`;
`final` has a run named before it; `owner`, `current`, `stored_key`, `present`, `select`, `while_true` and `flatten` have
their arities; `every item of` names a sequence and a predicate; `let` binds a name or a pattern once, for the clauses
after it, and refuses a name already bound), and `libstats` prints the new forms (14 clauses: 4 `called by`, 1 `fails …
in`, 1 `final`, 5 handle expressions, 1 `every item of`, 1 `let`, 1 `present`). None of the new clauses is compiled
against `std`: the three new subjects (the `FromIterator` impl of `String`, and the `from_iter` methods of it and of
`Result`) are, and the views `is_err`, `ok_value` and `key` resolve.

- **Stated (13).** *Closures of adapters (8).* `map`, `filter`, `take_while`, `flat_map`: a `callback … called by
  result.next` and the equations over its logs. `map`: `yields(result) == y`, `x == old(yields(self))`
  (iterator.rs:780-785; the count, at most once per call, is the body of `Map::next`, map.rs:106-108). `filter`: `map(deref,
  x) == old(yields(self))` and `yields(result) == select(keep, old(yields(self)))` (iterator.rs:890-895; `Filter::next` is
  `find`, so the count per call is unconstrained). `take_while`: `yields(result) == while_true(keep, old(yields(self)))`,
  `map(deref, x) == take(size(keep), old(yields(self)))` and `final(yields(self)) == skip(size(keep), old(yields(self)))`
  (iterator.rs:1190-1195 and 1254-1255; take_while.rs:44-56). `flat_map`: `yields(result) == flatten(map(yields, y))` and
  `x == old(yields(self))` (iterator.rs:1509-1514). `enumerate`: `fails panic in result.next when old(count(self)) >
  18446744073709551615 given build overflow_checks` (iterator.rs:1018-1020, "enumerating more than `usize::MAX` elements
  … If overflow checks are enabled, a panic is guaranteed"); what is promised without overflow checks is "wrong result or
  panics", which is not stated. The *no promise about a result's later calls* of `map` and `filter` (laziness: called only as
  the result is advanced) is `called by result.next`. *`Entry::or_insert` (2).* `ensure stored : self is Vacant(e) implies
  owner(self)[old(key(self))] == default` (map.rs:2498-2499) and `mutates only owner(self)[old(key(self))]` (the body,
  map.rs:2517-2520, and hashbrown's `insert_no_grow`); `key` is a view of `Entry`. *`HashMap::insert` (1).* `ensure
  key_kept : exists old(self[k]) implies same(stored_key(self, k), old(stored_key(self, k)))` (map.rs:1310-1311) and
  `lifetime k ends in call when exists old(self[k])` (the body never moves `k` in the found arm). *`vec::IntoIter` (1).*
  `lifetime every item of yields(self) ends with self` (into_iter.rs:541-558). *`collect` (1).* `ensure delegates : result
  == '<B as FromIterator<Item>>::from_iter'(old(self))` from the default body, and the impls it pointed to are subjects:
  `String`'s `FromIterator<char>` (`scalars(result) == yields(iter)`, string.rs:2379-2385) and `Result`'s (`first_err` and
  `all_ok`, result.rs:2113-2116). `Iterator::next`'s `advances` uses `present` and is no longer vacuous for an iterator
  of options. `split_at_mut` binds `let (head, tail) = result` for its `part_sizes`.
- **Stay (3).** *No successor of a type parameter* 1 (`Range<Idx>`: `Step` is an unstable trait no bound can name, and the
  library has no integer-only range subject). *No promise* +1 (`Clone::clone`: the documentation gives no rule for what a
  clone shares with the original, so it was never a gap of the language). *No arm of a macro in a clause* +1 (`vec![elem; n]`
  "uses clone": the relation of `elem`, `n` and the elements needs the names of one arm of the host profile; the identity of
  the clones is the element type's own).
- **Unsure.** (1) `x == old(yields(self))` for `filter` and `flat_map` reads "uses a closure to determine if an element
  should be yielded" and "returns an iterator for each element" as "every element is passed once, in order"; the
  documentation says each element, and the order and the single pass are `next`'s. (2) `final(yields(self))` of
  `take_while` assumes `self` is read through the result that holds it, or through the caller's `&mut I`, which is the
  documentation's own example. (3) `owner(h)` has no host function: it is checked by the wrapper that made the entry,
  and only the declaration of the handle (`hold : borrows`) makes it well formed. (4) `Result::from_iter` leaves
  open that `from_iter` of the values is what `V` makes of an iterator of the right items; it states it as a call of
  `V`'s own, with `ok_value` (`Result::ok`) as a view. (5) A host `Option<Option<U>>` loses the difference between
  `Some(None)` and `None` in `T?`; `present` carries it where a clause needs it.
- **Tool limits.** No sequence, log or handle clause is compiled against `std` by `libcheck`; the lint reads names,
  arities and shapes. The state of the clauses of one subject, which `final` and `let` need, lives in the `Scope` of
  `clause.rs` and is shared by its clones, because `libcheck` lints a callback with a copy. `libcheck`'s `heads` still
  has no entry for `let` (the lint reads the whole text), and the schema's kind list gains `let`.

## Class B5: sequences

B5's 20 *no sequence value* clauses (14 of the plan, with the 6 that B3 and B4 added) are now 8: 12 are stated, and the 8 stay
under labels of their own. The new forms are language § 6.15's `yields(it)`, `xs[i]`, `xs[i..j]`, `++`, `reverse`, `take`,
`skip`, `zip`, `enumerate`, `map` and `filter` by a named function, and `scalars` and `deref`; the file's kind is `ensure`,
with no new one. `libcheck` lints each operator's arguments and that `map` and `filter` are given a name from the file's
views or the language's own, never a parameter or callback; `libstats` prints 13 clauses with sequences (12 with
`yields`, 1 index, 2 slices, 1 `++`, 3 with `scalars` or `deref`). The new clauses are not compiled
against `std`: only the lint reads them.

- **Stated (12).** `str::chars`: `yields(result) == scalars(self)` (core/src/str/mod.rs:1013-1016; the
  `DoubleEndedIterator` half is the type's `implements`, and `next_back` is the same sequence from the back by § 6.15).
  `slice::iter`: `map(deref, yields(result)) == self`. `split_at_mut`: `result.0 == old(self)[..mid]` and `result.1 ==
  old(self)[mid..]` (mod.rs:1962-1964; "views, not copies" is the result's `hold`). `Iterator::next`: `head`, `result ==
  old(yields(self))[0]`, and `advances`, `exists result implies yields(self) == old(yields(self))[1..]`
  (iterator.rs:50-53; after a `None` nothing is said). `enumerate`: `enumerate(old(yields(self)))`; `skip`: `skip(n, …)`;
  `take`: `take(n, …)`; `chain`: `old(yields(self)) ++ old(yields('<U as IntoIterator>::into_iter'(other)))`; `zip`:
  `zip(…, …)`; `copied`: `map(deref, old(yields(self)))`; `IntoIterator::into_iter`: `yields(result) == yields(self)`;
  and `Vec`'s `into_iter`: `yields(result) == self` (vec/mod.rs:3948-3950, "from start to end"), which takes the claim from
  `vec::IntoIter`'s "yields the elements of the vector it was made from". Each cites the line the entry already cited.
- **Stay, relabelled (8; B6 above closed 6: the four closures, the elements of `IntoIter`, and `collect`).** *No callback as a function* 4 (`map`, `filter`, `take_while`, `flat_map`: the argument is a
  closure, not a named function, and `callback` states one call); *no promise of disorder* 1 (`HashMap`'s arbitrary
  iteration order); *no lifetime for the elements of a sequence* 1 (`vec::IntoIter`'s drop of the elements it has not
  yielded: `lifetime` names one entity of a call); *no successor of a type parameter* 1 (`Range<Idx>`, `Idx: Step`);
  *no from_iter subject* 1 (`collect`: the `from_iter` of `Result` and `String` has no subject here). *No promise about a
  result's later calls* stays at 4.
- **Unsure.** `into_iter`'s clause says little: `yields` of a collection is defined as what its `IntoIterator` gives.
  `next`'s clauses are about the present state and cannot say what an iterator that resumes does. `copied` reads a
  reference and its value as `deref`; `slice::iter`'s "by reference" is carried by the `Iter` type's `Item = ref<T>`.
  `Range`'s integer case could be written (`range(a, b)`), and is not, because `Idx` is any `Step` type.
- **Tool limits.** `libcheck` lints names, arity and the named-function rule; it does not compile a sequence clause or
  check a `yields` against a running iterator, and has no conformance suite for the operators. A `map` or `filter` name
  must be a view of the file (checked by compiling its host path) or `scalars` or `deref`; a quoted host path is refused.

## Class B4: eight small additions

B4's 47 `unexpressed` clauses (*no const-ness* 18, *must-use on a type* 14, *no identity of moved values* 6, *no drop* 4,
*no resource exhaustion* 2, one each of *no expected or amortised cost*, *no tuple parts* and *no frame condition on a
collection*), with `vec::IntoIter`'s fifth *no drop*, are now 11: 37 are stated, and 11 stay under corrected labels
(`insert` and `entry`'s two *no sufficient condition* became *no condition on an associated type's implementation*).
The new forms are language § 6.14's `evaluable at compile time`, `must be used`, `moved from`, `ends in call` and
`survive call`, `never panics except exhausted`, `expected` and `amortised`, `result.0` and `is (a, b)`, `mutates only
self[key]` and `obliges caller`; the file's kinds are those they extend, plus `obliges`. `libcheck` lints each form and
compiles every `const` claim by calling the item from a `const fn`; a claim on `cmp::max` (unstable const-ness) or on
`str::chars` (not const) is refused, with E0658 and E0015. `libstats` prints the nine forms: 14 `evaluable at compile
time`, 14 `must be used`, 5 `moved from`, 3 `ends in call` or `survive call`, 2 `never panics except exhausted`, 2
expected cost, 2 tuple-part clauses, no element frame and no `obliges` in this file.

- **Stated (37).** *Const (14)*: `mem::swap`, `mem::replace`, `Option::as_ref`, `as_mut`, `expect`, `is_none`,
  `slice::len`, `split_at_mut`, `str::is_empty`, `String::len`, `char::is_whitespace`, `f64::abs`, `usize::saturating_sub`
  and `u64::wrapping_mul`: each `evaluable at compile time`, from the `rustc_const_stable` attribute on the item (status
  lifted; each cited line was read in the 1.97.0 `rust-src`). *Must-use (14)*: `must be used` on `Chars`, `Chain`,
  `Enumerate`, `Filter`, `Map`, `Skip`, `Take`, `TakeWhile`, `Zip`, `Copied`, `FlatMap`, `slice::Iter`, `Result` and the
  trait `Iterator`, from their `#[must_use]` (the message stays in `source`). *Moves (3)*: `ensure tie_second : (v1 <= v2 and
  v1 >= v2) implies result moved from v2` on `cmp::max`, `tie_first` on `cmp::min` (`result moved from v1`), and the same
  for `Ord::max` (`other`): cmp.rs:1539 and 1646, documentation. *Ends (2)*: `lifetime x, y survive call` on `mem::swap`
  (mod.rs:763, "without deinitializing either one") and `lifetime dest, src survive call` on `mem::replace` (mod.rs:855,
  "Neither value is dropped"), which also gains `hold dest : moved from src` and `hold result : moved from dest` (mod.rs:853,
  "Moves `src` into the referenced `dest`, returning the previous `dest` value"). *Exhaustion (2)*: `never panics except
  exhausted given K.hash, K.eq, S.hash_one never panic` on `HashMap::insert` and `entry`: hashbrown 0.17.1 panics only in
  `Fallibility::Infallible`'s capacity overflow and aborts through `handle_alloc_error` (`src/raw.rs:33-45`), and
  everything else it calls is the hasher and `Eq`. *Cost (1)*: `budget work expected O(1)` on `HashMap::get`, and `expected
  amortised O(1)` on `insert`, from the table in `std/src/collections/mod.rs:117` and its legend (lines 88-89). The old
  entry said std states no cost, and cited hashbrown; it was wrong, and its "worst case linear in the capacity" is not a
  std promise and is gone. *Tuple parts (1)*: `split_at_mut`: `size(result.0) == mid and size(result.1) == old(size(self))
  - mid`, and `disjoint result.0, result.1` (mod.rs:1962-1964).
- **Stay, relabelled (11).** *No stable const-ness* 4 (`cmp::max`, `cmp::min`, `Option::map_or_else`, `slice::iter`
  are `rustc_const_unstable`); *no identity of cloned values* 2 (`Clone::clone`, `vec![elem; n]`) and *no identity of a
  stored key* 1 (`insert` keeps the old key); *no effects of a destructor* 1, *no host state* +1 (`Entry::or_insert`:
  the drop of `default` is stated as `lifetime default ends in call when self is Occupied(e)`, from the body; the rest is
  V's `Drop`, and the map the entry holds is not a name for `mutates only self[key]`); *no arm of a macro in a clause* 1
  (`vec![expr; 0]`); *no sequence value* +1 (`IntoIter`'s unyielded elements, was *no drop*); *no condition on an
  associated type's implementation* +2 (the hasher's `write` and `finish` in `insert` and `entry`).
- **Unsure.** (1) `max`, `min` and `Ord::max`'s "and the other is dropped" is not stated: it follows from ownership, and a
  `lifetime … ends in call` on a generic value that a provided method may override cannot carry a `given`. (2)
  `never panics except exhausted` for `insert` ignores the destructor of the key it drops when the map has an equal one,
  as `cmp::max`'s `never panics` ignores the drop of the argument it does not return. (3) `must be used` on the trait
  `Iterator` is read as applying to a `some Iterator` result, which is what the attribute does for `impl Iterator`. (4)
  The `given` list of the two `never panics` is `get`'s, minus the borrow. (5) `lifetime default ends in call` is read
  from the body of `or_insert` (map.rs:2517-2520), not a documented promise. (6) A `const fn` that panics is still
  `evaluable at compile time`: the claim is that a constant context may call it, as `Option::expect` is.
- **Tool limits.** `strip` and `weave` carry `const fn` and a plain `#[must_use]` on a function only (a type's
  `#[must_use]`, and one with a message, stay host text), and `unsafe fn` is host text, with no `obliges`. The `const`
  check calls a function item from a `const fn` and skips a `some` result and a macro; a trait method with a `const` claim is
  refused by rustc, which is right today. Nothing compiles `must be used`, `moved from`, `ends in call` or `survive call`
  against std: they are lints of the clause, not checks of the host. `let (a, b)` is in the language (§ 6.14.7) and
  the lint handles `is (a, b)`; the library file has no `let` clause.

## Class B3: callbacks, their results, conditions and order

B3's 15 `unexpressed` clauses (*no value of a callback result* 7, *callback clause has no condition* 2, *no order of
calls* 6) are now 8: 7 are stated, and 8 stay under corrected labels. The new forms are language § 6.13's `callback c :
called exactly once when p, with a, returning r`, whose `returning` and new `with` names the subject's other clauses may
use, and `protocol`, a regular expression over the subject's callbacks, parameters and the methods of a parameter's
bounds. `libcheck` lints every callback and protocol (`when` and `with` over the subject's names, protocol events that
exist), and `libstats` counts callback clauses by what they name (3 callback clauses: 3 with `when`, 2 with `with`, 2
with `returning`; 2 protocols).

- **Stated (7).** `Option::map_or_else`: `callback default : called exactly once when self is none, returning d`,
  `callback f : called exactly once when exists self, with self, returning r`, and `ensure from_callback : result == (if
  exists self then r else d)` (option.rs:1272-1275), replacing two weaker `at most once` clauses and two `unexpressed`
  ones. `Entry::and_modify`: `callback f : called exactly once when self is Occupied(e), with v` and `ensure in_place :
  result is Occupied(r) implies same(current(r), v)`: `f` gets the entry's own value and the same entry is returned
  (map.rs:2621-2628). `Ord`: `law max_agrees_with_cmp` and `law min_agrees_with_cmp`, which write out the `max_by` and
  `min_by` the contract names, with `cmp` as `compare` (cmp.rs:1605-1611, 1712-1718; the default bodies, cmp.rs:1038 and
  1077, are `other < self`, which `cmp` makes the same by duality). `Iterator::count`: `protocol self.next+`.
  `assert_eq!`: `protocol left right`.
- **Stay, relabelled (8).** *No sequence value* +4 (`map`, `filter`, `take_while` and `flat_map` yield the items of
  `self` after the callback; `count(result)` is stated, the items are not); *no promise about a result's later calls* +3
  (the callback of `map`, `filter` and `take_while` is called by `next` on the adapter, not by the function, so a
  `callback` or `protocol` on the function would be false; `take_while`'s consumed item is the same); *no promise* +1
  (`Hasher::write`: the documentation promises nothing about how the `write_*` methods relate).
- **Unsure.** `count`'s protocol says `next` is called, and at least once, which is the documented side-effect
  promise; "until it returns None" depends on a value the calls returned, so it is carried by `ensure counted` and not
  by the protocol, and the clause is counted as closed. `Ord`'s laws drop the contract's sentence on `clamp` (a pointer
  to its own docs) and "violating these is a logic error, which unsafe code must not rely on being absent", which is
  what a law of the language means for a type that breaks it; `max_by` is written out, not called, because the language
  has no function values. `and_modify`'s `same(current(r), v)` relies on `current` being the `OccupiedEntry::get` view
  and on the returned entry being the one that was passed, which the body shows. `assert_eq!`'s protocol treats a
  macro's parameter as the evaluation of its argument; the "borrowed, not moved" half is carried by the `ref` parameters.

## Class B2: `given`

B2's 30 `unexpressed` clauses (*promise conditional on a bound* 22, *requirement on bound implementations* 6, *no build
profile* 2) are now 13: 20 are stated, 10 stay under corrected labels, and 3 are new residuals. Every `given` of the 24
conditional clauses is checked by `libcheck` (it names a parameter of the subject, a method of its bound, a profile
setting, or `default`).

- **Stated (20).** `HashMap::get`: two laws over `K`, `Q` and `S` (`borrow_eq`, `borrow_hash`) and `never panics`, `effects
  none`, `terminates` given `Q.hash, Q.eq, K.borrow, S.build_hasher` (hashbrown's `get` hashes with `hash_one` and compares
  with `Equivalent`); `cmp::max` and `min`: `never panics` and `terminates` given `T.max` or `T.min`; `Ord::max`: the same
  three given `default` and `Self.lt, Self.cmp`; `Iterator::collect`: given `default` and `B.from_iter, Self.next`;
  `enumerate`, `skip`, `take`, `map`, `filter`, `take_while`, `copied`: `effects none given default`; `chain` and `zip`
  add `U.into_iter effects none`; `flat_map` adds `Self.map effects none`, because its default body calls `Iterator::map`,
  which an implementation may override (the old text missed this); `Iterator::count`: `ensure counted` and `fails panic
  when old(count(self)) > 18446744073709551615` given `build overflow_checks`; the requirement that `k1 == k2` implies
  equal hashes is a `law` on `Hash`, `HashMap` and `HashSet` (stated through `BuildHasher::hash_one`).
- **Stay, relabelled (10).** *No promise* 5 (`Iterator::next`, `Clone::clone`'s arbitrary code, `From::from`, `Deref`,
  `DerefMut`: the documentation leaves the behaviour to impls or only asks them not to fail); *no sufficient condition* 2
  (`HashMap::insert` and `entry`: the text names what the call needs of `Hash`, `Eq` and `build_hasher`, which is necessary
  but not sufficient because the table can overflow when it grows); *no promise about a result's later calls* 1
  (`enumerate`'s overflow panic is that of `next` on the result); *no law limited to a second trait* 1 (`Clone` and
  `Copy`); *no law across several types' impls* 1 (`PartialEq`).
- **New residuals (3).** *No condition on an associated type's implementation* 1 (`S::Hasher`'s `write` and `finish`
  run inside `get`, and no `given` item names a method of an associated type); *no frame condition on a value held by a
  collection* 2 (a key or element must not change while stored, in `HashMap` and `HashSet`).
- **Unsure.** `Hash`'s law uses `RandomState`'s `hash_one`, which is one hasher where the documentation says any;
  `HashMap::get`'s given list adds `K.borrow` and `S.build_hasher`, which the old text left out, from the body of hashbrown
  0.17.1; `count`'s `ensure counted` is now conditional on overflow checks, which weakens it, as § 6.12 writes it.

## Class A: what moved

- **Views** (`views` on the file, or on a subject): `{ "name": "capacity", "of": "'std::vec::Vec'", "host": "'std::vec::Vec::capacity'" }`.
  A clause calls it by name (`invariant fits : size(self) <= capacity(self)`). `libcheck` checks that `host` is under
  `of` and compiles `<Vec<T>>::capacity` against rustc: 3 views, all resolved (`HashMap` and `Vec` capacity,
  `Hasher::finish` as `hash`).
- **Failure payloads**: `failures.panic` is `{ "class": "runtime", "payload": { "message": "text", "location": "location" } }`;
  `fails panic { message: msg, location: caller } when self is none` fills it, and a field the failure lacks is refused.
  `#[track_caller]` is `location: caller`.
- **`budget` and `layout`** are clause kinds (`budget allocates at most 0 B per call when capacity == 0`, `size 0`).
- **Derives**: `derive Clone` on the macro subject names a template; a type subject's `"derive": [...]` is expanded per
  field by `src/derive.rs` (one table), shown by `bare-rust libfmt --expand <file>`. `weave` does not consume it yet.
  Debug is not a template: the text a derive writes is what a function computes.
- **Host profile** `libraries/host-rust-1.97.0.json` (a file of its own: none of it is a promise of a `std` item): the
  panic strategy (`unwind`), the location attribute, and 10 macro arms for `assert_eq!`, `panic!`, `vec!` and `write!`,
  one signature each. `libcheck` validates it against the library (a line `host`).
- **Named functions**: `sign_bit`, `is_nan` join `total_order` as words of the clause lint.

One finding reaches beyond the language: the externals lists name a trait method, never the impl a call resolved to, so
a promise that depends on the impl (`try_from` on integers, `into_iter` on a `Vec`) has no subject.

## The repair round: what an independent review found, and what became `unexpressed`

Ten findings, each fixed in the file or in the tools; where the language cannot say a promise, it moved to an
`unexpressed` clause with its reason rather than stay as an ill-formed clause. The new gaps are marked **gap**.

1. **Overclaims from default bodies.** The `effects none` and `calls only` clauses of Iterator's `enumerate`, `skip`,
   `take`, `chain`, `zip`, `map`, `filter`, `take_while`, `flat_map` and `copied`, of `Ord::max`, and of `HashMap::get`
   (whose `Hash` and `Eq` run inside the call) are gone from the stated clauses: a provided method may be overridden,
   and a bound's impl may do anything. **Gap:** no clause makes a promise conditional on "unless an implementation
   overrides this method", so the 22 clauses of *promise conditional on a bound* now include these 11. `an_argument` of
   `cmp::max`, `cmp::min` and `Ord::max` now cites the documented contract ("returns the maximum of two values"), which
   binds every impl, with status `assumed … documentation`, where it had a body, or none.
2. **The laws.** `Rhs` is explicit: a law calls `'<Self as std::cmp::PartialEq<Self>>::eq'`, a quoted host path
   (language 15's `STABLE`), so each is stated for `Rhs = Self`, and the rest stays an `unexpressed` clause. Paths in
   every clause are quoted. `std::cmp::Ordering` is a subject with its variants, so `is Less` has a type. `panic` is a
   declared failure: the file has a top-level `failures` map (`failure panic : runtime` of language 15), and `fails panic
   when` is checked against it.
3. **`Ord`**'s law is now `duality`, citing PartialOrd's "Duality" (cmp.rs:1156-1157), its conditions 2 and 3
   (1136-1137) and `partial_cmp(a, b) == Some(cmp(a, b))` (739), from which the `iff` follows.
4. **Patterns and sizes on host text.** Slices are `list<T> as 'std::primitive::slice'` (printed `[T]`), so `size(self)`
   on `slice::len`, `iter` and `split_at_mut` is well typed, as are `Hasher::write`'s bytes (`list<u8>`); `slice::iter`
   returns `std::slice::Iter<T>`, with its lifetime elided as `Chars` does. `Entry<'a, K, V>` stays host text: the
   type language has no lifetime argument, and the enum's variants hold host types. **Gap:** the three clauses with
   `is Occupied(e)` / `is Vacant(e)` (`entry`, `or_insert`, `and_modify`) are `unexpressed`, label *no variants of a host
   enum*.
5. **`count`.** `count(it)` is the number of items an iterator would yield from its state at that point; for an iterator
   moved into a call it is written `old(count(self))`, at entry, in every adapter clause and in `Iterator::count`.
   **Gap:** `chain` and `zip` take `other: U` with `U: IntoIterator`, which is no iterator, so their `count(result)`
   clauses are `unexpressed` (*call resolves to an unnamed impl*).
6. **`write!`** has its result: `()` failing with `fmt::Error`, the `fmt::Result` of strsim's use. The `io::Result` of an
   `io::Write` has no slot (a macro has no per-arm signature), and its clause says so.
7. **`never panics`.** The 13 stated ones are all `observed`, each status beginning "from the body", and
   `mem::replace`'s source is the comment in its body, no longer labelled documentation. `From::from` no longer states
   it: the documentation asks every impl not to fail, and impls that allocate (`String::from`) can still panic or
   abort, which is the same promise as `Deref::deref`'s, already `unexpressed`, and is now `unexpressed` the same way
   (*promise conditional on a bound*, one reason worded for both).
8. **Citations.** `HashMap::insert`'s `size` and the new `others_kept`, and `entry`'s `unchanged`, are `observed` with
   the body of hashbrown 0.17.1 (the version in `library/Cargo.lock` at 1.97.0): `insert` at `src/map.rs:1814-1821`,
   `rustc_entry` at `src/rustc_entry.rs:35-54`. The panics of `insert` and `entry` cite `reserve`'s doc (map.rs:904) as
   that, with the growth in the body that makes it apply (`rustc_entry.rs:46`, `self.reserve(1)`). The `or_insert` panic
   clause was wrong: `VacantEntry::insert` calls `insert_no_grow` (`rustc_entry.rs:574-578`), so the room was reserved by
   `entry`. It now says that, and that an occupied entry drops `default` (*no drop*). Four part-B citations that named
   a signature line (`as_ref`, `as_mut`, `map_or_else`, `and_modify`) now name the body.
9. **Reasons that looked wrong, now stated.** `entry`: `ensure unchanged : self == old(self)`; `insert`: `ensure
   others_kept : all k2 in old(self) : k2 != k implies self[k2] == old(self[k2])`; `Ord`: `law agrees_with_partial_ord`;
   `Clone`: `law equal_clone`; `char::MAX`: the type `text where length(value) == 1 and value matches /^\x{10FFFF}$/ as
   'char'` (a refinement, which `Ty` now parses and Rust erases). The `Eq` clause no longer contradicts its law: it says
   what the law does not (f32 and f64 are not Eq; the compiler cannot check an `impl Eq`), and its reason is *no impl or
   associated type*. **Gap:** `Clone`'s "for a `Copy` type `y.clone()` equals `*y`" and `PartialEq`'s laws across two
   types need a law limited to the types that implement another trait; both are `unexpressed` (*requirement on bound
   implementations*, which fell from 8 to 6). `max_by` and `min_by` in `Ord`'s contract take `cmp` as a callback, so
   that clause was *no value of a callback result*; B3 states it as two laws that write out `max_by` and `min_by`.
10. **`Chars`** carries the must-use of its type (iter.rs:28), like the other 11 iterator types.

**The unstable allocator parameter** is left out of every generic list. `HashMap<K, V, S, A>`, `HashSet<T, S, A>`,
`Entry<'a, K, V, A>`, `Vec<T, A>` and `vec::IntoIter<T, A>` all have `A: Allocator` under the unstable `allocator_api`
(issue 32838); the file states the stable arity, which is what the corpora and `libcheck` compile against.

## How the entries were written

By an agent, from the declarations and documentation in `core`, `alloc` and `std` at tag 1.97.0, fetched from
`raw.githubusercontent.com`, and, where `std` delegates, from `hashbrown` 0.17.1. Signatures follow § 4.4. Every
`source` is a line of that tag with a verbatim doc quote, a code snippet, an attribute or a list of impls, and was
checked against the fetched file (quotes by exact match, line numbers from the match); the repair round re-checked the
doc quotes, body snippets and attributes of every source by a throwaway script, and all match but two `unexpressed`
sources that are prose (a default body, a trait head). Documented promises are `assumed … documentation`; guards and
effects found in a body are `observed` or `lifted`. Part A's four exemplars were re-cited the same way, their reasons
relabelled, and `result must be used` added to `cmp::max`. The second agent's check against the documentation (check 5,
human half) has not run.

Conventions the clauses assume, which language 15 does not state:

- A `ref` parameter names its referent (`x == old(y)`); `Self` is in scope in a trait subject, as it is in Rust.
- `count(it)` of an iterator is the number of items it would yield from where it is, without advancing it; `old(count(self))`
  is that at entry. `size` is for a collection or a slice.
- `k in m` on a map is key membership, `all k in m : p` ranges over its keys, and `self[k]` is its optional lookup.
- `x == v` with `x` optional is true only when `x` is present and equal (§ 4), so `partial_cmp(a, b) == cmp(a, b)`
  says `Some`. `Option` and `Result` are matched as variants (`Err(e)`), and `Ordering` as `Less`, `Equal`, `Greater`.
- A law calls the methods of its trait by quoted qualified path, `'<Self as std::cmp::Ord>::cmp'(a, b)`, and applies
  to the types for which every method it names is implemented.
- A slice is `list<T> as 'std::primitive::slice'`, a name this file gives, the path of `std::primitive::slice::len`.

## Tooling: what the repair changed

- **`libcheck` checks the form of every clause's text** (`src/clause.rs`, about 125 lines, not a parser of language 15):
  brackets balance and quotes close; the clause starts with a keyword of its kind; a host path is quoted and looks like
  one (`'std::…'`, `'<Self as …>::…'`); a failure after `fails` is declared in the file's `failures`; and every other
  name is a parameter, `result`, `self`, a generic, `Self` in a trait, a variant of a subject of the file, a name bound
  by `all x` or `is V(x)` before it is used, or a word of language 4. It found 16 problems in the file as it stood.
  **What it does not do:** type-check an expression, resolve a quoted path to an item (the paths of laws name methods
  that are not subjects), check arity, or read the free text of an `unexpressed` clause.
- Earlier in part B: `#[derive(Eq)]` and `#[derive(Copy)]` are generated beside `PartialEq` and `Clone`, which they need
  to compile; each scratch crate has a name of its own, because cargo's fingerprint ignores the directory; and
  `bare-rust libstats` is new.
- `types.rs`: a slice's host form; a refinement after `where` (or `and`, after a char) is kept in the string and erased in
  Rust. The schema has a `failures` map, and `libfmt` knows its place in the key order. `libstats` itself is unchanged.
