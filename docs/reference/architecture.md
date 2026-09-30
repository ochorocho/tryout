# Architecture

This page explains how tryout works inside. You do not need it to use tryout. It
helps when something behaves in a way you did not expect, or when you want to
change tryout.

## One program, two sides

All of `ddev tryout` is one program, written in Rust. The same program is the
command line, the part that does the work inside DDEV, the post-start hook, the
PHP-FPM processes, tab completion and the terminal UI. Its source is in `tui/`.
Only three small shell scripts are left:

- `commands/host/tryout` passes every argument to the program, unchanged.
- `commands/host/autocomplete/tryout` asks the program for tab completions.
- `tryout/tryout` is the launcher. It picks the right binary for your machine
  (using `uname`) and uses `TRYOUT_BIN` when set. It also runs on the old bash 3.2
  that macOS ships.

The binaries are part of the add-on, in `tryout/bin/`:

| File | For |
|---|---|
| `tryout-macos-universal` | macOS, arm64 and x86_64 |
| `tryout-linux-x86_64` | Linux and the web container |
| `tryout-linux-aarch64` | Linux and the web container |

They are committed to the repository, so `ddev add-on get` works for any branch
or checkout without building anything. Each binary contains the text
`#ddev-generated`. That is how DDEV recognises it as a file of the add-on.

### Host and container

`ddev tryout <verb>` starts on the **host**, which is your computer. Some things
only work there, so the host does them:

- prompts and pickers;
- the confirmation before destructive verbs;
- tab completion;
- opening the browser;
- restarting DDEV;
- fetching a pull request with the user's git credentials.

Everything else runs in DDEV's **web container**. The host starts it with one
call:

```text
ddev exec --raw -- env TRYOUT_BRANCH=… TRYOUT_GERRIT_USER=… TRYOUT_PATCHES=… TRYOUT_EVENTS=… \
  /var/www/html/.ddev/tryout/tryout ctr <verb> <args…>
```

`--raw` passes each argument exactly as it is; no shell splits it again. DDEV
does not pass your environment into the container, so the host names the
variables the container needs. Inside the container, tryout uses the same git,
Composer, PHP and database tools your app runs with.

