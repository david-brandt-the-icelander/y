# Sourced by prepare.sh and grade.sh.
EXP="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export BARE_RUST="${BARE_RUST:-/tmp/claude-0/-home-user-y/9673def1-2027-5ffe-9d6e-8413dc645c64/scratchpad/bin/bare-rust}"
LIBFILE="${LIBFILE:-$EXP/../libraries/std-1.97.0.constraints.json}"

# materialize_base <task> <dir>: the base crate source of a task, in <dir> (created)
materialize_base() {
  local task="$1" dir="$2" crate
  crate="$(jq -r .crate "$EXP/tasks/$task/task.json")"
  mkdir -p "$dir"
  cp -r "$EXP/crates/$crate/." "$dir/"
  (cd "$dir" && patch -s -p1 < "$EXP/tasks/$task/base.patch")
}
# materialize_ref <task> <dir>: the reference solution's source
materialize_ref() {
  local task="$1" dir="$2" crate
  crate="$(jq -r .crate "$EXP/tasks/$task/task.json")"
  mkdir -p "$dir"
  cp -r "$EXP/crates/$crate/." "$dir/"
  if [ -s "$EXP/tasks/$task/ref.patch" ]; then (cd "$dir" && patch -s -p1 < "$EXP/tasks/$task/ref.patch"); fi
}
# strip_into <crate-src-dir> <dest-dir> [strip options]: .brs/.constraints.json of the crate, plus Cargo.toml and every non-.rs file
strip_into() { # [strip options...]
  local src="$1" dest="$2" tmp
  shift 2
  tmp="$(mktemp -d)"
  "$BARE_RUST" strip "$src" --out "$tmp/strip" "$@" >/dev/null
  mkdir -p "$dest"
  (cd "$src" && find . -type f ! -name '*.rs' ! -path './target/*' ! -path './.git/*' -print0 | xargs -0 -I{} cp --parents {} "$dest/")
  (cd "$tmp/strip" && find . -type f ! -name refusals.json -print0 | xargs -0 -I{} cp --parents {} "$dest/")
  rm -rf "$tmp"
}
