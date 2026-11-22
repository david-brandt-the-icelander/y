# Y

Y is an AI-oriented systems and application programming language built on one assumption: most Y source will be written by an AI agent and read by a human reviewer, and the **compiler**, not either of them, is the authority on meaning, safety and correctness. Y does not contain a model and does not accept English. Its semantics are deterministic, so an agent's output is only a proposal that the compiler accepts or rejects. AI proposes; deterministic tooling verifies. A language that works this way has to make the facts a reviewer would otherwise hold in their head (what a function may mutate, whether it can suspend, whether a retry can charge a customer twice, which errors escape) cheap for the compiler to compute and expensive to violate. Y source is elaborated into a backend-neutral **Core IR**, which is lowered to Rust, Zig or C.

> **Status: working design plus a prototype toolchain.** The language is specified in prose and its syntax is still illustrative. A prototype compiler lowers the example programs to Rust, and those programs build and pass their tests. This is not a finished language, and the Zig and C backends are design only. See [`documentation/`](documentation/) for what is decided and what is open.

## Read

- [`Y-current-language-design.md`](Y-current-language-design.md): the canonical statement of what Y is.
- [`documentation/`](documentation/): the language and toolchain written up one capability at a time, with a reading order in [`documentation/README.md`](documentation/README.md).
- [`proposal/`](proposal/): the proposals behind the design, including the constraint language, `bare-rust`, and the idea harvest.

## Build and run an example

You need a stable Rust toolchain (`cargo`). Each folder under [`examples/`](examples/) is a Cargo crate holding a program written in Y (`y/src/*.y`), the Rust it lowers to, and a README on the lowering. The terminal examples need nothing else:

```bash
cd examples/hangman
cargo run
```

```bash
cargo test
```

`checkers`, `memory` and `snake` open a window (Macroquad) and need a desktop with the usual graphics libraries; their tests run without one. The list of examples is in [`examples/README.md`](examples/README.md).

## The toolchain

| Tool | What it does |
|---|---|
| [`tools/y-parse`](tools/y-parse/) | Parses Y source into a syntax tree. |
| [`tools/y-compile`](tools/y-compile/) | The prototype Y compiler: lowers an example's Y to bare Rust plus constraint files. |
| [`tools/bare-rust`](tools/bare-rust/) | Strips Rust to its bare bodies and constraint files and weaves them back into Rust, checking the constraints. |
| [`tools/paths`](tools/paths/) | Scripts that enumerate each example's execution paths and check that its tests cover them. |

Run each tool's tests from its own folder, for example `cd tools/y-parse && cargo test --release`.

## Licence

[Mozilla Public License 2.0](LICENSE).
