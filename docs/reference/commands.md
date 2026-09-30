# Commands

Every command starts with `ddev tryout`, followed by a verb, for example
`ddev tryout status`.

Some commands only make sense for one kind of project. The **Mode** column tells
you where a command works:

- **both**: a TYPO3 Core checkout and a project of your own
- **core**: only a TYPO3 Core checkout
- **project**: only a project of your own

If you run a command in the wrong mode, it stops with
`` `<verb>` is not available for this project `` and exits with code 1.
[The introduction](/guide/introduction) explains the two modes.

## Overview

| Command | Mode | What it does |
|---|---|---|
| [`status`](#status) | both | Shows an overview of the project |
| [`help`](#help) | both | Lists the commands |
| [`worktree`](#worktree) | both | Creates checkouts side by side and serves them as sites |
| [`exec`](#exec) | both | Runs a command in a site, with that site's PHP version and database |
| [`launch`](#launch) | both | Opens a site in the browser |
| [`delete`](#delete) | both | Wipes a site back to a fresh start |
| [`ui`](#ui) | both | Opens the terminal UI |
| [`download`](#download) | core | Clones or updates TYPO3 Core |
| [`checkout`](#checkout) | core | Switches a site to another TYPO3 version |
| [`composer`](#composer) | core | Regenerates the Composer overlay file |
| [`patch`](#patch) | core | Applies changes from Gerrit |
| [`reset`](#reset) | core | Resets Core to its branch and rebuilds |
| [`cs`](#cs) | core | Sets up your checkout to contribute to TYPO3 Core |

### Naming a site

Many commands take a site name. A site has the name of its worktree.

The **primary** site is the one at the project URL. You can name it in three ways:

- leave the name out;
- write `@primary`;
- write the name of the checkout it currently serves.

If you leave the name out and several sites are served, tryout shows a list to
pick from. This only happens in a terminal. Without a terminal, tryout uses the
primary, or reads one line from standard input.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Done. |
| 1 | Refused or failed. The line before says why. A line starting with `→` says what to run instead. |
| other | `exec` returns the exit code of the command it ran, or 127 when the command is not found. |
| 64 | Internal: the container side got a verb it does not know. |
| 69 | There is no tryout build for this machine. Builds exist for macOS and Linux only. |

## Commands per project type

tryout shows only the commands that make sense for your project, in `ddev
tryout help`, in tab completion, in `ddev help tryout` and in the terminal UI.

- **Every project** has `status`, `help`, `worktree`, `exec`, `launch`, `ui`
  and `delete`. In a project of your own, `delete` resets a site's database to
  a fresh copy of the primary's.
- **Only a TYPO3 Core checkout** has `download`, `checkout`, `composer`,
  `patch`, `reset`, `cs` and `worktree use`. They work on the Core clone, its
  Composer overlay or Gerrit, which a project of your own does not have.
- **Your framework's own tool** is a tryout command named after it. It runs in a
  site, with the site's PHP and its own database: `ddev tryout drush feat
  status` is the same as `ddev tryout exec feat vendor/drush/drush/drush.php
  status`. Use `@primary` for the project's own site.
- **`launch --backend`** opens the admin of your framework, where it has one.

| Project type | Tool command | Runs | `launch --backend` opens |
|---|---|---|---|
| TYPO3 Core checkout | `typo3` | `vendor/bin/typo3` | `/typo3/` |
| `typo3` (a TYPO3 site) | `typo3` | `vendor/bin/typo3` | `/typo3/` |
| `drupal`, `drupal7` – `drupal12` | `drush` | `vendor/drush/drush/drush.php` | `/user/login` |
| `drupal6` | — | — | `/user` |
| `backdrop` | — | — | `/user/login` |
| `wordpress` | `wp` | `/usr/local/bin/wp` (WP-CLI) | `/wp-admin/` |
| `wp-bedrock` | `wp` | `/usr/local/bin/wp` (WP-CLI) | `/wp/wp-admin/` |
| `laravel` | `artisan` | `artisan` | — |
| `symfony` | `console` | `bin/console` | — |
| `shopware6` | `console` | `bin/console` | `/admin` |
| `craftcms` | `craft` | `craft` | `/admin` |
| `codeigniter` | `spark` | `spark` | — |
| `cakephp` | `cake` | `bin/cake.php` | — |
| `silverstripe` | — | — | `/admin` |
| `magento2` | `magento` | `bin/magento` | — (the admin path is set per install) |
| `magento`, `maho` | — | — | `/admin` |
| `joomla` | `joomla` | `cli/joomla.php` | `/administrator/` |
| `modx` | — | — | `/manager/` |
| `asterios`, `php`, `generic` | — | — | — |

Where a type has no tool command, `exec` runs any command in the site:
`ddev tryout exec feat vendor/bin/sake db:build`.

## status

Shows an overview of the project.

```text
ddev tryout status
```

Mode: both. It takes no arguments.

It shows:

- **Mode:** `TYPO3 Core`, or `project (<type>)`.
- **Sites** (project mode): how well tryout supports the project's DDEV type. See
  [frameworks](/frameworks/).
- **Core** or **Repo:** the branch and commit of the project root, and whether it
  has uncommitted changes.
- **Worktree:** the worktrees.
- In core mode also: applied patches, the patch list, `packages/`, Composer,
  TYPO3 setup and the contribution setup.
- **Site:** the URL of the primary site.
- **Database:** the database servers in use, and which sites run on each.

It also warns you when the project runs an older copy of the add-on than the one
you installed from. This check runs on your computer and works while DDEV is
stopped.

## help

Lists the commands, or explains one group of commands.

```text
ddev tryout help
ddev tryout worktree help
ddev tryout launch --help
ddev tryout cs help
```

Mode: both.

## worktree

Creates extra checkouts of your repository next to the main one, and serves them
as sites.

```text
ddev tryout worktree [list|branches|add|use|serve|unserve|remove|rm|rename|help] …
```

Mode: both. Only `worktree use` is core mode only. Without a subcommand,
`worktree` runs `worktree list`.

Some rules for worktrees:

- They live in `worktrees/<name>`.
- They are always **detached**: they do not hold a git branch. This way, any
  number of worktrees can start from the same branch.
- A name can have at most 60 characters. `primary` is reserved.
- Two worktrees cannot share a database. The names `feat-x`, `feat.x` and
  `feat_x` all turn into the database `db_feat_x`, so only one of them is allowed.

### worktree list

Lists all worktrees.

```text
ddev tryout worktree list [--plain|--json]
```

| Flag | Meaning |
|---|---|
| (none) | One card per worktree: base branch, patches on top, uncommitted changes, and the site it serves. |
| `--plain` | Plain columns for scripts: `NAME HEAD BRANCH STATE PHP DB URL`. |
| `--json` | One JSON array, one object per worktree. |

With `--plain`:

- `STATE` is `clean` or `dirty`.
- `PHP`, `DB` and `URL` are `-` when the worktree serves no site.
- The primary's row is marked with `← primary`.

With `--json`, each object has these keys:

```text
name dir head branch base patches modified untracked primary url php db
subject php_versions changes db_engine
```

- `dir` is relative to the project root. `.` is the project root itself.
- `branch` is `null` when the worktree is detached.
- `patches` is the number of commits on top of `base`.
- `changes` lists the Gerrit change numbers among those commits.
- `php_versions` lists the PHP versions the site could run on.

The terminal UI and the browser tests read both formats. New fields may be added,
but existing ones are never renamed or removed.

```bash
ddev tryout worktree list --plain
```

### worktree branches

Lists the branches a new worktree can start from.

```text
ddev tryout worktree branches [--json]
```

- Core mode: the branches of `origin`.
- Project mode: the branches of `origin`, plus your local branches.

It never asks anything. If your clone has only one branch, it fetches the other
branch names once.

| Flag | Meaning |
|---|---|
| `--json` | Print a JSON array of branch names. |

### worktree add

Creates a new worktree, and serves it as a site if you ask for it.

```text
ddev tryout worktree add [<name>] [<branch>] [flags]
ddev tryout worktree add [<name>] --pr <number> [flags]
```

The new worktree is `worktrees/<name>`, starting at `origin/<branch>`:

- **Core mode:** tryout fetches `origin` first.
- **Project mode:** tryout uses `origin/<branch>` if origin has that branch.
  Otherwise it uses your local branch. If the fetch fails, you get the branches you
  already have.

In a terminal, tryout asks for a missing name or branch.

| Flag | Meaning |
|---|---|
| `--serve` | Serve the new worktree right away (see [`worktree serve`](#worktree-serve)). |
| `--php <x.y>` | Serve it with this PHP version. Implies `--serve`. |
| `--db <type>[:<version>]` | Serve it on this database server. Implies `--serve`. |
| `--db-from <site>` | Project mode: copy the database of this site. Implies `--serve`. |
| `--db-empty` | Project mode: start with an empty database. Implies `--serve`. |
| `--pr <number>` | Project mode: open pull request or merge request `<number>` from origin. The name defaults to `pr-<number>`. The site is served. You cannot give a branch as well. |
| `--no-restart` | Do not restart DDEV, even if the new hostname needs it. |
| `--detach`, `--branch` | Still accepted, but they change nothing. |

```bash
ddev tryout worktree add v13 13.4 --serve --php 8.2
ddev tryout worktree add feature-x feature/x --serve --db postgres:16
ddev tryout worktree add --pr 42
```

About `--pr`: your git on the host fetches the pull request. It fetches
`refs/pull/<n>/head` (GitHub) or `refs/merge-requests/<n>/head` (GitLab) into
`refs/tryout/pr/<n>`. If origin is neither GitHub nor GitLab, tryout tries both.
In core mode, `--pr` is refused, and tryout points you to `patch`. See
[pull requests](/guide/pull-requests).

### worktree serve

Gives a worktree its own site at `https://<name>.<project>.ddev.site`.

```text
ddev tryout worktree serve <name> [--php <x.y>] [--db <type>[:<version>]] [--switch]
                                  [--db-from <site>|--db-empty] [--no-restart]
```

Mode: both. The site gets its own PHP, its own web server configuration and its
own database.

| Flag | Meaning |
|---|---|
| `--php <x.y>` | Run the site on this PHP version. |
| `--db <type>[:<version>]` | Run the site on this database server. |
| `--switch` | Move an already served site to the server given with `--db`. |
| `--db-from <site>` | Project mode: copy the database of this site. |
| `--db-empty` | Project mode: start with an empty database. |
| `--no-restart` | Do not restart DDEV, even if the new hostname needs it. |

```bash
ddev tryout worktree serve v13 --php 8.3
ddev tryout worktree serve v13 --db mysql --switch
ddev tryout worktree serve try --db-empty
```

**PHP version.** Without `--php`, the site gets the highest PHP version that the
worktree's `composer.json` allows and that DDEV's web image has. If you pick a PHP
version the worktree does not allow, tryout refuses before Composer runs.

**Core mode** builds a TYPO3 instance in `TYPO3-Instances/<name>/`:

- it creates the Composer overlay for that worktree;
- it runs `composer install`;
- it runs `typo3 setup`, or brings back the saved `settings.php` if the site's
  database was kept from before.

**Project mode** serves the worktree as it is:

- It runs the worktree's own `composer install` through `exec`, so Composer
  scripts already see the site's database.
- A new database starts as a copy of the primary's database.
- If the worktree has no `.env`, `.env.local` or settings file, it gets a copy of
  the project's.

**Database servers.** `--db` accepts these:

| Type | Versions |
|---|---|
| `mariadb` | 11.8, 11.4, 10.11, 10.6 |
| `mysql` | 8.4, 8.0 |
| `postgres` | 18, 17, 16, 15, 14 |
| `sqlite` | no server, no version |

A type without a version means its newest version. A site that is already served
stays on its server. To move it, add `--switch`: tryout stops serving the site
(and keeps the old database), then serves it again on the new server.

**Where a new database comes from** (project mode):

- By default: a copy of the primary's database.
- With `--db-from <site>`: a copy of that site's database.
- With `--db-empty`: nothing, it starts empty.

Only a new database gets filled. A database kept by `unserve` is used as it is.
A copy only works between MariaDB and MySQL, or from Postgres to Postgres. In all
other cases the site starts empty, and tryout tells you so. `--switch` and
`rename` copy the site's own old database.

**Restarts.** A new hostname needs a DDEV restart, and tryout does it for you.
Serving a site again, for example to change the PHP version, only reloads the web
server. With `--no-restart`, or with `TRYOUT_NO_RESTART=1`, tryout skips the
restart and you run `ddev restart` later.

### worktree unserve

Stops serving a site. The worktree stays.

```text
ddev tryout worktree unserve <name> [--drop-db] [--no-restart]
```

What happens to the files:

- **Core mode:** tryout deletes `TYPO3-Instances/<name>/`. The database stays.
  tryout saves the site's `settings.php` as
  `TYPO3-Instances/.<name>[.<type>-<version>].settings.php`. A SQLite database is
  moved to `TYPO3-Instances/.<name>.sqlite/`.
- **Project mode:** tryout removes only its own marker. The database stays, and a
  `.kept` note in `.ddev/tryout-sites/` remembers which server holds it.

| Flag | Meaning |
|---|---|
| `--drop-db` | Delete the database too. This happens first, while tryout still knows which server holds it. |
| `--no-restart` | Do not restart DDEV. |

Put the flags after the name.

```bash
ddev tryout worktree unserve v13 --drop-db
```

### worktree remove

Deletes a worktree and its directory.

```text
ddev tryout worktree remove|rm <name> [--force] [--yes] [--no-restart]
```

If the worktree is served, tryout stops serving it first and deletes its
database. tryout always asks before it deletes, and names the directory. You
cannot remove the project root, or the worktree the primary site serves.

| Flag | Meaning |
|---|---|
| `--yes` | Do not ask. |
| `--force` | Also delete an unmerged branch (see below). |
| `--no-restart` | Do not restart DDEV. |

A worktree created by an older version of tryout may still hold a branch. tryout
deletes that branch if it is merged. An unmerged branch stays, unless you add
`--force`.

When no site and no kept database needs a database server any more, tryout stops
that server and removes its data volume.

```bash
ddev tryout worktree remove v13 --yes
```

### worktree rename

Renames a worktree. Its branch stays the same.

```text
ddev tryout worktree rename <old> <new>
```

A served site moves with the worktree: tryout stops serving it (keeping its
database) and serves it again under the new name. In project mode, the new site's
database starts as a copy of the old one.

```bash
ddev tryout worktree rename v13 v13-cache
```

### worktree use

Makes a worktree the TYPO3 Core behind the primary site.

```text
ddev tryout worktree use <name>
```

Mode: core. tryout points the primary's Composer overlay at the worktree's system
extensions and rebuilds `vendor/`. To switch back to the project root, run
`worktree use` with the root's branch name.

```bash
ddev tryout worktree use v13
```

## exec

Runs a command in a site: in the site's folder, with the site's PHP version
and its own database.

```text
ddev tryout exec <site> <command> <arguments…>
```

Mode: both.

- **A PHP program runs with the site's PHP.** That is `php` itself, a PHP option
  like `-r`, a `.php` or `.phar` file, or a script whose first line names PHP —
  `composer`, `artisan`, `bin/console`, `vendor/bin/typo3`. tryout uses `php`, or
  `php<x.y>` when the site runs a different PHP version than the project.
- **Anything else runs as it is**: `bash -c '…'`, a shell script like
  `vendor/bin/drush`, any program in the web container.
- No shell splits the arguments again.
- It runs in the site's root directory.
- It sets the site's database variables:
  - core mode: `TYPO3_DB_DBNAME`;
  - project mode: `TRYOUT_DB_*`, `TRYOUT_URL`, `DATABASE_URL` and the framework's
    own names (see [frameworks](/frameworks/)).
- It sets `TRYOUT_SITE` to the site's name.

The exit code is the exit code of your command.

```bash
ddev tryout exec v13 vendor/bin/typo3 cache:flush
ddev tryout exec feat artisan migrate
ddev tryout exec feat vendor/bin/drush status
ddev tryout exec feat composer show
ddev tryout exec feat php -r 'echo getenv("TRYOUT_DB_NAME");'
ddev tryout exec feat bash -c 'ls -la web/sites/default'
```

For your framework's own tool there is a shorter command, named after it, like
`ddev tryout drush feat status` — see
[commands per project type](#commands-per-project-type).

In a terminal, tryout asks for a missing site or command.

## launch

Opens a site in your browser.

```text
ddev tryout launch [<worktree>] [--backend|-b]
```

Mode: both. It runs on your computer, not in DDEV.

- Inside a worktree directory, it opens that worktree's site.
- Otherwise it shows a list to pick from.

A worktree that is not served has no URL, and `launch` tells you so.

| Flag | Meaning |
|---|---|
| `--backend`, `-b` | Open the admin area instead (`/typo3/` for TYPO3 Core). A project of your own has none, so this is refused there. |

```bash
ddev tryout launch v13 --backend
```

## delete

Wipes a site back to a fresh start.

```text
ddev tryout delete [<site>|--all] [--yes|-y]
```

Mode: both, but it works differently per mode:

| | Core mode | Project mode |
|---|---|---|
| `delete <site>` | Recreates the site's database, empties `fileadmin/`, removes `settings.php`, then runs `typo3 setup` and `extension:setup` | Recreates the site's database as a fresh copy of the primary's |
| `delete` (the primary) | The same, for the primary | Refused: the project's own database belongs to DDEV. Use `ddev snapshot` and `ddev import-db` instead. |
| `delete --all` | The primary and every served site | Every served site, but never the primary |

Before it deletes anything, tryout lists what goes and asks you.

| Flag | Meaning |
|---|---|
| `--all` | Every site (see the table). |
| `--yes`, `-y` | Do not ask anything. Without a site name, this means the primary. |

Without a terminal and without `--yes`, tryout refuses, because there is nobody
to ask.

```bash
ddev tryout delete v13 --yes
```

## ui

Opens the terminal UI.

```text
ddev tryout ui
ddev tryout ui stop
```

Mode: both. It runs on your computer and needs a terminal. It connects to the
project's terminal UI session, and starts the session if needed.

- `q` leaves the UI. The session keeps running.
- `Q`, or `ddev tryout ui stop`, ends the session.

See [terminal UI](/guide/terminal-ui).

## download

Clones TYPO3 Core, or updates it.

```text
ddev tryout download [<site>] [--reset|-r]
```

Mode: core.

- **If Core is not cloned yet:** it clones TYPO3 Core into the project root.
- **Otherwise:** it updates the site's Core with `git pull --rebase` on its base
  branch, then rebuilds the site.

| Flag | Meaning |
|---|---|
| `--reset`, `-r` | Reset the site's Core to `origin/<base>` instead of updating it. Applied patches and local changes are lost. |

A worktree is always detached, so it can only be updated with `--reset`. Without
it, `download` refuses.

```bash
ddev tryout download v13 --reset
```

## checkout

Switches a site to another TYPO3 version.

```text
ddev tryout checkout <branch> [--site <site>]
ddev tryout checkout <branch> <site>
```

Mode: core. tryout switches the site's Core to `origin/<branch>` (`main`, `13.4`,
`12.4` …). Then it regenerates the overlay, deletes `vendor/` and rebuilds.

- **A worktree site** switches **detached** and does not hold the branch. So
  several sites can run the same version.
- **The project root** takes the branch itself. If a worktree still holds that
  branch from an older version of tryout, tryout first detaches that worktree at
  the same commit. Nothing is lost.

| Flag | Meaning |
|---|---|
| `--site <site>` | The site to switch. Default: the primary. |

In a terminal, tryout asks for a missing branch.

```bash
ddev tryout checkout 13.4
ddev tryout checkout 13.4 --site v12
```

## composer

Regenerates the Composer overlay file of the primary site.

```text
ddev tryout composer
```

Mode: core. It takes no arguments. tryout rewrites the `require` block of the
primary's `composer.tryout.json` from the system extensions on disk. It does
**not** run Composer. Use `ddev composer …` for that.

## patch

Applies changes from Gerrit to a site's TYPO3 Core.

```text
ddev tryout patch [<change>…] [--site <site>]
ddev tryout patch <change> <site>
ddev tryout patch --list [--json] [--all-branches] [--site <site>]
```

Mode: core. tryout cherry-picks the latest version (patch set) of each change,
then rebuilds the site once.

| How you call it | What happens |
|---|---|
| `patch <change>…` | Applies these changes. |
| `patch`, with a patch list configured | Applies the changes in `TRYOUT_PATCHES`. |
| `patch`, with no list, in a terminal | Shows the open changes for the site's branch. You pick some, and tryout offers to add them to the patch list. |

| Flag | Meaning |
|---|---|
| `--site <site>` | The site to patch. Default: the primary. |
| `--list` | Print up to 50 open changes, tab-separated: number, subject, owner, votes. |
| `--json` | With `--list`: print a JSON array of `{"number","subject","owner","scores"}`. |
| `--all-branches` | With `--list`: list changes on every branch, not only the site's. |

Good to know:

- Merged and abandoned changes are skipped. So is a change that is already
  applied.
- If a change does not apply cleanly, tryout stops the cherry-pick.
- If `patch <change>` fails, it exits with code 1.

```bash
ddev tryout patch 91234 --site v13
```

See [Gerrit patches](/guide/gerrit).

## reset

Resets a site's TYPO3 Core to its branch and rebuilds.

```text
ddev tryout reset [<site>]
```

Mode: core. tryout resets the Core to `origin/<base>`. Applied patches and local
changes are lost. Then it rebuilds the site.

```bash
ddev tryout reset v13
```

## cs

Sets up your checkout so you can contribute to TYPO3 Core.

```text
ddev tryout cs [setup [<user>]|doctor|uninstall|help]
```

Mode: core. Without a subcommand, `cs` runs `setup`.

| Subcommand | What it does |
|---|---|
| `setup [<user>]` | Installs the git hooks (commit-msg, pre-commit), the commit message template and the Gerrit push URL. It also sets your git author name and email from your Gerrit account. |
| `doctor` | Checks the setup, including the SSH connection to `review.typo3.org`. |
| `uninstall` | Removes the setup again. |
| `help` | Explains the subcommands. |

tryout looks for your Gerrit username in this order: the argument,
`TRYOUT_GERRIT_USER`, `git config tryout.gerritUser`, and finally it asks you.
All worktrees share the same setup.

```bash
ddev tryout cs setup jdoe
```

See [contributing to TYPO3 Core](/guide/contributing-to-core).

## Machine-readable output

These outputs are meant for scripts and tools:

| Output | Format |
|---|---|
| `worktree list --plain` | Columns `NAME HEAD BRANCH STATE PHP DB URL` |
| `worktree list --json` | The keys listed under [worktree list](#worktree-list) |
| `worktree branches --json` | A JSON array of branch names |
| `patch --list --json` | `[{"number","subject","owner","scores"}]` |
| `TRYOUT_EVENTS=1` | Every info, success, warning and error line is also printed as `@@tryout {"level":"info","msg":"…"}` on the same output. The terminal UI reads these lines. |

New fields and columns may be added. Existing ones are never renamed or removed.

## Internal commands

tryout calls these commands itself. They do not show up in `help` or in tab
completion.

| Command | Called by |
|---|---|
| `__post-start` | The DDEV post-start hook in `config.tryout.yaml` |
| `__fpm <x.y>` | The PHP-FPM process for one PHP version, in `config.worktrees.yaml` |
| `__mode [<root>]` | `install.yaml`. Prints `core` or `project`. |
| `__complete …` | Tab completion |
| `__version` | Prints the version. It also keeps the `#ddev-generated` marker in the binary. |
| `ctr <verb> …` | The host side, to run a command inside the web container (`ddev exec --raw`) |
