# CakePHP

## Is it supported?

Yes, through **environment variables**. A CakePHP app reads its database from
`DATABASE_URL` in `config/app_local.php` (`env('DATABASE_URL')`). tryout sets
`DATABASE_URL` and the site's full base URL for each served worktree.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/cakephp/app.git cakephp-tryout
cd cakephp-tryout
ddev config --project-type=cakephp --docroot=webroot

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://book.cakephp.org/5/en/installation.html).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Usually nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat -r 'echo getenv("DATABASE_URL"), "\n";'
```

## What tryout does for you

| Variable | Value |
|---|---|
| `DATABASE_URL` | `mysql://db:db@db:3306/db_<site>?…` (see [PHP and generic](/frameworks/php)) |
| `APP_FULL_BASE_URL` | `https://<site>.<project>.ddev.site` |

Every site also gets tryout's own variables (see
[PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env` and `.env.local` from the project root, if
the worktree has none. DDEV writes CakePHP's settings into `config/.env`. tryout
does **not** copy that file.

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

- If your worktree has its own `config/.env` that sets `DATABASE_URL`, check
  that it does not replace the value tryout sets. Not tested yet.
- `bin/cake` is a shell script. `ddev tryout exec` runs PHP, so use CakePHP's
  PHP entry file instead (`bin/cake.php` in a standard app). Not tested yet.

## Is it tested?

Not yet. There is no CI job that installs CakePHP.
