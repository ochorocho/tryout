# Configuration

This page lists every file and environment variable tryout reads or writes. Paths
are relative to the project root. `.ddev/` is your project's DDEV directory.

Every file the add-on ships contains a `#ddev-generated` line. DDEV only updates
or removes files that contain this line. If you want to change such a file and
keep your changes, delete that line: the file is then yours.

## DDEV configuration files

| File | Mode | Written by | Holds |
|---|---|---|---|
| `.ddev/config.yaml` | both | you (`ddev config`) | Project name, type, docroot, PHP and database. tryout never writes it. |
| `.ddev/config.tryout.yaml` | both | the add-on (shipped) | The post-start hook `exec: .ddev/tryout/tryout __post-start`, and nothing else |
| `.ddev/config.tryout-core.yaml` | core | install, from `tryout/config.tryout-core.yaml` | The TYPO3 and Composer environment (below) |
| `.ddev/config.tryout-patches.yaml` | core | install, once | `TRYOUT_PATCHES=`, your Gerrit patch list |
| `.ddev/config.worktrees.yaml` | both | `worktree serve` / `unserve` | Served sites' hostnames and extra PHP-FPM daemons |
| `.ddev/docker-compose.tryout-db.yaml` | both | `worktree serve --db`, on the host | Extra database servers |
| `.ddev/config.local.yaml` | both | you | Your personal overrides. DDEV reads all `config.*.yaml` files in alphabetical order. |

### config.tryout-core.yaml

tryout writes this file only for a TYPO3 Core checkout. A project of your own
keeps its environment as it is. (`COMPOSER=composer.tryout.json` would make its
`ddev composer` look for a file that does not exist.) An update replaces the file
as long as it still has its `#ddev-generated` line. Uninstalling removes it.

```yaml
web_environment:
  - TYPO3_CONTEXT=Development
  - TYPO3_DB_HOST=db
  - TYPO3_DB_PORT=3306
  - TYPO3_DB_DBNAME=db
  - TYPO3_DB_USERNAME=db
  - TYPO3_DB_PASSWORD=db
  - TYPO3_SETUP_ADMIN_USERNAME=admin
  - TYPO3_SETUP_ADMIN_PASSWORD=Password.1
  - TYPO3_SETUP_ADMIN_EMAIL=admin@example.com
  - COMPOSER=composer.tryout.json
  - COMPOSER_POLICY_ADVISORIES_BLOCK=0
```

`COMPOSER_POLICY_ADVISORIES_BLOCK=0` lets Composer install older TYPO3 versions
that require packages with known security advisories. Packages marked as malware
are still blocked, and `composer audit` still reports every advisory.

### config.tryout-patches.yaml

```yaml
web_environment:
  - TRYOUT_PATCHES=56947,12345
```

This file is yours. It has no `#ddev-generated` line, so an update never
overwrites it. Uninstalling removes it only if the list is still empty.
`ddev tryout patch` can add changes to it for you. Your changes take effect on
the next `ddev restart`, or when you run `ddev tryout patch`.

### config.worktrees.yaml

```yaml
additional_hostnames:
  - v13.my-project          # DDEV appends .ddev.site
web_extra_daemons:
  - name: tryout-php-8.2
    command: "/var/www/html/.ddev/tryout/tryout __fpm 8.2"
    directory: /var/www/html
```

tryout writes this file again every time you serve or unserve a site. It is
marked `#ddev-silent-no-warn`. There is one PHP-FPM process for each PHP version
that differs from the project's.

### docker-compose.tryout-db.yaml

This file declares one service per extra database server:

- the service is named `tryout-<type>-<version>`, for example
  `tryout-postgres-16` or `tryout-mariadb-10-11`;
- each has a volume of its own;
- the credentials are `db`/`db`, with `root`/`root` for MariaDB and MySQL.

A server stays in this file as long as a served site uses it, or `unserve` has
kept a database on it. When neither is true any more, tryout restarts DDEV without
the server and removes its data volume. `ddev delete` removes the volumes of the
servers still in the file.

## Per-site state

| Path | Mode | What it is |
|---|---|---|
| `TYPO3-Instances/primary/` | core | The primary instance: `public/`, `vendor/`, `composer.tryout.json`, `config/system/` |
| `TYPO3-Instances/<name>/` | core | A served worktree's instance. `.tryout-site` marks it served. |
| `TYPO3-Instances/.<name>.settings.php` | core | `settings.php` kept by `unserve`, for the project's database server |
| `TYPO3-Instances/.<name>.<type>-<version>.settings.php` | core | The same, for a site on another server |
| `TYPO3-Instances/.<name>.sqlite/` | core | A SQLite database kept by `unserve` |
| `.ddev/tryout-sites/<name>/.tryout-site` | project | Marks a worktree served. It is kept out of your worktree. |
| `.ddev/tryout-sites/<name>/sqlite/` | project | A SQLite site's database file |
| `.ddev/tryout-sites/.<name>[.<type>-<version>].kept` | project | A database `unserve` kept, and the server it is on |
| `.ddev/nginx_full/tryout-site-<name>.conf` | both | The vhost (nginx) |
| `.ddev/apache/tryout-site-<name>.conf` | both | The vhost (Apache) |
| `.ddev/nginx_full/tryout-server-names-hash.conf` | both | nginx's `server_names_hash_bucket_size`, sized for the longest served name |

