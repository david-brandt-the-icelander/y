# Second hosts: TypeScript, Python and C#

This is a plan, not a build. It says how `strip` and `weave` would be built for three more hosts once
[`bare-rust.md`](bare-rust.md) stage 3 recommends going on, what each host can prove from the constraint file, and in
what order to build them. Nothing here is built yet.

---

## 0. The plan on one page

- **One design, three hosts.** Each host keeps the decisions of bare Rust § 2: the bare language is the image of
  erasure, a mixed construct is split in two, the file is authoritative and the weaver prints without inferring, holes
  fail the weave, the JSON is the source of truth, and a file that cannot be split is refused and counted.
- **The constraint file is shared.** One JSON Schema, one canonical form, one set of library files per library and
  version. Only the slot table, the type mapping's `as` names, and the profile of holes are per host. A clause written
  against the Rust profile and bound by a stable name (language § 4) must bind the TypeScript, Python or C# host
  unchanged; that is the test of host neutrality the Rust trial could not run.
- **Each host differs in what its checker proves.**
  - TypeScript's checker proves types and `readonly` shallowly.
  - Python proves nothing without a checker; mypy or pyright proves types where hints exist.
  - C# proves types, nullability where enabled, `readonly`, `ref` and `in`, and `Span` lifetimes.
  - Where the checker proves nothing, `weave` prints run-time checks or analyser annotations (language § 6, the hosts'
    table), so the weaving is the same but the evidence differs.
- **Order: TypeScript first, then C#, then Python.** TypeScript is the cheapest: its compiler already defines what
  erases. C# is next, because it is the second-largest language of the studio's code (language
  [`adaptations/stillfront.md`](adaptations/stillfront.md)) and its checker proves more than TypeScript's. Python comes
  last. It is closest to the game code the language exists for, but it has the least to check against, so its trial
  measures the least per dollar.
- **Each host has three stages and runs E1 to E5 again.** No new experiments are needed. E3 and E4 compare across hosts:
  the same task in Rust and in TypeScript, from the same constraint file.

---

## 1. What a host needs

Each host needs nine parts. In bare Rust these are the 4,600 lines of stage 1 and the library tooling of stage 2. Most
of the Rust tool is shared, and is moved into a host-neutral crate before the second host is built (§ 5):

1. **A parser that keeps every byte and comment.** It is used for both the host language and the bare form.
2. **The grammar delta:** the productions that are emptied.
3. **The slot table:** each emptied production, its slot and its clause.
4. **The type mapping:** host types to the language's types, with host text as the fallback.
5. **The profile of holes:** what the body leaves that only the file can fill.
6. **The derived cases:** what `weave` leaves to the compiler, as Rust does for `let mut`.
7. **The checker run,** with hole reports read from its errors, and error mapping back to the bare body and the JSON.
8. **The lint** against constraints disguised as code.
9. **Library files** for the standard library and the dependencies the corpora call.

---

## 2. TypeScript

### 2.1 What erases

TypeScript is defined as JavaScript plus erasable syntax, and `tsc --erasableSyntaxOnly` and Node's type stripping
already name the erasable productions. So the bare language is nearly JavaScript: strip is close to type stripping
plus visibility, `readonly` and modifiers. This is why it is the cheapest host. The non-erasable constructs (`enum`,
`namespace` with values, parameter properties) are refused at first, as Rust's unbuilt productions were, and counted.

### 2.2 Parser

Use tree-sitter-typescript, extended the same way as tree-sitter-rust (bare Rust § 3.1): the type productions become
empty, and an emptied parameter keeps its name. The grammar delta is expected to be under 40 lines, against Rust's 21.

### 2.3 Slots

| Emptied production | Slot | Clause |
| --- | --- | --- |
| `export`, `public`/`private`/`protected`, `#private` | `visible` | `visible to public \| module \| type` |
| A parameter's, result's or property's type annotation | `params[i].type`, `result.type`, `fields[f].type` | The type |
| `readonly` on a property, `ReadonlyArray`, `Readonly<T>` | `params[i].access`, `fields[f].access` | `access : read readers` |
| `?` on a property or parameter; `T \| undefined` | The type | `T?` |
| Generic parameters, `extends` constraints | `generics` | `for T where T: Bound` |
| `implements`, `extends` of an interface | `implements` | The set of implementations (bare Rust § 2.2) |
| `async` | Stays in the body (behaviour) | `Promise<T>` is printed from `result.type` and `async` |
| `throws` (JSDoc `@throws` only) | `fails only` | `fails only E`, which is not checked; `weave` prints a JSDoc tag |
| A `let` or `const` annotation | `holes.locals[name]` | `local name : T` |
| `as T`, `<T>x`, `satisfies T` | A hole: `x as _` is not TypeScript, so the bare form is `x as unknown` with the target in `holes.hints` | Body hint |
| A non-null assertion `x!` | Kept in the body, like `.unwrap()`; on an input, the presence moves to `require` | `require … else` |

