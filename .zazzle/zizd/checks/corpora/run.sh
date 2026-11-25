# Runs bare-rust's corpora tests, tools/bare-rust/tests/corpora.rs, after fetching the pinned crates they read with
# tools/bare-rust/corpora/fetch.sh: strip and weave over the tool's own source, strsim, fnv, itoa, memchr, miniz_oxide
# and png, and the golden file, each checked with cargo and rustc (proposal 9). They are apart from the `test` step
# because they take an hour and more on a builder's CPUs. Leaves nothing in /zazzle/output.
set -uo pipefail
export HOME=/tmp CARGO_HOME=/tmp/cargo npm_config_cache=/tmp/.npm CARGO_TERM_COLOR=never
src=$(ls -d /zazzle/input/repos/*/y | head -1)
rm -rf /tmp/y && cp -R "$src" /tmp/y || exit 1
cd /tmp/y/tools/bare-rust
echo "$(rustc --version), node $(node --version)"
sh corpora/fetch.sh || exit 1
cargo test --release --test corpora || exit 1
echo "bare-rust's corpora checked"