The `.tryout-site` marker holds two lines: `php=<x.y>` and `db=<type>:<version>`.
If there is no `db=` line, the site uses the project's own database server.

## The add-on's own directory

| Path | What it is |
|---|---|
| `.ddev/tryout/tryout` | The launcher. It picks the right binary for your machine, uses `TRYOUT_BIN` when set, and makes the binary executable again if that got lost. |
| `.ddev/tryout/bin/tryout-{macos-universal,linux-x86_64,linux-aarch64}` | The binaries |
| `.ddev/tryout/VERSION` | The payload version |
| `.ddev/tryout/.version` | The version at install time. `status` compares the two. |
| `.ddev/tryout/.mode` | `core` or `project`, decided at install |
| `.ddev/tryout/.state/php-versions` | The PHP versions the web image has, written by post-start for the host |
| `.ddev/commands/host/tryout` | The `ddev tryout` shim |
| `.ddev/commands/host/autocomplete/tryout` | The completion shim |
| `.ddev/web-build/Dockerfile.tryout` | Builds git 2.53 into the web image. tryout needs it for relative worktree paths. |

## git

| What | Mode | Value |
|---|---|---|
| `.git/info/exclude` | core | `/.ddev/`, `/worktrees/`, `/TYPO3-Instances/`, `/packages/` |
| `.git/info/exclude` | project | `/worktrees/`, `/.ddev/tryout-sites/`, `/.ddev/config.worktrees.yaml`, `/.ddev/docker-compose.tryout-db.yaml`, `/.ddev/nginx_full/tryout-*`, `/.ddev/apache/tryout-*`, `/.ddev/tryout/.state/`, `/.ddev/tryout/.version`, `/.ddev/tryout/.mode` |
| `worktree.useRelativePaths` | both | `true`: worktree metadata is readable on the host and in the container |
| `refs/tryout/pr/<n>` | project | A pull request's head, fetched by `worktree add --pr` |
| `tryout.change-<Change-Id>` | core | The Gerrit change number of an applied patch, so lists can name it offline |
| `tryout.gerritUser` | core | The Gerrit username, cached by `cs setup` |

tryout never changes a `.gitignore` file in your repository. It uses
`.git/info/exclude` instead, which is local and never committed. Uninstalling
removes only tryout's own lines from it.

## Environment variables

### Set by you

In the **Side** column, *host* means your computer and *container* means DDEV's
web container.

| Variable | Side | Effect |
|---|---|---|
| `TRYOUT_BIN` | host / container | Run this binary instead of the shipped one, for example your own build |
| `TRYOUT_CONTAINER_BIN` | host | Passed to the container as its `TRYOUT_BIN`: a `/var/www/html/...` path to a Linux build |
| `TRYOUT_BRANCH` | host → container | The TYPO3 Core branch. Install clones this branch, and it takes priority over the root's branch. |
| `TRYOUT_GERRIT_USER` | host → container | The Gerrit username for `cs setup` |
| `TRYOUT_PATCHES` | web environment | Gerrit changes to apply on every start (`config.tryout-patches.yaml`) |
| `TRYOUT_NO_RESTART` | host | `1` works like `--no-restart` on every verb that restarts DDEV |
| `TRYOUT_EVENTS` | host → container | `1` adds the `@@tryout {…}` event lines (see [commands](/reference/commands#machine-readable-output)) |
| `TRYOUT_GERRIT_API`, `TRYOUT_GERRIT_SSH_HOST` | both | Use another Gerrit server (for tests) |
| `TRYOUT_FPM_RUN_DIR` | container | Where the PHP-FPM sockets go (default `/run/php`) |

`ddev exec` does not pass your environment into the container. So tryout passes
`TRYOUT_BRANCH`, `TRYOUT_GERRIT_USER`, `TRYOUT_PATCHES` and `TRYOUT_EVENTS` on
itself.

### Set by tryout, for a site

Each served site gets these variables, both in the browser (through its web
server configuration) and in `ddev tryout exec`:

| Variable | Mode | Value |
|---|---|---|
| `TRYOUT_SITE` | both | The site's name |
| `TYPO3_DB_DBNAME` | core | The site's database (`db_<name>`) |
| `TRYOUT_DB_DRIVER` | project | `mariadb`, `mysql`, `postgres` or `sqlite` |
| `TRYOUT_DB_NAME` | project | `db_<name>`, or the SQLite file's path |
| `TRYOUT_DB_HOST`, `TRYOUT_DB_PORT` | project | The site's server: `db` or `tryout-<type>-<version>`. Not set for SQLite. |
| `TRYOUT_DB_USER`, `TRYOUT_DB_PASSWORD` | project | `db` / `db`. Not set for SQLite. |
| `TRYOUT_URL` | project | `https://<name>.<project>.ddev.site` |
| `DATABASE_URL` | project | The database as one URL, in the format Doctrine uses, with `serverVersion` |
| framework names | project | `DB_*`, `CRAFT_DB_*`, `SS_DATABASE_*`, `database.default.*` … depending on the type; see [frameworks](/frameworks/) |

The primary is DDEV's own site and gets none of these. A site's database is
called `db_<name>`. Any character other than a letter, a digit or `_` becomes
`_`.

### Set by tryout, internally

`TRYOUT_IN_CONTAINER=1` tells the binary that it runs inside the web container.
`TRYOUT_TUI_SESSION` is set in every shell the terminal UI starts. It names the
session's socket.
