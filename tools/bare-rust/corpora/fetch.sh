#!/bin/sh
# Fetch the pinned corpora crates into corpora/src/ (ignored by git).
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
dest="$here/src"
mkdir -p "$dest"

fetch() { # name url rev
  if [ ! -d "$dest/$1/.git" ]; then
    git clone --quiet "$2" "$dest/$1"
  fi
  git -C "$dest/$1" fetch --quiet origin "$3" 2>/dev/null || true
  git -C "$dest/$1" checkout --quiet --detach "$3"
  echo "$1 at $(git -C "$dest/$1" rev-parse HEAD)"
}

fetch strsim https://github.com/rapidfuzz/strsim-rs dacc84c0dc61eff0ee0ff66962bcf2e17018ad26
fetch fnv https://github.com/servo/rust-fnv 4e55052a343a4372c191141f29a17ab829cf1dbc
