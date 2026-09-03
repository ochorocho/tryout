#!/usr/bin/env bats

# Slow tests: these clone TYPO3 Core (hundreds of MB), run composer install and
# set up a real TYPO3. Each takes minutes. They are tagged so CI can shard them
# away from the fast install/removal suite in test.bats:
#   bats ./tests/lifecycle.bats --filter-tags lifecycle
#   bats ./tests --filter-tags '!lifecycle,!release'    # everything fast

setup() { load setup.sh; }
teardown() { load teardown.sh; }

# Clone only what the test needs. `ddev start` runs the post-start hook, which
# clones Core, syncs the overlay, installs dependencies and sets up TYPO3.
addon_start() {
  run ddev add-on get "${DIR}"
  assert_success
  run ddev start -y
  assert_success
}

# bats test_tags=lifecycle
@test "ddev start provisions a working TYPO3 from the Core git repository" {
  set -eu -o pipefail
  echo "# full lifecycle in ${TESTDIR} — clones TYPO3 Core, takes several minutes" >&3
  addon_start

  # Core was cloned by the post-start hook.
  assert_dir_exist "${TESTDIR}/typo3-core/typo3/sysext/core"

  # The overlay was generated from the sysexts actually on disk. It ships empty,
  # so a populated require block proves sync-composer.php ran before install.
  run bash -c "grep -c 'typo3/cms-' '${TESTDIR}/composer.tryout.json'"
  assert_success
  [ "${output}" -gt 20 ]

  # The user's composer.json is still absent — we never created one.
  assert_file_not_exist "${TESTDIR}/composer.json"

  # Dependencies resolved through the path repository, as symlinks into the clone.
  assert_dir_exist "${TESTDIR}/vendor/typo3/cms-core"
  assert_file_exist "${TESTDIR}/vendor/bin/typo3"

  # TYPO3 was set up and answers on the backend.
  assert_file_exist "${TESTDIR}/config/system/settings.php"
  run curl -sfI "https://${PROJNAME}.ddev.site/typo3/"
  assert_success
  assert_output --partial "HTTP/2 200"

  # And the status command reflects all of it.
  run ddev tryout status
  assert_success
  # Match the whole label, since "installed" also appears in "not installed".
  assert_output --partial "Composer:"
  refute_output --partial "not installed"
  assert_output --partial "TYPO3:"
  refute_output --partial "not set up"
  refute_output --partial "not cloned"
}

# bats test_tags=lifecycle
@test "checkout switches the Core branch and regenerates the overlay" {
  set -eu -o pipefail
  addon_start

  run ddev tryout checkout 13.4
  assert_success

  run bash -c "cd '${TESTDIR}/typo3-core' && git branch --show-current"
  assert_output "13.4"

  run ddev exec vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 13.4"

  # theme-camino only exists on main/v14+, so the overlay must have dropped it.
  run grep -q 'typo3/theme-camino' "${TESTDIR}/composer.tryout.json"
  assert_failure

  # The merge-plugin wiring must survive regeneration.
  run grep -q 'wikimedia/composer-merge-plugin' "${TESTDIR}/composer.tryout.json"
  assert_success

  run curl -sfI "https://${PROJNAME}.ddev.site/typo3/"
  assert_success
  assert_output --partial "HTTP/2 200"
}

# bats test_tags=lifecycle
@test "a Gerrit patch is applied and reported, and reset drops it" {
  set -eu -o pipefail
  addon_start

  # Resolve an open change against main from the Gerrit REST API, so the test does
  # not rot when a hard-coded change is merged or goes stale.
  local change
  change=$(curl -s 'https://review.typo3.org/changes/?q=status:open+project:Packages/TYPO3.CMS+branch:main&n=1' \
    | tail -c +6 | sed -n 's/.*"_number": *\([0-9]*\).*/\1/p' | head -1)
  [ -n "${change}" ]
  echo "# applying Gerrit change ${change}" >&3

  run ddev tryout patch "${change}"
  assert_success

  run ddev tryout status
  assert_success
  assert_output --partial "1 applied"

  run ddev tryout reset
  assert_success
  run ddev tryout status
  assert_success
  assert_output --partial "none applied"
}