### 2.4 Types

| TypeScript | Constraint language | Note |
| --- | --- | --- |
| `boolean`, `string` | `bool`, `text` | |
| `number` | `f64`, or an integer type with `as 'number'` when the file states a range | `weave` prints `number` either way; the range is checked at run time where stated |
| `bigint` | `i128 as 'bigint'`, or unbounded integer | |
| `T \| null`, `T \| undefined`, `T?` | `T?` | `null` and `undefined` are distinguished only by `as` |
| `T[]`, `Map<K, V>`, `Set<T>` | `list<T>`, `map<K, V>`, `set<T>` | |
| `[A, B]` | `(A, B)` | |
| A discriminated union of object types | A variant type | |
| Object types, intersections, mapped and conditional types, `keyof` | Host text | Counted per form, as in E1 |

### 2.5 Holes and derived cases

- **Holes.** TypeScript infers widely, so the holes are few. They are an empty array or map literal (`[]` infers
  `never[]` or `any[]`), a parameter of a callback that is not passed in context, and a generic call whose type
  argument comes only from the result. `tsc --noImplicitAny` reports each as an error at the hole. The E2 of TypeScript
  is expected to be smaller than Rust's.
- **Derived.** `let` versus `const` is derived like Rust's `let mut`: the bare body writes `let`, `weave` prints
  `const` where nothing reassigns it. This is what ESLint's `prefer-const` does, and it gives the round trip.

### 2.6 What the checker proves, and what is printed instead

- **Proved by `tsc`:** types, nullability under `strictNullChecks`, and shallow `readonly`.
- **Printed instead:**
  - Clauses with no static form become checks at entry and exit, in a wrapper:
    - deep `access : read`, as `Object.freeze` in debug builds;
    - `require … else`, as a guard at the head of the function;
    - ranges on `number`.
  - Clauses about holding (`owns`, `borrows from`) and `sendable` have no proof under tracing (language § 18). `weave`
    prints them as comments or as analyser annotations, and E1 counts them as unchecked.

### 2.7 Corpora, libraries and checks

- **Corpora:** two small, well-tested packages with their histories, which are also the pool of E4 tasks. One should
  be pure logic (a string-distance or a semver package, like strsim) and one a small data structure (an LRU cache).
- **Libraries:** a library file for the ECMAScript built-ins and the Node APIs the corpora call, written by agents and
  reviewed as in stage 2. The `.d.ts` files give the signatures, so the compile check of check 5 is
  `tsc` against them.
- **Checks 1 to 5:** the same as Rust's. "Up to `rustfmt`" becomes "up to Prettier".

---

## 3. C#

### 3.1 What erases

C# has more of the constraint in its syntax than TypeScript, and less than Rust:

- types and nullability annotations;
- `readonly` and `init`;
- `ref`, `in`, `out` and `ref readonly`;
- `scoped`, and `ref struct` with its lifetime rules;
- visibility;
- `static`, `sealed` and `abstract`;
- generic constraints;
- `async`.

The bare form keeps names, and `var`, which is already valid C#, everywhere a local's type is erased.

### 3.2 Parser

Use tree-sitter-c-sharp, extended as above. Roslyn is the more faithful parser and keeps all trivia, but it would put a
.NET process in the tool, so it is used only as the checker. Tree-sitter's C# grammar lags behind the language. Its
gaps are found by check 1 over the corpora and refused with their reason, as § 2.7 of bare Rust requires.

### 3.3 Slots

| Emptied production | Slot | Clause |
| --- | --- | --- |
| `public`, `internal`, `protected`, `private`, `file` | `visible` | `visible to public \| crate \| type \| module` (`internal` is the assembly, so `crate`) |
| A parameter's, result's, field's or property's type | The type slots | The type |
| `?` on a reference type with nullable enabled | The type | `T?` |
| `ref`, `in`, `ref readonly`, `out` on a parameter | `params[i].access` | `write exclusive`, `read readers`, `read readers`; `out` is a second result |
| `scoped`, `ref` returns, `ref struct` fields | `params[i].hold`, `result.hold` | `hold … : borrows from p`; this is where C# checks memory clauses, as Rust does |
| `readonly` field, `init`, `readonly struct` | `fields[f].access` | `access : read readers` after construction |
| Generic parameters and `where T : …` | `generics` | `for T where T: Bound` |
| `[Pure]`, `[return: NotNull]`, `[MustUseReturnValue]` | Clauses | `pure`, `result must be used` |
| `throw` at the head of a method on an argument (`ArgumentNullException.ThrowIfNull`) | Lint: a disguised constraint | `require … else` |
| A cast `(T)x` | A hole; the bare form is `(var)x`, which is not C#, so the bare grammar adds it | Body hint |

