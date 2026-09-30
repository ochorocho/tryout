# Drupal

This page covers the DDEV types `drupal`, `drupal6`, `drupal7`, `drupal8`,
`drupal9`, `drupal10`, `drupal11` and `drupal12`.

## Is it supported?

Yes. Drupal is supported through a **settings file**. DDEV writes the file
`<docroot>/sites/default/settings.ddev.php` with the database `db`, and
`settings.php` loads it. Both files are usually not committed (DDEV adds a
`.gitignore` next to them), so a new worktree has neither of them. tryout
writes them for each served worktree.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat vendor/drush/drush/drush.php status --field=db-name
```

## What tryout does for you

For each served worktree, tryout:

1. copies `sites/default/settings.php` from your project, if the worktree has
   none;
2. writes the worktree's own `sites/default/settings.ddev.php`: a copy of
   DDEV's file, with a short piece of code (a *snippet*) added at the end.

The snippet reads the site's database from the environment. The web server and
`ddev tryout exec` set that environment for each site. The primary does not get
these variables, so DDEV's own settings still apply there. The snippet for
Drupal 7 to 12:

```php
// tryout: this site's own database, from the environment its vhost and
// `ddev tryout exec` set. Absent for the primary: DDEV's settings stand.
if (getenv('TRYOUT_DB_NAME')) {
  $tryout_driver = getenv('TRYOUT_DB_DRIVER');
  $databases['default']['default'] = $tryout_driver === 'sqlite'
    ? ['driver' => 'sqlite', 'database' => getenv('TRYOUT_DB_NAME')]
    : [
      'driver' => $tryout_driver === 'postgres' ? 'pgsql' : 'mysql',
      'database' => getenv('TRYOUT_DB_NAME'),
      'host' => getenv('TRYOUT_DB_HOST'),
      'port' => getenv('TRYOUT_DB_PORT'),
      'username' => getenv('TRYOUT_DB_USER'),
      'password' => getenv('TRYOUT_DB_PASSWORD'),
      'prefix' => '',
    ];
}
```

Drupal 6 keeps its database in one string. There the snippet sets
`$db_url = 'mysqli://db:db@<host>:<port>/<database>'` (`pgsql://` on
PostgreSQL).

Everything else in DDEV's file stays as it is: `hash_salt`,
`trusted_host_patterns`, the config sync directory and the mail settings.

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

- `ddev tryout exec` runs PHP. `vendor/bin/drush` is a shell script, so call
  Drush's PHP file instead:
  `ddev tryout exec feat vendor/drush/drush/drush.php status`.
- If the worktree has its own committed `settings.ddev.php`, tryout does not
  touch it and shows a warning. That site then uses the primary's database.
- The copied database has the primary's users, so you log in with the same
  account.
- `sites/default/files` is not in the worktree (it is not committed). Drupal
  creates what it needs.

## Is it tested?

Yes. The CI job `frameworks (drupal)` builds `drupal/recommended-project` with
Drush and installs a site. It serves a worktree and checks that `drush status`
reports `db_feat` there, that the primary still uses `db`, and that
`/user/login` loads.
