# Craft CMS

## Is it supported?

Yes, through **environment variables**. DDEV puts Craft's `CRAFT_DB_*`
variables into the web container. tryout sets the same variables for each
served worktree. They win over the container's values, both for web requests
and in `ddev tryout exec`.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/craftcms/craft.git craft-tryout
cd craft-tryout
ddev config --project-type=craftcms --docroot=web

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install

# Install Craft (it asks for an admin account and the site URL)
ddev craft install
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://craftcms.com/docs/5.x/install.html).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# craft is a PHP file, so it runs through exec
ddev tryout exec feat craft migrate/all
```

## What tryout does for you

| Variable | Value |
|---|---|
| `CRAFT_DB_DRIVER` | `mysql` or `pgsql` |
| `CRAFT_DB_SERVER` | `db`, or the site's own database server |
| `CRAFT_DB_PORT` | `3306` or `5432` |
| `CRAFT_DB_DATABASE` | `db_<site>` |
| `CRAFT_DB_USER`, `CRAFT_DB_PASSWORD` | `db`, `db` |
| `PRIMARY_SITE_URL` | `https://<site>.<project>.ddev.site` |

Craft does not support SQLite, so on SQLite none of these are set. Every site
also gets tryout's own variables (`TRYOUT_*` and `DATABASE_URL`, see
[PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none.

## Commands in this project

Your framework's own tool is a tryout command: `ddev tryout craft <site> …`
runs it in that site, with the site's PHP and its own database.

```bash
# Run craft in the site feat
ddev tryout craft feat project-config/apply

# And in the project's own site
ddev tryout craft @primary project-config/apply
```

`ddev tryout launch feat --backend` opens the site's admin at `/admin`.

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

Unit tests check the variables. There is no CI job that installs Craft yet.
