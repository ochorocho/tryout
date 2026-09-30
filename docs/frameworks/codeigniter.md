# CodeIgniter 4

## Is it supported?

Yes, through **environment variables**. CodeIgniter 4 reads its settings from
the environment, with dots in the names (for example
`database.default.database`). Its `.env` loader does not replace a variable
that is already set. tryout sets these variables for each served worktree.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/codeigniter4/appstarter.git codeigniter-tryout
cd codeigniter-tryout
ddev config --project-type=codeigniter --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install

# Run the migrations
ddev exec php spark migrate
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://codeigniter.com/user_guide/installation/).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# spark is a PHP file, so it runs through exec
ddev tryout exec feat spark migrate
```

## What tryout does for you

| Variable | Value |
|---|---|
| `database.default.DBDriver` | `MySQLi`, `Postgre` or `SQLite3` |
| `database.default.hostname` | `db`, or the site's own database server |
| `database.default.port` | `3306` or `5432` |
| `database.default.database` | `db_<site>` |
| `database.default.username`, `database.default.password` | `db`, `db` |
| `app.baseURL` | `https://<site>.<project>.ddev.site/` |

On SQLite, host, port, user and password are not set. Every site also gets
tryout's own variables (`TRYOUT_*` and `DATABASE_URL`, see
[PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none.

## Commands in this project

Your framework's own tool is a tryout command: `ddev tryout spark <site> …`
runs it in that site, with the site's PHP and its own database.

```bash
# Run spark in the site feat
ddev tryout spark feat migrate

# And in the project's own site
ddev tryout spark @primary migrate
```

`ddev tryout launch feat` opens the site. There is no admin for `--backend` to open.

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

Unit tests check the variables. There is no CI job that installs CodeIgniter
yet.
