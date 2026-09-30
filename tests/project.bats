#!/usr/bin/env bats

# Project mode: tryout on a project of the user's own, not a TYPO3 Core
# checkout. A tiny PHP app is built in the test — composer.json, a VERSION file
# that differs per branch, and public/index.php printing its site, version and
# whether it reached its own database (tests/fixture-app.sh) — then served
# from two worktrees.
#
#   bats ./tests/project.bats
#
# The URLs are fetched inside the web container, against its own webserver with
# the site's Host header: that needs no DNS for *.ddev.site on the host.

setup() {
  load setup.sh
  # setup.sh configures a TYPO3 Core project; these are projects of their own.
  # The first config made the Core docroot; none of them has that.
  rm -rf TYPO3-Instances
}
teardown() { load teardown.sh; }

# The fixture app as a plain `php` project.
fixture_project() {
  run ddev config --project-type=php --docroot=public
  assert_success
  bash "${DIR}/tests/fixture-app.sh"
}

# One statement as root against a database of the project's server.
sql() {
  ddev exec mysql -uroot -proot -N "$1" -e "$2"
}

# The HTTP status a site answers with, like `page`.
status_of() {
  local host="${1%%/*}" path="/"
  [ "${host}" = "$1" ] || path="/${1#*/}"
  ddev exec curl -sk -o /dev/null -w '%{http_code}' --max-time 60 \
    -H "Host: ${host}" -H "X-Forwarded-Proto: https" "https://127.0.0.1${path}"
}

# A started DDEV project of a type, empty.
new_project() {
  local type="$1" docroot="$2"
  run ddev config --project-type="${type}" --docroot="${docroot}"
  assert_success
  run ddev start -y
  assert_success
}

# Everything committed on main, as a user's repository would be.
commit_all() {
  git -c init.defaultBranch=main init -q
  git add -A
  git -c user.name=t -c user.email=t@t commit -qm "the app"
}

# A framework's own app, created by its installer, committed.
framework_project() {
  local type="$1" package="$2"
  new_project "${type}" public
  run ddev composer create-project -n "${package}"
  assert_success
  commit_all
}

# What a site answers at its own hostname (and path), from inside the web
# container: `page one.x.ddev.site/wp-login.php`.
page() {
  local host="${1%%/*}" path="/"
  [ "${host}" = "$1" ] || path="/${1#*/}"
  # X-Forwarded-Proto as DDEV's router sends it: without it an app that insists
  # on HTTPS (WordPress) redirects to where the request already is.
  ddev exec curl -sk --max-time 20 -H "Host: ${host}" -H "X-Forwarded-Proto: https" \
    "https://127.0.0.1${path}"
}

# bats test_tags=project
@test "install leaves a project of your own as it was" {
  set -eu -o pipefail
  fixture_project
  run ddev add-on get "${DIR}"
  assert_success
  assert_output --partial "Not a TYPO3 Core checkout"

  # No clone, no Core layout, no Core environment, no patch list.
  assert_dir_not_exist "${TESTDIR}/TYPO3-Instances"
  assert_dir_not_exist "${TESTDIR}/packages"
  assert_file_not_exist "${TESTDIR}/.ddev/config.tryout-core.yaml"
  assert_file_not_exist "${TESTDIR}/.ddev/config.tryout-patches.yaml"
  run git -C "${TESTDIR}" diff --exit-code HEAD
  assert_success
  run grep -q 'docroot: public' "${TESTDIR}/.ddev/config.yaml"
  assert_success

  # The post-start hook runs, and touches nothing of the project's either.
  run ddev start -y
  assert_success
  run ddev exec 'echo "[${COMPOSER:-}]"'
  assert_output "[]"
  run git -C "${TESTDIR}" status --porcelain --untracked-files=no
  assert_output ""

  run ddev tryout status
  assert_success
  assert_output --partial "project"

  # What needs TYPO3 Core says so.
  for verb in patch cs checkout download; do
    run ddev tryout "${verb}"
    assert_failure
    assert_output --partial "not available"
  done
  run ddev tryout worktree use main
  assert_failure
  assert_output --partial "not available"
}

