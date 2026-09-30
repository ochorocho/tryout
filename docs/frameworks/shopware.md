# Shopware 6

## Is it supported?

Yes, through **environment variables**. Shopware reads `DATABASE_URL` and
`APP_URL` from `.env.local`, but a real environment variable always wins. tryout
sets both for each served worktree.

## Set it up from scratch

Start with a fresh copy of the framework's starter project. tryout needs your project to be a git repository, and a clone is one. Its `origin` is the framework's own repository, so `--pr` would open that repository's pull requests.

```bash
# Get the starter project and set up DDEV
git clone https://github.com/shopware/production.git shopware-tryout
cd shopware-tryout
ddev config --project-type=shopware6 --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart

# Install the dependencies
ddev composer install

# Install Shopware with a basic setup
ddev exec bin/console system:install --basic-setup
```

The steps after `ddev restart` are the framework's own first-time setup, not tryout's. They are not tested by tryout's CI. If they changed, follow the [framework's installation guide](https://developer.shopware.com/docs/guides/installation/).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# bin/console is a PHP file, so it runs through exec
ddev tryout exec feat bin/console cache:clear
```

## What tryout does for you

| Variable | Value |
|---|---|
| `DATABASE_URL` | `mysql://db:db@db:3306/db_<site>?serverVersion=…&charset=utf8mb4` |
| `APP_URL` | `https://<site>.<project>.ddev.site` |

Every site also gets tryout's own variables (see
[PHP and generic](/frameworks/php)).

**Copied into the worktree**, if the worktree does not have them yet. Git
ignores these files, so a new worktree has none of them:

- `.env.local` and `.env`: your project's settings.
- `install.lock`: without it, Shopware sends every request to its installer.
- `config/jwt/private.pem` and `config/jwt/public.pem`: the keys the
  administration signs its logins with.

**After a copy:** Shopware keeps the storefront's domain in the database, and
its compiled theme in `public/theme/`, which git ignores. So after tryout copies
the primary's database into a new site, it runs this in the site, after
`composer install`:

```bash
# Move the storefront to the site's own address
bin/console sales-channel:update:domain <site>.<project>.ddev.site

# Build the storefront theme for this worktree
bin/console theme:compile
```

If one of them fails, the site is still served, and tryout warns you that the
storefront may still link to the primary's URL.

## Commands in this project

Your framework's own tool is a tryout command: `ddev tryout console <site> …`
runs it in that site, with the site's PHP and its own database.

```bash
# Run console in the site feat
ddev tryout console feat cache:clear

# And in the project's own site
ddev tryout console @primary cache:clear
```

`ddev tryout launch feat --backend` opens the site's admin at `/admin`.

See [commands per project type](/reference/commands#commands-per-project-type).

## What happens to the database

A new site does not start empty. tryout copies the primary's database into it.
The primary is the site DDEV serves at your project's own URL; its database is
called `db`.

- If the site's code is an older version of Shopware than the primary's, it starts
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

- Shopware needs MariaDB or MySQL.

## Is it tested?

Yes. A CI job (`frameworks (shopware)`) creates a shop with
`composer create-project shopware/production` and `system:install
--basic-setup`, then serves a worktree. It checks that the site's database is a
copy with the site's own domain, that the primary keeps its own domain, and that
the administration and the storefront answer at the site's address.
