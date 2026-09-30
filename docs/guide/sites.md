# Served sites

A worktree becomes a **site** when you serve it. It then has its own URL, its own
PHP version and its own database. It runs in the same DDEV web container as the
primary.

```bash
ddev tryout worktree serve v13 --php 8.2   # serve the worktree v13 on PHP 8.2
```

```text
https://my-project.ddev.site       → the primary        PHP 8.5   db
https://v13.my-project.ddev.site   → worktrees/v13      PHP 8.2   db_v13
```

Each served site gets its own PHP process, its own web server entry and its own
database. In Core mode it also gets its own TYPO3 site folder
`TYPO3-Instances/<name>/` with its own `vendor/` (about 175 MB per site). In
project mode the worktree itself is the app.

What exactly happens when you serve a site:
[project mode](/guide/project-mode#serving-a-worktree),
[TYPO3 Core mode](/guide/typo3-core#several-core-versions-in-one-project).

## The worktree commands

```bash
ddev tryout worktree add v13 13.4 [--serve] [--php 8.2] [--db postgres:16]   # create
ddev tryout worktree list [--plain|--json]                                   # list
ddev tryout worktree serve v13 [--php 8.3] [--db mysql] [--switch]           # serve
ddev tryout worktree unserve v13 [--drop-db]                                 # stop serving
ddev tryout worktree rename v13 old                                          # rename
ddev tryout worktree remove v13 [--force] [--yes]                            # delete (also: worktree rm)
ddev tryout worktree branches [--json]                                       # list branches
ddev tryout worktree help                                                    # show help
```

Put flags **after** the name: `worktree remove v13 --force`, not
`worktree remove --force v13`. All flags are listed in
[commands](/reference/commands).

## PHP per site

Without `--php`, a site gets the newest PHP version its own `composer.json`
allows (`require.php`), out of the versions DDEV's web image has. For example, a
branch that needs `>=8.2 <8.4` gets PHP 8.3.

You can change it at any time:

```bash
ddev tryout worktree serve v13 --php 8.4    # takes effect at once, no restart
```

The primary always runs on the project's PHP version (`ddev config --php-version`).

## Databases

Every site has its own database. By default it is on the project's database
server. You can also put a site on another server.

### Database servers

A DDEV project has one database server (`ddev config --database=…`). Every site
gets its own database on it. With `--db` you put a site on another server. Write
it the way DDEV does: a type, or a type and a version.

```bash
ddev tryout worktree serve v13 --db postgres:16       # PostgreSQL 16
ddev tryout worktree serve v12 --db mariadb:10.11     # an older MariaDB
ddev tryout worktree add lite main --db sqlite        # SQLite (--db also serves the site)
```

A type without a version means its newest version. You can choose from:

| Type | Versions |
|---|---|
| `mariadb` | 11.8, 11.4, 10.11, 10.6 |
| `mysql` | 8.4, 8.0 |
| `postgres` | 18, 17, 16, 15, 14 |
| `sqlite` | no server, no version |

This is a selection of what DDEV runs: versions that current apps use, and that
the web image's database clients can connect to. (MySQL 9 no longer accepts the
MariaDB client.) In Core mode, the picker notes that get.typo3.org lists MariaDB
only up to 10.x for TYPO3 12.4 to 14.3. MariaDB 11.x is DDEV's default and runs
them fine.

How extra servers work:

- Every server other than the project's runs as its own service, one per type
  and version (for example `tryout-postgres-16`, `tryout-mariadb-10-11`). They
  are listed in `.ddev/docker-compose.tryout-db.yaml`, each with its own data
  volume.
- The first site on a new server needs a DDEV restart to start that server.
- A server keeps running while a site runs on it or a kept database is stored on
  it. When neither is left, tryout stops it and deletes its data volume.
- `ddev delete` deletes the data volumes of the servers that are still running.

SQLite needs no server. The database is a file:

- in Core mode, in the site's `var/sqlite/`,
- in project mode, in `.ddev/tryout-sites/<name>/sqlite/`.

### Moving a site to another server

A served site stays on its server, unless you add `--switch`:

```bash
ddev tryout worktree serve v13 --db mysql --switch   # move v13 to MySQL
```

`--switch` stops the site, keeps its old database, and serves it again on the new
server. Without `--switch`, tryout refuses to change the server, because the
site's settings point to the old one. In project mode, the new database starts as
a copy of the old one if both are in the same family.

## Unserving and removing

```bash
ddev tryout worktree unserve v13             # stop serving; keep the worktree and the database
ddev tryout worktree unserve v13 --drop-db   # stop serving and delete the database
ddev tryout worktree remove v13              # stop serving if needed, then delete the worktree
```

`unserve` removes the site's web server entry. In Core mode it also deletes the
site's TYPO3 folder, `vendor/` included. It keeps the database and the git
worktree. When you serve the site again, it comes back with its content:

- In Core mode, tryout saves the site's `settings.php` next to the kept database
  and puts it back.
- In project mode, tryout simply uses the kept database again.

`remove` always asks first and names the folder it deletes. A worktree always has
files git does not track (`vendor/`, `var/`), so git's own safety check does not
protect you. `--yes` skips the question, for scripts.

## URLs and restarts

DDEV manages the routing and the TLS certificate for all URLs of a project. When
the list of URLs changes, DDEV must restart. So `serve`, `unserve`, `rename`,
`add --serve` and `remove` **restart DDEV for you** when a URL is added or
removed.

Serving a site again that already has its URL (for example to change `--php`)
takes effect at once. The web server reloads, and the other sites keep running.

To serve several worktrees and restart only once:

```bash
ddev tryout worktree serve a --no-restart
ddev tryout worktree serve b --no-restart
ddev restart
```

`TRYOUT_NO_RESTART=1` does the same for every command.

### `/etc/hosts`

If `*.ddev.site` does not work through DNS on your computer, DDEV writes each URL
into `/etc/hosts`. That needs your password (`sudo`). A command you start in a
terminal shows the password prompt. In the [terminal UI](/guide/terminal-ui), a
popup asks for it and hides what you type.

## Naming sites

Wherever a command needs a site, you can use a worktree's name. That includes the
name the primary has in `worktree list`, so `ddev tryout exec main …` means the
primary. `@primary` always means the primary.

Commands for one site take its name as an argument, or ask you:

```bash
ddev tryout exec v13 vendor/bin/typo3 cache:flush   # run PHP in v13
ddev tryout launch v13                 # open its URL
ddev tryout delete v13 --yes           # reset it
ddev tryout patch 93202 v13            # Core mode (or --site v13)
ddev tryout checkout 13.4 v13          # Core mode (or --site v13)
ddev tryout reset v13                  # Core mode
ddev tryout download v13 --reset       # Core mode
```

If you leave the name out, `exec`, `patch`, `reset`, `launch` and `delete` ask
which site you mean. (`patch`, `reset` and `delete` only ask once a site besides
the primary is served.)

```text
Reset which site?
> primary       https://my-typo3-site.ddev.site
  v13           https://v13.my-typo3-site.ddev.site  PHP 8.4
  v12           https://v12.my-typo3-site.ddev.site  PHP 8.2
```

## Opening a site

`launch` opens the site of the worktree you are in:

```bash
cd worktrees/v13
ddev tryout launch              # opens https://v13.<project>.ddev.site
ddev tryout launch --backend    # Core mode: opens /typo3/ directly
```

Outside a worktree it asks which site to open. A worktree that is not served has
no URL. `launch` then tells you so, instead of opening another site.

## Lists for scripts

`ddev tryout worktree list` shows one card per worktree: its base branch, the
patches on top, uncommitted changes and the site it serves. For scripts, use:

- `--plain`: columns `NAME HEAD BRANCH STATE PHP DB URL`, padded with spaces. The
  format does not change.
- `--json`: one array with fixed keys: `name dir head branch base patches
  modified untracked primary url php db subject php_versions changes`. `branch`
  is `null` for a worktree without a branch. `dir` is relative to the project
  root.

`worktree branches --json` and `patch --list --json` also print only JSON. With
`TRYOUT_EVENTS=1`, every command also prints its progress as
`@@tryout {"level":…,"msg":…}` lines.

## Mutagen (macOS)

With Mutagen, DDEV keeps a copy of your files in the container and syncs it with
your computer. Git runs on the container side, your editor on your computer.

- The `.git` folders of the checkouts **must** stay in the sync. The container's
  git needs them.
- You can leave out Core mode's `TYPO3-Instances/*/vendor`, which only the
  container needs. To do so, remove the `#ddev-generated` line from
  `.ddev/mutagen/mutagen.yml` (so the file is yours) and add the path under
  `ignore.paths`.

Every `ddev tryout` command that changes files waits for the sync to finish
before it returns.
