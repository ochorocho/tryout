# WordPress with Bedrock

## Is it supported?

Yes, through **environment variables**. Bedrock reads its settings from `.env`,
but a real environment variable always wins. tryout sets Bedrock's variable
names for each served worktree.

## What you need to do

Nothing. Add a worktree and serve it:

```bash
# Create worktrees/feat from the branch main and serve it
# at https://feat.<project>.ddev.site
ddev tryout worktree add feat main --serve

# Check which database the site uses: it prints db_feat
ddev tryout exec feat -r 'echo getenv("DB_NAME"), "\n";'
```

## What tryout does for you

| Variable | Value |
|---|---|
| `DB_NAME` | `db_<site>` |
| `DB_USER`, `DB_PASSWORD` | `db`, `db` |
| `DB_HOST` | `<host>:<port>`, for example `db:3306` |
| `WP_HOME` | `https://<site>.<project>.ddev.site` |
| `WP_SITEURL` | `https://<site>.<project>.ddev.site/wp` |

Every site also gets tryout's own variables (`TRYOUT_*` and `DATABASE_URL`, see
[PHP and generic](/frameworks/php)).

**Copied into the worktree:** `.env` and `.env.local` from your project, if the
worktree has none. `.env` holds the salts. The variables above replace its
database and URL lines.

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

- Like WordPress, Bedrock needs MariaDB or MySQL. On SQLite tryout sets none of
  the variables above.
- Links inside your posts still point to the primary's URL. `wp search-replace`
  in the site can change them.

## Is it tested?

Unit tests check the variables. There is no CI job that installs Bedrock yet.
