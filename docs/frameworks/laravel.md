# Laravel

## Is it supported?

Yes. Laravel is supported through **environment variables**. Laravel reads its
database settings from `.env`, but a real environment variable always wins
over `.env`. tryout sets these variables for each served worktree, so each
site talks to its own database.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/laravel/laravel.git laravel-tryout
cd laravel-tryout
ddev config --project-type=laravel --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install

# Create the app key (DDEV has written .env from .env.example)
ddev artisan key:generate

# Create the tables
ddev artisan migrate --force
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://laravel.com/docs/installation).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat artisan tinker --execute='echo DB::connection()->getDatabaseName();'
```

## What tryout does for you

tryout sets Laravel's own variable names for the site:

| Variable | Value |
|---|---|
| `DB_CONNECTION` | `mysql` (for MariaDB and MySQL), `pgsql` or `sqlite` |
| `DB_HOST` | `db`, or the site's own database server |
| `DB_PORT` | `3306` or `5432` |
| `DB_DATABASE` | `db_<site>` (for SQLite: the path of the file) |
| `DB_USERNAME`, `DB_PASSWORD` | `db`, `db` |
| `APP_URL` | `https://<site>.<project>.ddev.site` |

On SQLite, host, port, user and password are not set. Every site also gets
tryout's own variables (`TRYOUT_*` and `DATABASE_URL`, see
[PHP and generic](/frameworks/php)).

The web server passes these variables to PHP. `ddev tryout exec` sets them for
the command line.

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none. `.env` is usually not committed, but Laravel needs it for
`APP_KEY`. The variables above replace its database lines.

## Commands in this project

Your framework's own tool is a tryout command: `ddev tryout artisan <site> …`
runs it in that site, with the site's PHP and its own database.

```bash
# Run artisan in the site feat
ddev tryout artisan feat migrate

# And in the project's own site
ddev tryout artisan @primary migrate
```

`ddev tryout launch feat` opens the site. There is no admin for `--backend` to open.

See [commands per project type](/reference/commands#commands-per-project-type).

## What happens to the database

A new site does not start empty. tryout copies the primary's database into it.
The primary is the site DDEV serves at your project's own URL; its database is
called `db`.

- If the site's code is an older version of Laravel than the primary's, it starts
  with an empty database instead, and tryout says so: a database of a newer
  version would break the older code.
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

## What to watch out for

- tryout sets `DB_CONNECTION=mysql` for MariaDB too. Every Laravel version
  understands it.
- A cached config (`bootstrap/cache/config.php`) ignores the environment. A new
  worktree has none. If you run `artisan config:cache` in a site, clear it again
  with `ddev tryout exec <site> artisan config:clear`.
- `ddev tryout artisan feat migrate` runs Artisan in the site.

## Is it tested?

Yes. The CI job `frameworks (laravel)` creates a new `laravel/laravel` app,
runs the migrations and writes a row in the primary. Then it serves a worktree,
reads the row back from the worktree's database `db_feat`, checks that a write
in the worktree does not reach the primary, and loads the welcome page.
