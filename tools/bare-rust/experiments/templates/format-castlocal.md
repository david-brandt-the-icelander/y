### Local and cast hints

What rustc cannot infer from the bare body goes in `holes`, keyed by the subject: a `locals` entry per `let` (keyed by the pattern; the second `let x` is `x#2`) and `hints` for a `targ` (a turbofish `::<_>`), a `literal`, or a closure's parameter or return type, each numbered `n` from 0 in body order.

**Casts are written as locals.** A conversion between built-in types (`as u64`, `as f64`, ...) is not written in the body. Bind the value to a `let` and give the local the entry `{"type": "<T>", "cast": true}`: `let n = e;` weaves to `let n: T = e as T;` (with brackets around `e` when it needs them). Write the `let` even where the Rust would cast in the middle of an expression: `1.0 - d / m` with `d` and `m` cast locals, rather than `1.0 - (d as f64) / (m as f64)`.

Rust:

```rust
pub fn mean(values: &[f64]) -> f64 {
    let total: f64 = values.iter().sum();
    let n = values.len() as f64;
    total / n
}
```

Bare (`.brs`):

```brs
fn mean(values) {
    let total = values.iter().sum();
    let n = values.len();
    total / n
}
```

Constraint file (`.constraints.json`, under `"subjects"` and `"holes"`):

```json
{
  "kind": "function",
  "name": "format_demo::mean",
  "visible": "public",
  "params": [{"name": "values", "type": "ref<list<f64> as 'std::primitive::slice'>", "access": {"rights": "read", "guarantee": "readers"}}],
  "result": {"type": "f64"}
}
{
  "holes": {
    "format_demo::mean": {
      "locals": {
        "total": "f64",
        "n": {"type": "f64", "cast": true}
      }
    }
  }
}
```

(Code stripped from Rust can still contain `e as _` where a cast is not the whole initialiser of a typed `let`; its target type is a hint `{"kind": "cast", "n": <ordinal of the cast in the body>, "type": "<T>"}`. Do not write new ones; use a cast local.)

