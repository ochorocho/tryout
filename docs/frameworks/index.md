# Frameworks

tryout works with every DDEV project type. For each worktree you serve, it
gives the site its own URL, its own PHP version and its own database.

The one thing that differs between frameworks is this: **how does the app find
out which database is its own?** Each framework reads its database settings in
its own way. This page shows how tryout handles each one.

(If your project is a checkout of TYPO3 Core itself, tryout works in a
different mode. See [how the mode is decided](/guide/introduction).)

## Support levels

Each project type has one of three support levels:

- **environment**: the framework reads its database and URL from environment
  variables, and a real variable wins over its `.env` file. tryout sets these
  variables for each site: in the web server (for web requests) and in
  `ddev tryout exec` (for the command line). tryout writes no file.
- **settings file**: the framework reads a PHP settings file that DDEV writes
  for the project. tryout gives each worktree its own copy of DDEV's file, with
  a short piece of code (a *snippet*) added. The snippet reads the site's
  database and URL from the environment.
- **served only**: tryout serves the worktree on its own URL, PHP and
  database, and sets its variables. But the app's own configuration still uses
  DDEV's database `db`. You need to point the app at the site's database
  yourself. See [Not supported yet](/frameworks/not-supported).

Whatever the type, every site also gets tryout's own variables:
`TRYOUT_DB_DRIVER`, `TRYOUT_DB_NAME`, `TRYOUT_DB_HOST`, `TRYOUT_DB_PORT`,
`TRYOUT_DB_USER`, `TRYOUT_DB_PASSWORD`, `DATABASE_URL` and `TRYOUT_URL`
([details](/frameworks/php)).

The *primary* is the site DDEV serves at your project's own URL. It uses
DDEV's own settings and database `db`; tryout does not change it.

Run `ddev tryout status` to see the support level for your project's type.

## All DDEV project types

Click a type to see the details.

| Type | Wired through | What |
|---|---|---|
| `asterios` | environment | [DB_*, APP_URL](/frameworks/asterios) |
| `backdrop` | settings file | [settings.ddev.php](/frameworks/backdrop) |
| `cakephp` | environment | [DATABASE_URL, APP_FULL_BASE_URL](/frameworks/cakephp) |
| `codeigniter` | environment | [database.default.*, app.baseURL](/frameworks/codeigniter) |
| `craftcms` | environment | [CRAFT_DB_*, PRIMARY_SITE_URL](/frameworks/craftcms) |
| `drupal` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `drupal6` | settings file | [settings.ddev.php ($db_url)](/frameworks/drupal) |
| `drupal7` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `drupal8` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `drupal9` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `drupal10` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `drupal11` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `drupal12` | settings file | [settings.ddev.php](/frameworks/drupal) |
| `generic` | environment | [TRYOUT_DB_*, DATABASE_URL](/frameworks/php) |
| `joomla` | served only | [configuration.php is the app's own](/frameworks/not-supported) |
| `laravel` | environment | [DB_*, APP_URL](/frameworks/laravel) |
| `magento` | served only | [local.xml is the app's own](/frameworks/not-supported) |
| `magento2` | served only | [app/etc/env.php and the base URL in the database](/frameworks/not-supported) |
| `maho` | served only | [app/etc/local.xml is the app's own](/frameworks/not-supported) |
| `modx` | served only | [config.inc.php is the app's own](/frameworks/not-supported) |
| `php` | environment | [TRYOUT_DB_*, DATABASE_URL](/frameworks/php) |
| `shopware6` | environment | [DATABASE_URL, APP_URL; storefront domain updated](/frameworks/shopware) |
| `silverstripe` | environment | [SS_DATABASE_*, SS_BASE_URL](/frameworks/silverstripe) |
| `symfony` | environment | [DATABASE_URL](/frameworks/symfony) |
| `typo3` | settings file | [config/system/additional.php](/frameworks/typo3) |
| `wordpress` | settings file | [wp-config-ddev.php (MariaDB/MySQL only)](/frameworks/wordpress) |
| `wp-bedrock` | environment | [DB_*, WP_HOME](/frameworks/wp-bedrock) |

A type not listed is served like `php`.
