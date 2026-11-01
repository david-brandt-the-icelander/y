#!/bin/sh
# Remakes an example's bare files and bare/y-facts.json from its Rust and Y (committed or edited), then runs its
# y_examples checks. Usage: tools/paths/rebare.sh <example folder name, e.g. nim or tic-tac-toe>
set -e
[ -n "$1" ] || { echo "usage: $0 <example>" >&2; exit 2; }
P=$(cd "$(dirname "$0")" && pwd)
Y=$(cd "$P/../.." && pwd)
B=$Y/tools/bare-rust
E=$Y/examples/$1
S=$(mktemp -d)
trap 'rm -rf "$S"' EXIT
(cd "$B" && cargo build -q --release)
"$B/target/release/bare-rust" strip "$E" --out "$S/$1" --src-only | tail -1
cd "$Y/examples" && python3 "$P/regen.py" "$1" "$S/$1" | grep -v "^no clauses" || true
"$B/target/release/bare-rust" libfmt $(ls "$E"/bare/src/*.constraints.json "$E"/bare/src/*/*.constraints.json 2>/dev/null) | tail -1
cd "$Y/examples" && python3 "$P/pathfacts.py" "$1"
T=$(echo "$1" | tr - _)
cd "$B" && cargo test -q --release --test y_examples -- "${T}_"