# bats test_tags=project
@test "two worktrees of a project are served side by side, each on its own database" {
  set -eu -o pipefail
  fixture_project
  run ddev add-on get "${DIR}"
  assert_success
  run ddev start -y
  assert_success
  sql db "CREATE TABLE seed (v VARCHAR(20)); INSERT INTO seed VALUES ('from-primary')"

  # A local branch, no origin: the worktree starts from it — and its database
  # as a copy of the primary's.
  run ddev tryout worktree add one feature --serve
  assert_success
  run sql db_one "SELECT v FROM seed"
  assert_output "from-primary"
  # What the site writes stays in its own.
  sql db_one "INSERT INTO seed VALUES ('from-one')"
  run sql db "SELECT COUNT(*) FROM seed"
  assert_output "1"
  run ddev tryout worktree add two main --serve --db sqlite
  assert_success

  # The worktrees are the user's; tryout's state stays in .ddev/.
  assert_file_exist "${TESTDIR}/worktrees/one/VERSION"
  assert_file_not_exist "${TESTDIR}/worktrees/one/.tryout-site"
  assert_file_exist "${TESTDIR}/.ddev/tryout-sites/one/.tryout-site"
  run git -C "${TESTDIR}" status --porcelain
  refute_output --partial "worktrees"
  refute_output --partial "tryout-sites"

  run page "one.${PROJNAME}.ddev.site"
  assert_output "fixture site=one version=feature db=db_one connected vendor=installed"
  run page "two.${PROJNAME}.ddev.site"
  assert_output --partial "fixture site=two version=main db=/var/www/html/.ddev/tryout-sites/two/sqlite/db_two.sqlite connected"
  # The primary is DDEV's own site, on DDEV's own database.
  run page "${PROJNAME}.ddev.site"
  assert_output --partial "fixture site=primary version=main db=db connected"

  # `exec` runs the site's PHP in the site, with its database in the environment.
  run ddev tryout exec one -r 'echo getenv("TRYOUT_DB_NAME"), "\n";'
  assert_success
  assert_output "db_one"

  run ddev tryout worktree list --plain
  assert_success
  assert_output --partial "one.${PROJNAME}.ddev.site"

  # delete resets a site's database to a fresh copy; the project's own is not
  # tryout's to wipe.
  run ddev tryout delete one --yes
  assert_success
  run sql db_one "SELECT v FROM seed"
  assert_output "from-primary"
  run ddev tryout delete --yes
  assert_failure
  assert_output --partial "project's own"

  # Unserve keeps the worktree and the database; remove takes both.
  run ddev tryout worktree unserve one
  assert_success
  assert_file_exist "${TESTDIR}/worktrees/one/VERSION"
  assert_file_not_exist "${TESTDIR}/.ddev/tryout-sites/one/.tryout-site"
  run ddev exec mysql -N -e "SHOW DATABASES LIKE 'db_one'"
  assert_output "db_one"

  run ddev tryout worktree remove two --yes
  assert_success
  assert_dir_not_exist "${TESTDIR}/worktrees/two"
  assert_dir_not_exist "${TESTDIR}/.ddev/tryout-sites/two"

  # A kept database is served again as it was; --db-empty starts from nothing.
  run ddev tryout worktree serve one
  assert_success
  run sql db_one "SELECT COUNT(*) FROM seed"
  assert_output "1"
  run ddev tryout worktree unserve one --drop-db
  assert_success
  run ddev tryout worktree serve one --db-empty
  assert_success
  run sql db_one "SHOW TABLES LIKE 'seed'"
  assert_output ""
  # --db-from copies another served site's.
  run ddev tryout worktree add three main --serve --db-from one --no-restart
  assert_success
}

# bats test_tags=project
@test "a pull request of origin opens as its own served worktree" {
  set -eu -o pipefail
  fixture_project
  # A bare repository stands in for the forge: it publishes request 7 the way
  # GitHub does, as refs/pull/7/head.
  local remote="${TESTDIR}.origin.git"
  git init -q --bare "${remote}"
  git remote add origin "${remote}"
  git push -q origin main "feature:refs/pull/7/head"
  run ddev add-on get "${DIR}"
  assert_success
  run ddev start -y
  assert_success

  run ddev tryout worktree add --pr 7
  assert_success
  assert_dir_exist "${TESTDIR}/worktrees/pr-7"
  run page "pr-7.${PROJNAME}.ddev.site"
  assert_output --partial "fixture site=pr-7 version=feature"

  # One it does not publish fails before anything is made.
  run ddev tryout worktree add --pr 8
  assert_failure
  assert_output --partial "no pull or merge request #8"
  assert_dir_not_exist "${TESTDIR}/worktrees/pr-8"
  rm -rf "${remote}"
}

