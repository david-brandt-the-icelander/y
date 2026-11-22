#!/bin/bash
# grade.sh <task> <arm> <workspace>  -> one JSON line (compiles, hidden tests, constraint-file entries changed, lines changed)
set -u
exec python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/grade.py" "$@"
