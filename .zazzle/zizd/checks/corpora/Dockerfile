# The `corpora` step of y's `checks` pipeline: bare-rust's corpora tests (tools/bare-rust/tests/corpora.rs), proposal
# 9's checks over whole crates, with Rust 1.97.0 as in `test`. The image is the `test` step's (the same lines, so Docker
# builds it once): Debian 13 for the tree-sitter-cli binary (glibc 2.39), Node for tree-sitter, rustfmt and rust-src,
# and the targets corpora/fetch.sh adds for the corpora's gated code.
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
