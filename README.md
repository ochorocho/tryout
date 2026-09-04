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

### Installing from the repository

`ddev add-on get` takes a local directory, a GitHub repo or a tarball URL, so you can
install an unreleased version — a branch, a commit, or a working checkout — the same
way you install a release.

**From a local checkout** — what you want when developing the add-on itself. No
commit, push or release is needed; DDEV copies the working tree as it is:

```bash
git clone https://github.com/bmack/tryout.git ~/src/tryout

mkdir my-typo3-site && cd my-typo3-site
ddev config --project-type=typo3 --docroot=public --php-version=8.5
ddev add-on get ~/src/tryout
ddev start
```

Re-run `ddev add-on get ~/src/tryout` after every change you want to try; it
overwrites the installed payload in `.ddev/` and leaves your data alone.

**From a branch or commit** — `--version` takes a tag, a branch name, or a SHA:

```bash
ddev add-on get bmack/tryout --version main        # the default branch
ddev add-on get bmack/tryout --version v1.2.0      # a tag
ddev add-on get bmack/tryout --version b50ac77     # a commit
```

**From a fork:**

```bash
ddev add-on get ochorocho/tryout --version main
```

After any of these, `ddev restart` picks up changed DDEV config, and `ddev start`
provisions Core on a fresh project. To see what is installed:

```bash
ddev add-on list --installed
```

> Removing the add-on leaves `packages/` alone — those are your extensions, not
> the add-on's — along with `typo3-core*`, your `composer.json` and your patch list.

> An install **overwrites** every file the add-on owns — those carrying a
> `#ddev-generated` marker. Files you have taken ownership of by deleting that line
> are left alone, as are your `config.yaml`, your `composer.json`, the Gerrit patch
> list and the `typo3-core*` checkouts.

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
ddev tryout checkout <branch>   Switch TYPO3 version (main, 13.4, 12.4, ...)
ddev tryout composer            Regenerate the Composer overlay from Core sysexts
                                (it does not run Composer — that is `ddev composer`)
ddev tryout patch <change-id>   Apply a Gerrit patch
ddev tryout patch               Apply all patches from config
ddev tryout reset               Reset Core to current branch + rebuild
ddev tryout delete              Wipe DB + fileadmin, fresh setup
ddev tryout help                Show the built-in help

ddev tryout worktree            Manage side-by-side Core checkouts (own help)
ddev tryout exec <site> <cmd>   Run a command in a site's PHP, root and database

ddev tryout herdr               Open every Core worktree as a herdr workspace
ddev tryout herdr new           Create a worktree and open it
ddev tryout herdr setup-keys    Bind the three herdr keys (see below)
ddev tryout herdr unsetup-keys  Undo the binding

ddev tryout cs                  Prepare instance for Core contribution
ddev tryout cs doctor           Check hooks, template, and push URL
ddev tryout cs uninstall        Remove hooks and reset push URL
```

Once you serve more than one site, `patch`, `reset`, `checkout` and `delete` take an
optional site name and `delete` takes `--all` — see
[Serving several sites at once](#serving-several-sites-at-once).

### Tab completion

Every command, subcommand and flag completes with <kbd>Tab</kbd>, as do the names
the add-on knows about at that moment:

```console
$ ddev tryout <TAB>
status  download  checkout  composer  patch  worktree  cs  herdr  exec  reset  delete  help

$ ddev tryout cs <TAB>
setup  doctor  uninstall  help

$ ddev tryout herdr <TAB>
new  setup-keys  unsetup-keys  main  v13  --no-agent  --no-focus  --no-attach

$ ddev tryout worktree use <TAB>
main  v13                       # your Core worktrees

$ ddev tryout checkout <TAB>
main  13.4  12.4  11.5  ...     # branches already fetched into typo3-core/

