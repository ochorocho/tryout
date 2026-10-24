# TYPO3 site projects

This page is about a TYPO3 **site**: your own Composer project of the DDEV type
`typo3`. If your project is a checkout of **TYPO3 Core** itself, tryout works
in [TYPO3 Core mode](/guide/typo3-core) instead.

## Is it supported?

Yes. A TYPO3 site is supported through a **settings file**. DDEV writes a file
with the database `db`:

- TYPO3 12 and newer, Composer mode: `config/system/additional.php`
- TYPO3 12 and newer, legacy mode: `<docroot>/typo3conf/system/additional.php`
- TYPO3 11 and older: `<docroot>/typo3conf/AdditionalConfiguration.php`

This file is usually not committed, so a new worktree does not have it. tryout
writes it for each served worktree.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/TYPO3/TYPO3.CMS.BaseDistribution.git typo3-site-tryout
cd typo3-site-tryout
ddev config --project-type=typo3 --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install

# Set up TYPO3
ddev exec vendor/bin/typo3 setup -n --server-type=other --driver=mysqli --host=db --port=3306 --dbname=db --username=db --password=db --admin-username=admin --admin-user-password=Password.1! --admin-email=admin@example.com --project-name=tryout
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://docs.typo3.org/m/typo3/tutorial-getting-started/main/en-us/Installation/Install.html).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# vendor/bin/typo3 is a PHP file, so it runs through exec
ddev tryout exec feat vendor/bin/typo3 cache:flush
```

## What tryout does for you

For each served worktree, tryout:

1. copies `config/system/settings.php` (or `LocalConfiguration.php`) from your
   project, if the worktree has none;
2. writes the worktree's own copy of DDEV's file, with this snippet added at
   the end:

```php
if (getenv('TRYOUT_DB_NAME')) {
    $tryoutDriver = getenv('TRYOUT_DB_DRIVER');
    $GLOBALS['TYPO3_CONF_VARS']['DB']['Connections']['Default'] = $tryoutDriver === 'sqlite'
        ? ['driver' => 'pdo_sqlite', 'path' => getenv('TRYOUT_DB_NAME')]
        : array_merge($GLOBALS['TYPO3_CONF_VARS']['DB']['Connections']['Default'] ?? [], [
            'driver' => $tryoutDriver === 'postgres' ? 'pdo_pgsql' : 'mysqli',
            'dbname' => getenv('TRYOUT_DB_NAME'),
            'host' => getenv('TRYOUT_DB_HOST'),
            'port' => (int)getenv('TRYOUT_DB_PORT'),
            'user' => getenv('TRYOUT_DB_USER'),
            'password' => getenv('TRYOUT_DB_PASSWORD'),
        ]);
}
```

The web server and `ddev tryout exec` set these variables for each site. The
primary does not get them, so DDEV's settings still apply there. DDEV's
`trustedHostsPattern` (`.*.*`) comes along in the copy, so the backend answers
at the worktree's URL.

Each site also gets the names many TYPO3 projects read themselves:
`TYPO3_DB_DRIVER` (`mysqli` or `pdo_pgsql`), `TYPO3_DB_DBNAME`, `TYPO3_DB_HOST`,
`TYPO3_DB_PORT`, `TYPO3_DB_USERNAME` and `TYPO3_DB_PASSWORD`. They are not set
for a SQLite site.

## A committed additional.php

Some projects commit their own `config/system/additional.php`. The
[TYPO3 GitLab project template](https://gitlab.com/gitlab-org/project-templates/typo3-distribution)
is one of them: its file reads the database from `TYPO3_DB_*`, and
`.ddev/.env` sets those to DDEV's database `db`.

tryout never changes a file your worktree has of its own. Instead, it sets the
`TYPO3_DB_*` variables for each site, and a site's own variables win over the
ones in `.ddev/.env`. So the committed file finds the site's database without
any change on your side. When tryout serves such a worktree, it says so:

```text
  'feat' has its own config/system/additional.php, reading the site's environment — its database is db_feat
```

If the committed file reads neither `TYPO3_DB_*` nor `TRYOUT_DB_*`, tryout
warns instead, and that site uses the primary's database `db`. Add one of the
two to the file, or let tryout write the file by not committing it.

For the GitLab template, two things are still yours to do in each worktree:

- **Frontend assets.** The Vite build output is not committed. Run
  `ddev tryout exec feat npm install` and `ddev tryout exec feat npm run build:production`.
  The template has no lockfile, so `npm ci` does not work.
- **Site configuration.** `config/sites/` must be committed. Otherwise the
  pages copied with the database have no site, and the frontend shows an error.

## Commands in this project

Your framework's own tool is a tryout command: `ddev tryout typo3 <site> …`
runs it in that site, with the site's PHP and its own database.

```bash
# Run typo3 in the site feat
ddev tryout typo3 feat cache:flush

# And in the project's own site
ddev tryout typo3 @primary cache:flush
```

`ddev tryout launch feat --backend` opens the site's admin at `/typo3/`.

See [commands per project type](/reference/commands#commands-per-project-type).

## What happens to the database

A new site does not start empty. tryout copies the primary's database into it.
The primary is the site DDEV serves at your project's own URL; its database is
called `db`.

- If the site's code is an older version of TYPO3 than the primary's, it starts
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

- Your site configuration may have an absolute `base`, such as
  `https://<project>.ddev.site/`. Then the frontend of a worktree still points
  to the primary. The backend (`/typo3/`) works at the worktree's URL. A
  relative `base` should avoid the problem, but this is not tested yet.
- If the worktree has its own committed `additional.php` that reads neither
  `TYPO3_DB_*` nor `TRYOUT_DB_*`, tryout does not touch it and shows a warning.
  That site then uses the primary's database. See
  [A committed additional.php](#a-committed-additional-php).

## Is it tested?

Yes. The CI job `frameworks (typo3)` creates a `typo3/cms-base-distribution`
project, runs `typo3 setup` and serves a worktree. It checks that the
worktree's database is `db_feat` and that `/typo3/` loads.
