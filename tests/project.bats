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
  # setup.sh configures a TYPO3 Core project; this one is a plain PHP app.
  run ddev config --project-type=php --docroot=public
  assert_success
  # The first config made the Core docroot; this project has none of that.
  rm -rf TYPO3-Instances
  bash "${DIR}/tests/fixture-app.sh"
}
teardown() { load teardown.sh; }

# What a site answers at its own hostname, from inside the web container.
page() {
  local host="$1"
  ddev exec curl -sk --max-time 20 -H "Host: ${host}" https://127.0.0.1/
}

# bats test_tags=project
@test "install leaves a project of your own as it was" {
  set -eu -o pipefail
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
  run ddev add-on get "${DIR}"
  assert_success
  run ddev start -y
  assert_success

  # A local branch, no origin: the worktree starts from it.
  run ddev tryout worktree add one feature --serve
  assert_success
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

  # delete empties a site's database; the project's own is not tryout's.
  run ddev tryout delete one --yes
  assert_success
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
}
