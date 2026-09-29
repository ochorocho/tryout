# Port `ddev tryout` from bash to the Rust binary

Full plan: `~/.claude/plans/ddev-tryout-worktree-list-cuddly-sky.md`. Everything
lands on this branch; the bash stays runnable until Stage 8 so
`tests/parity/run.sh` can diff the two implementations. Delete this file when
Stage 8 is done.

## Stage 0: Restructure
**Goal**: one crate `tryout` (lib + bin), TUI under `src/tui/`, launcher, marker, local build of all three binaries, parity harness skeleton.
**Success Criteria**: `tryout ui` works as before; `tryout/tryout __version` runs on macOS, linux/amd64, linux/arm64; each binary greps `#ddev-generated`; `PARITY_SELFTEST=1 tests/parity/run.sh` passes.
**Tests**: existing 126 TUI tests; launcher run in debian containers.
**Status**: Complete

## Stage 1: Foundations
**Goal**: `core::{ctx, out, git, vsort, phpjson, php, site, worktree}` — paths, branch resolution, output + event twin, sort -V, PHP-compatible JSON, PHP constraints, site/worktree naming.
**Success Criteria**: pure-logic unit.bats cases have Rust twins.
**Tests**: branch fallback / TRYOUT_BRANCH, site dirs, db names, hostnames, name validation, name_for_path, detached base, dirty on non-checkout, sort -V corpus, picker order, JSON escaping, event twin, PHP constraints vs PHP's own verdicts, PHP pretty-print byte parity.
**Status**: Complete

## Stage 2: Read-only verbs and completion
**Goal**: verb table, `Argv` parser, `ctr` entry, `help`, `status`, `worktree list` (cards/--plain/--json), `worktree branches`, `patch --list` (ureq), `launch`, `__complete`; TUI loads in-process.
**Success Criteria**: parity cases for each verb pass; completion corpus identical.
**Tests**: parity cases; ported completion tests (unit.bats L284–400, L608–812).
**Status**: Complete — 28 parity cases (help, status, launch, worktree list/branches, patch --list, completion corpus); the TUI reads worktrees, branches and Gerrit in-process (listing runs per worktree in parallel, ~1.2s warm for 8); jobs still run `ddev tryout …` until Stage 6.

## Stage 3: Generators
**Goal**: composer overlays (sync/site/use-core), vhosts (nginx/apache), hash conf, config.worktrees.yaml (`__fpm`).
**Success Criteria**: byte parity with the PHP/bash output (whitelist: `[]`→`{}`).
**Tests**: unit.bats L454, L2290, L2342, L2365, L3580, L3613, L3667 as Rust tests.
**Status**: Complete — `tui/tests/generators.rs` compares every generator byte for byte with goldens captured from the bash/PHP by `tests/parity/goldens.sh` (the one intended change: the FPM daemon runs `tryout __fpm`); `composer` is ported with 3 parity cases.

## Stage 4: Prompts
**Goal**: `prompt.rs` (inquire + no-TTY fallbacks) replacing gum.
**Success Criteria**: every ui_*/ask_* behaviour the bats suite pins holds.
**Tests**: piped stdin; real PTY via portable-pty.
**Status**: Complete — inquire on stderr when stdin+stderr are terminals (select, multi-select, text, confirm defaulting to No), the plain-line fallbacks otherwise, a stderr-only spinner; ask_site/worktree/branch/text, ask_new_worktree_branch (kept/fallback/cancel), pick/describe_patches. `tests/prompts.rs` drives Enter, arrow-down and ESC on a real PTY, plus the piped and no-answer paths. Confirm on a PTY is tested with remove/delete in Stage 6.

## Stage 5: Mutating container verbs
**Goal**: worktree add/use/remove/rename, checkout, download, reset, composer, patch, exec, delete, serve/unserve, site setup, db, fpm.
**Success Criteria**: call-log + tree parity for each.
**Tests**: ported unit.bats cases listed in the plan.
**Status**: Complete — 43 new parity cases (74 in all) covering add/serve/unserve/use/remove/rename, download, checkout, reset, patch (apply, twice, merged, unknown, TRYOUT_PATCHES list), exec and delete, both sides; the host halves came along (confirmations, restart on hostname change, patch picker + persist). One intended fix: a failed `patch <id>` exits 1 (bash exited 0). Verified live in the scratch project through `ddev exec --raw` with the Linux binary: status, list, a --php switch without restart (FPM 8.4 started by `tryout __fpm`, site 200), unserve/serve round trip keeping the database.

