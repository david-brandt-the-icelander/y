# The `test` step of y's `checks` pipeline: `cargo test` of every Rust crate in tools/ and examples/ (README.md).
# Rust is 1.97.0, the version tools/bare-rust/libraries/ describes (std-1.97.0 and host-rust-1.97.0: the library checks
# read the file of the running rustc); Node is pinned. Debian 13 (trixie), because the tree-sitter-cli binary that npm
# installs needs glibc 2.39 or newer. Node runs tree-sitter for bare-rust's generated parser (tools/bare-rust/build.rs);
# python3 the path tools some tests call; rustfmt and rust-src bare-rust's tests (they format what they weave and read
# the standard library's source); libasound2-dev with pkg-config lets the Macroquad examples build (their tests need no
# window). The extra targets are the ones tools/bare-rust/corpora/fetch.sh adds for the corpora's gated code; the
# toolchain is read-only when the step runs, so they are added here.
FROM node:26.13.1-bookworm-slim AS node
FROM rust:1.97.0-trixie
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
 && rustup component add rustfmt rust-src \
 && rustup target add aarch64-unknown-linux-gnu i686-unknown-linux-gnu s390x-unknown-linux-gnu x86_64-unknown-linux-gnu \
      wasm32-unknown-unknown aarch64-apple-darwin x86_64-pc-windows-msvc thumbv6m-none-eabi \
 && apt-get update && apt-get install -y --no-install-recommends python3 pkg-config libasound2-dev \
 && rm -rf /var/lib/apt/lists/*
COPY run.sh /run.sh
CMD ["bash", "/run.sh"]
