# tryout

All of `ddev tryout`, in one program: the command line on the host (`tryout
<verb>`, what the `ddev tryout` shim runs), its far end in the web container
(`tryout ctr <verb>`), the post-start hook (`__post-start`), the PHP-FPM daemons
(`__fpm`), tab completion (`__complete`) — and the terminal UI, `tryout ui`.

- `src/core/` — the add-on's logic: worktrees, sites, Composer overlays, vhosts,
  Gerrit, patches, contribution setup, prompts, output.
- `src/cli/` — argument parsing and the verbs, host and container side.
- `src/tui/` — the terminal UI and its session server.

The terminal UI: the Core worktrees on the left, and on
the right that worktree's tabs — as many shells as you open, kept running while
you look at another worktree. Below the worktrees, an agents pane lists every tab
running a coding agent and whether it needs you. Every tryout command for a
worktree is behind `a` or a right-click: the TUI asks its questions itself — a
form, a branch list, a list of open Gerrit changes, a confirmation that names
what goes — and runs it as a job in the **Activity** panel.

The source in `tui/` is not shipped; its builds are, in `tryout/bin/` of a
release (see *Releasing*).

```bash
cargo test                                            # everything, in seconds
TRYOUT_BIN=$PWD/target/release/tryout ddev tryout ui  # a local build, through the add-on
scripts/stage-bins.sh                                 # all three builds into ../tryout/bin/
```

In a shell every key is the shell's except **Ctrl-G**, which returns to the
list — so from a shell, *Ctrl-G then a key* runs any list command below.

| Key (list) | | Mouse | |
|---|---|---|---|
| `↑` `↓` / `j` `k` | select a worktree | click a worktree | select it |
| `Enter` | focus its active tab (opening one if none) | click a tab | switch to it |
| `t` | new shell tab | click `+` | new shell tab |
| `1`–`9`, `[` `]` | switch tab | click the terminal | focus it |
| `w` | close the active tab | click an agent | jump to its tab |
| `,` | rename the active tab | double-click a tab | rename it |
| `<` `>` | move the active tab left / right | drag a tab onto another | move it there |
| `+` | new worktree (`worktree add`) | click **+ new** above the list | new worktree |
| | | right-click a worktree | its `worktree` commands: serve/unserve, use, rename, remove |
| | | click the URL in the title | open the site (as `launch`) |
| `n` | next agent — waiting ones first | | |
| `L` | the newest job's log | click an Activity row | its log |
| `↑` `↓` `PgUp` `PgDn` in a log | scroll it | mouse wheel over it | scroll it |
| `r` in a finished job's log | run the command again | click `↻ retry` in its border | same |
| `R` | run the command that just failed again | click the `↻` on a failed Activity row | same |
| `{` `}` | sidebar narrower / wider | drag the border between list and pane | resize; double-click resets |
| `a` / `Space` | the commands for this worktree | | |
| `r` | reload the list | | |
| `q` | detach (the session keeps running) | | |
| `Q` | close the session (asks y/n) | | |

A tab whose program exits closes itself; closing a worktree's last tab brings its
details back. Tabs name themselves after the title their program sets (zsh's
`user@host`, an agent's task), falling back to `shell`, until you name one: the
rename prompt opens with the current name selected, so typing replaces it (an
arrow key keeps it for editing, Ctrl-U clears), and an empty name hands the tab
back to its program's title. A name you give also shows in the agents pane. With mouse reporting on,
the terminal's own text selection needs a modifier: Option-drag in iTerm2 and
Terminal.app, Shift-drag in most Linux terminals.

### The agents pane

Once a second the TUI asks `ps` what each tab's foreground process is
(`agents.rs`). One whose executable is `claude`, `codex`, `gemini`, `aider`,
`opencode` or `amp` — or an npm install of one, `node …/claude-code/…` — is an
agent, and gets a row: `◐` **working** (output in the last two seconds), `●`
**waiting** (it rang the bell, or went quiet after working while you were looking
elsewhere), `○` **idle**. The tab on screen never waits for you: looking at it is
the answer. Claude rings the bell on permission prompts when its notification
channel is the terminal bell (`/config` → notifications).

### Commands: forms and jobs

A command never takes over the screen. What it needs to know is asked by a
native form (`forms.rs`): text with the add-on's own validation (a worktree
name that is taken or malformed is refused before anything runs), a filterable
pick list (branches: main, then releases newest first), ticks (Gerrit changes),
or — for anything destructive — a confirmation that says exactly what goes and
runs only on an explicit `y`. The answers become one fully-argued `ddev tryout …`
command, queued as a **job** (`jobs.rs`).

