#!/usr/bin/env bash
# A tiny PHP app of a project's own, built in the current directory: the
# fixture of project mode (tests/project.bats, the browser suite). A git
# repository with `main` and a `feature` branch and no remote; VERSION differs
# per branch; public/index.php prints its site, version, whether it reached its
# own database and whether `composer install` ran.
set -euo pipefail

mkdir -p public
printf '{\n    "name": "acme/fixture",\n    "require": {}\n}\n' > composer.json
echo main > VERSION
cat > public/index.php <<'PHP'
<?php
$site = getenv('TRYOUT_SITE') ?: 'primary';
$driver = getenv('TRYOUT_DB_DRIVER') ?: 'mariadb';
$name = getenv('TRYOUT_DB_NAME') ?: 'db';
$dsn = match ($driver) {
  'sqlite' => "sqlite:$name",
  'postgres' => 'pgsql:host=' . getenv('TRYOUT_DB_HOST') . ';port=' . getenv('TRYOUT_DB_PORT') . ";dbname=$name",
  default => 'mysql:host=' . (getenv('TRYOUT_DB_HOST') ?: 'db') . ';port=' . (getenv('TRYOUT_DB_PORT') ?: '3306') . ";dbname=$name",
};
try {
  $pdo = new PDO($dsn, getenv('TRYOUT_DB_USER') ?: 'db', getenv('TRYOUT_DB_PASSWORD') ?: 'db');
  $pdo->exec('CREATE TABLE IF NOT EXISTS visits (n INT)');
  $db = 'connected';
} catch (Throwable $e) {
  $db = 'failed: ' . $e->getMessage();
}
$vendor = is_file(__DIR__ . '/../vendor/autoload.php') ? 'installed' : 'missing';
echo "fixture site=$site version=" . trim(file_get_contents(__DIR__ . '/../VERSION'))
  . " db=$name $db vendor=$vendor\n";
PHP
git -c init.defaultBranch=main init -q
git add composer.json VERSION public
# The project's own DDEV config is committed, as it usually is.
if [ -f .ddev/config.yaml ]; then git add .ddev/config.yaml; fi
git -c user.name=t -c user.email=t@t commit -qm "fixture app"
git branch feature
git checkout -q feature
echo feature > VERSION
git -c user.name=t -c user.email=t@t commit -qam "feature"
git checkout -q main