If you use Mutagen (DDEV's file sync on macOS), the host waits for the sync after
every command that changes files. Only `status`, `exec`, `worktree list` and
`cs doctor` skip this. In the code, only the module `core::ddev` runs `ddev`, and
a test checks that.

Both sides read the same git worktrees. Git stores the path of each worktree, and
an absolute path is only right on one side: your computer and the container see
the project at different paths. So tryout sets `worktree.useRelativePaths`, which
needs git 2.48 or newer. The web image is based on Debian trixie, which has git
2.47. That is why `web-build/Dockerfile.tryout` builds git 2.53 from a fixed
source archive. An older git on your computer can still read the worktrees.

## Two modes

tryout looks at the project root to decide the mode (`core::kind::detect_mode`):

- **Core mode:** there is no git repository yet (the install is about to clone
  TYPO3 Core), or the root contains `typo3/sysext/core/composer.json`, or
  `origin` is TYPO3's repository.
- **Project mode:** any other repository. This is a project of your own, of any
  DDEV type.

In the code, everything that differs between the two modes goes through one
interface, `ProjectKind` (`tui/src/core/kind.rs`). It answers:

- the clone source and review remote;
- the base branches;
- the git excludes;
- the PHP constraint;
- the setup and rebuild commands;
- the backend path;
- which verbs apply;
- whether sites seed their databases;
- whether pull requests open.

There are two implementations: `Typo3Core` and `GenericProject`. Worktrees, sites,
database servers, jobs and the terminal UI work the same in both modes.

In project mode, what each *framework* needs comes from `core::types`: which
types are supported and how, and the settings files each site gets.
`core::appenv` holds the environment variable names. See
[frameworks](/frameworks/).

## Directory layout

### Core mode

The project root **is** the TYPO3 Core clone:

```text
my-typo3-site/
├── .ddev/
│   ├── commands/host/tryout (+ autocomplete/tryout)
│   ├── tryout/                     # launcher, bin/, VERSION, .mode, .version, .state/
│   ├── config.yaml                 # yours
│   ├── config.tryout.yaml          # the post-start hook
│   ├── config.tryout-core.yaml     # TYPO3 + COMPOSER environment
│   ├── config.tryout-patches.yaml  # your Gerrit patch list
│   ├── config.worktrees.yaml       # served sites (generated)
│   └── web-build/Dockerfile.tryout
├── TYPO3-Instances/
│   ├── primary/                    # the site at the project URL
│   │   ├── public/                 #   docroot
│   │   ├── vendor/
│   │   ├── composer.tryout.json    #   the Composer overlay
│   │   └── config/system/additional.php
│   └── <name>/                     # a served worktree's instance
├── typo3/sysext/                   # Core's source
├── Build/                          # Core's own build tooling, untouched
├── packages/                       # your extensions (path repository)
└── worktrees/<name>/               # further checkouts
```

TYPO3 instances live in `TYPO3-Instances/`, never in `Build/`, which belongs to
TYPO3 Core itself. tryout adds its generated files to `.git/info/exclude`, so they
do not show up in `git status`. It never changes Core's own `.gitignore`, so none
of this can end up in a Gerrit patch.

### Project mode

Your project stays as it was. tryout adds worktrees, and keeps its own files
inside `.ddev/`:

```text
my-app/
├── .ddev/
│   ├── tryout/ …, config.tryout.yaml, config.worktrees.yaml
│   ├── docker-compose.tryout-db.yaml   # extra database servers, when used
│   └── tryout-sites/<name>/            # markers, SQLite files, kept-database notes
├── (your app)                          # the primary: DDEV's own site
└── worktrees/<name>/                   # a worktree = a site's app, own docroot
```

Your committed `.ddev/` directory stays visible to git. Only the files tryout
generates are hidden from `git status`.

## The Composer overlay (core mode)

An add-on should not rewrite your `composer.json`. So tryout uses a second file
instead, the overlay: `composer.tryout.json` in the instance. The environment
variable `COMPOSER=composer.tryout.json` tells Composer to use it. It declares two
path repositories:

- `../../packages/*`;
- `../../typo3/sysext/*`, symlinked.

Your own `composer.json` is merged in with `composer-merge-plugin`. tryout only
reads it.

The `require` block is generated from the system extensions on disk
(`core::composer::sync`). Packages that are not `typo3/cms-*` or `typo3/theme-*`
stay as they are. The JSON is formatted exactly as PHP's `json_encode` would
format it, so the file does not change without reason.

`worktree use <name>` points the primary's sysext repository at
`../../worktrees/<name>/typo3/sysext/*` and rebuilds.

## Post-start

`ddev start` runs `tryout __post-start` inside the web container.

**Core mode:**

1. Clone Core, if nothing was cloned yet.
2. Apply `TRYOUT_PATCHES`.
3. Sync the overlay, then run `composer install`.
4. Run `typo3 setup` on the first start.
5. Run `extension:setup` and flush caches.

**Project mode:** post-start writes down the PHP versions of the web image for the
host, and keeps the git excludes and relative worktree paths in place. Everything
else about starting your project is left to DDEV.

## Serving

A served site gets the following:

**Its own web server configuration (vhost).** `.ddev/nginx_full/tryout-site-<name>.conf`
or `.ddev/apache/tryout-site-<name>.conf`, with:

- the site's docroot;
- `fastcgi_param HTTPS $fcgi_https` (without it a TLS site builds `http://` URLs);
- the site's environment (`TRYOUT_SITE`, and the database variables).

**Its own PHP-FPM**, when the site runs a different PHP version than the project.
There is one PHP-FPM process per PHP version. `__fpm` starts it before the web
server reloads, and never starts a second one.

**A hostname** in `config.worktrees.yaml`. nginx's `server_names_hash_bucket_size`
is set large enough for the longest site name.

**Reload first, restart only when needed.** tryout copies the vhosts into the
running web server, checks the configuration (`nginx -t` / `apachectl configtest`)
and then reloads it (`supervisorctl signal HUP nginx` / `apachectl -k graceful`).
DDEV only needs a restart when the list of hostnames or database services
changes, because its routing and TLS certificate depend on that list. The host
does the restart, unless you pass `--no-restart`.

## Databases

Every site gets its own database, `db_<name>`, on the project's server or on
another one (`--db`):

- An extra server runs as the service `tryout-<type>-<version>`, with its own
  data volume.
- A server stays as long as a served site uses it, or `unserve` kept a database
  on it.
- When DDEV restarts without a server, the host removes that server's volume.
- SQLite needs no server. The database is a file in the site's tryout folder.

In **project mode**, a fresh database starts as a copy of the primary's, or of
another site's with `--db-from`:

- MariaDB and MySQL copy with `mysqldump | mysql`.
- Postgres copies with `pg_dump | psql`.
- Between other database types nothing is copied, and the site starts empty.
- A kept database is never overwritten.

In **core mode**, `typo3 setup` sets up the site. If the site's database was kept
from before, the site gets its saved `settings.php` back instead.

## The terminal UI

`ddev tryout ui` connects to the project's **session server**. The server keeps
the whole state of the UI and draws the screen. The window you see only shows
what changed. When you press `q`, the window closes, but the session keeps
running, with its jobs and shells.

The session's socket folder must be private: it must belong to you, have mode
0700, and not be a symlink. Commands run as **jobs** in their own pseudo-terminal.
They run side by side, except when they would get in each other's way:

- one job per worktree;
- whatever can restart DDEV, rewrite webserver config or write the primary overlay
  runs alone.

Jobs that write to the shared git repository wait for each other using
`.git/tryout.lock`. The UI reads worktrees, branches, Gerrit changes and pull
requests itself, on the host.
