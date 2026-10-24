<?php

// #ddev-generated — provided by the tryout DDEV add-on.
// Remove the line above if you want to own and edit this file yourself.

if (getenv('IS_DDEV_PROJECT') == 'true') {
    // The project's database: DDEV's `db` service, its engine from
    // DDEV_DATABASE (e.g. "mariadb:10.11", "postgres:16").
    $ddevDatabase = getenv('DDEV_DATABASE') ?: 'mariadb:10.11';
    $isPostgres = str_starts_with($ddevDatabase, 'postgres');

    // The instance's OWN settings.php decides which database it uses. Read it
    // rather than overwrite it: config.tryout.yaml sets TYPO3_DB_DBNAME=db for the
    // whole container, so taking the environment first meant every instance
    // reached without a vhost — every CLI command — silently landed on the
    // PRIMARY's database while its settings.php said otherwise.
    //
    // The environment is still the fallback, and is not dead code: it is what the
    // first run depends on. setup_site_typo3 runs `typo3 setup` through site_exec,
    // which exports the right name explicitly at a point where settings.php does
    // not exist yet. Order is therefore: what is already loaded, then the
    // environment, then plain 'db'.
    //
    // The vhosts still inject TYPO3_DB_DBNAME (see generate_site_vhost). That is
    // belt and braces now rather than the mechanism — it agrees with settings.php,
    // so over HTTP nothing changes.
    //
    // Driver, host and port follow the same rule: a site served on the other
    // engine (`worktree serve <name> --db postgres`) talks to an extra server,
    // and its settings.php — written by its own setup — says which. Only where
    // settings.php says nothing does the project's database stand in.
    $loaded = $GLOBALS['TYPO3_CONF_VARS']['DB']['Connections']['Default'] ?? [];
    $pick = static fn(string $key, $fallback) => ($loaded[$key] ?? '') !== '' ? $loaded[$key] : $fallback;
    $dbName = $pick('dbname', getenv('TYPO3_DB_DBNAME') ?: 'db');
    $dbDriver = $pick('driver', $isPostgres ? 'pdo_pgsql' : 'mysqli');
    $dbHost = $pick('host', 'db');
    $dbPort = $pick('port', $isPostgres ? 5432 : 3306);

    $overrides = [
        'GFX' => [
            'processor' => 'ImageMagick',
            'processor_path' => '/usr/bin/',
            'processor_path_lzw' => '/usr/bin/',
        ],
        'MAIL' => [
            'transport' => 'smtp',
            'transport_smtp_encrypt' => false,
            'transport_smtp_server' => 'localhost:1025',
        ],
        'SYS' => [
            'trustedHostsPattern' => '.*.*',
            'devIPmask' => '*',
            'displayErrors' => 1,
        ],
    ];
    // A SQLite site (`--db sqlite`) is a file its settings.php points at: no
    // server, so nothing to add — a host or user would only confuse it.
    if ($dbDriver !== 'pdo_sqlite') {
        $overrides['DB']['Connections']['Default'] = [
            'dbname' => $dbName,
            'driver' => $dbDriver,
            'host' => $dbHost,
            'password' => 'db',
            'port' => $dbPort,
            'user' => 'db',
        ];
    }
    $GLOBALS['TYPO3_CONF_VARS'] = array_replace_recursive($GLOBALS['TYPO3_CONF_VARS'], $overrides);
}
