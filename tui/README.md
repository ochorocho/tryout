# tryout-tui

The terminal UI behind `ddev tryout ui`: the Core worktrees on the left, a live
shell in the selected one on the right — one per worktree, kept running while you
look at another — and every tryout command for that worktree behind `a`, run in
a popup terminal so its prompts work.

It is **not shipped** by `ddev add-on get` — `install.yaml` lists its payload
explicitly and `tui/` is not in it. Users get a release binary, fetched by
`ddev tryout ui` on first use (see *Releasing*).

```bash
cargo run -- /path/to/tryout-project     # or run it from inside the project
TRYOUT_TUI_BIN=$PWD/target/release/tryout-tui ddev tryout ui   # through the add-on
```

| Key (list) | | Key (shell) | |
|---|---|---|---|
| `↑` `↓` / `j` `k` | select a worktree | `Ctrl-G` | back to the list |
| `Enter` | open / focus its shell | anything else | goes to the shell |
| `a` / `Space` | the commands for it | `Enter` on an exited shell | start a new one |
| `r` / `q` | reload / quit | | |

In a popup every key belongs to the command until it ends (Esc included — that
is how a gum prompt is cancelled). It then closes by itself on success, unless
its output is the point (`status`, `exec`); a failure always stays, with a red
border and its exit code, until Enter.

## How it talks to tryout

It never re-implements tryout. The list is `ddev tryout worktree list --json`,
the add-on's machine-readable contract (pinned by the add-on's unit suite), run
off the UI thread. Commands are `ddev tryout <verb> …` on a PTY at the project
root. `launch` is the one that runs without a popup — it raises the browser.

The command menu follows the removed herdr panel's rules (`actions.rs`): a row is
only offered where it can work, and every command names its own worktree — a bare
verb acts on whichever Core is primary when it runs, not the one you selected.

## Layout

- `worktrees.rs` — parse `--json`, find the project
- `actions.rs` — which commands a worktree is offered, and how each runs
- `pane.rs` — a program on a real PTY (`portable-pty`), screen emulated by `vt100`,
  exit code kept
- `keys.rs` — key events to the bytes a terminal sends
- `app.rs` — state and key handling; no terminal I/O, so all of it is unit-tested
- `ui.rs` — layout and theme (`ratatui`, panes drawn by `tui-term`)

Every style names its own foreground colour; a test fails on any drawn cell that
inherits the terminal's. (A bare modifier inherits the theme — the herdr panel
once drew black on black that way.)

crossterm's `use-dev-tty` feature is load-bearing: `ddev tryout ui` hands the TUI
`/dev/tty`, which kqueue cannot poll on macOS.

## Tests

```bash
cargo test                                  # unit, real-PTY and snapshot tests
cargo clippy --all-targets -- -D warnings   # as CI runs it (.github/workflows/tui.yml)
cargo insta review                          # after an intended layout change
```

## Portable builds

One codebase, one self-contained file per platform. Locally this needs the rustup
targets `x86_64-apple-darwin`, `x86_64-unknown-linux-musl`,
`aarch64-unknown-linux-musl`, plus `zig` and `cargo-zigbuild`
(`cargo install --locked cargo-zigbuild`).

```bash
cargo build --release                                           # macOS arm64
cargo zigbuild --release --target x86_64-apple-darwin
cargo zigbuild --release --target x86_64-unknown-linux-musl     # static ELF
cargo zigbuild --release --target aarch64-unknown-linux-musl    # static ELF
lipo -create -output tryout-tui-macos-universal \
  target/release/tryout-tui target/x86_64-apple-darwin/release/tryout-tui
```

Verified: the universal file runs natively on arm64 and under Rosetta; the arm64
ELF runs unchanged in DDEV's (Debian) web container, the x86_64 one in Alpine.
Sizes are ~2.5 MB (universal) and ~1 MB per Linux binary.

## Releasing

`.github/workflows/tui-release.yml` builds the three assets
(`tryout-tui-macos-universal`, `tryout-tui-linux-x86_64`,
`tryout-tui-linux-aarch64`), a `.sha256` beside each, and publishes them as a
GitHub release on a `tui-v<version>` tag. `fetch_tui` in `tryout/functions.sh`
downloads from exactly there and refuses a file whose checksum does not match.

To release: bump `version` in `Cargo.toml` **and** `TRYOUT_TUI_VERSION` in
`tryout/functions.sh` (a unit test fails when they differ), then push the tag
`tui-v<version>`.
