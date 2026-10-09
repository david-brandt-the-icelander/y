#!/bin/bash
# prepare.sh <task> <arm> <trial> <outdir>   arms: rust | bare | bare-nolib | bare-nounexpressed
set -euo pipefail
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"
task="${1:?task}"; arm="${2:?arm}"; trial="${3:?trial}"; out="${4:?outdir}"
[ -d "$EXP/tasks/$task" ] || { echo "no such task: $task" >&2; exit 2; }
[ -e "$out" ] && [ -n "$(ls -A "$out" 2>/dev/null)" ] && { echo "outdir $out is not empty" >&2; exit 2; }
mkdir -p "$out"; out="$(cd "$out" && pwd)"
crate="$(jq -r .crate "$EXP/tasks/$task/task.json")"
statement="$(cat "$EXP/tasks/$task/STATEMENT.md")"
base="$(mktemp -d)"; trap 'rm -rf "$base"' EXIT
materialize_base "$task" "$base"

render() { # template -> TASK.md, filling @TASK@ @CRATE@ @TRIAL@ @STATEMENT@
  python3 - "$1" "$out/TASK.md" "$task" "$crate" "$trial" "$statement" <<'P'
import sys
t=open(sys.argv[1]).read()
for k,v in zip(("@TASK@","@CRATE@","@TRIAL@","@STATEMENT@"),sys.argv[3:7]): t=t.replace(k,v)
open(sys.argv[2],"w").write(t)
P
}

case "$arm" in
  rust)
    cp -r "$base/." "$out/"
    render "$EXP/templates/rust.md"
    ;;
  bare|bare-nolib|bare-nounexpressed)
    strip_into "$base" "$out"
    sed "s|@BARE_RUST@|$BARE_RUST|" "$EXP/templates/weave.sh" > "$out/weave.sh"; chmod +x "$out/weave.sh"
    cp "$EXP/../docs/bare-format.md" "$out/FORMAT.md"
    case "$arm" in
      bare) mkdir -p "$out/libraries"; cp "$LIBFILE" "$out/libraries/"; render "$EXP/templates/bare.md" ;;
      bare-nounexpressed)
        mkdir -p "$out/libraries"
        # drop every clause of kind "unexpressed", at any depth
        jq 'walk(if type=="object" and has("clauses") and (.clauses|type=="array") then .clauses |= map(select(.kind!="unexpressed")) else . end)' \
          "$LIBFILE" > "$out/libraries/$(basename "$LIBFILE")"
        render "$EXP/templates/bare-nounexpressed.md" ;;
      bare-nolib) render "$EXP/templates/bare-nolib.md" ;;
    esac
    ;;
  *) echo "unknown arm: $arm" >&2; exit 2 ;;
esac
echo "prepared $task/$arm/$trial in $out"
