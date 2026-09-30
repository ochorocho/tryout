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

# A framework's own app, created by its installer, committed as a user's would be.
framework_project() {
  local type="$1" package="$2"
  run ddev config --project-type="${type}" --docroot=public
  assert_success
  run ddev start -y
  assert_success
  run ddev composer create-project -n "${package}"
  assert_success
  git -c init.defaultBranch=main init -q
  git add -A
  git -c user.name=t -c user.email=t@t commit -qm "${type} app"
}

# What a site answers at its own hostname, from inside the web container.
page() {
  local host="$1"
  ddev exec curl -sk --max-time 20 -H "Host: ${host}" https://127.0.0.1/
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
  run ddev tryout exec feat artisan tinker --execute='echo DB::connection()->getDatabaseName(), " ", DB::table("users")->value("name");'
  assert_success
  assert_output --partial "db_feat primary"

  run ddev tryout exec feat artisan tinker --execute='DB::table("users")->insert(["name" => "feat", "email" => "f@example.com", "password" => "x"]);'
  assert_success
  run sql db "SELECT COUNT(*) FROM users"
  assert_output "1"

  run page "feat.${PROJNAME}.ddev.site"
  assert_output --partial "Laravel"
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
  run ddev tryout exec feat bin/console dbal:run-sql "SELECT CONCAT(DATABASE(), ' ', v) AS x FROM seed"
  assert_success
  assert_output --partial "db_feat from-primary"
}
