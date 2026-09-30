# Symfony

## Is it supported?

Yes. Symfony is supported through **environment variables**. Symfony reads
`DATABASE_URL` from `.env` and `.env.local`, but a real environment variable
always wins. tryout sets `DATABASE_URL` for each served worktree, so Doctrine
connects to the site's own database.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat bin/console dbal:run-sql "SELECT DATABASE()"
```

## What tryout does for you

| Variable | Value |
|---|---|
| `DATABASE_URL` | `mysql://db:db@db:3306/db_<site>?serverVersion=11.8-MariaDB&charset=utf8mb4` |

The value follows the site's database server:

- MySQL: `serverVersion=8.4` (no `-MariaDB`).
- PostgreSQL: `postgresql://db:db@<host>:5432/db_<site>?serverVersion=16&charset=utf8`.
- SQLite: `sqlite:///` followed by the full path of the file.

Symfony has no variable for its own URL. The site's URL is in `TRYOUT_URL`.
Every site also gets tryout's other variables (see
[PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env.local` and `.env` from your project, if the
worktree has none. DDEV writes its `DATABASE_URL` into `.env.local`; the
variable tryout sets wins over it.

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

- `ddev tryout exec` runs PHP, and `bin/console` is a PHP file:
  `ddev tryout exec feat bin/console doctrine:migrations:migrate`.

## Is it tested?

Yes. The CI job `frameworks (symfony)` creates a `symfony/skeleton` app with
`symfony/orm-pack`, writes a row in the primary and reads it back from the
worktree's database `db_feat` with `bin/console dbal:run-sql`.
