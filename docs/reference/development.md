# Development

This page is for you if you want to change tryout itself. The source is at
[github.com/bmack/tryout](https://github.com/bmack/tryout).

Changes to TYPO3 Core do not go through this repository. They go through
[review.typo3.org](https://review.typo3.org); see
[contributing to TYPO3 Core](/guide/contributing-to-core).

## Repository layout

| Path | What it is |
|---|---|
| `install.yaml`, `commands/`, `tryout/`, `config.tryout.yaml`, `web-build/Dockerfile.tryout` | The add-on files. `ddev add-on get` copies them into a project's `.ddev/`. |
| `tryout/bin/` | The binaries, committed to the repository |
| `tui/` | The Rust source: `src/cli/` (commands, host and container side, completion, help), `src/core/` (the logic), `src/tui/` (the terminal UI) |
| `tests/` | The bats test suites, `ci.sh`, the small test app, and `e2e/` (browser tests with Playwright) |
| `docs/` | This site (VitePress) |

## Build

```bash
(cd tui && cargo build --release)   # a build for this machine
tui/scripts/stage-bins.sh           # all three builds into tryout/bin/ (macOS: lipo + cargo-zigbuild)
tui/scripts/build-linux.sh          # the one for a Linux CI runner
```

The binaries in `tryout/bin/` are committed. **After any change under `tui/src`,
run `stage-bins.sh` again and commit the new binaries together with your
change.** Otherwise the repository ships an old build.

To try your own build without installing it again:

```bash
TRYOUT_BIN=$PWD/tui/target/release/tryout ddev tryout status        # the host side
TRYOUT_CONTAINER_BIN=/var/www/html/<path-to-linux-build> ddev tryout status   # the container side
```

To install your checkout into a test project:

```bash
mkdir ~/tmp/tryout-test && cd ~/tmp/tryout-test
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get /path/to/tryout
ddev start
```

## Checks

```bash
cd tui && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
curl -fsSL https://ddev.com/s/addon-update-checker.sh | bash   # DDEV add-on conventions
```

The DDEV checker suggests an Apache-2.0 license. tryout is MIT-licensed, so that
one check fails on purpose.

## Tests

| Suite | Run | What it covers |
|---|---|---|
| Rust | `cd tui && cargo test` | Unit tests for each module. Generated files compared with saved copies (`tests/generators.rs`). Prompts in a real terminal. Tab completion. Screenshots of the terminal UI (insta). |
| unit | `bats tests/unit.bats` | The shell scripts, the `install.yaml` steps in both modes, and the config. One test checks that every command and flag is used in a test against real DDEV. Takes seconds. |
| install | `bats tests/test.bats --filter-tags '!release'` | Install, config, the Composer overlay, protected files, removal |
| project | `bats tests/project.bats --filter-tags 'project,!db'` | Project mode with a small test app (`tests/fixture-app.sh`): install leaves it alone, two served worktrees, database copies, `--pr` |
| frameworks | `bats tests/project.bats --filter-tags project,laravel` (also `symfony`, `drupal`, `wordpress`, `typo3`, `shopware`) | Installs one framework with its own tools, and checks that each worktree uses its own database |
| lifecycle | `bats tests/lifecycle.bats` | Clones TYPO3 Core and runs every command from start to end, including sites on Postgres, MySQL and SQLite |
| e2e | `cd tests/e2e && TRYOUT_PROJECT=… npx playwright test` | Frontend and backend login in a real browser. `project.spec.ts` uses `TRYOUT_APP_PROJECT`. |

Install the bats helpers once:

```bash
brew tap bats-core/bats-core
brew install bats-core bats-assert bats-file bats-support
```

Good to know about the suites:

- The lifecycle suite skips its URL checks when `*.ddev.site` does not resolve on
  your machine.
- The test that serves a site through `/etc/hosts` needs `sudo` without a
  password, as CI has.
- The project suite loads pages from inside the web container and sends the
  `X-Forwarded-Proto` header like DDEV's router does. So it needs no DNS.
- To see more while debugging, add `--show-output-of-passing-tests --verbose-run
  --print-output-on-failure`.

`bash tests/ci.sh <suite>` runs a suite the same way CI does. The suites are
`unit`, `install`, `project`, `project-<framework>`, `lifecycle` and `release`.

## CI

| Workflow | Jobs |
|---|---|
| `tests.yml` | On every push and pull request: `unit`, `install` and `project`, on DDEV stable and HEAD. Every night, when started by hand, or on a pull request with the label **full-ci**: also `lifecycle` (stable, HEAD), `frameworks` (laravel, symfony, drupal, wordpress, typo3, shopware), `browser e2e` and `install-from-release`. `lifecycle` runs as four parallel shards per DDEV version (each journey in `tests/lifecycle.bats` is tagged `shard1`–`shard4`, balanced by duration; `bash tests/ci.sh lifecycle-shard2` runs one locally). A `build` job compiles the Linux binary once (with a build cache); every job that installs the add-on downloads it into `tryout/bin/` instead of compiling it again. |
| `tui.yml` | `cargo fmt`, `clippy` and the Rust tests on Linux and macOS, when something under `tui/` changes |
| `publish.yml` | Builds this documentation and publishes it on GitHub Pages |
| `release.yml` | Started by hand: builds the three binaries, commits them on a separate release commit, tags that commit and publishes the release |

## Documentation

```bash
cd docs
npm ci
npm run docs:dev       # live preview
npm run docs:build     # what CI builds; fails on broken links
npm run docs:preview   # serve the build
```

`publish.yml` publishes the site to
[bmack.github.io/tryout](https://bmack.github.io/tryout/). A test in
`tui/src/core/types.rs` checks that the support table on the
[frameworks](/frameworks/) page matches the code.

## Releasing

1. Raise the number in `tryout/VERSION` whenever something a user sees changes:
   a command, a flag or a completion. The binary contains the same number, and
   `status` compares it with the number an install wrote down. That is how users
   learn that their copy is old.
2. Start the **release** workflow from the Actions tab and give it the tag, for
   example `v1.4.0`. It builds all three binaries from scratch, commits them on a
   separate release commit, tags that commit, and adds the binaries and their
   SHA-256 checksums to the GitHub release.

`ddev add-on get bmack/tryout` installs the latest release's tagged tree.
