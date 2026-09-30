# Not supported yet

For these DDEV types, tryout **serves** a worktree, but it cannot point the app
at the site's own database:

| Type | Where the app keeps its database settings |
|---|---|
| `joomla` | `configuration.php` (DDEV writes none) |
| `magento` | `app/etc/local.xml` |
| `magento2` | `app/etc/env.php`, and the base URL in the database |
| `maho` | `app/etc/local.xml` |
| `modx` | `core/config/config.inc.php` |

## What works

Everything that does not depend on the app's own configuration:

- The worktree is served at `https://<site>.<project>.ddev.site` from its own
  checkout, after its own `composer install`, on its own PHP version.
- It gets its own database, a copy of the primary's. `--db-from`, `--db-empty`
  and `delete` work as for every other type.
- The variables `TRYOUT_DB_*`, `DATABASE_URL` and `TRYOUT_URL` are set, both
  for web requests and in `ddev tryout exec`.

## What does not work

The app's own configuration file still names DDEV's database `db`. So the site
reads and writes the **primary's** data.

tryout does not change these files. They are XML files or PHP files that return
an array. They belong to the app and are often committed, so tryout cannot edit
them safely. Magento 2 also keeps its base URL in the database, so a copied
database still redirects to the primary's URL.

## What you can do by hand

You can change the app's local configuration yourself, so it reads the site's
database from the environment. Keep DDEV's values as the fallback for the
primary. None of this is tested with tryout yet.

For Magento 2, in `app/etc/env.php`:

```php
'db' => ['connection' => ['default' => [
    'host' => (getenv('TRYOUT_DB_HOST') ?: 'db') . ':' . (getenv('TRYOUT_DB_PORT') ?: '3306'),
    'dbname' => getenv('TRYOUT_DB_NAME') ?: 'db',
    'username' => 'db',
    'password' => 'db',
    // …
]]],
```

After the copy, set the site's base URL:

```bash
ddev tryout exec feat bin/magento setup:store-config:set \
  --base-url="https://feat.<project>.ddev.site/" \
  --base-url-secure="https://feat.<project>.ddev.site/"
```

The same `getenv('TRYOUT_DB_NAME') ?: 'db'` idea works in MODX's
`config.inc.php` (`$dbase` and `$database_dsn`) and in Joomla's
`configuration.php` (`public $db`).

Would you like one of these types to be fully supported? Contributions are
welcome: see [Development](/reference/development).
