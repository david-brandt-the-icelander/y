#!/bin/bash
# verify_tasks.sh [task...]: for each task checks (1) base fails the hidden tests, (2) the reference passes them,
# (3) strip of base succeeds, (4) the reference weaves back from its own strip. Prints one line per task.
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"
tasks=("$@"); [ ${#tasks[@]} -eq 0 ] && tasks=($(ls -d "$EXP"/tasks/*/ | xargs -n1 basename | grep -v '^_'))
fail=0
for t in "${tasks[@]}"; do
  tmp="$(mktemp -d)"
  # 1. base, as the rust arm would get it
  "$EXP/prepare.sh" "$t" rust 0 "$tmp/rust" >/dev/null
  b="$("$EXP/grade.sh" "$t" rust "$tmp/rust")"
  # 2. reference
  materialize_ref "$t" "$tmp/refsrc"
  r="$("$EXP/grade.sh" "$t" rust "$tmp/refsrc")"
  # 3. strip base; 4. reference strips and weaves back
  materialize_base "$t" "$tmp/basesrc"
  s3=ok; strip_into "$tmp/basesrc" "$tmp/bs" >/dev/null 2>&1 || s3=FAIL
  s4=ok; strip_into "$tmp/refsrc" "$tmp/rs" >/dev/null 2>&1 && "$EXP/grade.sh" "$t" bare "$tmp/rs" > "$tmp/rs.json" && [ "$(jq .pass "$tmp/rs.json")" = true ] && [ "$(jq .weaves "$tmp/rs.json")" = true ] || s4=FAIL
  bp=$(echo "$b" | jq -r '"\(.tests_passed)/\(.tests_total) hidden_compiled=\(.hidden_compiled) lib_compiles=\(.compiles)"')
  rp=$(echo "$r" | jq -r '"\(.tests_passed)/\(.tests_total)"')
  bf=$(echo "$b" | jq -r .pass); rf=$(echo "$r" | jq -r .pass)
  st=OK; { [ "$bf" = false ] && [ "$rf" = true ] && [ "$s3" = ok ] && [ "$s4" = ok ]; } || { st=PROBLEM; fail=1; }
  printf '%-34s %s  base[%s] ref[%s] strip_base=%s ref_weaves_back=%s\n' "$t" "$st" "$bp" "$rp" "$s3" "$s4"
  rm -rf "$tmp"
done
exit $fail