# bats test_tags=lifecycle
@test "an unresolvable Gerrit change fails without touching the checkout" {
  set -eu -o pipefail
  addon_start

  run ddev tryout patch 999999999
  assert_failure

  run bash -c "cd '${TESTDIR}/typo3-core' && git status --porcelain | wc -l | tr -d ' '"
  assert_output "0"
}

# bats test_tags=lifecycle
@test "a served worktree gets its own URL, PHP version and database" {
  set -eu -o pipefail
  addon_start

  run ddev tryout worktree add v13 13.4
  assert_success
  assert_dir_exist "${TESTDIR}/typo3-core-v13"
  # The primary becomes a symlink to the active worktree.
  assert_link_exist "${TESTDIR}/typo3-core"

  run ddev tryout worktree serve v13 --php 8.4
  assert_success
  run ddev restart -y
  assert_success

  # Its own tree, its own overlay, its own marker.
  assert_file_exist "${TESTDIR}/sites/v13/composer.tryout.json"
  assert_file_exist "${TESTDIR}/sites/v13/.tryout-site"
  assert_dir_exist "${TESTDIR}/sites/v13/vendor"

  # The served site's overlay points at its own worktree and back at the shared
  # packages/ and the project's composer.json.
  run grep -q '\.\./\.\./typo3-core-v13/typo3/sysext/\*' "${TESTDIR}/sites/v13/composer.tryout.json"
  assert_success
  run grep -q '\.\./\.\./packages/\*' "${TESTDIR}/sites/v13/composer.tryout.json"
  assert_success

  # Both sites answer, on different TYPO3 and PHP versions.
  run curl -sfI "https://${PROJNAME}.ddev.site/typo3/"
  assert_success
  assert_output --partial "HTTP/2 200"
  run curl -sfI "https://v13.${PROJNAME}.ddev.site/typo3/"
  assert_success
  assert_output --partial "HTTP/2 200"

  run ddev tryout exec v13 vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 13.4"
  assert_output --partial "PHP 8.4"

  run ddev tryout worktree list
  assert_success
  assert_output --partial "v13"
  assert_output --partial "db_v13"

  # Unserving drops the site but keeps the worktree and its git state.
  run ddev tryout worktree unserve v13
  assert_success
  assert_dir_not_exist "${TESTDIR}/sites/v13"
  assert_dir_exist "${TESTDIR}/typo3-core-v13"
}

# bats test_tags=lifecycle
@test "TRYOUT_PATCHES from the patch list is applied on start" {
  set -eu -o pipefail
  run ddev add-on get "${DIR}"
  assert_success

  local change
  change=$(curl -s 'https://review.typo3.org/changes/?q=status:open+project:Packages/TYPO3.CMS+branch:main&n=1' \
    | tail -c +6 | sed -n 's/.*"_number": *\([0-9]*\).*/\1/p' | head -1)
  [ -n "${change}" ]
  sed -i.bak "s/TRYOUT_PATCHES=\$/TRYOUT_PATCHES=${change}/" "${TESTDIR}/.ddev/config.tryout-patches.yaml"

  run ddev start -y
  assert_success

  run ddev tryout status
  assert_success
  assert_output --partial "1 applied"
}

# bats test_tags=lifecycle
@test "an existing project keeps its own dependencies alongside Core" {
  set -eu -o pipefail
  cat > "${TESTDIR}/composer.json" <<'JSON'
{
    "name": "acme/site",
    "require": { "psr/log": "^3.0" }
}
JSON
  addon_start

  # The user's requirement resolved through the merge-plugin include...
  assert_dir_exist "${TESTDIR}/vendor/psr/log"
  # ...alongside Core, from the path repository.
  assert_dir_exist "${TESTDIR}/vendor/typo3/cms-core"
  # ...and their file was never written to.
  run grep -q 'typo3/cms-core' "${TESTDIR}/composer.json"
  assert_failure
}