Jobs run one at a time (a second waits, visibly), with stdin closed: a prompt
nobody expected fails fast instead of hanging the queue. Their progress comes
from the add-on's event lines — with `TRYOUT_EVENTS=1` every `info`/`success`/
`warn`/`error` also prints `@@tryout {"level":…,"msg":…}` — so the **Activity**
row shows the current step (`⠹ PHP 8.4 · Installing dependencies…`), then
`✓ 13s` or `✗ exit 1` with the error in the footer. `Enter`/click on a row, or
`L`, opens its log in the right pane (colours kept, PgUp/PgDn scroll). A job
whose output is the answer (`status`, Run command…) opens its log by itself. A
job that changes worktrees reloads the list. Jobs live in the session server,
so they finish while you are detached.

## Sessions

Everything runs in a **session server**, one per project, that outlives the
terminal — as herdr and tmux do. `ddev tryout ui` attaches to the project's
session, starting it when none is running. **`q` detaches**: the shells, agents
and any running job carry on, and the next `ddev tryout ui` finds them — and
the UI — exactly as you left them. Closing the terminal detaches the same way.

The session ends only when asked: **`Q`** (it asks y/n and says how many tabs and
agents that ends) or **`ddev tryout ui stop`**. One terminal is attached at a
time; attaching from a second one takes the session over, and the first is told
why. Running `ddev tryout ui` inside one of the session's own tabs is refused —
it would draw the session within itself.

How it works (`src/session/`): the server renders and the client paints —
tmux's model. The server runs the whole app and draws it into `WireBackend`, a
ratatui backend with no terminal behind it; ratatui's own diffing makes each
frame just the cells that changed, sent over a Unix socket as length-prefixed
postcard. The client forwards crossterm events and paints frames; it holds no
state, which is what lets it come and go. The socket is per user and per project
in `$TMPDIR/tryout-tui-<uid>/` (short on purpose: macOS caps a socket path at 104
bytes), named from a stable FNV hash of the project path; the server's output
goes to a `.log` beside it. A client and server of different protocol versions
refuse each other with the way out — a server outlives an add-on update.

## How it talks to tryout

It never re-implements tryout. The list is `ddev tryout worktree list --json`,
the add-on's machine-readable contract (pinned by the add-on's unit suite), run
off the UI thread. The pickers read `worktree branches --json` and
`patch --list --json`; commands run as `ddev tryout <verb> …` jobs at the
project root. `launch` is the one that skips the queue — it raises the browser.

The command menu follows the removed herdr panel's rules (`actions.rs`): a row is
only offered where it can work, and every command names its own worktree — a bare
verb acts on whichever Core is primary when it runs, not the one you selected.

## Layout

- `worktrees.rs` — parse `--json`, find the project
- `agents.rs` — which process is an agent, and its working/waiting/idle state
- `actions.rs` — which commands a worktree is offered, and how each runs
- `forms.rs` — the native questions: text, pick list, ticks, confirmation
- `jobs.rs` — the one-at-a-time queue, the event parser, logs
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
lipo -create -output tryout-macos-universal \
  target/release/tryout target/x86_64-apple-darwin/release/tryout
```

Verified: the universal file runs natively on arm64 and under Rosetta; the arm64
ELF runs unchanged in DDEV's (Debian) web container, the x86_64 one in Alpine.
Sizes are ~2.5 MB (universal) and ~1 MB per Linux binary.

## Releasing

`.github/workflows/release.yml`, run from the Actions tab with a version, builds
`tryout-macos-universal`, `tryout-linux-x86_64` and `tryout-linux-aarch64`,
checks each carries `#ddev-generated` (DDEV manages only files that do), commits
them into `tryout/bin/` on a release-only commit, tags that commit and publishes
the release with the binaries and their `.sha256` attached. `ddev add-on get`
installs the tagged tree, so the binaries have to be in it; main stays free of
them (`tryout/bin/` is gitignored).

Bump `tryout/VERSION` whenever what a user sees changes: `ddev tryout status`
compares it with the stamp an install recorded.