### 3.4 Types

| C# | Constraint language | Note |
| --- | --- | --- |
| `bool`, `byte` to `ulong`, `sbyte` to `long`, `float`, `double`, `decimal` | `bool`, `u8` to `u64`, `i8` to `i64`, `f32`, `f64`, `decimal` | `int` overflow is unchecked unless `checked`, so `weave` prints `checked(…)` where the file says `overflow fails` |
| `string`, `char` | `text`, `text where length(value) == 1 as 'char'` | UTF-16 code unit, named by `as` |
| `T?` (value types), `T?` (reference types, nullable enabled) | `T?` | |
| `List<T>`, `Dictionary<K, V>`, `HashSet<T>`, `T[]`, `ImmutableArray<T>` | `list<T>`, `map<K, V>`, `set<T>`, with `as` | `ImmutableArray` and `IReadOnlyList` also give `access : read` |
| `Span<T>`, `ReadOnlySpan<T>` | `ref<list<T>>` with `access` | A borrow: it is checked by the compiler |
| `Task<T>`, `ValueTask<T>` | `T`, with `async` | |
| Tuples `(A, B)` | `(A, B)` | |
| Delegates, `Func<…>`, dynamic, pointers | Host text | |

### 3.5 Holes and derived cases

- **Holes.** `var` already exists, so a local's erased type is no hole unless the initializer is `null`, `default`, a
  lambda, or a target-typed `new()` or collection expression (`[]`). These are the C# holes, and the compiler's errors
  CS0815, CS8754 and CS9176 report them at the hole.
- **Derived.** `static` on a lambda or local function, and `readonly` on a struct member that writes nothing, are
  derived as `let mut` is in Rust. The analysers IDE0062 and IDE0251 suggest them, and `weave` applies the
  suggestions.

### 3.6 What the checker proves

- **Proved by Roslyn:** types and nullability (as warnings, so `weave` runs with `<Nullable>enable</Nullable>` and
  warnings as errors), and `ref`/`scoped` lifetimes.
- **Printed instead:** clauses about holding over ordinary references have no static form under tracing. `weave`
  prints them as attributes that a Roslyn analyser reads. That analyser is built only if E5 on C# shows the clauses are
  relied on.

### 3.7 Corpora and libraries

- **Corpora:** two small NuGet packages with tests and histories, and a slice of the studio's own C# service code if
  its licence allows the trial to publish its numbers.
- **Libraries:** a file for the BCL items in use. The reference assemblies give the signatures, and check 5's compile
  check is a generated call site built with `dotnet build`.

---

## 4. Python

### 4.1 What erases

Python's syntax holds almost no constraint:

- the type hints;
- `@property` without a setter, which acts as `readonly`;
- `Final`;
- `__slots__`;
- `@dataclass(frozen=True)`;
- a leading underscore, which works as visibility by convention.

So the bare form is Python with every hint removed, which is valid Python that runs. This makes it the host where the
claim of language § 11.2 is weakest to test, and where it matters most to the game code.

### 4.2 Parser

Use LibCST. It keeps every byte, comment and blank line, and its concrete tree can print itself back exactly, which
is the role tree-sitter plays for Rust. The tool calls it as a small Python program over a pipe, or uses
tree-sitter-python if LibCST's round trip is not needed. LibCST's round trip is stronger, so it is the leaning.

### 4.3 Slots

| Emptied production | Slot | Clause |
| --- | --- | --- |
| A leading `_`, `__all__` | `visible` | `visible to module` or `public`; renaming is behaviour, so `weave` checks the name and does not print it |
| Parameter, result and attribute annotations | The type slots | The type |
| `Final`, a property without a setter, `frozen=True` | `access` | `access : read readers` |
| `TypeVar` with bounds, PEP 695 `[T: Bound]` | `generics` | `for T where T: Bound` |
| `Protocol` and ABC bases | `implements` | The set of implementations |
| `raise ValueError` at the head of a function on an argument, `assert` on an input | Lint: a disguised constraint | `require … else` |
| A local's annotation | `holes.locals[name]` | `local name : T` |
| `cast(T, x)` | A hole: the bare form is `x` itself, with `T` as a hint | Body hint |

### 4.4 Types

