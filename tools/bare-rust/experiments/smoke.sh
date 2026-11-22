#!/bin/bash
# smoke.sh [task...]: prepare rust and bare workspaces (SMOKE_ARMS to choose arms), grade untouched (must fail), apply the reference, grade (must pass)
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"
tasks=("$@"); [ ${#tasks[@]} -eq 0 ] && tasks=(fnv-write-order sorensen-dice)
rc=0
for t in "${tasks[@]}"; do
  tmp="$(mktemp -d)"
  materialize_ref "$t" "$tmp/ref"
  for arm in ${SMOKE_ARMS:-rust bare bare-renamed bare-nolib bare-nounexpressed}; do
    "$EXP/prepare.sh" "$t" $arm 1 "$tmp/$arm" >/dev/null || { echo "prepare failed $t $arm"; rc=1; continue; }
    u="$("$EXP/grade.sh" "$t" $arm "$tmp/$arm" | jq -c '{pass,compiles,tests_passed,tests_total,json_entries_changed,lines_changed}')"
    if [ $arm = rust ]; then
      (cd "$tmp/ref" && find . -name '*.rs' -print0 | xargs -0 -I{} cp --parents {} "$tmp/$arm/")
    else
      rm -rf "$tmp/refstrip"; strip_into "$tmp/ref" "$tmp/refstrip"
      [ $arm = bare-renamed ] && rename_into "$tmp/refstrip" "$(jq -r .crate "$EXP/tasks/$t/task.json")"
      (cd "$tmp/refstrip" && find . \( -name '*.brs' -o -name '*.constraints.json' \) -print0 | xargs -0 -I{} cp --parents {} "$tmp/$arm/")
    fi
    g="$("$EXP/grade.sh" "$t" $arm "$tmp/$arm" | jq -c '{pass,compiles,weaves,tests_passed,tests_total,json_entries_changed,lines_changed}')"
    echo "$t $arm: untouched=$u"; echo "$t $arm:   reference=$g"
    [ "$(echo "$u" | jq .pass)" = false ] && [ "$(echo "$g" | jq .pass)" = true ] || { echo "  ^^^ SMOKE FAILURE"; rc=1; }
  done
  rm -rf "$tmp"
done
exit $rc
