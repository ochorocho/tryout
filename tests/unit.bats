#!/usr/bin/env bats

# Unit tests for the pure helpers in tryout/functions.sh. These source the library
# directly with a fake project root, so they need no DDEV project and no containers
# and run in milliseconds:
#   bats ./tests/unit.bats

setup() {
  set -eu -o pipefail
  TEST_BREW_PREFIX="$(brew --prefix 2>/dev/null || true)"
  export BATS_LIB_PATH="${BATS_LIB_PATH:-}:${TEST_BREW_PREFIX}/lib:/usr/lib/bats"
  bats_load_library bats-assert
  bats_load_library bats-file
  bats_load_library bats-support

  export DIR="$(cd "$(dirname "${BATS_TEST_FILENAME}")/.." >/dev/null 2>&1 && pwd)"
  export FAKEROOT="$(mktemp -d "${BATS_TMPDIR:-/tmp}/tryout-unit.XXXXXX")"

  # functions.sh derives everything from these; sourcing it with no Core checkout
  # present exercises exactly the branch a fresh project is in.
  export DDEV_APPROOT="${FAKEROOT}"
  export DDEV_SITENAME="unitproj"
  export DDEV_PHP_VERSION="8.5"
  export DDEV_WEBSERVER_TYPE="apache-fpm"
  export DDEV_DATABASE="mariadb:10.11"
}

teardown() {
  set -eu -o pipefail
  [ -n "${FAKEROOT:-}" ] && rm -rf "${FAKEROOT}"
}

# Sourcing prints nothing; run helpers through this so failures surface cleanly.
helper() {
  # shellcheck disable=SC1090
  source "${DIR}/tryout/functions.sh" >/dev/null 2>&1
  "$@"
}

# Evaluate an expression with functions.sh sourced. Unlike `helper bash -c`, this
# stays in the same shell, so the library's functions and variables are visible.
helper_eval() {
  # shellcheck disable=SC1090
  source "${DIR}/tryout/functions.sh" >/dev/null 2>&1
  eval "$1"
}

@test "functions.sh is syntactically valid and sources cleanly" {
  set -eu -o pipefail
  run bash -n "${DIR}/tryout/functions.sh"
  assert_success
  run helper true
  assert_success
}

@test "BRANCH falls back to main with no Core checkout" {
  set -eu -o pipefail
  run helper_eval 'echo "${BRANCH}"'
  assert_success
  assert_output "main"
}

@test "TRYOUT_BRANCH overrides the detected branch" {
  set -eu -o pipefail
  export TRYOUT_BRANCH=13.4
  run helper_eval 'echo "${BRANCH}"'
  assert_success
  assert_output "13.4"
}

@test "the primary site resolves to the project root" {
  set -eu -o pipefail
  run helper site_dir
  assert_output "${FAKEROOT}"
  run helper site_dir "@primary"
  assert_output "${FAKEROOT}"
  run helper site_docroot
  assert_output "${FAKEROOT}/public"
}

@test "a named site resolves under sites/" {
  set -eu -o pipefail
  run helper site_dir v13
  assert_output "${FAKEROOT}/sites/v13"
  run helper site_docroot v13
  assert_output "${FAKEROOT}/sites/v13/public"
}

@test "the primary keeps the plain db name, extras get their own" {
  set -eu -o pipefail
  run helper site_database
  assert_output "db"
  run helper site_database v13
  assert_output "db_v13"
}

@test "a site database name is sanitised for characters MySQL rejects" {
  set -eu -o pipefail
  # Dots and dashes are legal in a worktree name but not in a database name.
  run helper site_database "feature-x.1"
  assert_output "db_feature_x_1"
}

@test "hostnames derive from the DDEV project name" {
  set -eu -o pipefail
  run helper site_hostname
  assert_output "unitproj.ddev.site"
  run helper site_hostname v13
  assert_output "v13.unitproj.ddev.site"
  # additional_hostnames wants the un-suffixed form; DDEV appends the TLD itself.
  run helper site_hostname_short v13
  assert_output "v13.unitproj"
}

@test "the primary Core dir follows the symlink, a named one does not" {
  set -eu -o pipefail
  run helper site_core_dir
  assert_output "${FAKEROOT}/typo3-core"
  run helper site_core_dir v13
  assert_output "${FAKEROOT}/typo3-core-v13"
}

@test "worktree names are validated" {
  set -eu -o pipefail
  for good in main v13 feature-x my.branch under_score; do
    run helper validate_worktree_name "${good}"
    assert_success
  done
  for bad in "" "." ".." "has space" "sla/sh" 'semi;colon' '$(whoami)'; do
    run helper validate_worktree_name "${bad}"
    assert_failure
  done
}

@test "the primary counts as served, a missing site does not" {
  set -eu -o pipefail
  run helper site_is_served
  assert_success
  run helper site_is_served v13
  assert_failure

  # A site is served once its marker exists.
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"
  run helper site_is_served v13
  assert_success
  run helper site_php_version v13
  assert_output "8.2"
}

@test "a site with no marker falls back to the project PHP version" {
  set -eu -o pipefail
  run helper site_php_version
  assert_output "8.5"
}

@test "served_site_names lists only sites with a marker" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/sites/v13" "${FAKEROOT}/sites/v12" "${FAKEROOT}/sites/scratch"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"
  printf 'php=8.3\n' > "${FAKEROOT}/sites/v12/.tryout-site"

  run helper_eval 'served_site_names | sort | tr "\n" " "'
  assert_output "v12 v13 "
}

@test "the vhost file goes to the directory for the webserver in use" {
  set -eu -o pipefail
  run helper site_vhost_file v13
  assert_output "${FAKEROOT}/.ddev/apache/tryout-site-v13.conf"

  export DDEV_WEBSERVER_TYPE=nginx-fpm
  run helper site_vhost_file v13
  assert_output "${FAKEROOT}/.ddev/nginx_full/tryout-site-v13.conf"
}

@test "require_core exits with a next step when Core is missing" {
  set -eu -o pipefail
  run helper require_core
  assert_failure
  assert_output --partial "TYPO3 Core not found"
  assert_output --partial "ddev tryout download"
}

@test "every shipped script is valid bash" {
  set -eu -o pipefail
  for f in "${DIR}"/tryout/*.sh "${DIR}/commands/host/tryout"; do
    run bash -n "${f}"
    assert_success
  done
}

@test "every shipped PHP script is valid" {
  set -eu -o pipefail
  for f in "${DIR}"/tryout/*.php; do
    run php -l "${f}"
    assert_success
  done
}

@test "the shipped overlay template is valid JSON and carries its marker" {
  set -eu -o pipefail
  run php -r 'json_decode(file_get_contents($argv[1]), true, 512, JSON_THROW_ON_ERROR);' \
    "${DIR}/tryout/composer.tryout.json"
  assert_success
  run grep -q 'ddev-generated' "${DIR}/tryout/composer.tryout.json"
  assert_success
}

@test "install.yaml lists every file the add-on ships" {
  set -eu -o pipefail
  for entry in commands/host/tryout config.tryout.yaml tryout; do
    run grep -qE "^  - ${entry}\$" "${DIR}/install.yaml"
    assert_success
  done
}

@test "every project_files entry exists in the repo" {
  set -eu -o pipefail
  run bash -c "
    cd '${DIR}'
    sed -n '/^project_files:/,/^[a-z_]*:/p' install.yaml \
      | sed -n 's/^  - //p' \
      | while read -r p; do [ -e \"\$p\" ] || echo \"MISSING: \$p\"; done
  "
  assert_output ""
}
