# PHP and generic projects

This page covers the DDEV types `php` and `generic`, and any type tryout does
not know.

## Is it supported?

Yes, through **environment variables**. tryout cannot know how your app finds
its database. So it gives each served worktree a set of variables, and your app
reads them.

## Set it up from scratch

Start from your own project's repository. tryout needs the project to be a git
repository; a clone is one.

```bash
# Get your project and set up DDEV
git clone <your repository> my-project
cd my-project
ddev config --project-type=php --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart
```

Use `--project-type=generic` instead if your project runs its own web server setup.

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Read the database settings from the environment in your app's configuration,
and keep DDEV's values as the fallback for the primary. For example:

```php
$database = getenv('TRYOUT_DB_NAME') ?: 'db';
```

Then add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat -r 'echo getenv("TRYOUT_DB_NAME"), "\n";'
```

## What tryout does for you

Every served site gets these variables, whatever its type:

| Variable | Value |
|---|---|
| `TRYOUT_DB_DRIVER` | `mariadb`, `mysql`, `postgres` or `sqlite` |
| `TRYOUT_DB_NAME` | `db_<site>` (for SQLite: the path of the file) |
| `TRYOUT_DB_HOST` | `db`, or the site's own database server (for example `tryout-postgres-16`) |
| `TRYOUT_DB_PORT` | `3306` or `5432` |
| `TRYOUT_DB_USER`, `TRYOUT_DB_PASSWORD` | `db`, `db` |
| `DATABASE_URL` | for example `mysql://db:db@db:3306/db_feat?serverVersion=11.8-MariaDB&charset=utf8mb4` |
| `TRYOUT_URL` | `https://<site>.<project>.ddev.site` |
| `TRYOUT_SITE` | the site's name |

On SQLite, host, port, user and password are not set. `DATABASE_URL` follows
the site's database server:

- PostgreSQL: `postgresql://…?serverVersion=16&charset=utf8`
- SQLite: `sqlite:////var/www/html/.ddev/tryout-sites/<site>/sqlite/db_<site>.sqlite`

The web server passes the variables to PHP; read them with `getenv()` or
`$_SERVER`. `ddev tryout exec <site> …` sets them for the command line.

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none. These files are usually not committed. The variables above
still win over them, as long as your `.env` loader does not replace existing
variables (most do not).

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

## What to watch out for

- `ddev tryout exec` runs the site's PHP with the arguments you give it:
  `ddev tryout exec feat bin/console …` runs `php bin/console …`.

## Is it tested?

Yes. The CI suite `project` serves a small plain-PHP app from two worktrees
(MariaDB and SQLite). It checks that each site answers with its own database,
and it tests copying, `delete`, `--db-empty`, `--db-from` and `--pr`.
