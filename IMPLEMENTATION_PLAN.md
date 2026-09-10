# Core clone as the project root

Invert the layout: the TYPO3 Core clone *is* the DDEV project root, its worktrees
live inside it under `worktrees/`, and the Composer instance is built in `Build/`.

Proven before starting: Core's `.gitignore` already carries `/.ddev/*` (since 2018,
`10a9e0ee805`); `.git/info/exclude` keeps the checkout clean without touching a
tracked file and never reaches a Gerrit patch; that exclude is shared by every
worktree; and nested worktrees keep relative metadata, so host and container paths
stay interchangeable.

## Stage 1: Path model
**Goal**: `PROJECT_ROOT` is the Core clone; the instance root is its own variable.
**Success Criteria**: `CORE_DIR` == `PROJECT_ROOT`; `INSTANCE_DIR`/`WORKTREES_DIR`
exist; `core_worktree_dir` returns `worktrees/<name>`; `worktree_name_for_path`
resolves the new shapes; unit suite green.
**Tests**: unit.bats path-helper tests retargeted.
**Status**: Complete

## Stage 2: Clone into the root, keep it clean
**Goal**: `ddev start` clones Core into the project root and it stays `git status` clean.
**Success Criteria**: clone works into a directory that already holds `.ddev/`;
`.git/info/exclude` written idempotently; `git status` empty.
**Tests**: unit test for `ensure_core_excludes`; lifecycle clone check.
**Status**: Complete

## Stage 3: Instance in Build/
**Goal**: composer installs into `Build/`, docroot `Build/public`.
**Success Criteria**: `ddev start` yields a working backend; `git status` still clean.
**Tests**: overlay assertions; e2e backend login.
**Status**: Complete

## Stage 4: Worktrees and served sites
**Goal**: every `worktree` verb works with nested worktrees.
**Success Criteria**: `worktree add v13 13.4 --serve` serves a second site.
**Tests**: lifecycle.bats worktree + serve.
**Status**: Complete

## Stage 5: Packaging, docs, tests
**Goal**: install/removal, README and the layout assertions match.
**Success Criteria**: conformance checker clean (bar the known LICENSE finding).
**Tests**: test.bats install/removal.
**Status**: Complete — README, CLAUDE.md, install.yaml and the suites retargeted.

## Also verified
- **A Gerrit patch carries nothing of the add-on's.** `git add -A` after editing a
  Core file stages that file and nothing else — zero entries under `Build/`,
  `worktrees/`, `sites/`, `.ddev/` or `packages/`. This is the whole point of
  using `.git/info/exclude` rather than Core's tracked `.gitignore`.
- **`cs setup` works from the root**: hooks land in `.git/hooks/`, the commit
  template resolves as `.ddev/tryout/gitmessage.txt` (a direct child now, so no
  `../`), Gerrit SSH authenticates, and the checkout stays clean. The push command
  is now plain `git push origin HEAD:refs/for/main` with no `cd` first.
- **`worktree serve` builds a nested worktree's site** with its own vendor, public
  and database, both checkouts still clean.

## Verified end to end
`ddev config --docroot=Build/public` + `ddev add-on get` + `ddev start` on a clean
directory produced: the project root a TYPO3 Core clone on `main` with both remotes
and a CLEAN `git status`; the instance in `Build/` (public/, vendor/, config/system);
the backend answering HTTP 200 with a real TYPO3 CMS login page; and
`worktree add v13 13.4` creating `worktrees/v13` with a relative pointer
(`../../.git/worktrees/v13`), both checkouts clean.
