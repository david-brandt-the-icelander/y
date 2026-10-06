# Stage 2, part B: the library file for `std`, by hand

`libraries/std-1.97.0.constraints.json` has a subject for each of the 105 external items of strsim, fnv and the golden
crate (49 functions, 25 types, 19 traits, 11 macros, 1 const), and one more, `std::cmp::Ordering`, that the laws need
(260 subjects: 106 entries for the corpora's items, `OccupiedEntry` for a view, 147 impls and impl methods of B1, and 5 trait methods and one trait that B2's `given` items name: `Hash::hash`, `PartialEq::eq`, `PartialOrd` with `lt`, `Ord::cmp`, `Ord::min`). `bare-rust libcheck` passes for all three: no item missing, schema ok, every clause's text well formed,
0 signatures mismatched. Strict library coverage is the default of `bare-rust check` (opt out with
`--allow-missing-libraries`) and is asserted in `tests/corpora.rs`. The counts below are the output of
`bare-rust libstats libraries/std-1.97.0.constraints.json`, which reads the file and compiles its signatures.

## What the file holds

| Entries | Count | | Clauses | Stated | `unexpressed` |
| --- | ---: | --- | --- | ---: | ---: |
| function (27 are trait methods) | 72 | | `ensure` | 65 | |
| type | 27 | | `implements` | 140 | |
| trait | 20 | | `effects` | 31 | |
| macro | 11 | | `result must be used` | 18 | |
| const | 1 | | `never panics` | 18 | |
| impl | 129 | | `law` | 14 | |
| | | | `fails panic`, `fails` | 10, 1 | |
| | | | `hold`, `derive`, `callback` | 5, 6, 3 | |
| | | | `budget`, `layout`, `invariant`, `on failure` | 2, 1, 1, 1 | |
| | | | `unexpressed` | | 121 |

437 clauses in all: 316 stated and 121 `unexpressed`, on 240 subjects. B1 (step 4 of `proposal/language-gaps.md` § 6)
took `unexpressed` from 175 to 138 and added 129 impl subjects, each compiled against the real std; B2 took it from 138 to 121.
24 of the stated clauses are conditional, qualified by `given` (on 16 subjects): 15 on a bound's implementations, 15 on the
default body, 2 on a build setting. Stated clauses by
status: documentation 70, observed 205, `#[must_use]` lifted 26 (counted as lifted), hand-written 15.

**Signatures**, compiled against rustc 1.97.0: 250 checked, 6 path-resolved only (`PartialEq`, `PartialOrd`, `From`, `TryFrom`,
`Index`, `IndexMut`: a trait with generics), 4 unchecked (the bang macros `assert_eq!`, `panic!`, `vec!`, `write!`),
0 mismatched. Of 188 type slots, 181 map to the type language (96%) and 7 are host text. Slices are no longer host text (below).

## What the language lacked: the `unexpressed` promises (121)

Each reason in the file starts with its label. Class A took the file from 199 to 175; B1 (traits, implementations and
associated types) from 175 to 138. Of B1's 52, 38 are closed outright and 14 stay under corrected labels; 6 of the
host-state ones stay (the vacant half of `Entry::or_insert` is all that moved).

- **Left from B1, relabelled (16).** No trait items 3 (what a trait's associated types and provided methods are); no
  negative impl 2 (`Eq` and `Copy`); no elided lifetime in a trait argument 3 (`From<&str>`, `Index<&Q>`,
  `Extend<&T>`); no deref coercion 1; no mutable reference as an impl's self type 1; no unsized text as an associated
  type 1 (`Deref<Target = str>`); no unstable or negative bound 1 (`Step` for `Range`); no view of two arguments 1
  (`is_char_boundary`); no sequence value +1 (`collect`'s Result and String impls); no drop +1 (`vec::IntoIter`).
- **Host state (6)**, unchanged: `Entry::or_insert` (the map after a vacant insert), `Formatter`, `TryFromIntError`,
  `Display::fmt`, `Hasher::write`, `Hash`.
- **Others, untouched.** No const-ness 18; no sequence value 15; must-use on a type 14; what a function computes 8; no
  value of a callback result 7; no identity of moved values 6; no order of calls 6; no drop 5; no positive effect 3; and
  the smaller labels.

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
   that clause is *no value of a callback result*.
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