$ ddev tryout exec <TAB>
@primary  v13                   # the primary site and every served worktree
```

Branches come from the refs already in `typo3-core/`, never from the network, so a
<kbd>Tab</kbd> never stalls; `ddev tryout download` and `checkout` both fetch, so the
list stays current.

This needs DDEV's own shell completion to be installed — the add-on cannot do that
for you. Homebrew installs the scripts with DDEV, but Bash also needs
`brew install bash-completion` and Zsh needs `$(brew --prefix)/share/zsh/site-functions`
on `FPATH` before `compinit`. See
[DDEV's shell completion docs](https://docs.ddev.com/en/stable/users/install/shell-completion/)
for your shell and platform.

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
│   │   ├── tryout                # The ddev tryout command (runs on the host)
│   │   └── autocomplete/
│   │       └── tryout            # Tab-completion for it (DDEV runs this on TAB)
│   ├── tryout/                   # Add-on payload, namespaced so it cannot collide
│   │   ├── functions.sh          # Shared helpers (Gerrit API, patching, worktrees, herdr)
│   │   ├── post-start.sh         # Runs on ddev start (clone, patch, setup)
│   │   ├── sync-composer.php     # Regenerates the overlay from Core sysexts
│   │   ├── site-composer.php     # Builds a served site's own composer root
│   │   ├── tryout-php-fpm.sh     # Extra php-fpm master for a site on another PHP
│   │   ├── resolve-patch-ref.sh  # Fetches + parses a Gerrit change (runs in-container)
│   │   ├── resolve-gerrit-account.sh  # Looks up a Gerrit account (runs in-container)
│   │   ├── herdr-new-worktree.sh # Popup bound by `ddev tryout herdr setup-keys`
│   │   ├── gitmessage.txt        # Commit template installed by `ddev tryout cs`
│   │   └── …                     # Templates copied out on install (see below)
│   ├── config.yaml               # Yours, from `ddev config` (name, docroot, PHP, DB)
│   ├── config.tryout.yaml        # Add-on: TYPO3 env vars + the post-start hook
│   ├── config.tryout-patches.yaml  # Gerrit patch list (yours to edit)
│   └── config.worktrees.yaml     # Generated by `worktree serve` — hostnames + daemons
├── config/system/additional.php  # TYPO3 DB + mail + GFX config for DDEV
├── packages/                     # Custom extensions (path repository)
├── composer.tryout.json          # Composer overlay (add-on owned, generated)
├── composer.json                 # Yours, if you have one — never rewritten
├── typo3-core                    # Symlink to the active checkout (see worktrees)
├── typo3-core-main/              # The Core clone (gitignored, created on first start)
├── typo3-core-<name>/            # Further worktrees, one per `worktree add`
└── sites/<name>/                 # A served site's own root (`worktree serve`)
```