# bats test_tags=project,db,laravel
@test "a Laravel project's worktrees start from the primary's data and keep their own" {
  set -eu -o pipefail
  framework_project laravel laravel/laravel
  run ddev add-on get "${DIR}"
  assert_success
  run ddev restart -y
  assert_success
  run ddev artisan migrate --force
  assert_success
  sql db "INSERT INTO users (name, email, password) VALUES ('primary', 'p@example.com', 'x')"

  # .env is not committed: the worktree gets the project's (APP_KEY), and the
  # site's own database from the environment.
  run ddev tryout worktree add feat main --serve
  assert_success
  assert_file_exist "${TESTDIR}/worktrees/feat/.env"
  # Laravel's own tool, as a tryout command: artisan in the site.
  run ddev tryout artisan feat tinker --execute='echo DB::connection()->getDatabaseName(), " ", DB::table("users")->value("name");'
  assert_success
  assert_output --partial "db_feat primary"

  run ddev tryout artisan feat tinker --execute='DB::table("users")->insert(["name" => "feat", "email" => "f@example.com", "password" => "x"]);'
  assert_success
  run sql db "SELECT COUNT(*) FROM users"
  assert_output "1"

  run page "feat.${PROJNAME}.ddev.site"
  assert_output --partial "Laravel"

  # status finds a database that ran migrations the code does not have.
  run ddev tryout status
  refute_output --partial "Schema:"
  sql db_feat "INSERT INTO migrations (migration, batch) VALUES ('2099_01_01_000000_from_the_future', 9)"
  run ddev tryout status
  assert_output --partial "Schema:"
  assert_output --partial "from_the_future"
}

# bats test_tags=project,db,symfony
@test "a Symfony project's worktrees reach their own database through DATABASE_URL" {
  set -eu -o pipefail
  framework_project symfony symfony/skeleton
  run ddev composer require -n symfony/orm-pack
  assert_success
  git add -A
  git -c user.name=t -c user.email=t@t commit -qm "doctrine"
  run ddev add-on get "${DIR}"
  assert_success
  run ddev restart -y
  assert_success
  sql db "CREATE TABLE seed (v VARCHAR(20)); INSERT INTO seed VALUES ('from-primary')"

  run ddev tryout worktree add feat main --serve
  assert_success
  run ddev tryout console feat dbal:run-sql "SELECT CONCAT(DATABASE(), ' ', v) AS x FROM seed"
  assert_success
  assert_output --partial "db_feat from-primary"
}

# bats test_tags=project,db,shopware
@test "a Shopware shop's worktrees get their own database, domain and theme" {
  set -eu -o pipefail
  new_project shopware6 public
  run ddev composer create-project -n shopware/production
  assert_success
  # Committed before tryout is installed: a repository of its own is what
  # makes this a project. The template's .gitignore keeps vendor/, var/,
  # .env.local and install.lock out.
  commit_all
  run ddev add-on get "${DIR}"
  assert_success
  # The restart writes DDEV's .env.local; the shop installs against it.
  run ddev restart -y
  assert_success
  run ddev exec bin/console system:install --basic-setup --no-interaction
  assert_success

  run ddev tryout worktree add feat main --serve
  assert_success
  assert_output --partial "sales-channel:update:domain feat.${PROJNAME}.ddev.site"
  assert_file_exist "${TESTDIR}/worktrees/feat/install.lock"

  # The copy moved to the site's own domain; the primary kept its own.
  run sql db_feat "SELECT url FROM sales_channel_domain"
  assert_output --partial "https://feat.${PROJNAME}.ddev.site"
  run sql db "SELECT url FROM sales_channel_domain"
  refute_output --partial "feat."

  # The administration and the storefront answer on the site's own host.
  run page "feat.${PROJNAME}.ddev.site/admin"
  assert_output --partial "Shopware"
  run status_of "feat.${PROJNAME}.ddev.site/"
  assert_output "200"
}

