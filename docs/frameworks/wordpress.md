# WordPress

## Is it supported?

Yes, on MariaDB and MySQL. WordPress is supported through a **settings file**.
DDEV writes `<docroot>/wp-config-ddev.php`, and `wp-config.php` loads it. DDEV
only defines a constant there if it is not defined yet, so whoever defines it
first wins. tryout uses that.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/WordPress/WordPress.git wordpress-tryout
cd wordpress-tryout
ddev config --project-type=wordpress

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install WordPress
ddev wp core install --url=https://wordpress-tryout.ddev.site --title=tryout --admin_user=admin --admin_password=admin --admin_email=admin@example.com
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://developer.wordpress.org/cli/commands/core/install/).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check the site's database and URL: it prints db_feat and the worktree's URL
ddev tryout exec feat -r 'require "wp-load.php"; echo DB_NAME, " ", home_url();'
```

## What tryout does for you

For each served worktree, tryout:

1. copies `wp-config.php` from your project, if the worktree has none;
2. writes the worktree's own `wp-config-ddev.php`: DDEV's file with this
   snippet added at the **top**, right after `<?php`. That way it comes before
   DDEV's definitions and wins.

```php
if (getenv('TRYOUT_DB_NAME')) {
	define( 'DB_NAME', getenv( 'TRYOUT_DB_NAME' ) );
	define( 'DB_USER', getenv( 'TRYOUT_DB_USER' ) );
	define( 'DB_PASSWORD', getenv( 'TRYOUT_DB_PASSWORD' ) );
	define( 'DB_HOST', getenv( 'TRYOUT_DB_HOST' ) . ':' . getenv( 'TRYOUT_DB_PORT' ) );
	define( 'WP_HOME', getenv( 'TRYOUT_URL' ) );
	define( 'WP_SITEURL', getenv( 'TRYOUT_URL' ) );
}
```

The web server and `ddev tryout exec` set these variables for each site. The
primary does not get them, so DDEV's settings still apply there.

`WP_HOME` and `WP_SITEURL` make the site answer at its own URL, even though
the copied database still stores the primary's URL. (DDEV works out
`WP_SITEURL` from the folder, which gives a wrong result inside a worktree.)

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

- WordPress runs on MariaDB and MySQL only. Do not serve it with
  `--db postgres` or `--db sqlite`.
- Links inside your posts still point to the primary's URL. If that matters,
  replace them in the site with `wp search-replace`. Not tested yet with
  tryout: `ddev tryout exec feat /usr/local/bin/wp search-replace
  https://<project>.ddev.site https://feat.<project>.ddev.site`.
- If the worktree has its own committed `wp-config-ddev.php`, tryout does not
  touch it and shows a warning. That site then uses the primary's database.

## Is it tested?

Yes. The CI job `frameworks (wordpress)` downloads and installs WordPress with
wp-cli and serves a worktree. It checks that `DB_NAME` is `db_feat`, that
`home_url()` is the worktree's URL, and that `/wp-login.php` loads.