Four files in `tryout/` are templates rather than runtime code: `additional.php`,
`composer.tryout.json`, `gitignore` and `patches.yaml` are copied out to the project
on install, which is where the entries above them come from.

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
ddev tryout worktree serve v13      # give it its own URL, PHP and database
ddev tryout worktree unserve v13    # drop that site, keep the worktree
ddev tryout worktree remove v13     # remove the worktree itself
ddev tryout worktree rename v13 old # rename the checkout, keep the branch
ddev tryout worktree help           # flags and the full picture
```

Without `--php`, a served worktree gets **the highest PHP its own Core accepts** —
read from `require.php` in that branch's `composer.json`, not from the project's
PHP version. A 13.4 worktree (`^8.2`) and a main one (`^8.5`) therefore get the
right runtime each, and a branch with an upper bound like `>=8.2 <8.4` gets 8.3.

`add` takes `--branch` (a real local branch instead of a detached HEAD), `--serve`,
`--php 8.2` (which implies `--serve`), and `--herdr` to open it as a
[herdr workspace](#opening-worktrees-in-herdr) straight away.

> `use`, `remove` and `unserve` take their flag **after** the name —
> `worktree use v13 --force`, not `worktree use --force v13`.

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

#### Stopping a site

```bash
ddev tryout worktree unserve v13    # keeps the database
ddev restart                        # releases the hostname
```

`unserve` removes the site's vhost and its whole `sites/v13/` tree — including that
`vendor/`, so the ~175 MB comes back — but **keeps the database and the git
worktree**. Serving it again later restores the site with its content intact. Add
`--drop-db` to discard the database too.

To remove the checkout as well, use `ddev tryout worktree remove v13`; it unserves
first if it has to.

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

### Opening worktrees in herdr

If you use [herdr](https://herdr.dev) — a terminal multiplexer built around coding
agents — one command lays out every Core worktree:

```bash
ddev tryout herdr              # every worktree
ddev tryout herdr v13          # just that one
ddev tryout herdr --no-agent   # two shells instead of claude + shell
```

Each worktree becomes its own herdr workspace, labelled `core-<name>` so it is
unambiguous in the sidebar, holding a Claude session and a shell both rooted at that
worktree:

```text
workspace "core-main"              workspace "core-v13"
├── claude   (typo3-core-main)     ├── claude   (typo3-core-v13)
└── shell    (typo3-core-main)     └── shell    (typo3-core-v13)
```

Worktrees already open are skipped, so it is safe to re-run, and the first Core
workspace is focused when it is done (`--focus` is the default; `--no-focus` stays put). On the first run in
a worktree Claude asks you to trust the folder — the command says so rather than
waiting.

**It runs in a herdr session of its own**, `tryout-<project>`, started on demand, and
drops you straight into it. Your default session is never touched, so Core worktrees
stay out of your everyday sidebar.

Where attaching is not possible it prints what to do instead: with no terminal (a
script or CI) it gives you `herdr session attach tryout-my-typo3-site`, and from
inside herdr — which refuses to nest — it tells you to switch to that session.

Pass `--no-attach` to always print rather than attach.

### Creating a worktree from herdr

`ddev tryout herdr new <name> [<branch>]` creates a Core worktree and opens it in one
step. Run it bare in a terminal and it asks for both. The same thing from the other
direction is `ddev tryout worktree add <name> <branch> --herdr`.

herdr's own **New worktree** (`prefix+shift+G`) is no use here: it prompts only for a
branch and always checks out under `~/.herdr/worktrees`, where no tryout command can
see it. That action cannot be redirected — herdr has no pre-create hook — so the key
has to be rebound:

```bash
ddev tryout herdr setup-keys        # shows the block, asks, then writes it
ddev tryout herdr setup-keys --yes  # skip the question
ddev tryout herdr unsetup-keys      # removes it again
```

That binds **three** keys:

| Key | What it opens |
|---|---|
| `prefix+shift+G` | New Core worktree — asks for a name and branch |
| `prefix+shift+T` | The tryout menu — every `ddev tryout` command |
| `prefix+shift+D` | The dashboard — worktrees, served URLs, running jobs |

`prefix` is herdr's own, `ctrl+b` by default — so the menu is <kbd>ctrl+b</kbd> then
<kbd>shift</kbd>+<kbd>T</kbd>, pressed in sequence. `ddev tryout herdr` reminds you of
the key when it finishes, and `ctrl+b ?` lists every active binding.

The menu covers every command the CLI dispatches, with `worktree` and `cs` as
submenus. It runs instant commands (`status`, `worktree list`, `herdr jobs`) in the
popup itself (also `worktree adopt` and `help`), and pushes anything slow
(`serve`, `use`, `reset`, `checkout`,
`cs doctor`, `cs setup`) into a new pane you can watch. Destructive ones —
`delete`, `worktree remove`, `reset`, `checkout`, `cs uninstall` — make you type a
confirmation first.

```text
ctrl+b shift+T

  1 status        7 composer          2 worktree…        8 cs…
  2 worktree…     8 cs…                 1 list             1 doctor
  3 herdr new     9 reset               2 add              2 setup
  4 patch         0 delete              3 use              3 uninstall
  5 checkout      e exec                4 serve            q back
  6 download      j jobs                5 unserve
                  d dashboard           6 remove
                  h help  q quit        7 adopt
                                        q back
```

The menu **stays open**: run something, come back, run something else. <kbd>ESC</kbd>
goes back one level — out of a submenu, out of an output view — and closes the popup
from the top. `q` does the same.

Output from an instant command, and from any job you choose to watch, fills the popup
in a scrollable view:

```text
worktree list

  NAME    HEAD         BRANCH      STATE  PHP  DB         URL
  benni   101b9ca0454  (detached)  clean  8.5  db_benni   https://benni.…
  …

  ↑↓/jk scroll  g/G top/bottom  r refresh  ESC back  (1/10)
