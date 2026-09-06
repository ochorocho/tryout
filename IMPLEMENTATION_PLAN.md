# Run `ddev tryout` work inside the web container

Hybrid design: `commands/host/tryout` stays the entry point (TTY, gum prompts, herdr),
but the *work* of every container-safe verb runs inside the web container through one
`ddev exec` per verb. Git, composer, php, curl and ssh are then the container's.
Git worktree metadata uses relative paths so host and container both read it, which
needs git >= 2.48 in the image (Debian trixie ships 2.47).

## Stage 1: git >= 2.48 in the image, relative worktree paths
**Goal**: `web-build/Dockerfile.tryout` builds git from source; the Core repo is
configured with `worktree.useRelativePaths=true`; existing worktrees are repaired.
**Success Criteria**: image builds; `git worktree add` in the container yields a
worktree whose `.git` file is relative and which `git status` accepts on the host.
**Tests**: unit — Dockerfile pins version + sha256 and carries the marker;
install.yaml lists it; `ensure_relative_worktree_paths` sets the config and repairs.
**Status**: Complete

## Stage 2: container entrypoint and delegation
**Goal**: `tryout/tryout-container.sh` dispatches `cmd_*` bodies (moved to
`tryout/commands.sh`) inside the container; `functions.sh` helpers call
php/composer/mysql/git directly; the host command resolves prompts then delegates.
The post-start hook becomes an `exec:` hook.
**Success Criteria**: no `ddev exec|composer|typo3|php|mysql|mutagen` call remains in
container-side code; every verb in the host `case` is delegated or host-only.
**Tests**: unit — no bare ddev call in container-side files; delegation forwards
`"$@"`; host case still covers every verb for completion and the menu.
**Status**: Complete

## Stage 3: contribution setup, hints, docs
**Goal**: `cs doctor`/`setup` probe Gerrit SSH from both sides and say which agent is
missing a key; README/CLAUDE.md describe the new split and the image build cost.
**Success Criteria**: doctor output names both probes; docs match behaviour.
**Tests**: unit — hint text mentions `ddev auth ssh`.
**Status**: Complete

## Stage 4: verification
**Goal**: green unit suite, conformance checker, scratch-project install and a real
`ddev start` + worktree round trip on both sides.
**Success Criteria**: `bats tests/unit.bats` passes; `git -C typo3-core-<x> status`
works on host and via `ddev exec` for a worktree created inside.
**Status**: Not Started
