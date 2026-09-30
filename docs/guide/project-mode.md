# Project mode

Project mode is for a DDEV project of your own: Laravel, Symfony, Drupal,
WordPress, a TYPO3 site, plain PHP or anything else DDEV runs. Your project
stays as it is. tryout runs other branches of it next to it. Each branch gets
its own URL, its own PHP version and its own database.

A quick example:

```bash
ddev tryout worktree add feature-x feature/x --serve   # serve branch feature/x as feature-x
ddev tryout worktree add try main --serve --db-empty    # a site with an empty database
ddev tryout worktree add --pr 42                        # serve pull request 42 as pr-42
ddev tryout exec feature-x bin/console cache:clear      # run a command in the feature-x site
ddev tryout launch feature-x                            # open feature-x in the browser
```

## The primary and the sites

The **primary** is your project as DDEV runs it: the project root, at
`https://<project>.ddev.site`, with DDEV's own settings and the database `db`.
tryout never changes it.

Every other **site** is a worktree you serve. For a project called `shop`:

```text
https://shop.ddev.site              → the project root    PHP 8.4   db  (DDEV's own)
https://feature-x.shop.ddev.site    → worktrees/feature-x PHP 8.4   db_feature_x
https://pr-42.shop.ddev.site        → worktrees/pr-42     PHP 8.3   db_pr_42
```

A site runs directly from its worktree. It uses the same docroot setting as
your project (for example `worktrees/feature-x/public`). tryout keeps its own
notes about a site in `.ddev/tryout-sites/<name>/`, never inside your worktree.

## Creating a worktree

```bash
ddev tryout worktree add feature-x feature/x
```

This creates `worktrees/feature-x` with the code of branch `feature/x`.

- The worktree does not take the branch for itself (it is "detached"). So
  several worktrees can start from the same branch.
- tryout first tries to fetch from `origin` and uses `origin/<branch>`. If there
  is no `origin`, or the fetch fails, it uses your local branch.
- Add `--serve` to serve the site right away. `--php`, `--db`, `--db-from` and
  `--db-empty` also serve it.

Rules for names:

- Letters, digits, `.`, `_` and `-`, at most 60 characters.
- Two worktrees cannot share a database name. `feat-x`, `feat.x` and `feat_x`
  all become the database `db_feat_x`, so only one of them is allowed.

## Serving a worktree

```bash
ddev tryout worktree serve feature-x            # the newest PHP its composer.json allows
ddev tryout worktree serve feature-x --php 8.3  # a specific PHP version
ddev tryout worktree serve feature-x --db postgres:16   # on a PostgreSQL 16 server
```

When you serve a worktree, tryout does these steps:

1. **Creates the database** `db_<name>`, on the project's database server or on
   the one you name with `--db`. A new database is filled with data (see below).
   A database kept from an earlier `unserve` stays as it was.
2. **Copies local settings.** A worktree only has committed files. If it has no
   `.env` or `.env.local`, tryout copies the project's (with your `APP_KEY` and
   similar values). For frameworks that use a settings file, it also writes the
   site's own copy of DDEV's settings file. See [frameworks](/frameworks/).
3. **Installs dependencies.** If the worktree has a `composer.json`,
   `composer install` runs there, on the site's PHP version. Composer scripts
   see the site's database, not the primary's.
4. **Starts the site.** tryout sets up the web server for
   `<name>.<project>.ddev.site`, a PHP process for its PHP version, and passes
   the site's database settings to it.

A new URL needs a DDEV restart. tryout does it for you. See
[served sites](/guide/sites) for PHP versions, database servers, restarts and
`/etc/hosts`.

## How the app finds its database

DDEV writes database settings for your project, with the database name `db`.
Each site needs the same settings with its own database name. tryout passes them
on the way your framework reads them:

- **Environment variables:** Laravel, Symfony, Craft CMS, Shopware, CakePHP,
  Silverstripe, CodeIgniter, Bedrock and plain PHP. In these frameworks, real
  environment variables win over `.env`, so nothing else is needed.
- **A settings file:** Drupal, Backdrop, WordPress and TYPO3 site projects. The
  worktree gets a copy of DDEV's settings file. A short addition at the start or
  end of that file reads the site's database from the environment.