```

A job's output is read back out of the pane it runs in, so the pane is still there to
watch full-size or scroll in herdr itself. Close that pane and the view says so rather
than showing you an error blob.

Flags are the exception: the menu prompts for names, not options, so
`download --reset`, `worktree add --php 8.2` and `delete --all` stay CLI-only.

#### Watching what runs

A command launched from the menu gets a pane of its own, named after it, and its
exit code is recorded. You are notified when it finishes — and told when it fails,
which used to look identical to success.

```bash
ddev tryout herdr jobs        # ✓ / ✗ / ⟳ per job
ddev tryout herdr dashboard   # the live view, same as prefix+shift+D
```

The dashboard shows worktrees with their branch, dirty state and served URL, the
patch count, and the running jobs, refreshing every five seconds. It only ever reads
local state — no fetch, no container, no SSH — so a refresh cannot hang, and it never
writes anything: every change goes through the menu.

It backs your config up first (`config.toml.tryout-backup-<timestamp>`) and delimits
its block with markers, so `unsetup-keys` restores the file byte for byte.

#### Worktrees always land in the project

A TYPO3 Core worktree has to be at `<project>/typo3-core-<name>` — that is what the
`typo3-core` symlink points at, what `worktree list` finds, and what `serve` builds a
site from.

herdr's own **New worktree** cannot be pointed there. Its only setting,
`worktrees.directory`, puts checkouts at `<directory>/<repo>/<branch-slug>`, and it names
them from a generated word list. `setup-keys` rebinds the key, but the same action is
still reachable from the sidebar right-click menu, which a keybinding cannot intercept.

So `setup-keys` also links a small herdr plugin that watches for a Core worktree created
outside its project and moves it back with `git worktree move`. It does nothing at all to
worktrees of any other repository.

For a checkout that predates this — or if you would rather not run the plugin:

```bash
ddev tryout worktree adopt --dry-run   # list strays
ddev tryout worktree adopt             # move them into the project
```

`ddev tryout status` flags them too, so one cannot sit unnoticed.

The **directory name and the branch are independent**. herdr's own action names a
checkout after the branch it invents (`worktree/wilie-wonka`), and `adopt` derives
one the same way, but that is only a starting point:

```bash
ddev tryout worktree adopt <path> wonka   # adopt one stray under a name you pick
ddev tryout worktree rename wonka spike   # rename later; the branch is untouched
```

A rename moves everything keyed on the name — the checkout, the `typo3-core`
symlink if it is the active one, and a served site's tree, vhost and database. A
served site is unserved and re-served under the new name, so it needs a
`ddev restart` afterwards to pick up the new hostname.

> These are keybindings, not additions to herdr's own UI. A herdr plugin cannot add an
> entry to its menus — plugin actions are reachable only by a keybinding or a
> ctrl-click — so the menu is a popup of our own rather than tryout entries appearing
> in herdr's right-click menu.

> herdr has **one** global config, so these keys are live in every session, not just
> tryout's. Pressed outside a tryout project, each popup says so and does nothing.

Manage the session like any other:

```bash
herdr session list                                  # what is running
herdr --session tryout-my-typo3-site server stop    # stop it (panes and agents end)
herdr session delete tryout-my-typo3-site           # forget a stopped session
```

> herdr is optional and the add-on never installs it. The command needs `herdr` and
> `jq` on the host, and works from any terminal — it does not have to be run from
> inside a herdr pane. Without those it exits with a hint and changes nothing.

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
- [gum](https://github.com/charmbracelet/gum) — renders the tables, prompts and
  spinners the commands produce (`brew install gum`, or your package manager).
  Installation stops with the right command for your platform if it is missing.
- A `bash` shell on the host — the `ddev tryout`/`ddev tryout cs` commands and the
  post-start hook run on the host rather than inside the container. On
  Windows this means Git Bash (bundled with Git for Windows); DDEV finds it
  automatically. An SSH client is also needed for `ddev tryout cs` to reach Gerrit.
- Optional, for `ddev tryout herdr` only: [herdr](https://herdr.dev) and `jq` on the
  host. The add-on installs neither; without them that one command exits with the
  install command for your platform and everything else works as usual.

Output is meant to be read by people: `ddev tryout worktree list` draws a table,
and `ddev tryout status` a bordered report. **For scripting, use
`ddev tryout worktree list --plain`** — space-padded columns
(`NAME HEAD BRANCH STATE PHP DB URL`), which is the format the Playwright suite
parses and the one that stays stable.

The host scripts are POSIX-minded bash and run on **macOS and Linux alike**: no
GNU-only utilities (`readlink -f`, `grep -P`, `stat -c`, `date -d`), and nothing that
needs bash 4, since macOS still ships 3.2. Tests enforce both.

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
bats tests --filter-tags '!release,!lifecycle'   # the 100 fast tests
bats tests --filter-tags '!release'              # everything runnable locally
```

The lifecycle suite asserts that each served worktree really is a separate instance:
every URL check fetches the page body and looks for the TYPO3 login `<title>`, not
just a 200, because a misconfigured site answers 200 with a broken page. It runs the
primary and two served worktrees side by side, checks each has its own populated
database, and that `unserve` stops one hostname answering while the others keep
serving.

> Those URL assertions **skip** on a machine where `*.ddev.site` does not resolve.
> DDEV writes project hostnames to `/etc/hosts`, which needs sudo, and the suite runs
> with `DDEV_NONINTERACTIVE=true` — so a throwaway test project never gets an entry.
> CI, and any host with a wildcard resolver, runs them for real.

`tests/e2e/` holds opt-in Playwright tests that log into the backend of every
served worktree with a real browser — the one thing curl cannot prove, since a
login posts a form, sets a secure cookie and redirects into a module. They are not
part of any default run; see `tests/e2e/README.md`.

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
