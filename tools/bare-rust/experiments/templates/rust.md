# Task @TASK@ (crate `@CRATE@`, trial @TRIAL@)

You are a maintainer of the Rust crate in this directory. Resolve the following issue.

## Issue

@STATEMENT@

## Notes

- This is an ordinary Cargo crate: edit the `.rs` files, build and test with `cargo test` (rustc 1.97 is installed; no network access is needed).
- Your change will be checked by tests you cannot see; they exercise the public API described in the issue. Existing tests in `tests/` should keep working. You may add your own tests.
- Keep the change focused; do not reformat unrelated code.

## When you are done

Finish your final message with a single line of JSON, and nothing after it:

```
{"done":true,"attempts":3,"json_edits":0,"library_promises_relied_on":[],"notes":"one or two sentences"}
```

- `done`: true if you believe the task is complete and the code compiles and works.
- `attempts`: how many times you ran `cargo` (build, check or test) in total.
- `json_edits`: always 0 for this crate (there are no constraint files).
- `library_promises_relied_on`: always `[]` for this crate.
- `notes`: anything surprising, in one or two sentences.

Do not look for, ask about, or try to reconstruct hidden tests. Work only inside this directory.
