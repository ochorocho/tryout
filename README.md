[![add-on registry](https://img.shields.io/badge/DDEV-Add--on_Registry-blue)](https://addons.ddev.com)
[![tests](https://github.com/bmack/tryout/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/bmack/tryout/actions/workflows/tests.yml?query=branch%3Amain)
[![last commit](https://img.shields.io/github/last-commit/bmack/tryout)](https://github.com/bmack/tryout/commits)
[![release](https://img.shields.io/github/v/release/bmack/tryout)](https://github.com/bmack/tryout/releases/latest)

# TYPO3 tryout

Get a working TYPO3 development setup in minutes — backed by the real TYPO3 Core
git repository, with Gerrit patches one command away.

**tryout** is a DDEV add-on for people who want to contribute to TYPO3 Core, test
Gerrit patches, or develop custom extensions against the latest Core source — without
wrestling with manual setup. It is aimed at Core contributors, extension developers, and
anyone who wants to quickly spin up a TYPO3 instance backed by the actual Core repository.

## Quick Start

Pick a folder name for your project (e.g. `my-typo3-site`) and run:

```bash
mkdir my-typo3-site && cd my-typo3-site
ddev config --project-type=typo3 --docroot=public --php-version=8.5
ddev add-on get bmack/tryout
ddev start
```

On the first run this will:

1. Clone the TYPO3 Core repository into `typo3-core/`
2. Resolve every Core system extension through Composer
3. Set up a TYPO3 instance

Once finished, open the backend:

- **URL:** `https://my-typo3-site.ddev.site/typo3/`
- **User:** `admin` / `Password.1`

To update the add-on later, run `ddev add-on get bmack/tryout` again; to remove it,
`ddev add-on remove tryout`.

### Adding tryout to an existing project

tryout can be installed into a project that already has its own `composer.json`.
It never rewrites that file: Composer is pointed at an overlay
(`composer.tryout.json`) which pulls your `composer.json` in as an include, so your
own dependencies keep resolving alongside the Core sysexts.

### Migrating from the old template layout

tryout used to be a template repository — you cloned it, ran `rm -rf .git && git init`,
and the scaffold *was* your project. That layout is superseded. To move an existing
tryout to the add-on:

```bash
cd your-existing-tryout
rm -rf .ddev/scripts .ddev/templates .ddev/commands/host/cs \
       .ddev/config.patches.yaml .ddev/commands/host/tryout
ddev add-on get bmack/tryout
ddev restart
```

Your `.ddev/config.yaml` stays as it is. Two things change:

- **`ddev cs` is now `ddev tryout cs`** — a two-letter command was too likely to
  collide with other add-ons.
- **Composer moves to an overlay.** Your generated `composer.json` is replaced by
  `composer.tryout.json`; delete the old `composer.json` and `composer.lock` unless
  you added your own dependencies to them, in which case keep `composer.json` — it
  is merged into the overlay from now on.

Your `typo3-core/` checkout, database and patches are untouched.

## Commands

Everything is accessed through a single `ddev tryout` entry point:

```text
ddev tryout status              Show project overview
ddev tryout download            Clone or update TYPO3 Core
ddev tryout download --reset    Hard reset Core to current branch
ddev tryout checkout <branch>   Switch TYPO3 version (main, 14.3, 13.4, 12.4, ...)
ddev tryout composer            Regenerate the Composer overlay from Core sysexts
ddev tryout patch <change-id>   Apply a Gerrit patch
ddev tryout patch               Apply all patches from config
ddev tryout reset               Reset Core to current branch + rebuild
ddev tryout delete              Wipe DB + fileadmin, fresh setup

ddev tryout cs                  Prepare instance for Core contribution
ddev tryout cs doctor           Check hooks, template, and push URL
ddev tryout cs uninstall        Remove hooks and reset push URL
```

## Contributing to TYPO3 Core

`ddev start` keeps the instance read-only against Gerrit — you can pull and
test patches but not submit them. Run **`ddev tryout cs`** once to turn the instance
into a full contribution workspace:

```bash
ddev tryout cs             # prompts for your review.typo3.org username
ddev tryout cs setup jdoe  # or pass it explicitly
```

This is opt-in (nothing runs automatically on `ddev start`) and installs:

1. The **Gerrit `commit-msg` hook** — adds a `Change-Id` footer to every commit.
2. The **TYPO3 Core `pre-commit` hook** — runs CGL / PHP-CS-Fixer checks.
3. A **commit-message template** — wired via `commit.template`, opens a
   TYPO3-style skeleton (`[BUGFIX]`, `Resolves:`, `Releases:` …) whenever
   you run `git commit` without `-m`.
4. The **Gerrit SSH push URL** on `origin` — so `git push origin HEAD:refs/for/main`
   submits your change for review.

Check the state at any time:

```bash
ddev tryout cs doctor
```

Doctor reports whether each piece is wired up and probes Gerrit SSH live
(requires a public key uploaded at https://review.typo3.org/settings/#SSHKeys).

The username is resolved from (in order): command argument → `TRYOUT_GERRIT_USER`
environment variable → cached `tryout.gerritUser` git config → interactive prompt.
To persist it across instances, set it in `.ddev/config.local.yaml`:

```yaml
web_environment:
  - TRYOUT_GERRIT_USER=jdoe
```

To revert everything:

```bash
ddev tryout cs uninstall
```

## Working with Gerrit Patches

Apply a patch directly from [review.typo3.org](https://review.typo3.org) by its change number:

```bash
ddev tryout patch 56947
```

The latest patchset is resolved automatically via the Gerrit REST API,
fetched, and cherry-picked onto your local Core branch.

Check what is currently applied:

```bash
ddev tryout status
```

Start over:

```bash
ddev tryout reset
```

### Auto-Applying Patches

To have patches applied on every `ddev start` or `ddev restart`,
list their change IDs in `.ddev/config.tryout-patches.yaml`:

```yaml
web_environment:
  - TRYOUT_PATCHES=56947,12345
```

On start the Core is reset to the current branch and the listed patches
are cherry-picked in order.

## Switching TYPO3 Versions

Important

TYPO3 uses a `main`-based commit workflow. Usually only mergers commit to a non-main branch only. Even if your fix targets an earlier version, please provide patches against `main`.

By default tryout clones the `main` branch (latest development). To work
against a different major version:

```bash
ddev tryout checkout 14.3
```

This single command switches the Core branch, regenerates the Composer overlay,
and rebuilds everything. Run without arguments to see all available branches.

Different TYPO3 versions ship different sets of system extensions.
`checkout` handles this automatically: a PHP script scans
`typo3-core/typo3/sysext/*/composer.json` and rewrites the `require`
section of `composer.tryout.json` to match exactly what exists on disk.

You can also regenerate the overlay independently at any time:

```bash
ddev tryout composer
```

To pin the branch via environment variable (e.g. in `.ddev/config.local.yaml`):

```yaml
web_environment:
  - TRYOUT_BRANCH=14.3
```

## Custom Extensions

The `packages/` directory is a Composer path repository. Drop an extension
folder in there and require it:

```bash
ddev composer require myvendor/my-extension:@dev
ddev typo3 extension:setup
```

Composer resolves it from the local path — no Packagist publish required.
Run `ddev typo3 extension:setup` after every `composer require` to activate
the extension and run its database schema updates.
This makes it easy to develop an extension side-by-side with Core.

## Running Multiple Instances

Every tryout is an ordinary DDEV project, so running several side by side is just
a matter of creating several of them. Each gets its own database, TYPO3
installation, and set of patches:

```bash
mkdir tryout-main && cd tryout-main
ddev config --project-type=typo3 --docroot=public --php-version=8.5
ddev add-on get bmack/tryout && ddev start   # → https://tryout-main.ddev.site

mkdir ../tryout-v13 && cd ../tryout-v13
ddev config --project-type=typo3 --docroot=public --php-version=8.5
ddev add-on get bmack/tryout && ddev start   # → https://tryout-v13.ddev.site
```

`ddev config` derives the project name from the folder; pass
`--project-name=my-custom-name` to override it.

If what you actually want is several TYPO3 versions inside *one* project — sharing
a single Core object store, and optionally all reachable at once — use
`ddev tryout worktree` instead. See
[Multiple Core Checkouts Side by Side](#multiple-core-checkouts-side-by-side).

## How It Works

### Directory Layout

After `ddev add-on get` the add-on's payload lives under `.ddev/`, and your project
root holds the Core checkout and the Composer overlay:

```text
my-typo3-site/
├── .ddev/
│   ├── commands/host/
│   │   └── tryout                # The ddev tryout command (runs on the host)
│   ├── tryout/                   # Add-on payload, namespaced so it cannot collide
│   │   ├── functions.sh          # Shared helpers (Gerrit API, patching, hooks)
│   │   ├── post-start.sh         # Runs on ddev start (clone, patch, setup)
│   │   ├── resolve-patch-ref.sh  # Fetches + parses a Gerrit change (runs in-container)
│   │   ├── sync-composer.php     # Regenerates the overlay from Core sysexts
│   │   └── gitmessage.txt        # Commit template installed by `ddev tryout cs`
│   ├── config.yaml               # Yours, from `ddev config` (name, docroot, PHP, DB)
│   ├── config.tryout.yaml        # Add-on: TYPO3 env vars + the post-start hook
│   └── config.tryout-patches.yaml  # Gerrit patch list (yours to edit)
├── config/system/additional.php  # TYPO3 DB + mail + GFX config for DDEV
├── packages/                     # Custom extensions (path repository)
├── composer.tryout.json          # Composer overlay (add-on owned, generated)
├── composer.json                 # Yours, if you have one — never rewritten
└── typo3-core/                   # TYPO3 Core clone (gitignored, created on first start)
```

Everything the add-on owns carries a `#ddev-generated` marker. DDEV refuses to
overwrite a file whose marker you removed, and removes only marked files on
uninstall — so deleting that line is how you take ownership of any of them.

### The Composer Overlay

An add-on must not rewrite the `composer.json` of the project it is installed
into. So tryout ships its own **overlay**, `composer.tryout.json`, and points
Composer at it with `COMPOSER=composer.tryout.json` in `config.tryout.yaml`.

The overlay declares two path repositories:

```json
{
  "repositories": [
    { "type": "path", "url": "packages/*" },
    { "type": "path", "url": "typo3-core/typo3/sysext/*", "options": { "symlink": true } }
  ],
  "extra": {
    "merge-plugin": { "include": ["composer.json"] }
  }
}
```

Every system extension inside the Core clone is required at `@dev`. Composer
resolves them from the local path and creates symlinks, so any edit inside
`typo3-core/` is immediately active — no reinstall needed. The same mechanism
applies to `packages/*`: local extensions are symlinked into `vendor/` and behave
as if they were installed from Packagist.

`composer-merge-plugin` pulls your own `composer.json` in as an include, so if the
project had dependencies before you installed tryout they keep resolving. Your file
is only ever read, never written.

The `require` block of the overlay is generated: `sync-composer.php` scans
`typo3-core/typo3/sysext/*/composer.json` and rewrites it, which is what keeps the
sysext list correct across `ddev tryout checkout`. Anything you add to the overlay
that is not a `typo3/cms-*` or `typo3/theme-*` package is preserved.

### DDEV Configuration

- **`config.yaml`** — yours, created by `ddev config`. Project name, docroot,
  PHP and database versions live here.
- **`config.tryout.yaml`** — installed by the add-on. Carries the TYPO3
  environment variables, the `COMPOSER` overlay selection and the post-start hook.
  Deliberately sets no `name`, `type`, `docroot` or `php_version`: those are yours.
- **`config.tryout-patches.yaml`** — defines `TRYOUT_PATCHES` for auto-applying
  Gerrit changes on start. Not marked generated, so your patch list survives
  add-on updates.
- **`config.local.yaml`** — gitignored, for personal overrides (PHP version,
  xdebug, etc.). DDEV merges `config.*.yaml` files in lexicographic order, so
  anything sorting after `config.tryout.yaml` wins.

### Post-Start Hook

On every `ddev start` the post-start script runs on the host (it shells out
to `ddev composer`/`ddev exec`/`ddev typo3` for anything that needs to happen
inside the container):

1. **Clone** — if `typo3-core/` does not exist, clones from GitHub and adds a
   Gerrit remote.
2. **Patch** — if `TRYOUT_PATCHES` is set, resets Core to the current branch
   and cherry-picks each change via the Gerrit REST API.
3. **Sync + Composer install** — regenerates `composer.tryout.json` from the
   sysexts in this Core checkout, then resolves everything from the path repositories.
4. **TYPO3 setup** — on first run, creates `settings.php` and sets up the database.
5. **Extension setup + cache flush** — activates extensions and clears caches.

### Gerrit Integration

Patches are resolved through the Gerrit REST API at `https://review.typo3.org`.
Given a change number (e.g. `56947`), the API returns the latest patchset ref
(e.g. `refs/changes/47/56947/12`). That ref is fetched and cherry-picked.

Merged or abandoned changes are detected and skipped. Conflicts abort the
cherry-pick automatically and report the failure.

## Multiple Core Checkouts Side by Side

Within a single tryout you can keep several TYPO3 Core checkouts and switch
between them instantly. They are git worktrees of the Core clone you already
have, so they share one object store: one fetch, a fraction of the disk, and
no second clone.

```bash
ddev tryout worktree add v13 13.4   # create typo3-core-v13 at origin/13.4
ddev tryout worktree list           # show all, marking the active one
ddev tryout worktree use v13        # make it the active Core, then rebuild
ddev tryout worktree remove v13     # remove it again
```

The first `worktree add` moves your existing `typo3-core/` to
`typo3-core-main/` and turns `typo3-core` into a symlink pointing at whichever
checkout is active. Every path keeps working, so nothing else in the project
changes.

```text
typo3-core        -> typo3-core-v13   (symlink: the active Core)
typo3-core-main/                      (the clone; owns the git object store)
typo3-core-v13/                       (worktree, detached at origin/13.4)
```

Typical use: run a Gerrit patch against v13 while keeping main untouched.

```bash
ddev tryout worktree add v13 13.4
ddev tryout worktree use v13
ddev tryout patch 56947
ddev tryout worktree use main        # back to main, patch stays on v13
```

New worktrees are created with a **detached HEAD** by default. Git refuses to
check out one branch in two worktrees, and detached is the normal state here
anyway: patches are cherry-picked on top and pushed to `refs/for/<branch>`.
Pass `--branch` if you want a real local branch.

Because `use` swaps the Core underneath Composer, it always runs
`composer install` afterwards — without it `vendor/` would keep pointing at
the previous checkout. `ddev tryout status` warns if the two ever drift apart.

Two guards worth knowing: you cannot remove the active worktree, and switching
away from one with uncommitted changes is refused (`--force` overrides).
Since all worktrees share one object store, a single `ddev tryout cs` sets up the
Gerrit hooks and commit template for all of them.

### Serving several sites at once

`use` gives you one site at the project URL. To have every worktree reachable
**at the same time**, each on its own hostname, PHP version and database, serve
it instead:

```bash
ddev tryout worktree add v13 13.4 --php 8.2 --serve
ddev restart          # registers the hostname and issues its certificate
```

```text
https://tryout-git.ddev.site       → typo3-core        PHP 8.5   db
https://v13.tryout-git.ddev.site   → typo3-core-v13    PHP 8.2   db_v13
```

Both run in the same container: the served site gets its own php-fpm on its own
socket, its own vhost, its own `sites/<name>/` tree with its own `vendor/`, and
its own database. Roughly 175 MB per extra site — the Core object store stays
shared.

The site-scoped commands take an optional site name, defaulting to the primary
so existing usage is unchanged:

```bash
ddev tryout patch 93202 v13     # cherry-pick onto that site's Core only
ddev tryout reset v13
ddev tryout checkout 13.4 v13
ddev tryout delete v13          # only that site's DB and fileadmin
ddev tryout delete --all        # every site, named in the confirmation
ddev tryout exec v13 vendor/bin/typo3 cache:flush
```

Use `ddev tryout worktree unserve <name>` to drop a site but keep the worktree
(`--drop-db` to discard its database too).

### Optional: speed up Mutagen sync (macOS)

DDEV's Mutagen ignore list is root-anchored, so the Core clone's `.git` — around
650 MB — is synced into the container even though git only ever runs on the host.
To skip it, edit `.ddev/mutagen/mutagen.yml`, **remove the `#ddev-generated`
line**, and add under `ignore.paths`:

```yaml
      - "typo3-core*/.git"
      - "sites/*/vendor"
```

Removing the marker means you own that file and DDEV will no longer update it,
which is why this is opt-in rather than done for you.

## Sharing one TYPO3 Core checkout across multiple tryouts (git worktree)

By default every tryout instance clones its own copy of TYPO3 Core into
`typo3-core/` (which is gitignored). When you run several tryouts at once —
for example to test two Gerrit patches side by side — that means several full
Core clones and several separate fetches.

Because the `post-start` hook only clones Core when `typo3-core/` does not yet
exist, you can pre-seed `typo3-core/` as a **git worktree** of a single, shared
Core clone. All instances then share one `.git` object store: one place to
fetch, far less disk, and instant switching between versions.

### One-time: create the shared Core clone

```bash
# Clone TYPO3 Core once, somewhere central
git clone https://github.com/typo3/typo3.git ~/typo3-core-shared
cd ~/typo3-core-shared
# optional: add the Gerrit remote so patches can be fetched
git remote add gerrit ssh://<username>@review.typo3.org:29418/Packages/TYPO3.CMS.git
```

### Per tryout: attach Core as a worktree instead of cloning

```bash
# run inside each tryout folder, BEFORE `ddev start`
cd /path/to/tryout
git -C ~/typo3-core-shared worktree add --detach "$PWD/typo3-core" main
ddev start   # post-start sees typo3-core/ already exists and skips the clone
```

Use `--detach` so several tryouts can be based on the **same** branch tip:
git worktree refuses to check out one branch in two worktrees, but detached
HEADs pointing at the same commit are fine. Each instance can then cherry-pick
a different Gerrit change on top:

```bash
cd /path/to/tryout      && ddev tryout patch 56947
cd /path/to/tryout-wip  && ddev tryout patch 57001
```

### Cleaning up

```bash
git -C ~/typo3-core-shared worktree remove /path/to/tryout/typo3-core
git -C ~/typo3-core-shared worktree prune
```

### Notes / trade-offs

- All worktrees share one object store, so a `git gc` or fetch in one instance
  affects all of them — avoid heavy git maintenance in two instances at once.
- Composer path repositories still point at each instance's own
  `typo3-core/typo3/sysext/*`, so symlinks and autoloading behave exactly as
  before.
- This shares one Core object store across *separate DDEV projects*. To keep
  several Core versions inside a single project — optionally all served at once —
  use `ddev tryout worktree`, which manages the same mechanism for you.

## Requirements

- [DDEV](https://ddev.readthedocs.io/en/stable/) v1.24.10+ (enforced by the add-on)
- Docker Desktop or Colima
- Git
- A `bash` shell on the host — the `ddev tryout`/`ddev tryout cs` commands and the
  post-start hook run on the host rather than inside the container. On
  Windows this means Git Bash (bundled with Git for Windows); DDEV finds it
  automatically. An SSH client is also needed for `ddev tryout cs` to reach Gerrit.

## Contributing

tryout itself lives at [github.com/bmack/tryout](https://github.com/bmack/tryout).
If you have improvements to the add-on — better defaults, new `ddev tryout`
subcommands, fixes to the post-start hook, documentation tweaks — pull requests
and issues are welcome there.

To work on the add-on, install it into a scratch project straight from your
checkout — no release or tarball needed:

```bash
mkdir /tmp/tryout-test && cd /tmp/tryout-test
ddev config --project-type=typo3 --docroot=public --php-version=8.5
ddev add-on get /path/to/your/tryout/checkout
ddev start
```

The payload lives at the repo root (`install.yaml`, `commands/`, `tryout/`,
`config.tryout.yaml`) and is copied into the project's `.ddev/` on install.

### Tests

The suite is [bats](https://bats-core.readthedocs.io/). Install it and the helper
libraries once:

```bash
brew tap bats-core/bats-core
brew install bats-core bats-assert bats-file bats-support
```

It is split by cost, because every DDEV-backed test builds and destroys a whole
project:

```bash
bats tests/unit.bats      # seconds — pure helpers in tryout/functions.sh, no containers
bats tests/test.bats --filter-tags '!release'
                          # minutes — install, config, overlay, guarded files, removal
bats tests/lifecycle.bats # much longer — clones TYPO3 Core, patches, served worktrees
bats tests --filter-tags '!release,!lifecycle'   # the 42 fast tests
bats tests --filter-tags '!release'              # everything runnable locally
```

`tests/test.bats` carries an `install from release` test tagged `release`; it needs
a published GitHub release, so exclude it locally with `--filter-tags '!release'`.
CI runs the fast suites on every push and the lifecycle and release suites nightly.

For debugging: `bats tests/test.bats --show-output-of-passing-tests --verbose-run
--print-output-on-failure`.

This repo also tracks DDEV's add-on conventions, which are machine-checkable:

```bash
curl -fsSL https://ddev.com/s/addon-update-checker.sh | bash
```

Note that contributions to **TYPO3 Core** itself do not go through this repo.
Core development happens on [review.typo3.org](https://review.typo3.org) via Gerrit.
tryout is just a local environment for working on Core; once you have a patch ready,
push it to Gerrit as usual.

## License

MIT — see [LICENSE](LICENSE).
