# Silverstripe

## Is it supported?

Yes, through **environment variables**. DDEV writes the database settings into
`.env`. tryout sets the same `SS_*` variables for each served worktree, and a
real environment variable wins over `.env`.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/silverstripe/silverstripe-installer.git silverstripe-tryout
cd silverstripe-tryout
ddev config --project-type=silverstripe --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install

# Build the database
ddev exec vendor/bin/sake db:build --flush
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://docs.silverstripe.org/en/getting_started/).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat -r 'echo getenv("SS_DATABASE_NAME"), "\n";'
```

## What tryout does for you

| Variable | Value |
|---|---|
| `SS_DATABASE_CLASS` | `MySQLDatabase`, `PostgreSQLDatabase` or `SQLite3Database` |
| `SS_DATABASE_SERVER` | `db`, or the site's own database server |
| `SS_DATABASE_PORT` | `3306` or `5432` |
| `SS_DATABASE_NAME` | `db_<site>` |
| `SS_DATABASE_USERNAME`, `SS_DATABASE_PASSWORD` | `db`, `db` |
| `SS_BASE_URL` | `https://<site>.<project>.ddev.site` |

On SQLite, server, port, user and password are not set. PostgreSQL and SQLite
need their Silverstripe modules installed. Every site also gets tryout's own
variables (`TRYOUT_*` and `DATABASE_URL`, see [PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none.

## Commands in this project

This type has no tool command in tryout. Run any PHP file in a site with
`ddev tryout exec <site> <file> …`.

`ddev tryout launch feat --backend` opens the site's admin at `/admin`.

See [commands per project type](/reference/commands#commands-per-project-type).

## What happens to the database

A new site does not start empty. tryout copies the primary's database into it.
The primary is the site DDEV serves at your project's own URL; its database is
called `db`.

- tryout only fills a new, empty database. If you unserved the site earlier and
  kept its database, you get that database back as it was.
- `--db-from <site>` copies from another served site instead. `@primary` means
  the primary.
- `--db-empty` gives the site an empty database. Use it when the app installs
  itself.
- `ddev tryout delete <site>` throws the site's database away and copies the
  primary's again.

A copy only works between servers of the same family: MariaDB and MySQL copy
into each other, PostgreSQL copies into PostgreSQL. Nothing is copied into
SQLite, or from MariaDB/MySQL to PostgreSQL. In that case the site starts empty,
and tryout tells you so. To serve a site on another database type or version,
see [Databases](/guide/sites#databases).

## Is it tested?

Unit tests check the variables. There is no CI job that installs Silverstripe
yet.