Every site gets these variables: `TRYOUT_DB_DRIVER`, `TRYOUT_DB_NAME`,
`TRYOUT_DB_HOST`, `TRYOUT_DB_PORT`, `TRYOUT_DB_USER`, `TRYOUT_DB_PASSWORD`,
`DATABASE_URL` and `TRYOUT_URL`. It also gets the names its framework uses. The
[frameworks](/frameworks/) pages list them, and say which types are not
supported yet.

## Where a new site's data comes from

A new site's database starts as a **copy of the primary's database**. So the site
has data from the first page you open.

```bash
ddev tryout worktree serve feature-x                  # copy of the primary's database
ddev tryout worktree serve feature-x --db-from pr-42  # copy of the pr-42 site's database
ddev tryout worktree serve feature-x --db-empty       # empty: the app sets itself up
```

Good to know:

- tryout copies with the database's own tools: `mysqldump | mysql` between
  MariaDB and MySQL, `pg_dump | psql` between PostgreSQL servers.
- A copy only works within the same family. A MariaDB copy cannot go into
  PostgreSQL, and SQLite has no server to copy from. In those cases the site
  starts with an empty database, and tryout tells you.
- Only a **new** database gets a copy. A database kept by `unserve` is used again
  as it was.
- `--db-from` takes `@primary` (the default) or the name of a served site.
- `--db-from` and `--db-empty` do not work on a TYPO3 Core checkout. There,
  TYPO3's own setup creates each site.

What a site writes stays in its own database. The primary never sees it.


::: warning Older code gets an empty database
A database of a newer framework version breaks older code, and nothing
downgrades a database. So when a worktree runs an older Drupal, WordPress,
Laravel, Symfony, TYPO3, Shopware or Craft than the primary, tryout does not copy
the primary's database. The site starts empty, and tryout tells you how to set
it up (for example `ddev tryout drush <site> site:install standard -y`).
:::

## Resetting, unserving and removing

```bash
ddev tryout delete feature-x --yes      # reset its database to a new copy of the primary's
ddev tryout delete --all --yes          # reset every served site (never the primary)
ddev tryout worktree unserve feature-x  # stop serving; keep the worktree and the database
ddev tryout worktree unserve feature-x --drop-db   # stop serving and delete the database
ddev tryout worktree remove feature-x   # stop serving, delete the database and the worktree
```

- `delete` resets a site's database to a new copy of the primary's.
- tryout never deletes the primary's database. It belongs to DDEV. `delete` on
  the primary stops and points you to `ddev snapshot` and `ddev import-db`.
- `unserve` stops the site but keeps your worktree and its database. The
  database server keeps running while a kept database needs it.
- `remove` asks before it deletes, and names the folder. `--yes` skips the
  question.

## Moving and renaming

```bash
ddev tryout worktree serve feature-x --db postgres:17 --switch   # move to PostgreSQL 17
ddev tryout worktree rename feature-x feature-y                  # give it a new name
```

- `--switch` moves a served site to another database server. tryout stops the
  site, keeps its old database, and serves it again on the new server. The new
  database starts as a copy of the old one if they are in the same family,
  otherwise empty.
- `rename` works the same way for a new name. The new database starts as a copy
  of the old one. The old database is kept.

## Running commands in a site

```bash
ddev tryout exec feature-x bin/console doctrine:migrations:migrate
ddev tryout exec feature-x artisan tinker
ddev tryout exec feature-x -r 'echo getenv("TRYOUT_DB_NAME");'
```

`exec` runs the command in the site's folder, with the site's database
settings. A PHP program (`php`, `composer`, `artisan`, `bin/console`, a `.php`
file) runs with the site's PHP version. Anything else, like `bash -c '…'` or
`vendor/bin/drush`, runs as it is.

::: tip
Your framework's tool has a shorter command of its own: `ddev tryout drush
feature-x status`, `ddev tryout artisan feature-x migrate`. See
[commands per project type](/reference/commands#commands-per-project-type).
:::

## Pull requests

`ddev tryout worktree add --pr 42` fetches pull request 42 from `origin` and
serves it as `pr-42`. See [pull requests](/guide/pull-requests).

## Commands that need TYPO3 Core

Some commands only work on a TYPO3 Core checkout. In a project, they stop with a
message:

```text
$ ddev tryout patch
✗ `patch` is not available for this project
✗   → ddev tryout help   lists what is
```

These are `download`, `checkout`, `patch`, `reset`, `composer`, `cs` and
`worktree use`. `launch --backend` has no backend to open; plain `launch` opens
the site. The terminal UI hides the same commands in its menus.
