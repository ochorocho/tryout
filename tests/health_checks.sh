#!/usr/bin/env bash

# Post-install assertions shared by every install variant. Deliberately does NOT
# require `ddev start` to have cloned TYPO3 Core — that is covered separately by
# the (slow) full-lifecycle test.

set -eu -o pipefail

# --- Payload landed where DDEV puts project_files -------------------------------
assert_file_exist "${TESTDIR}/.ddev/commands/host/tryout"
assert_file_exist "${TESTDIR}/.ddev/commands/host/autocomplete/tryout"
assert_file_exist "${TESTDIR}/.ddev/config.tryout.yaml"
assert_file_exist "${TESTDIR}/.ddev/config.tryout-patches.yaml"
for f in functions.sh commands.sh tryout-container.sh post-start.sh \
         sync-composer.php site-composer.php \
         tryout-php-fpm.sh resolve-patch-ref.sh resolve-gerrit-account.sh \
         gitmessage.txt composer.tryout.json additional.php \
         patches.yaml; do
  assert_file_exist "${TESTDIR}/.ddev/tryout/${f}"
done

# The add-on must not ship DDEV's own boilerplate or a config.yaml of its own.
assert_file_not_exist "${TESTDIR}/.ddev/tryout/install.yaml"

# --- Files copied out to the project root by post_install_actions ---------------
assert_file_exist "${TESTDIR}/Build/composer.tryout.json"
assert_file_exist "${TESTDIR}/Build/config/system/additional.php"
assert_dir_exist "${TESTDIR}/packages"

# --- Scripts are executable (post_install_actions chmod) ------------------------
assert_file_executable "${TESTDIR}/.ddev/commands/host/tryout"
# DDEV only wires up completion for an executable script.
assert_file_executable "${TESTDIR}/.ddev/commands/host/autocomplete/tryout"
assert_file_executable "${TESTDIR}/.ddev/tryout/post-start.sh"
assert_file_executable "${TESTDIR}/.ddev/tryout/tryout-container.sh"

# The payload version stamp, so `ddev tryout status` can spot a stale install.
assert_file_exist "${TESTDIR}/.ddev/tryout/.version"
run grep -qE '^[0-9]+$' "${TESTDIR}/.ddev/tryout/.version"
assert_success
# The web image gets the add-on's git, so DDEV must have been handed the fragment.
assert_file_exist "${TESTDIR}/.ddev/web-build/Dockerfile.tryout"
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/web-build/Dockerfile.tryout"
assert_success

# --- Ownership markers, so DDEV may update and remove these --------------------
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/commands/host/tryout"
assert_success
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/commands/host/autocomplete/tryout"
assert_success
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/config.tryout.yaml"
assert_success
run grep -q '#ddev-generated' "${TESTDIR}/.ddev/tryout/functions.sh"
assert_success
run grep -q 'ddev-generated' "${TESTDIR}/Build/composer.tryout.json"
assert_success
run grep -q 'ddev-generated' "${TESTDIR}/Build/config/system/additional.php"
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

# Status works right after install. Core is cloned by a post_install_action now —
# on the HOST, because DDEV's mutagen config ignores /.git at the project root —
# so by this point the root already IS the checkout and status reports its branch
# rather than "not cloned".
run ddev tryout status
assert_success
assert_output --partial "TYPO3 tryout"
