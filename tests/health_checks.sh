#!/usr/bin/env bash

# Post-install assertions shared by every install variant. Deliberately does NOT
# require `ddev start` to have cloned TYPO3 Core — that is covered separately by
# the (slow) full-lifecycle test.

set -eu -o pipefail

# --- Payload landed where DDEV puts project_files -------------------------------
assert_file_exist "${TESTDIR}/.ddev/commands/host/tryout"
assert_file_exist "${TESTDIR}/.ddev/commands/host/autocomplete/tryout"
assert_file_exist "${TESTDIR}/.ddev/tryout/herdr-plugin/herdr-plugin.toml"
assert_file_executable "${TESTDIR}/.ddev/tryout/herdr-plugin/relocate.sh"
assert_file_exist "${TESTDIR}/.ddev/config.tryout.yaml"
assert_file_exist "${TESTDIR}/.ddev/config.tryout-patches.yaml"
for f in functions.sh post-start.sh sync-composer.php site-composer.php \
         tryout-php-fpm.sh resolve-patch-ref.sh resolve-gerrit-account.sh \
         herdr-new-worktree.sh herdr-menu.sh \
         gitmessage.txt composer.tryout.json additional.php gitignore \
         patches.yaml; do
  assert_file_exist "${TESTDIR}/.ddev/tryout/${f}"
done

# The add-on must not ship DDEV's own boilerplate or a config.yaml of its own.
assert_file_not_exist "${TESTDIR}/.ddev/tryout/install.yaml"

# --- Files copied out to the project root by post_install_actions ---------------
assert_file_exist "${TESTDIR}/composer.tryout.json"
assert_file_exist "${TESTDIR}/config/system/additional.php"
assert_dir_exist "${TESTDIR}/packages"

# --- Scripts are executable (post_install_actions chmod) ------------------------
assert_file_executable "${TESTDIR}/.ddev/commands/host/tryout"
# DDEV only wires up completion for an executable script.
assert_file_executable "${TESTDIR}/.ddev/commands/host/autocomplete/tryout"
assert_file_executable "${TESTDIR}/.ddev/tryout/post-start.sh"

# --- Ownership markers, so DDEV may update and remove these --------------------
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/commands/host/tryout"
assert_success
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/commands/host/autocomplete/tryout"
assert_success
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/config.tryout.yaml"
assert_success
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/tryout/functions.sh"
assert_success
run grep -q 'ddev-generated' "${TESTDIR}/composer.tryout.json"
assert_success
run grep -q 'ddev-generated' "${TESTDIR}/config/system/additional.php"
assert_success

# The patch list is the user's copy: created once from tryout/patches.yaml with
# the marker stripped, so an update never overwrites it and DDEV never warns.
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/config.tryout-patches.yaml"
assert_failure

# --- The add-on is registered with DDEV ----------------------------------------
run ddev add-on list --installed
assert_success
assert_output --partial "tryout"

# --- The command is wired up and dispatches ------------------------------------
run ddev tryout help
assert_success
assert_output --partial "TYPO3 development toolkit"
assert_output --partial "cs [setup|doctor]"
assert_output --partial "worktree"

# Status works before Core is cloned, and says so.
run ddev tryout status
assert_success
assert_output --partial "not cloned"
assert_output --partial "ddev tryout download"
