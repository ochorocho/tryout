# tryout-tui (spike)

A terminal workspace for a ddev tryout project: the Core worktrees on the left, a
live shell in the selected one on the right. One shell per worktree, kept alive
while you look at another. This is a spike: it proves embedded terminals and
portable builds before anything product-shaped is built on them.

It is **not shipped** by `ddev add-on get` — `install.yaml` lists its payload
explicitly and `tui/` is not in it.

```bash
cargo run -- /path/to/tryout-project     # or run it from inside the project
```

| Key (list) | | Key (shell) | |
|---|---|---|---|
| `↑` `↓` / `j` `k` | select a worktree | `Ctrl-G` | back to the list |
| `Enter` | open / focus its shell | anything else | goes to the shell |
| `r` | reload the list | `Enter` on an exited shell | start a new one |
| `q` | quit | | |

## How it talks to tryout

It never re-implements tryout: the list is `ddev tryout worktree list --plain`, the
add-on's machine-readable contract, run off the UI thread. A worktree's directory
is `worktrees/<name>`, or the project root for the root checkout.

## Layout

- `worktrees.rs` — parse `--plain`, find the project, map names to directories
- `pane.rs` — a program on a real PTY (`portable-pty`), its screen emulated by `vt100`
- `keys.rs` — key events to the bytes a terminal sends
- `app.rs` — state and key handling; no terminal I/O, so all of it is unit-tested
- `ui.rs` — layout and theme (`ratatui`, panes drawn by `tui-term`)

Every style names its own foreground colour; a test fails on any drawn cell that
inherits the terminal's. (A bare modifier inherits the theme — the herdr panel
once drew black on black that way.)

## Tests

```bash
cargo test            # unit tests, two real-PTY tests, layout snapshots (insta)
cargo insta review    # after an intended layout change
```

## Portable builds

One codebase, one static file per platform. Needs the rustup targets
`x86_64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
plus `zig` and `cargo-zigbuild` (`cargo install --locked cargo-zigbuild`).

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

## Next

- A `--json` contract for `worktree list` and `status` in the add-on, replacing
  the `--plain` scrape.
- Release pipeline (`dist`), and a `post_install_action` that fetches the right
  binary by `uname -s`/`-m`, checksum-verified, launched by `ddev tryout ui`.
- Running tryout verbs from the TUI (popups that close on success, stay on failure).