| Python | Constraint language | Note |
| --- | --- | --- |
| `bool`, `int`, `float`, `str`, `bytes` | `bool`, unbounded integer, `f64`, `text`, `list<u8>` | `int` maps to a bounded type only when the file states a range |
| `X \| None`, `Optional[X]` | `X?` | |
| `list[T]`, `dict[K, V]`, `set[T]`, `tuple[A, B]`, `Sequence[T]`, `Mapping[K, V]` | `list<T>`, `map<K, V>`, `set<T>`, `(A, B)`; `Sequence` and `Mapping` give `access : read` | |
| `TypedDict`, `@dataclass` | A record type | |
| `Callable`, `Any`, `ParamSpec`, `TypeGuard` | Host text | |

### 4.5 Holes and the checker

- **Holes:** few in the sense of Rust, because nothing in a Python body needs a type to run. A hole here is a place
  where pyright in strict mode cannot infer, such as `[]` or `{}` with no annotation, or a callback parameter. The
  "compiles" of E2 to E4 becomes "pyright strict passes, and the tests pass".
- **What is checked:** types are checked statically by pyright, which is the checker the trial should choose because
  it reports more than mypy. Almost everything else is printed as run-time checks: `require`, `access : read` as
  read-only proxies in debug builds, and the effects named in language § 6. This makes the Python weave longer than the
  bare form, which is the cost of the host. E4 must count woven lines and lines that check at run time separately.
- **Derived:** nothing. Python has no binding mutability to derive.

### 4.6 Corpora and libraries

- **Corpora:** two small typed packages with tests and histories, and if possible a module of the studio's Python
  server code, which is the code the language is for.
- **Libraries:** typeshed gives the standard library's signatures, so the compile check of check 5 is pyright against
  typeshed, and the agent-written files add the promises typeshed cannot state: mutation, raising, and what is kept.

---

## 5. Shared work before the second host

The Rust tool mixes host-neutral and Rust-only code. Before the second host, it is split:

- **`y-constraints`** holds the shared parts:
  - the JSON Schema and canonical form (`json.rs`);
  - the clause parser and lint (`clause.rs`);
  - the type parser and the language's types (`types.rs`, without the Rust mapping);
  - the library-file format and its mechanical check;
  - the trace format;
  - error mapping;
  - the reports for E1 to E3.
- **`bare-rust`** keeps the grammar, `strip`, `weave`, `cargo`, `mut` derivation and the Rust mapping. The interface
  between the two is the `host.rs` trait that already exists, which each host implements.
- **`bare-ts`, `bare-cs` and `bare-py`** are built later in the same workspace. Each one is the host trait, a grammar
  extension and a mapping table. Python's parsing runs in a LibCST sidecar.

A constraint file is checked against every host that binds it. A stable name bound by both a Rust and a TypeScript
module must weave in both. That check is new; Rust alone could not need it.

---

## 6. Stages and cost

Each host runs the same three stages as Rust, with one difference: the corpora, library files and experiments are
smaller where the host's checker proves less.

| Stage | TypeScript | C# | Python |
| --- | --- | --- | --- |
| 0. Shared split (§ 5), done once | 3 build, 1 check | — | — |
| 1. The signature, as far as the corpora need it; E1 | 4 build, 2 check | 5 build, 2 check | 4 build, 2 check |
| 2. Library files; check 5 | 3 build, 2 check | 3 build, 2 check | 3 build, 2 check |
| 3. Error mapping, E2 to E5 | 2 build, 2 check, 80 experiment runs | the same | the same |
| **Runs** | **12 build, 7 check, 80 experiment** | **10 build, 6 check, 80 experiment** | **9 build, 6 check, 80 experiment** |
| **Cost at the units of bare Rust § 14.2, with $6 orchestration a stage** | **about $100** | **about $95** | **about $95** |

E4 and E5 are 80 experiment runs per host: 20 tasks × 2 arms × 2 trials, with 10 tasks × 2 E5 variants as a subset
that reuses its bare arm. This is fewer than Rust's 120, because the E5 variants reuse the second trial of the bare
arm. The units are Rust's measured ones, which stage 2 showed are only reliable when agents run short. Each build run is
kept to one part of a stage, and each batch of library entries to about 20. The three hosts together are about
$290, which is about 55% of a Max 5x week (bare Rust § 14.3).

---

## 7. What would change this plan

- **If Rust's stage 3 recommends changing language § 11.2,** this plan changes with it. A design where bodies need
  hints for most functions (E2), or where agents do worse in the bare arm (E4), argues for a lighter split in the
  hosts that infer more. TypeScript and Python would then keep their annotations in the body and use the file only for
  the clauses their syntax cannot state.
- **If E5 shows the `unexpressed` promises are what agents use,** library files come before the host's stage 1, not
  after.
- **If TypeScript's E4 matches Rust's,** C# and Python can skip their own E4 and run only E1 to E3. That saves about
  $60 each.
