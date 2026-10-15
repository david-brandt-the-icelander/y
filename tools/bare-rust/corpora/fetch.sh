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
# Crates with the forms the others lack: pointers (itoa, memchr), constants computed by a cast (memchr, miniz_oxide),
# and suffixed literals in patterns (png).
fetch itoa https://github.com/dtolnay/itoa 1577ed901354d0d7448ac162328f9dbf5183124c
fetch memchr https://github.com/BurntSushi/memchr bd6068c30e9074a90c285e47912fa0b047d07597
fetch miniz_oxide https://github.com/Frommi/miniz_oxide cdd7ddd56cf57a6255203bb9f5c6a92bf1144bbf
fetch png https://github.com/image-rs/image-png b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc
