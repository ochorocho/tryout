# Troubleshooting

When something goes wrong, tryout prints an error. Most errors end with a line
that starts with `→`. That line is the command that fixes the problem, or the
next step to take.

This page lists the errors by topic. For each one, it explains why it happens and
what to do. Look for the message you see (use your browser's search).

## Getting started

**`Not inside a DDEV project (DDEV_APPROOT is not set)`**

You started the tryout program directly. → Run it as `ddev tryout …`.

**`Could not run ddev`**

→ Is DDEV installed and on your `PATH`?

**`No tryout build for this machine in .ddev/tryout/bin/`** (during install)

The add-on has no program for your computer. tryout runs on macOS and Linux (on
Windows, use WSL2). → Install again from a release or a branch:
`ddev add-on get bmack/tryout`.

**`This project runs an older copy of the tryout add-on`** (in `status`)

Your project has an older copy of the add-on. It still works, with the commands
it had. → `ddev add-on get bmack/tryout && ddev restart`.

**`` `patch` is not available for this project ``**

Your project is in [project mode](/guide/project-mode). `download`, `checkout`,
`patch`, `reset`, `composer`, `cs` and `worktree use` only work on a TYPO3 Core
checkout.
→ `ddev tryout help` lists what is available. For changes to try, use
[pull requests](/guide/pull-requests).

**`TYPO3 Core not found — the project root is not a git checkout`**

This is Core mode, and TYPO3 Core is not cloned yet. → `ddev tryout download`.

## Worktrees

**`git 2.47 cannot write relative worktree paths (needs 2.48+)`**

DDEV has not yet rebuilt its web container with the newer git the add-on brings.
→ `ddev restart`.

**`Worktree 'x' already exists at worktrees/x`**

→ Pick another name, or use the one you have.

**`'feat-x' would share a database with worktree 'feat_x'`**

Each worktree gets a database named after it, with `.` and `-` turned into `_`.
So these two names would get the same database.
→ Pick a name that differs in more than `.`, `-` and `_`, or rename one:
`ddev tryout worktree rename <name> <new-name>`.

**`Invalid worktree name …`**

Letters, digits, `.`, `_` and `-`, at most 60 characters, not starting with `-`,
and not `primary`.

**`No branch 'x' here or on origin`** (project mode)

→ `git branch --all` lists the branches.

**`Branch 'x' does not exist on origin`** (Core mode)

→ `ddev tryout checkout` lists the available branches.

**`Cannot remove the active worktree 'x'`**

The primary site runs this worktree. → `ddev tryout worktree use <other>` first.

**`Cannot remove 'x': it owns the shared git object store`**

This is the project root's checkout. Every worktree depends on it, so it cannot
be removed.

**`Refusing to remove without confirmation — nothing to ask on.`**

There is no terminal to ask you. → Run it in a terminal, or add `--yes`.

**`Branch '13.4' is checked out in <dir>`**

Git allows a branch in only one worktree at a time. Current tryout versions never
give a worktree its own branch, so this only happens with a worktree an older
version created. → `git -C <dir> checkout --detach`. The worktree keeps the same
code.

## Serving sites

**`'x' runs on MariaDB 11.8 — its settings point there`**

A served site stays on its database server. → Move it with
`ddev tryout worktree serve x --db <type> --switch` (the old database is kept).

**`Unknown database 'x'`**

→ `--db mariadb[:version]`, `mysql[:version]`, `postgres[:version]` or `sqlite`;
the error lists the versions. See [served sites](/guide/sites#database-servers).

**`The PostgreSQL 16 server (tryout-postgres-16) does not answer`**

The extra database server is set up but not running. → `ddev restart`, or serve
again with the same `--db`, which adds it and restarts DDEV.

**`Failed to create database db_x`**

→ Is the server running? `ddev restart`.

**`… requires PHP ^8.5, but site 'x' runs PHP 8.4`**

The branch's `composer.json` does not allow that PHP version. → For a site:
`ddev tryout worktree serve x --php <version>`. For the primary:
`ddev config --php-version=<version> && ddev restart`.

**`PHP 8.3 FPM did not start`**

→ `ddev restart`.

**`Worktree 'x' is not served — it has no URL`** (from `launch`)

→ `ddev tryout worktree serve x`.

**A new site's URL does not answer**

A new URL needs a DDEV restart. `serve` restarts DDEV for you, unless you used
`--no-restart` (or set `TRYOUT_NO_RESTART=1`). → `ddev restart`.

If `*.ddev.site` does not work through DNS on your computer, DDEV writes the URL
into `/etc/hosts` and asks for your password. In the terminal UI, a popup asks
for it.

## Databases

**`No copy from MariaDB 11.8 into PostgreSQL 16 — 'x' starts with an empty database`**

A copy only works between MariaDB and MySQL, or between PostgreSQL servers.
→ Let the app set itself up, or serve the site on the project's database type.

**`Database db_x already holds an install, and there is no saved settings.php for 'x' to go with it.`** (Core mode)

TYPO3's setup does not install into a database that already has data.
→ `ddev tryout worktree unserve x --drop-db`, then serve again.

**`The primary's database is the project's own — tryout leaves it alone`** (project mode)

`delete` only resets served sites. The project's database belongs to DDEV.
→ Use `ddev snapshot` and `ddev import-db` for it, or name a site:
`ddev tryout delete <site>`.

**`Could not drop database db_x — nothing was removed`**

→ Keep it instead: `ddev tryout worktree unserve x` (without `--drop-db`).

**`A TYPO3 site's database is set up by typo3 setup, not copied`**

`--db-from` and `--db-empty` only work in project mode. → Leave them out.

**`No served site 'x' to copy a database from`**

→ `--db-from @primary`, or a site `worktree list` shows as served.

**`Refusing to wipe without confirmation — nothing to ask on.`**

→ `ddev tryout delete <site> --yes`.

## Framework settings (project mode)

**`No DDEV settings file for drupal11 in the project — 'x' uses the primary's database`**

DDEV writes its settings file on start. → `ddev restart`, then serve again.

**`'x' has its own sites/default/settings.ddev.php (committed?) — left as it is`**

The worktree has a committed copy of DDEV's settings file. tryout does not change
committed files, so the site uses the primary's database `db`. → Stop committing
DDEV's settings file (DDEV adds a `.gitignore` for it), then serve again. See
[frameworks](/frameworks/).

## Pull requests

**`This project has no origin to fetch a pull request from`**

→ `git remote add origin <url>`.

**`Origin has no pull or merge request #8`**

→ `git ls-remote origin 'refs/pull/*' 'refs/merge-requests/*'` shows what
origin publishes.

**`'x' is not a pull request number`**

→ `ddev tryout worktree add --pr 123`.

**`A pull request is its own base — leave out the branch`**

`--pr` takes no branch.

## Core mode: updates and patches

**`Detached checkout — update means resetting it to origin/13.4`**

A worktree has no branch of its own to pull into. → `ddev tryout download <site> --reset`
(this throws away local changes), or switch the version with
`ddev tryout checkout <branch>`.

**`Working tree has uncommitted changes`** / **`Pull failed`**

→ `ddev tryout download --reset`.

**`Cherry-pick failed for change 12345 (merge conflict)`**

tryout stopped adding the change. It does not fit this branch.
→ Check the change on Gerrit. To start clean, run `ddev tryout reset`.

**`Failed to fetch ref … from Gerrit`**

→ Verify the change exists and is open on review.typo3.org.

**`Could not reach Gerrit, or no open changes on 13.4`**

→ `ddev tryout patch <change-id>` applies one by number.

**`Failed to sync composer.tryout.json`** / **`Composer install failed`** (on start)

→ `ddev tryout download --reset && ddev restart`.

**`TYPO3 setup failed`**

→ `ddev tryout delete` empties the database and sets TYPO3 up again.

**`'composer' takes no arguments`**

`ddev tryout composer` only updates the overlay file. → `ddev composer …` runs
Composer.

## Contributing

**`Invalid Gerrit username 'x'`** / **`No Gerrit username provided.`**

→ The username shown at <https://review.typo3.org/settings/>:
`ddev tryout cs setup <username>`, or `export TRYOUT_GERRIT_USER=<username>`.

**`Could not set the push URL on origin`**

→ `git remote set-url --push origin <the URL it prints>`.

**Gerrit SSH fails in `cs doctor`**

From the container: run `ddev auth ssh` to give your SSH keys to DDEV. From your
computer: your SSH agent must hold the key you added at
<https://review.typo3.org/settings/#SSHKeys>.

## Terminal UI

**`ddev tryout ui needs a terminal`**

→ Run it from an interactive shell.

**Commands wait in the Activity panel**

Commands that can restart DDEV or change shared settings run alone. The row says
what the command waits for.

**After an update, the UI still behaves the old way**

A session keeps running the tryout version it started with. → `ddev tryout ui stop`,
then `ddev tryout ui`.
