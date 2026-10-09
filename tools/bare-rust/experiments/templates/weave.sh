#!/bin/bash
# weave.sh [check|test]   weave the .brs/.constraints.json files into a scratch crate (.weave/), then run cargo there.
#   check (default): weave (which runs cargo check, derives `mut`, reports holes and errors) and exit non-zero on failure.
#   test:            also run `cargo test` in .weave/.  Extra tests: put plain test files in extra-tests/*.rs (they are
#                    copied to .weave/tests/ and are not part of the bare crate).
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BARE_RUST="${BARE_RUST:-@BARE_RUST@}"
out="$here/.weave"
rm -rf "$out"
grep -q "^\[workspace\]" "$here/Cargo.toml" || printf "\n[workspace]\n" >> "$here/Cargo.toml"
"$BARE_RUST" weave "$here" --out "$out" --crate "$here" || { echo "weave: FAILED" >&2; exit 1; }
if [ "${1:-check}" = test ]; then
  [ -d "$here/extra-tests" ] && { mkdir -p "$out/tests"; cp "$here"/extra-tests/*.rs "$out/tests/" 2>/dev/null; }
  (cd "$out" && cargo test 2>&1 | tail -n 60)
  exit "${PIPESTATUS[0]}"
fi
