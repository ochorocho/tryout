# Asterios

## Is it supported?

Yes, through **environment variables**. Asterios reads Laravel-style `DB_*`
variables from `.env`, which DDEV writes. A real environment variable wins over
`.env`, and tryout sets these variables for each served worktree.

## Set it up from scratch

Start from your own project's repository. tryout needs the project to be a git
repository; a clone is one.

```bash
# Get your project and set up DDEV
git clone <your repository> my-project
cd my-project
ddev config --project-type=asterios --docroot=public

# Install tryout and restart DDEV
ddev add-on get https://github.com/ochorocho/tryout/tarball/feature/ddev-addon-ddev-subfolder
ddev restart
```

Then install your app the way you always do (`ddev composer install`, migrations).

The `add-on get` line installs tryout from its development branch. Once tryout is released, use `ddev add-on get bmack/tryout` instead.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat -r 'echo getenv("DB_DATABASE"), "\n";'
```

## What tryout does for you

| Variable | Value |
|---|---|
| `DB_HOST` | `db`, or the site's own database server |
| `DB_PORT` | `3306` or `5432` |
| `DB_DATABASE` | `db_<site>` |
| `DB_USERNAME`, `DB_PASSWORD` | `db`, `db` |
| `APP_URL` | `https://<site>.<project>.ddev.site` |

On SQLite none of these are set. Every site also gets tryout's own variables
(`TRYOUT_*` and `DATABASE_URL`, see [PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none.

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

Not yet. There is no CI job that installs Asterios.
