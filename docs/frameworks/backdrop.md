# Backdrop

## Is it supported?

Yes, through a **settings file**. DDEV writes `<docroot>/settings.ddev.php`
with a `$database` URL, and `settings.php` loads it. tryout writes both for
each served worktree.

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
