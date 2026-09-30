[![add-on registry](https://img.shields.io/badge/DDEV-Add--on_Registry-blue)](https://addons.ddev.com)
[![tests](https://github.com/bmack/tryout/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/bmack/tryout/actions/workflows/tests.yml?query=branch%3Amain)
[![last commit](https://img.shields.io/github/last-commit/bmack/tryout)](https://github.com/bmack/tryout/commits)
[![release](https://img.shields.io/github/v/release/bmack/tryout)](https://github.com/bmack/tryout/releases/latest)

# tryout

Every branch of your DDEV project, served side by side — each worktree at its
own URL, on its own PHP and its own database, driven from the command line or a
terminal UI.

**tryout** is a DDEV add-on that works in two modes, decided by what the project
root holds:

- **Your own project**, of any DDEV type (Laravel, Symfony, Drupal, WordPress, a
  TYPO3 site, plain PHP, …): worktrees of its repository and its pull requests,
  each served with a copy of the primary's database. The install changes nothing
  of the project's.
- **TYPO3 Core**: a working TYPO3 development setup in minutes, backed by the real
  Core git repository, with Gerrit patches one command away — for Core
  contributors, extension developers and anyone who wants a TYPO3 instance of the
  actual Core source.

## Quick Start

### Your own project

In an existing DDEV project whose root is a git repository:

```bash
ddev add-on get bmack/tryout
ddev restart
ddev tryout worktree add feature-x feature/x --serve   # https://feature-x.<project>.ddev.site
ddev tryout worktree add --pr 42                        # a pull request, served as pr-42
ddev tryout ui                                          # all of it in a terminal UI
```

See [Your own project](#your-own-project-project-mode) for databases, frameworks and
what each DDEV type supports.

### TYPO3 Core

Pick a folder name for your project (e.g. `my-typo3-site`) and run:

```bash
mkdir my-typo3-site && cd my-typo3-site
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get bmack/tryout
ddev start
```

`ddev add-on get` clones the TYPO3 Core repository into the project root — on
the host, because DDEV's file sync leaves a root `.git` out — and the first
`ddev start` then:

1. Resolves every Core system extension through Composer
2. Sets up a TYPO3 instance, with a rendered frontend

If the clone could not run during the install (no network, say), `ddev start`
clones instead.

Once finished, open the frontend or the backend:

- **Frontend:** a styleguide demo page (TYPO3 13.4+; a 12.4 instance serves the
  backend only)
- **Backend:** `https://my-typo3-site.ddev.site/typo3/`
- **User:** `admin` / `Password.1`

To update the add-on later, run `ddev add-on get bmack/tryout` again; to remove it,
`ddev add-on remove tryout`.

### What it runs on

macOS and Linux (on Windows: WSL2). There is nothing else to install: `ddev tryout`
is one program, shipped inside the add-on as a build per platform (`.ddev/tryout/bin/`), and `.ddev/tryout/tryout` runs
the one for your machine — on
the host and in the web container alike.

### Installing from the repository

`ddev add-on get` takes a local directory, a GitHub repo or a tarball URL, so you can
install an unreleased version — a branch, a commit, or a working checkout — the same
way you install a release.

**From a local checkout** — what you want when developing the add-on itself. No
commit, push or release is needed; DDEV copies the working tree as it is. The
binaries are committed in `tryout/bin/`, so a checkout installs as it is; after
changing the Rust source, rebuild them with `tui/scripts/stage-bins.sh` (it
needs Rust, `lipo` and `cargo-zigbuild`, so run it on a Mac):

```bash
git clone https://github.com/bmack/tryout.git ~/src/tryout

mkdir my-typo3-site && cd my-typo3-site
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get ~/src/tryout
ddev start
```

Re-run `stage-bins.sh` and `ddev add-on get ~/src/tryout` after every change you
want to try; it overwrites the installed payload in `.ddev/` and leaves your data
alone. `TRYOUT_BIN=/path/to/tryout ddev tryout …` runs a local build on the host
without reinstalling.

**From a release, branch or commit** — `--version` takes a tag, a branch name, or a
SHA. Only release tags carry the binaries; a branch or commit installs the source,
which has none (the install says so), so use those only together with the local
checkout route above:

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
> the add-on's — along with your `composer.json` and your patch list.

> An install **overwrites** every file the add-on owns — those carrying a
> `#ddev-generated` marker. Files you have taken ownership of by deleting that line
> are left alone, as are your `config.yaml`, your `composer.json`, the Gerrit patch
> list and the worktree checkouts.

### Adding tryout to an existing project

tryout can be installed into a project that already has its own `composer.json`.
It never rewrites that file: Composer is pointed at an overlay (`composer.tryout.json`) which pulls your `composer.json`
in as an include, so your
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

Your Core checkout, database and patches are untouched.

## Your own project (project mode)

tryout works on any DDEV project, not only a TYPO3 Core checkout. On a
repository of your own, `ddev add-on get` clones and changes nothing; tryout
serves the project's worktrees side by side, each at `<name>.<project>.ddev.site`
from its own checkout, on its own PHP and database:

```bash
ddev tryout worktree add feature-x feature/x --serve   # a branch, local or on origin
ddev tryout worktree add try main --serve --db-empty    # start with an empty database
ddev tryout worktree add --pr 42                        # pull/merge request #42, as pr-42
ddev tryout exec feature-x bin/console cache:clear      # the site's PHP and database
```

A new site's database starts as a copy of the primary's (`--db-from <site>`
copies another's, `--db-empty` none); `delete <site>` resets it to a fresh
copy. A pull or merge request of origin opens as a served worktree with
`--pr <number>`: your git fetches `refs/pull/<n>/head` (GitHub) or
`refs/merge-requests/<n>/head` (GitLab), no token needed; the terminal UI lists
the open ones through `gh` or `glab` where installed. The app finds its own
database the way its framework reads it — from
the environment, or through a per-site copy of DDEV's settings file — and a
worktree without `.env` gets the project's. What needs TYPO3 Core (`patch`,
`cs`, `checkout`, `download`, `composer`, `reset`, `worktree use`) says so.

`ddev tryout status` names the support for the project's type:

| Type | Wired through | What |
|---|---|---|
| `asterios` | environment | DB_*, APP_URL |
| `backdrop` | settings file | settings.ddev.php |
| `cakephp` | environment | DATABASE_URL, APP_FULL_BASE_URL |
| `codeigniter` | environment | database.default.*, app.baseURL |
| `craftcms` | environment | CRAFT_DB_*, PRIMARY_SITE_URL |
| `drupal` | settings file | settings.ddev.php |
| `drupal6` | settings file | settings.ddev.php ($db_url) |
| `drupal7` | settings file | settings.ddev.php |
| `drupal8` | settings file | settings.ddev.php |
| `drupal9` | settings file | settings.ddev.php |
| `drupal10` | settings file | settings.ddev.php |
| `drupal11` | settings file | settings.ddev.php |
| `drupal12` | settings file | settings.ddev.php |
| `generic` | environment | TRYOUT_DB_*, DATABASE_URL |
| `joomla` | served only | configuration.php is the app's own |
| `laravel` | environment | DB_*, APP_URL |
| `magento` | served only | local.xml is the app's own |
| `magento2` | served only | app/etc/env.php and the base URL in the database |
| `maho` | served only | app/etc/local.xml is the app's own |
| `modx` | served only | config.inc.php is the app's own |
| `php` | environment | TRYOUT_DB_*, DATABASE_URL |
| `shopware6` | environment | DATABASE_URL, APP_URL; storefront domain updated |
| `silverstripe` | environment | SS_DATABASE_*, SS_BASE_URL |
| `symfony` | environment | DATABASE_URL |
| `typo3` | settings file | config/system/additional.php |
| `wordpress` | settings file | wp-config-ddev.php (MariaDB/MySQL only) |
| `wp-bedrock` | environment | DB_*, WP_HOME |

A type not listed is served like `php`.

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
ddev tryout patch               Browse the open changes and pick one or several
                                (or apply the configured list, if there is one)
ddev tryout reset               Reset Core to current branch + rebuild
ddev tryout delete              Wipe DB + fileadmin, fresh setup
ddev tryout help                Show the built-in help

ddev tryout worktree            Manage side-by-side Core checkouts (own help)
ddev tryout exec <site> <cmd>   Run a command in a site's PHP, root and database
ddev tryout launch              Open the worktree you are in, in the browser
ddev tryout launch <worktree>   Open that worktree's site (--backend for /typo3/)
ddev tryout ui                  Terminal UI: worktrees, a live shell in each, and
                                every command above one keypress away

ddev tryout cs                  Prepare instance for Core contribution
ddev tryout cs doctor           Check hooks, template, and push URL
ddev tryout cs uninstall        Remove hooks and reset push URL
```

Wherever a site is asked for, a worktree's name works too — including the name the
primary goes by in `worktree list`, so `ddev tryout exec main …` means the primary.

Once you serve more than one site, `patch`, `reset`, `checkout`, `download`,
`launch` and `delete` take an optional site name and `delete` takes `--all` — see
[Serving several sites at once](#serving-several-sites-at-once). Leave the name off
and `exec`, `patch`, `reset`, `launch` and `delete` ask which site you mean,
showing what each one is (`patch`, `reset` and `delete` only ask once something
besides the primary is served):

```text
Reset which site?
> primary       https://my-typo3-site.ddev.site
  v13           https://v13.my-typo3-site.ddev.site  PHP 8.4
  v12           https://v12.my-typo3-site.ddev.site  PHP 8.2
```

`delete` adds an `--all` entry to that list, so wiping everything is a pick
rather than a flag to remember.

The `worktree` subcommands do the same with checkouts — `use`, `serve`, `unserve`,
`remove` and `rename` show which branch each one is on, whether it has uncommitted
work, and what it serves:

```text
Serve which worktree?
> main            main                  c76e554c343  clean  ← primary
  v13             (detached)            aa6a5bdadce  clean
  bugfix          bugfix-9421           1f3a2b8c9d0  dirty
```

Each list is filtered to what the command can actually act on: `serve` offers only
unserved checkouts, `unserve` only served ones, `use` and `remove` leave out the
primary.

### Keeping an installed project up to date

`ddev add-on get` copies the add-on into a project once; it is not refreshed
afterwards. A project installed before an update therefore keeps the older command *and* the older tab-completion — both
still work, they just offer the previous set
of verbs and flags, which looks a lot like completion being broken. `ddev tryout
status` says so when it notices:

```text
  ! This project runs an older copy of the tryout add-on
    the command and its tab-completion offer the previous feature set
    → ddev add-on get <path-to-tryout> && ddev restart
```

Re-running `ddev add-on get` is always safe: your `composer.tryout.json`, patch
list and `additional.php` are preserved.

### Tab completion

Every command, subcommand and flag completes with <kbd>Tab</kbd>, each with a short
description, and so do the names the add-on knows about at that moment — only the
ones that make sense where you are:

```console
$ ddev tryout <TAB>
status    -- Show project overview
download  -- Clone or update Core
checkout  -- Switch TYPO3 version (main, 13.4, 12.4, ...)
worktree  -- Manage side-by-side Core checkouts
...

$ ddev tryout worktree serve <TAB>            # only worktrees not served yet
fancy-pants  -- not served
main         -- primary — at the project URL

$ ddev tryout worktree unserve <TAB>          # only the served ones
benni   -- served · PHP 8.5
jochen  -- served · PHP 8.2

$ ddev tryout worktree serve wonka --php <TAB>   # what that Core's composer.json accepts
8.5  -- default for wonka

$ ddev tryout worktree add <TAB>
name for the new worktree (becomes worktrees/<name>)

$ ddev tryout checkout <TAB>
main  -- latest development — checked out
14.3  -- release branch
13.4  -- release branch
...

$ ddev tryout exec <TAB>
@primary  -- the primary site at the project URL
benni     -- served worktree · PHP 8.5
```

Flags already on the line are not offered again, `use` and `remove` leave out the
primary, `--` shows flags alone, and where the next word is free text a hint says
what it is for. Descriptions show in Zsh and Fish (Bash needs 4.4+; older versions
just show the names). One limit is DDEV's: it cannot suppress file-name completion,
so under a hint your shell still lists files.

Branches come from the refs already in the project root, never from the network, so a
<kbd>Tab</kbd> never stalls; `ddev tryout download` and `checkout` both fetch, so the
list stays current. Everything else is a directory listing or a marker file — a
<kbd>Tab</kbd> answers in a few tens of milliseconds.

**How it is wired up.** Nothing is generated, so there is nothing to run after an
install or an update. DDEV completes the command names itself: every script in
`.ddev/commands/host/` is a `ddev` subcommand, which is how `ddev tryout` gets
there. Everything after `ddev tryout` comes from
`.ddev/commands/host/autocomplete/tryout`, which DDEV runs on each <kbd>Tab</kbd>
and which asks the binary (`tryout __complete`). This also means a stray script
in `.ddev/commands/host/` shows up as a command until you delete it.

What it does need is DDEV's own shell completion, once per machine; the add-on
cannot install that for you. Homebrew installs the scripts with DDEV, but Bash
also needs `brew install bash-completion` and Zsh needs
`$(brew --prefix)/share/zsh/site-functions` on `FPATH` before `compinit`.
Elsewhere, `ddev completion <shell>` prints the script. See
[DDEV's shell completion docs](https://docs.ddev.com/en/stable/users/install/shell-completion/)
for your shell and platform.

### Guided commands

Leave an argument out and the command asks for it instead of failing:

```console
$ ddev tryout worktree serve          # pick from the worktrees not served yet
Serve which worktree?
> fancy-pants
  main
  wonka

$ ddev tryout checkout                # pick a branch: main, then releases newest first
$ ddev tryout worktree add            # asks for the name, then the branch
$ ddev tryout worktree rename         # pick the worktree, then type the new name
$ ddev tryout exec                    # pick the site, then type the command
```

`use`, `remove` and `unserve` ask the same way, each offering only
what makes sense — `unserve` lists served worktrees, `use` leaves out the primary.
Typing filters a list, <kbd>Esc</kbd> cancels. In a script or a pipe there is no
one to ask: a line piped to the command is taken as the answer, and with nothing
to read the command prints its usage line and exits 1.

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

Doctor reports whether each piece is wired up and probes Gerrit SSH live (requires a public key uploaded
at https://review.typo3.org/settings/#SSHKeys) —
twice: from inside the container, where `ddev auth ssh` supplies the keys, and
from the host, whose own agent is what a `git push` from a host shell uses.

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

### Browsing what is open

Run it bare and it asks which site to patch first (once something besides the
primary is served), then lists the 50 most recent changes up for review **on that
site's branch** — a 13.4 site is offered 13.4's changes, not main's:

```bash
ddev tryout patch
```

```text
? Apply which changes?
  [ ] 95347   [TASK] Skip database setup for database-free…   Wouter Wolters    CR+1 V+1
> [x] 95074   [BUGFIX] Avoid stale deleted state on reproc…   Benni Mack        CR+2 V+2
  [ ] 95671   [BUGFIX] Ensure numeric site identifiers sta…   Oli Bartsch       CR+1 V+1
  [ ] 94993   [FEATURE] Add table-specific hidden record v…   Matthias Vogel    V-2
[↑↓ to move, space to select one, → to all, ← to none, type to filter]
```

The columns are the change number, its subject, its owner and its review state (`CR` is Code-Review, `V` is Verified).
Changes still marked work-in-progress are
prefixed `WIP`. Typing filters the list, Enter applies what is ticked. Everything
picked is applied in the order shown, with a single rebuild at the end, and you
are asked once afterwards whether to add them to your patch list so they come back
on the next `ddev start` — listed by number *and* subject, since that is what ends
up in a file you keep:

```text
  Add to your patch list, so they reapply on every ddev start:
    95347 - [TASK] Skip database setup for database-free functional tests
    93838 - [FEATURE] Translate forms in the backend

? Add them? (y/N)
```

`--all-branches` widens the list beyond the branch in use. The picker needs a
terminal; without one — in `ddev start`, or any script — a bare `patch` applies
the configured list exactly as it always did, and so does a bare `patch` when a
list is configured.

For more than the latest 50, use **Apply Gerrit patch…** in
[the terminal UI](#the-terminal-ui): it searches Gerrit itself (a change number,
words from the commit message, or Gerrit operators such as `owner:jdoe
-is:wip`), pages through every open change 25 at a time and keeps what you ticked
across pages — see [Applying Gerrit changes](#applying-gerrit-changes).

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

TYPO3 uses a `main`-based commit workflow. Usually only mergers commit to a non-main branch only. Even if your fix
targets an earlier version, please provide patches against `main`.

By default tryout clones the `main` branch (latest development). To work
against a different major version:

```bash
ddev tryout checkout 14.3
```

This single command switches the Core branch, regenerates the Composer overlay,
and rebuilds everything. Run without arguments to see all available branches.

Different TYPO3 versions ship different sets of system extensions.
`checkout` handles this automatically: it scans
`typo3/sysext/*/composer.json` and rewrites the `require`
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
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
ddev add-on get bmack/tryout && ddev start   # → https://tryout-main.ddev.site

mkdir ../tryout-v13 && cd ../tryout-v13
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
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
│   │   ├── tryout                # The ddev tryout entry point — hands over to the binary
│   │   └── autocomplete/
│   │       └── tryout            # Tab-completion — the binary's `__complete`
│   ├── tryout/                   # Add-on payload, namespaced so it cannot collide
│   │   ├── tryout                # Runs the build for this machine (host and container)
│   │   ├── bin/                  # The tryout binary: macOS universal, Linux x86_64/aarch64
│   │   ├── VERSION               # Payload version, compared by `ddev tryout status`
│   │   ├── .state/php-versions   # Left by post-start: the PHPs the web image has
│   │   ├── gitmessage.txt        # Commit template installed by `ddev tryout cs`
│   │   └── …                     # Templates copied out on install (see below)
│   ├── config.yaml               # Yours, from `ddev config` (name, docroot, PHP, DB)
│   ├── config.tryout.yaml        # Add-on: TYPO3 env vars + the post-start hook
│   ├── config.tryout-patches.yaml  # Gerrit patch list (yours to edit)
│   ├── config.worktrees.yaml     # Generated by `worktree serve` — hostnames + daemons
│   └── web-build/Dockerfile.tryout  # Builds git ≥ 2.48 into the web image (see below)
├── TYPO3-Instances/              # Every instance, one directory each
│   ├── primary/                  #   served at the project URL
│   │   ├── public/               #     the docroot
│   │   ├── vendor/               #     Composer's install target
│   │   ├── config/system/additional.php   # TYPO3 DB + mail + GFX for DDEV
│   │   ├── composer.tryout.json  #     Composer overlay (add-on owned, generated)
│   │   └── composer.json         #     Yours, if you have one — never rewritten
│   └── <name>/                   #   a served worktree (`worktree serve`)
├── typo3/sysext/                 # Core source — the project root IS the clone
├── Build/                        # Core's OWN build tooling — untouched
├── packages/                     # Custom extensions (path repository)
└── worktrees/<name>/             # Further checkouts, one per `worktree add`
```

Instances live under `TYPO3-Instances/` rather than in `Build/`: that directory is
Core's own — `Gruntfile.js`, `phpstan/`, `Sources/`, some 700 tracked files — and
putting build output there would interleave the two.

**The project root is the TYPO3 Core clone itself.** Everything the add-on
generates is kept out of `git status` by `.git/info/exclude` — local to the clone,
never committed, so none of it can reach a Gerrit patch. Core's own `.gitignore`
is never touched; it has carried `/.ddev/*` since 2018 anyway.

Three files in `tryout/` are templates rather than runtime code:
`additional.php`, `composer.tryout.json` and `patches.yaml` are copied out on
install — into `TYPO3-Instances/primary/` and `.ddev/config.tryout-patches.yaml` —
which is where the entries above come from.

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
        {
            "type": "path",
            "url": "../../packages/*"
        },
        {
            "type": "path",
            "url": "../../typo3/sysext/*",
            "options": {
                "symlink": true
            }
        }
    ],
    "extra": {
        "merge-plugin": {
            "include": [
                "composer.json"
            ]
        }
    }
}
```

The overlay sits in `TYPO3-Instances/primary/`, hence the `../../`. Every system
extension inside the Core clone is required at `@dev`. Composer
resolves them from the local path and creates symlinks, so any edit inside
the project root is immediately active — no reinstall needed. The same mechanism
applies to `packages/*`: local extensions are symlinked into `vendor/` and behave
as if they were installed from Packagist.

`composer-merge-plugin` pulls your own `composer.json` in as an include, so if the
project had dependencies before you installed tryout they keep resolving. Your file
is only ever read, never written.

The `require` block of the overlay is generated: `ddev tryout composer` (run by
`checkout` and on every start) scans `typo3/sysext/*/composer.json` and rewrites
it, which is what keeps the sysext list correct across `ddev tryout checkout`.
It requires only what is on disk and drops `composer.lock`, so a sysext Core
removed really goes. Anything you add to the overlay that is not a `typo3/cms-*`
or `typo3/theme-*` package is preserved. `worktree use` points the sysext
repository at a worktree instead — `../../worktrees/<name>/typo3/sysext/*`.

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

### Where the work runs

`ddev tryout` is a host command, and the host keeps what only the host can do:
the prompts and pick-lists, the confirmation before `delete`, tab completion, and
opening the browser. **The work of every other verb runs inside the web
container** — the host resolves the arguments, then makes one `ddev exec` of the
same program, `.ddev/tryout/tryout ctr <verb>`, its Linux build. In there git,
Composer, PHP and the database clients are the container's own, so a Core clone, a
cherry-pick or a `composer install` all use the same tools TYPO3 itself runs on,
and never a host PHP or a host Composer.

Two consequences are worth knowing:

- **Git worktrees are shared between both sides.** A worktree's metadata records
  paths, and an absolute path is right on one side of the container boundary only.
  tryout therefore runs the Core repository with `worktree.useRelativePaths`, which
  git learned in 2.48, so a worktree the container created reads fine in your host
  editor and in `git status` on the host — and vice versa. Debian trixie,
  the base of DDEV's web image, ships git 2.47, so the add-on builds git 2.53 into
  the image from a checksum-pinned tarball (`.ddev/web-build/Dockerfile.tryout`).
  That costs about a minute the first time the image is built and nothing after.
  A host git older than 2.48 still reads such worktrees; installation notes it.
- **SSH keys come from `ddev-ssh-agent`** when something inside the container talks
  to Gerrit over SSH. Run `ddev auth ssh` once to hand it your host keys. Pushing
  from a host shell keeps using your host agent, and `ddev tryout cs doctor` reports
  both.

### Post-Start Hook

On every `ddev start` the post-start script runs inside the web container, as an
`exec` hook:

1. **Clone** — if the project root does not exist, clones from GitHub and adds a
   Gerrit remote.
2. **Patch** — if `TRYOUT_PATCHES` is set, resets Core to the current branch
   and cherry-picks each change via the Gerrit REST API.
3. **Sync + Composer install** — regenerates `composer.tryout.json` from the
   sysexts in this Core checkout, then resolves everything from the path repositories.
   The clone happened right where Composer runs, so nothing waits on a file sync.
4. **TYPO3 setup** — on first run, creates `settings.php` and sets up the database.
5. **Extension setup + cache flush** — activates extensions and clears caches.

### Gerrit Integration

Patches are resolved through the Gerrit REST API at `https://review.typo3.org`.
Given a change number (e.g. `56947`), the API returns the latest patchset ref (e.g. `refs/changes/47/56947/12`). That
ref is fetched and cherry-picked.

Merged or abandoned changes are detected and skipped. Conflicts abort the
cherry-pick automatically and report the failure.

## Multiple Core Checkouts Side by Side

Within a single tryout you can keep several TYPO3 Core checkouts and switch
between them instantly. They are git worktrees of the Core clone you already
have, so they share one object store: one fetch, a fraction of the disk, and
no second clone.

```bash
ddev tryout worktree add v13 13.4   # create worktrees/v13 at origin/13.4
ddev tryout worktree list           # one card each: base, patches, changes, site
ddev tryout worktree use v13        # serve it at the project URL instead, then rebuild
ddev tryout worktree serve v13      # give it its own URL, PHP and database
ddev tryout worktree unserve v13    # drop that site, keep the worktree
ddev tryout worktree remove v13     # remove the worktree itself
ddev tryout worktree rename v13 old # rename the checkout and its site
ddev tryout worktree help           # flags and the full picture
```

Without `--php`, a served worktree gets **the highest PHP its own Core accepts** —
read from `require.php` in that branch's `composer.json`, not from the project's
PHP version. A 13.4 worktree (`^8.2`) and a main one (`^8.5`) therefore get the
right runtime each, and a branch with an upper bound like `>=8.2 <8.4` gets 8.3.

The same constraint guards every Composer run. A project on PHP 8.4 with Core
`main` (`^8.5`), or a `--php` the branch rejects, stops **before** `composer
install` with the constraint, the version in use and the command that changes
it — `ddev config --php-version=8.5 && ddev restart` for the project, `worktree
serve <name> --php <version>` for a site — instead of Composer's resolver trace.

`add` takes `--serve` and `--php 8.2` (which implies `--serve`).

> `remove` and `unserve` take their flags **after** the name —
> `worktree remove v13 --force`, not `worktree remove --force v13`.

Nothing moves when you add the first worktree: the project root stays the Core
clone and owns the git object store, and every worktree is nested inside it.

```text
./                                   (the root checkout; owns the object store)
worktrees/v13/                       (worktree, detached at origin/13.4)
TYPO3-Instances/primary/             (the instance at the project URL)
TYPO3-Instances/v13/                 (v13's own instance, once it is served)
```

Typical use: run a Gerrit patch against v13 while keeping main untouched.

```bash
ddev tryout worktree add v13 13.4 --serve
ddev tryout patch 56947 v13          # cherry-picked onto v13's Core only
ddev tryout launch v13
```

New worktrees are **detached** at `origin/<branch>` — they get no branch of their
own, so two worktrees off the same base never collide, and removing one leaves
nothing behind. (`--branch` and `--detach` are still accepted and change
nothing.) The base branch is found again by walking back to the nearest
`origin/*` commit, which is how `patch` lists the right changes and `download
<name> --reset` knows what to reset to. Plain `download <name>` refuses on a
detached checkout rather than resetting away commits that may not be pushed yet.
Gerrit is unaffected either way: pushes go to `refs/for/<branch>` from `HEAD`.

`use` makes a worktree the Core behind the **primary** instance by rewriting that
instance's Composer overlay, then runs `composer install` — without it `vendor/`
would keep pointing at the previous checkout. `ddev tryout status` warns if the two
ever drift apart. `worktree use main` switches back: the root goes by its branch's name (`main`
unless it is on another branch).

The guards worth knowing: the active worktree and the root checkout cannot be
removed, and `remove` always asks first, naming the directory it deletes — a Core
checkout always carries untracked files (`vendor/`, `var/`), so git's own refusal
is no protection. `--yes` skips the question, for scripts. A worktree created by an
older version still has a branch; `remove` deletes it if it is merged, keeps it
and says why if it is not, and `--force` drops it anyway.
Since all worktrees share one object store, a single `ddev tryout cs` sets up the
Gerrit hooks and commit template for all of them.

### Serving several sites at once

`use` gives you one site at the project URL. To have every worktree reachable **at the same time**, each on its own
hostname, PHP version and database, serve
it instead:

```bash
ddev tryout worktree add v13 13.4 --php 8.2 --serve
```

```text
https://tryout-git.ddev.site       → the project root  PHP 8.5   db
https://v13.tryout-git.ddev.site   → worktrees/v13    PHP 8.2   db_v13
```

Both run in the same container: the served site gets its own php-fpm on its own
socket, its own vhost, its own `TYPO3-Instances/<name>/` tree with its own `vendor/`, and
its own database. Roughly 175 MB per extra site — the Core object store stays
shared.

A new hostname needs a `ddev restart` — DDEV owns the routing rule and the TLS
certificate, and both are keyed on the set of hostnames — so `serve`, `unserve`,
`rename` and `add --serve` **restart DDEV themselves** whenever that set changed.
Pass `--no-restart`, or set `TRYOUT_NO_RESTART=1`, to skip it and restart later. **Re-serving a site that already exists
applies immediately**, with no restart:
the webserver is reloaded in place, and the other sites keep serving throughout. So changing a
served site's PHP version, for instance, costs a reload rather than a full
container rebuild:

```bash
ddev tryout worktree serve v13 --php 8.3   # applied without a restart
```

#### On another database server

A DDEV project has one database server, set by `ddev config --database=…`, and
every site gets its own database on it. `--db` puts a site on another one — a
type, or a type at a version, written the way DDEV writes it — to try a change
against each:

```bash
ddev tryout worktree serve v13 --db postgres:16
ddev tryout worktree serve v12 --db mariadb:10.11  # the project's type, older
ddev tryout worktree add lite main --db sqlite       # --db implies --serve
ddev tryout worktree serve v13 --db mysql --switch   # move a served site
```

A bare type is its newest version. What can be picked:

| Type | Versions |
|---|---|
| `mariadb` | 11.8, 11.4, 10.11, 10.6 |
| `mysql` | 8.4, 8.0 |
| `postgres` | 18, 17, 16, 15, 14 |
| `sqlite` | whatever PHP brings — no server |

That is a curated part of what DDEV runs: versions current TYPO3 runs on, and
that the web image's clients can talk to (MySQL 9 no longer lets its MariaDB
client log in). get.typo3.org lists MariaDB up to 10.x for TYPO3 12.4 to 14.3;
11.x is DDEV's default and runs them, so that is a note in the picker, not a
refusal.

Every server other than the project's own runs as a service of its own, per
type and version (`tryout-postgres-16`, `tryout-mariadb-10-11`, declared in
`.ddev/docker-compose.tryout-db.yaml`), with a volume of its own — a Postgres 16
data directory is not one 17 can open. The first site on one costs a DDEV
restart to start it; later ones do not. It keeps running while a site runs on
it or holds a database `unserve` kept there, so an `unserve`d site comes back
with its database; once neither is left (`--drop-db`, `worktree remove`), it is
stopped and its volume removed. `ddev delete` removes the volumes of the
servers still running with the project. SQLite needs no server: the database is
a file in the site's `var/sqlite/`, kept aside on `unserve` just the same.

A served site keeps its server unless you say `--switch`, which unserves it —
keeping the old database — and serves it again on the new one. In the terminal
UI the same choice is **Serve on database ▸** for a worktree that is not
served, and **Database: … ▸** (the current one ticked) for one that is: every
type at every version, like the PHP picker. `ddev tryout status` shows which
servers run and which sites are on them.

#### Stopping a site

```bash
ddev tryout worktree unserve v13    # keeps the database; restarts to release the hostname
```

`unserve` removes the site's vhost and its whole `TYPO3-Instances/v13/` tree — including that
`vendor/`, so the ~175 MB comes back — but **keeps the database and the git
worktree**. Serving it again later restores the site with its content intact. Add
`--drop-db` to discard the database too.

To remove the checkout as well, use `ddev tryout worktree remove v13`; it unserves
first if it has to.

The site-scoped commands take an optional site name, defaulting to the primary
so existing usage is unchanged (and asking, on a terminal, once there is a choice):

```bash
ddev tryout patch 93202 v13     # cherry-pick onto that site's Core only
ddev tryout reset v13
ddev tryout checkout 13.4 v13
ddev tryout delete v13          # only that site's DB and fileadmin
ddev tryout delete --all        # every site, named in the confirmation
ddev tryout exec v13 vendor/bin/typo3 cache:flush
ddev tryout launch v13          # open its URL in the browser
```

### Opening a site in the browser

`launch` opens the site of the worktree you are standing in, so from inside a
checkout it needs no argument at all:

```bash
cd worktrees/v13
ddev tryout launch              # https://v13.<project>.ddev.site
ddev tryout launch --backend    # ...and straight into /typo3/
```

Outside a worktree it asks, listing each served site with the URL it would open.
A worktree that is not served has no URL, and `launch` says so rather than
opening some other site's: give it one with `worktree serve`, or make it the
primary with `worktree use`.

### Mutagen and the Core checkout (macOS)

With Mutagen the Core clone lives on both sides: git runs on it inside the
container, and your editor reads it on the host. Its
`.git` — around 650 MB — is part of the sync, and **must stay so**: excluding
the checkouts' `.git` in `.ddev/mutagen/mutagen.yml` would leave the container's git
with nothing to work on. What can safely be excluded is a served site's
`TYPO3-Instances/*/vendor`, which only the container needs; edit that file, **remove the
`#ddev-generated` line** to take ownership of it, and add the path under
`ignore.paths`.

A change made in the container reaches the host after a sync cycle, usually within
seconds; every `ddev tryout` verb that changes the checkout flushes the sync before
it returns.

## Sharing one TYPO3 Core checkout across multiple tryouts

Earlier versions suggested pre-seeding the project root as a worktree of one shared
clone somewhere on the host. That no longer works: git runs inside the web
container, and a worktree whose object store lives outside the project is
unreachable from there. To keep several Core versions side by side, use
`ddev tryout worktree` inside one project — it shares a single object store between
all of them, optionally serves each on its own hostname, and keeps every checkout
where both the host and the container can see it.

## Requirements

- [DDEV](https://ddev.readthedocs.io/en/stable/) v1.24.10+ (enforced by the add-on)
- Docker Desktop or Colima
- Git on the host, 2.48 or newer recommended: the host only *reads* the Core
  checkout (status before the first start, tab completion), but
  worktrees are recorded with relative paths, which older gits handle for reading
  and not for `git worktree` commands. The git that does the work is built into
  the web image by the add-on.
- macOS or Linux (Windows: WSL2). An SSH client on the host is only needed to push
  to Gerrit from a host shell; `ddev auth ssh` covers pushing from the container.

Output is meant to be read by people: `ddev tryout worktree list` draws one card
per worktree — its base branch, the patches on top, uncommitted changes and the
site it serves. **For scripting, use
`ddev tryout worktree list --plain`** — space-padded columns (`NAME HEAD BRANCH STATE PHP DB URL`), which is the format
the Playwright suite
parses and the one that stays stable — or **`--json`**, one array with a fixed set
of keys (`name dir head branch base patches modified untracked primary url php db
subject php_versions changes`; `branch` is `null` for a detached checkout, `dir` is
relative to the project root, `changes` the Gerrit change numbers among the
patches). `--json` prints nothing but the JSON; so do `ddev tryout worktree
branches --json` and `ddev tryout patch --list --json` (the 50 most recent open
changes). The terminal UI is the same program and reads all of this in-process.
With `TRYOUT_EVENTS=1` any command also prints its progress as
`@@tryout {"level":…,"msg":…}` lines.

## The terminal UI

`ddev tryout ui` opens a full-screen workspace for the whole project:

- **Worktrees**, on the left: one row each, with its branch, PHP version, the
  Gerrit changes applied to it, and its URL if it is served.
- **Agents**, below them: every tab running a coding agent (claude, codex, …),
  marked as working, waiting for you, or idle.
- **Activity**, below that: the commands you started, each with its current step
  and then ✓ or ✗.
- **Tabs**, on the right: the selected worktree's shells, as many as you open.
  They keep running while you look at another worktree.

Every tryout command for the selected worktree is behind `a` or a right-click.
The TUI asks what a command needs itself, in a form: names, branches, Gerrit
changes, and a confirmation naming what a destructive command removes. It then
runs the command as a job in the Activity panel while you keep working. If a command asks for your password (DDEV does, now and then, for
`/etc/hosts`), a popup asks for it.

The keys below apply while the **list** has the focus — the worktrees, agents
and Activity. Once you are in a shell, every key goes to that shell except
<kbd>Ctrl-G</kbd>, which brings you back.

### Moving around

| Keyboard            | Mouse                                 | Does                                                        |
|---------------------|---------------------------------------|-------------------------------------------------------------|
| `↑` `↓` or `k` `j`  | click a worktree                      | select a worktree                                           |
| `Home` / `End`      |                                       | first / last worktree                                       |
| `Enter`, `→` or `l` | click in the pane                     | into the worktree's active shell (a new one if it has none) |
| `Ctrl-G`            |                                       | out of the shell, back to the list                          |
| `{` / `}`           | drag the border between list and pane | narrow / widen the list (double-click the border resets it) |
| `r`                 |                                       | reload the list                                             |

### Shells and tabs

| Keyboard  | Mouse                    | Does                                                                                    |
|-----------|--------------------------|-----------------------------------------------------------------------------------------|
| `t`       | click `+` after the tabs | open a new shell tab in the selected worktree                                           |
| `1` … `9` | click a tab              | switch to that tab                                                                      |
| `[` / `]` |                          | previous / next tab                                                                     |
| `<` / `>` | drag a tab               | move the tab left / right                                                               |
| `,`       | double-click a tab       | rename it (Enter keeps it, Esc cancels, an empty name goes back to its program's title) |
| `w`       |                          | close the tab                                                                           |

### Commands

| Keyboard       | Mouse                          | Does                                                                                               |
|----------------|--------------------------------|----------------------------------------------------------------------------------------------------|
| `a` or `Space` | right-click a worktree         | the commands for that worktree, then the project-wide ones — the same menu both ways |
| `+`            | click **+ new** above the list | create a worktree: asks its name, the branch, and whether to serve it now                          |
|                | click the URL in the title     | open that site in the browser                                                                      |

Inside the menu:

| Keyboard            | Does                                                        |
|---------------------|-------------------------------------------------------------|
| `↑` `↓` or `k` `j`  | choose                                                      |
| `Enter`, `→` or `l` | run it, or open a submenu (marked ▸, e.g. **Serve on PHP**) |
| `←`, `Esc` or `h`   | out of a submenu                                            |
| `Esc` or `q`        | close the menu                                              |

The menu offers what fits the worktree: Serve, Serve on PHP ▸, Serve on
database ▸ (or, once served, PHP ▸ and Database ▸ to switch), Open site, Open
backend, Make primary, Unserve (with or without its database), Update from its
base branch, Switch TYPO3 version…, Apply Gerrit patch…, Reset Core + rebuild,
Run command…, Fresh install…, Rename…, Remove…, Status and Regenerate the overlay.
An entry ending in … asks something first.

### Forms

| Keyboard                      | Does                                                 |
|-------------------------------|------------------------------------------------------|
| `Tab` / `Shift-Tab`           | next / previous field                                |
| `Enter`                       | submit                                               |
| `Esc`                         | cancel                                               |
| typing, `Backspace`, `Ctrl-U` | edit a text field (`Ctrl-U` clears it)               |
| typing, `↑` `↓`               | in a branch list: filter, then choose                |
| `Space`                       | toggle a checkbox                                    |
| `y`                           | confirm a destructive command; any other key cancels |

### Applying Gerrit changes

**Apply Gerrit patch…** opens a form with a search box above a list of open
changes on the worktree's branch, 25 per page, each with its number, subject,
owner and votes (`CR` Code-Review, `V` Verified, `·` no vote yet). The search goes
to Gerrit, not just this page, shortly after you stop typing:

- a number finds that change;
- a term with a colon is a Gerrit operator, e.g. `owner:jdoe`, `-is:wip`,
  `topic:foo`;
- any other word must appear in the commit message.

Terms combine, so `owner:jdoe -is:wip cache` works. `Tab` moves between the search
box and the list.

| Keyboard                                  | Mouse                         | Does                                                                                         |
|-------------------------------------------|-------------------------------|----------------------------------------------------------------------------------------------|
| typing, `Backspace`                       |                               | edit the search (in the search box, `Space` types a space)                                   |
| `↓` or `Tab`                              |                               | from the search box into the list                                                            |
| `↑` `↓`                                   | wheel                         | choose a change (`↑` on the first row goes back to the search)                               |
| `Space`                                   | click a change                | tick or untick it                                                                            |
| `PgUp` / `PgDn`, or `←` / `→` in the list | click `‹ previous` / `next ›` | turn the page (ticks are kept across pages)                                                  |
| a letter                                  |                               | from the list back to the search, typing it                                                  |
| `Enter`                                   |                               | apply the ticked changes in the order you ticked them, or the selected one if none is ticked |
| `Esc`                                     |                               | cancel                                                                                       |

### Opening a pull request

In a project of your own, the menu offers **Open a pull request…** instead: the
same form, listing origin's open pull or merge requests (number, branch, title,
author) through `gh` or `glab`, whichever fits origin and is installed. `Enter`
opens the selected one as the served worktree `pr-<number>` — the same as
`ddev tryout worktree add --pr <number>`, which needs neither tool.

### Activity and logs

| Keyboard | Mouse                   | Does                         |
|----------|-------------------------|------------------------------|
| `L`      | click an Activity row   | open that command's log (`L` opens the first; `↑` `↓` then walk the list) |
| `R`      | click ↻ on a failed row | run the failed command again |

Commands on different worktrees run side by side: a patch on `v13` does not
wait for a rebuild of `main`. Two on the same worktree take turns, and serve,
unserve, rename, remove, Make primary and Regenerate the overlay run **alone** —
they can restart DDEV, which would end every other command, or rewrite
configuration every site shares. A command that has to wait says what for
(`Serve v13 · after Reset main`), and nothing jumps ahead of an earlier one it
conflicts with, so commands on one worktree keep the order you gave them.

A command that succeeded leaves the list after 5 seconds, and a failed one stays
until you retry it. Inside the log:

| Keyboard                          | Mouse                 | Does                  |
|-----------------------------------|-----------------------|-----------------------|
| `↑` `↓`                           |                       | the previous / next command's log, in Activity order (the title shows e.g. `2/4`) |
| `k` `j`                           | wheel (3 lines)       | scroll a line         |
| `PgUp` / `PgDn`, or `b` / `Space` |                       | scroll a page         |
| `Home` / `End`, or `g` / `G`      |                       | top / bottom          |
| `r`                               | click **↻ retry (r)** | run the command again |
| `Esc`, `q` or `Enter`             |                       | close the log         |

### Agents

| Keyboard | Mouse          | Does                                                                        |
|----------|----------------|-----------------------------------------------------------------------------|
| `n`      | click an agent | go to the next agent that needs you: waiting first, then working, then idle |

### Popups

| Popup                                     | Keyboard                                                                                                               |
|-------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| **Password**, when a command asks for one | type it (shown as •), `Enter` sends it to that command, `Esc` or `Ctrl-C` cancels. It is not stored.                   |
| **Rename tab**                            | the old name starts selected, so typing replaces it; `←` `→` `Home` `End` keep it; `Ctrl-U` clears it; `Enter` / `Esc` |
| **Close the session?**                    | `y` closes, any other key keeps it                                                                                     |

While a popup or form is open, clicks outside it do nothing.

### The session

| Keyboard        | Does                                                 |
|-----------------|------------------------------------------------------|
| `q` or `Ctrl-C` | detach: shells, agents and running commands carry on |
| `Q`, then `y`   | close the session and everything in it               |

**It runs as a session, like tmux.** The next `ddev tryout ui` picks the session
up exactly as you left it; only `Q` or `ddev tryout ui stop` ends it. One terminal
is attached at a time, and attaching from another takes the session over. Running
`ddev tryout ui` in one of its own shells is refused.

It is the same program as `ddev tryout` itself (Rust, in `tui/`). A session keeps
running the build it started with, so after an update `ddev tryout ui` says so and
offers to restart the session.

## Contributing

tryout itself lives at [github.com/bmack/tryout](https://github.com/bmack/tryout).
If you have improvements to the add-on — better defaults, new `ddev tryout`
subcommands, fixes to the post-start hook, documentation tweaks — pull requests
and issues are welcome there.

To work on the add-on, install it into a scratch project straight from your
checkout — no release or tarball needed:

```bash
mkdir /tmp/tryout-test && cd /tmp/tryout-test
ddev config --project-type=typo3 --docroot=TYPO3-Instances/primary/public --php-version=8.5
/path/to/your/tryout/checkout/tui/scripts/stage-bins.sh
ddev add-on get /path/to/your/tryout/checkout
ddev start
```

The payload lives at the repo root (`install.yaml`, `commands/`, `tryout/`,
`config.tryout.yaml`) and is copied into the project's `.ddev/` on install. All of
`ddev tryout` is the Rust program in `tui/` (`cargo test` there); the shell left in
the payload is the command and completion shims and the launcher. A release is
made from the Actions tab (`release.yml`): it builds the three binaries and tags a
release-only commit carrying them, since `ddev add-on get` installs the tagged tree.

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
(cd tui && cargo test)    # the program itself — seconds
bats tests/unit.bats      # seconds — the shims, launcher and install.yaml, no containers
bats tests/test.bats --filter-tags '!release'
                          # minutes — install, config, overlay, guarded files, removal
bats tests/lifecycle.bats # much longer — every command, against a real project
bats tests/lifecycle.bats --filter-tags postgres   # the same on Postgres
bats tests --filter-tags '!release,!lifecycle'   # the fast suites
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
