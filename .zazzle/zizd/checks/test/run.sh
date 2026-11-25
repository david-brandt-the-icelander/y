# Runs `cargo test` in each Rust crate of the repository, as README.md says: the tools with --release (y-parse,
# y-compile, bare-rust, whose tests also check every example's paths against its tests, after fetching the corpora its
# tests need with its corpora/fetch.sh; its long corpora tests are the `corpora` step), then every example crate.
# The crates share one target folder, so a dependency is built once. Every crate runs even after one fails; the step
# fails if any did. Leaves nothing in /zazzle/output.
set -uo pipefail
export HOME=/tmp CARGO_HOME=/tmp/cargo npm_config_cache=/tmp/.npm CARGO_TERM_COLOR=never CARGO_TARGET_DIR=/tmp/target
src=$(ls -d /zazzle/input/repos/*/y | head -1)
rm -rf /tmp/y && cp -R "$src" /tmp/y || exit 1
cd /tmp/y
echo "$(rustc --version), node $(node --version)"
echo "== tools/bare-rust/corpora/fetch.sh (the pinned crates its corpora tests read)"
sh tools/bare-rust/corpora/fetch.sh || exit 1
failed=""
for crate in tools/y-parse tools/y-compile; do
  echo "== cargo test --release ($crate)"
  (cd "$crate" && cargo test --release) || failed="$failed $crate"
done
# bare-rust's corpora tests (tests/corpora.rs: proposal 9's checks over whole crates, an hour and more on a builder's
# CPUs) are the `corpora` step's; every other test of bare-rust runs here.
corpora_tests="corpora_checks_1_and_2_without_cargo cargo_strsim cargo_fnv cargo_itoa cargo_memchr cargo_miniz_oxide cargo_png cargo_golden cargo_tool_own_source"
skips=(); for t in $corpora_tests; do skips+=(--skip "$t"); done
echo "== cargo test --release (tools/bare-rust, without tests/corpora.rs)"
(cd tools/bare-rust && cargo test --release -- --exact "${skips[@]}") || failed="$failed tools/bare-rust"
for manifest in examples/*/Cargo.toml; do
  crate=${manifest%/Cargo.toml}
  echo "== cargo test ($crate)"
  (cd "$crate" && cargo test) || failed="$failed $crate"
done
[ -z "$failed" ] || { echo "failed:$failed" >&2; exit 1; }
echo "y's crates tested"