## Stage 6: Host orchestration
**Goal**: delegate (`ddev exec --raw`), Mutagen flush, restart-on-hostname-change, confirmations, patch picker + persist, cs, `ui` on /dev/tty.
**Success Criteria**: `lifecycle.bats` green with `TRYOUT_IMPL=rust`.
**Status**: Complete — `cs setup/doctor/uninstall` ported (Gerrit account lookup, SSH diagnosis by TCP connect + ssh, host report), 9 more parity cases (83 in all). `TRYOUT_IMPL=rust` in the bash host command hands verbs to the launcher; lifecycle.bats copies the staged binaries in. Lifecycle against Rust: 8/9 on the first run; the ninth (checkout) failed identically on bash — a stale `ddev exec vendor/bin/typo3` from before TYPO3-Instances/, fixed to `ddev tryout exec @primary`.
Also: TUI jobs run on a pseudo-terminal (echo off) so DDEV's `sudo` can ask; a password prompt opens a masked popup that answers or cancels it.

## Stage 7: Container lifecycle
**Goal**: `__post-start`, `__fpm`, php-versions snapshot.
**Success Criteria**: full lifecycle matrix, `tests/e2e`, both webserver types.
**Status**: Complete — `tryout __post-start` ported (5 steps + `.ddev/tryout/.state/php-versions` for the host), 4 parity cases; `__fpm` live since Stage 5. With the `.impl-rust` marker the whole lifecycle suite ran on Rust alone (hook, verbs, FPM): 12/13 green, the /etc/hosts test skipped locally (needs passwordless sudo; runs in CI).

## Stage 8: Switch-over
**Goal**: delete the bash/PHP, shims + launcher, install.yaml, release workflow commits binaries, docs.
**Success Criteria**: test.bats + lifecycle.bats with shims only; conformance checker on the release commit.
**Status**: Not Started

## Load-bearing behaviours (tick when a Rust or bats test covers it)

- [ ] Core cloned into the root on the host at install (init + fetch + checkout, never clone); only `.git/info/exclude` touched
- [ ] `CORE_GIT_DIR` via `--git-common-dir` when `.git` is a file
- [ ] `worktree.useRelativePaths` + `worktree repair` after clone/migrate/add; refuse git < 2.48
- [x] worktrees always `add --detach`; download refuses detached in update mode; reset/download never check out the base
- [x] remove always asks (names the dir), `--yes` skips; `--force` always, prune before sweep; `branch -d`/`-D`; active and main refused
- [x] rename via `worktree move`, re-serves keeping the DB
- [x] `use` rewrites only the primary overlay's sysext repo; never `ln -sfn`; rebuild mandatory
- [x] dirty / change counts on a non-checkout = clean
- [x] only `worktree list` pays for the dirty check; completion never runs git status or the network
- [x] `--plain` columns and `--json` keys unchanged (add-only)
- [x] sync-composer: sysexts on disk only, `@dev`, non-managed entries kept, lock unlinked
- [x] site overlay; `wipe_site_vendor` before a Core switch; PHP check before every composer install
- [x] serve: hostname snapshot before marker; marker removed on failure; relative additional.php link; FPM master before reload
- [x] vhost: literal `fastcgi_param HTTPS $fcgi_https;`, TYPO3_DB_DBNAME, TRYOUT_SITE; apache SetEnvIf; socket paths
- [x] server_names_hash_bucket_size sizing, own http-level file, removed when nothing served
- [x] sync+reload: container `.ddev` source, only our prefix cleared, copy back by name, validate before reload, nginx HUP / apachectl graceful
- [x] restart only when the hostname set changed, on the host, `--no-restart`; unserve still syncs
- [x] unserve keeps DB, saves settings.php; setup restores it when tables exist
- [x] styleguide frontend best-effort; `db_site_sql` targets the site DB
- [ ] Gerrit: XSSI strip, merged/abandoned skipped, Change-Id dedupe, cherry-pick abort, one rebuild, site asked first, `@primary` blanked, persist offer, picker skipped without TTY/with list
- [x] cs: SSH verdict from ssh; ssh-add only classifies; host report; author identity by account id
- [x] Mutagen flush after mutating verbs only
- [x] nothing container-side calls `ddev`; `require_core` on the host first; env forwarded
- [x] `site_for_name` maps the active worktree to `@primary`
- [ ] `ctr` never prompts; ask-then-`explain_missing`; no TTY → BRANCH fallback
- [x] every error states a next step; `reject_args` for no-arg verbs
- [x] completion: `value<TAB>desc` / `_activeHelp_`, flags once and after `-`, `''` = empty, root from `$0`, exit 0, no stderr
- [x] `addon_is_stale` ignores a missing stamp
- [ ] `ui stop` never downloads or starts a session; session protocol VERSION bumped on wire changes
- [x] event twin `@@tryout {"level","msg"}` on the same stream, colours stripped