# bats test_tags=project,db,drupal
@test "a Drupal site's worktrees get their own settings.ddev.php and database" {
  set -eu -o pipefail
  new_project drupal11 web
  run ddev composer create-project -n drupal/recommended-project
  assert_success
  run ddev composer require -n drush/drush
  assert_success
  run ddev drush site:install -y --account-name=admin --account-pass=admin
  assert_success
  # As a Drupal project keeps it: settings and files are the site's own.
  printf '%s\n' /vendor/ /web/core/ /web/modules/contrib/ /web/themes/contrib/ \
    '/web/sites/*/files/' '/web/sites/*/settings*.php' > .gitignore
  commit_all
  run ddev add-on get "${DIR}"
  assert_success
  run ddev restart -y
  assert_success

  run ddev tryout worktree add feat main --serve
  assert_success
  assert_output --partial "sites/default/settings.ddev.php for 'feat'"
  # Drush as a tryout command (vendor/drush/drush/drush.php in the site).
  run ddev tryout drush feat status --field=db-name
  assert_success
  assert_output "db_feat"
  # Help is Drupal's: its tool and its admin, none of TYPO3 Core's commands.
  run ddev tryout help
  assert_output --partial "drush <site> <args>"
  assert_output --partial "--backend for /user/login"
  refute_output --partial "patch [<id>]"
  refute_output --partial "checkout <branch>"

  # status finds a database of a newer Drupal than the code.
  run ddev tryout status
  refute_output --partial "Schema:"
  sql db_feat "UPDATE key_value SET value='i:99999;' WHERE collection='system.schema' AND name='system'"
  run ddev tryout status
  assert_output --partial "feat: its database is at Drupal schema 99999"
  run ddev drush status --field=db-name
  assert_output "db"
  run page "feat.${PROJNAME}.ddev.site/user/login"
  assert_output --partial 'name="name"'
}

# bats test_tags=project,db,wordpress
@test "a WordPress site's worktrees get their own database and URL" {
  set -eu -o pipefail
  new_project wordpress ""
  run ddev wp core download
  assert_success
  run ddev wp core install --url="https://${PROJNAME}.ddev.site" --title=tryout \
    --admin_user=admin --admin_password=admin --admin_email=admin@example.com
  assert_success
  printf '%s\n' /wp-config.php /wp-config-ddev.php /wp-content/uploads/ > .gitignore
  commit_all
  run ddev add-on get "${DIR}"
  assert_success
  run ddev restart -y
  assert_success

  run ddev tryout worktree add feat main --serve
  assert_success
  run ddev tryout exec feat -r 'require "wp-load.php"; echo DB_NAME, " ", home_url(), "\n";'
  assert_success
  assert_output --partial "db_feat https://feat.${PROJNAME}.ddev.site"
  # WP-CLI as a tryout command, on the site's own database.
  run ddev tryout wp feat eval 'echo DB_NAME;'
  assert_success
  assert_output --partial "db_feat"
  run page "feat.${PROJNAME}.ddev.site/wp-login.php"
  assert_output --partial 'name="log"'
}

# bats test_tags=project,db,typo3
@test "a TYPO3 site project's worktrees get their own additional.php and database" {
  set -eu -o pipefail
  new_project typo3 public
  run ddev composer create-project -n typo3/cms-base-distribution
  assert_success
  run ddev exec vendor/bin/typo3 setup -n --server-type=other --driver=mysqli \
    --host=db --port=3306 --dbname=db --username=db --password=db \
    --admin-username=admin --admin-user-password=Password.1! \
    --admin-email=admin@example.com --project-name=tryout
  assert_success
  printf '%s\n' /vendor/ /var/ /public/_assets/ /public/typo3temp/ /public/fileadmin/ \
    /config/system/additional.php > .gitignore
  commit_all
  run ddev add-on get "${DIR}"
  assert_success
  assert_output --partial "Not a TYPO3 Core checkout"
  run ddev restart -y
  assert_success

  run ddev tryout worktree add feat main --serve
  assert_success
  run ddev tryout exec feat -r '$GLOBALS["TYPO3_CONF_VARS"]["DB"]["Connections"]["Default"] = []; include "config/system/additional.php"; echo $GLOBALS["TYPO3_CONF_VARS"]["DB"]["Connections"]["Default"]["dbname"], "\n";'
  assert_success
  assert_output "db_feat"
  # The site's own TYPO3 console, as a tryout command.
  run ddev tryout typo3 feat --version
  assert_success
  assert_output --partial "TYPO3 CMS"
  run page "feat.${PROJNAME}.ddev.site/typo3/"
  assert_output --partial "TYPO3"
}
