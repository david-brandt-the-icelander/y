# `.zazzle/zizd/`: y's zizd pipelines

The steps zazzle's builders and deployers run for this repository, written against zazzle's step-pipeline spec
(`src/macos/zazzle-pipeline/pipeline-spec.md`, sections 1–4). Two pipelines, so each step is a folder
`.zazzle/zizd/PIPELINE/STEP/` with a `Dockerfile` and a `run.sh`. The pipeline files themselves live in the
orchestrator (`continuous_integration_and_delivery`, `.zazzle/zizd/pipelines/david-brandt-the-icelander/y/PIPELINE.json`);
the copies below are what goes there.

## `checks`: the validators and the tests

| Step | Phase | Image | What it does | Leaves in `/zazzle/output` |
|---|---|---|---|---|
| `check` | build | `python:3.13.9-slim`, jsonschema 4.26.0 | both `.dev` validators, as `AGENTS.md` says | nothing |
| `test` | build | `rust:1.97.0-trixie` with rustfmt, rust-src and the cross targets of `corpora/fetch.sh`, Node 26.13.1 (tree-sitter, for bare-rust's generated parser), python3, libasound2-dev | `tools/bare-rust/corpora/fetch.sh`, then `cargo test --release` in `tools/y-parse`, `tools/y-compile` and `tools/bare-rust` (whose tests also check every example's paths against its tests) except bare-rust's `tests/corpora.rs`, then `cargo test` in every crate of `examples/`, as `README.md` says; one shared target folder | nothing |
| `corpora` | build | the `test` image | `tools/bare-rust/corpora/fetch.sh`, then bare-rust's `tests/corpora.rs` (proposal 9's checks over the tool's own source, the golden file and six pinned crates, each built with cargo) | nothing |

```json
{
  "id": "david-brandt-the-icelander/y:checks",
  "repos": ["https://github.com/david-brandt-the-icelander/y.git"],
  "paths": ["tools/**", "examples/**", "Meta/**", "**/.dev/**", ".zazzle/zizd/checks/**"],
  "steps": [
    { "name": "check", "path": ".zazzle/zizd/checks/check" },
    { "name": "test", "path": ".zazzle/zizd/checks/test" },
    { "name": "corpora", "path": ".zazzle/zizd/checks/corpora" }
  ],
  "supersede": true
}
```

**Rust 1.97.0, on purpose.** bare-rust's library checks read the description of the standard library for the running
`rustc` (`tools/bare-rust/libraries/std-1.97.0.constraints.json`, `host-rust-1.97.0.json`); with any other version
they find nothing and three tests fail. Move the image's Rust when those files move.

**`corpora` is its own step** because it is slow: 2744 s on a builder slot of 3 CPUs, close to the default
`builder.timeout` of 3600 s. With the default 2 CPUs it may pass that, so builders taking this pipeline want a
longer `builder.timeout`.

Not in it: `tools/paths/trace.py`, which `examples/README.md` asks for after changing an example. It is a check for the
person changing an example (it instruments the tests and reports a test that does not take the path it claims), and
the paths the tests claim are already checked by bare-rust's tests.

## `pages`: the documentation site

What `.github/workflows/pages.yml` does today, as two steps:

| Step | Phase | Image | What it does | Leaves in `/zazzle/output` |
|---|---|---|---|---|
| `site` | build | `ghcr.io/actions/jekyll-build-pages:v1.0.13` (the image of `actions/jekyll-build-pages@v1`: the github-pages gem 232, Jekyll 3.10) plus python3; **linux/amd64 only** | `.github/pages/stage.py`, then `github-pages build`, as the workflow's build job | `site.tar.gz` (the built `_site`, 37 files), `BUILD` (the commit) |
| `deploy` | deploy (`deploys: true`) | `python:3.13.9-slim` with git | from `/zazzle/input/artifacts/site/` only: one commit whose tree is the site plus `.nojekyll`, pushed to the branch `gh-pages` (nothing when the site did not change; never forced). Never rebuilds. | nothing |

```json
{
  "id": "david-brandt-the-icelander/y:pages",
  "repos": ["https://github.com/david-brandt-the-icelander/y.git"],
  "paths": ["documentation/**", "Y-current-language-design.md", ".github/pages/**", ".zazzle/zizd/pages/**"],
  "steps": [
    { "name": "site", "path": ".zazzle/zizd/pages/site", "platform": "linux/amd64" },
    { "name": "deploy", "path": ".zazzle/zizd/pages/deploy", "config": ["Y_PAGES_GITHUB_TOKEN"], "deploys": true }
  ],
  "supersede": true,
  "halt": { "deployment": "should" }
}
```

`paths` are the workflow's, with this folder in place of the workflow file. No `environments`: the site is published
from `main` only, as today.

**Why a branch, not the Pages API.** The workflow's deploy job (`actions/deploy-pages`) hands GitHub an artifact that an
Actions run uploaded; outside Actions there is no such artifact to hand it. So the deploy step publishes the built site
to `gh-pages`, and Pages serves that branch. The deploy step:

- refuses without `Y_PAGES_GITHUB_TOKEN` or without the site's artifacts;
- passes the token to git through a credential helper that reads it from the environment, never on a command line;
- checks `ZAZZLE_HALT_FILE` before preparing the commit and before the push, and exits **75** when asked to halt;
- with `DEPLOY_DRY_RUN=1` (by hand only; no pipeline names it) needs no token, fetches and pushes nothing, and shows
  the commit it would push.

## Config keys

Names only; the values are in the orchestrator's config files (later zeecrets), never here.

| Key | Step | Where it goes |
|---|---|---|
| `Y_PAGES_GITHUB_TOKEN` | `pages` / `deploy` | `deploy_keys`; a token that may push to `david-brandt-the-icelander/y` (a fine-grained token with Contents: read and write on this repository only) |

The other steps need no key: the repository is public. The builder's own `GITHUB_TOKEN` and z3 keys (`shared.json`)
are zazzle's and reach no step.

## Trying it by hand

Each step was run with the spec's "Trying a step by hand" recipe, `site`'s output copied into `deploy`'s
`/zazzle/input/artifacts/site/`. `check` and `site` were green; `deploy` refused without the token, made its commit
in a dry run (38 files, the site and `.nojekyll`) and pushed nothing, and exited 75 with a halt file in place. `test`
was green on a builder slot (3 CPUs) in 509 s: 921 tests in the three tools and twelve examples. `corpora` was
green in 2744 s: 9 of 9.

## What goes at cutover

Not removed yet: `.github/workflows/pages.yml`. At cutover, in this order: let the `pages` pipeline publish
`gh-pages` once, switch the repository's Pages source (Settings, Pages) from "GitHub Actions" to the branch `gh-pages`,
folder `/`, check the site, then remove `pages.yml` (and keep `.github/pages/`, which the `site` step uses). Until the
switch, a push to `gh-pages` changes nothing that is served. There is no legacy runner for this repository in
`continuous_integration_and_delivery`.
