# Backdrop

## Is it supported?

Yes, through a **settings file**. DDEV writes `<docroot>/settings.ddev.php`
with a `$database` URL, and `settings.php` loads it. tryout writes both for
each served worktree.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/backdrop/backdrop.git backdrop-tryout
cd backdrop-tryout
ddev config --project-type=backdrop

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Then open https://backdrop-tryout.ddev.site/core/install.php and follow the installer
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://docs.backdropcms.org/documentation/installation-instructions).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat -r 'echo getenv("TRYOUT_DB_NAME"), "\n";'
```

## What tryout does for you

For each served worktree, tryout:

1. copies `settings.php` from your project, if the worktree has none;
2. writes the worktree's own `settings.ddev.php`: DDEV's file with this snippet
   added at the end.

```php
if (getenv('TRYOUT_DB_NAME')) {
  $database = sprintf('mysql://%s:%s@%s:%s/%s', getenv('TRYOUT_DB_USER'),
    getenv('TRYOUT_DB_PASSWORD'), getenv('TRYOUT_DB_HOST'),
    getenv('TRYOUT_DB_PORT'), getenv('TRYOUT_DB_NAME'));
}
```

The web server and `ddev tryout exec` set these variables for each site. The
primary does not get them, so DDEV's settings still apply there.

## Commands in this project

This type has no tool command in tryout. Run any PHP file in a site with
`ddev tryout exec <site> <file> …`.

`ddev tryout launch feat --backend` opens the site's admin at `/user/login`.

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

## What to watch out for

- Backdrop runs on MariaDB and MySQL.
- If the worktree has its own committed `settings.ddev.php`, tryout does not
  touch it and shows a warning. That site then uses the primary's database.

## Is it tested?

Not yet. There is no CI job that installs Backdrop.
