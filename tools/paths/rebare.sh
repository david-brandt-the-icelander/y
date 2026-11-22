#!/bin/sh
# Remakes an example's bare files and bare/y-facts.json from its Rust and Y (committed or edited), then runs its
# y_examples checks. Usage: tools/paths/rebare.sh [--from-y] <example folder name, e.g. nim or tic-tac-toe>
#
# With --from-y the bare files and bare/y-facts.json come from the Y compiler (tools/y-compile) instead of from the
# hand-written Rust, and src/ is replaced by their weave, so that the checks compare the two. The hand-written Rust
# stays in git: `git checkout examples/<example>/src` brings it back.
set -e
FROM_Y=
[ "$1" = "--from-y" ] && { FROM_Y=1; shift; }
[ -n "$1" ] || { echo "usage: $0 [--from-y] <example>" >&2; exit 2; }
P=$(cd "$(dirname "$0")" && pwd)
Y=$(cd "$P/../.." && pwd)
B=$Y/tools/bare-rust
E=$Y/examples/$1
S=$(mktemp -d)
trap 'rm -rf "$S"' EXIT
(cd "$B" && cargo build -q --release)
(cd "$Y/tools/y-parse" && cargo build -q --release)
if [ -n "$FROM_Y" ]; then
    (cd "$Y/tools/y-compile" && cargo build -q --release)
    "$Y/tools/y-compile/target/release/y-compile" "$E" --out "$S/bare"
    # the woven crate sits beside the examples, so that its relative paths (`../y-runtime`) resolve
    W=$Y/examples/.y-compile-rebare-$1
    rm -rf "$W"
    "$B/target/release/bare-rust" weave "$S/bare" --out "$W" --crate "$E" | tail -2
    rm -rf "$E/bare" && cp -r "$S/bare" "$E/bare"
    rm -rf "$E/src" && cp -r "$W/src" "$E/src"
    rm -rf "$W"
    echo "src/ now holds the weave of y-compile's output; git checkout examples/$1/src restores the hand-written Rust"
    python3 "$P/ylint.py" "$E"/y/src/*.y
    T=$(echo "$1" | tr - _)
    cd "$B" && exec cargo test -q --release --test y_examples -- "${T}_"
fi
"$B/target/release/bare-rust" strip "$E" --out "$S/$1" --src-only | tail -1
cd "$Y/examples" && { python3 "$P/regen.py" "$1" "$S/$1" > "$S/regen.out" || { cat "$S/regen.out"; exit 1; }; }
grep -v "^no clauses" "$S/regen.out" || true
"$B/target/release/bare-rust" libfmt $(ls "$E"/bare/src/*.constraints.json "$E"/bare/src/*/*.constraints.json 2>/dev/null) | tail -1
cd "$Y/examples" && python3 "$P/pathfacts.py" "$1"
python3 "$P/ylint.py" "$E"/y/src/*.y
T=$(echo "$1" | tr - _)
cd "$B" && cargo test -q --release --test y_examples -- "${T}_"
