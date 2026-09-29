#!/usr/bin/env bats

# Slow tests: these clone TYPO3 Core (hundreds of MB), run composer install and
# set up a real TYPO3. Each takes minutes. They are tagged so CI can shard them
# away from the fast install/removal suite in test.bats:
#   bats ./tests/lifecycle.bats --filter-tags lifecycle
#   bats ./tests --filter-tags '!lifecycle,!release'    # everything fast

setup() { load setup.sh; }
teardown() { load teardown.sh; }

# A backend that ANSWERS is not a backend that WORKS: a misconfigured instance
# returns 200 with a broken page, and a HEAD request cannot tell the difference.
# The login page's <title> is the cheap proof that TYPO3 booted, resolved its site
# configuration and rendered — so assert on the body, not just the status.
assert_backend_loads() {
  local url="$1"
  # DDEV puts project hostnames in /etc/hosts, which needs sudo — and the suite runs
  # with DDEV_NONINTERACTIVE=true, so it never gets one. Where *.ddev.site does not
  # resolve on its own (a plain dev machine), skip rather than report a false
  # failure; CI and any host with a wildcard resolver still run the assertion.
  curl -sfI --max-time 10 "${url}" >/dev/null 2>&1 || {
    case "$?" in
      6) skip "${url} does not resolve — no /etc/hosts entry (needs sudo)" ;;
    esac
  }

  run curl -sfI "${url}"
  assert_success
  assert_output --partial "HTTP/2 200"

  run curl -sf --max-time 30 "${url}"
  assert_success
  assert_output --partial "<title>TYPO3 CMS Login"
}

# The frontend renders a real TYPO3 page, where the add-on could provision one.
# On 13.4+ the styleguide generator builds a demo; discover its base from the
# generated site config and assert the page is TYPO3-rendered. A version without
# the generator (12.4) has no styleguide config, so this skips rather than fails —
# the backend is all such an instance serves.
assert_frontend_renders() {
  local host="$1" instdir="$2"
  # The styleguide config is the one naming typo3/styleguide as a dependency; print
  # its base. grep -l finds that file, sed reads its base — no nested quoting.
  local cfg base=""
  cfg="$(grep -lR 'typo3/styleguide' "${instdir}/config/sites" 2>/dev/null | head -1)"
  [ -n "${cfg}" ] && base="$(sed -n 's/^base: *//p' "${cfg}" | head -1)"
  [ -n "${base}" ] || skip "no styleguide frontend on this TYPO3 version (backend only)"
  case "${base}" in */) ;; *) base="${base}/" ;; esac

  local url="https://${host}${base}"
  curl -sfI --max-time 10 "${url}" >/dev/null 2>&1 || {
    case "$?" in 6) skip "${url} does not resolve — no /etc/hosts entry (needs sudo)" ;; esac
  }
  run curl -sf --max-time 30 "${url}"
  assert_success
  # TYPO3 stamps every rendered page with this meta generator.
  assert_output --partial "TYPO3"
}

# The inverse: an unserved hostname must stop answering entirely.
assert_backend_gone() {
  local url="$1"
  # Only meaningful where the hostname could resolve in the first place.
  curl -sfI --max-time 10 "https://${PROJNAME}.ddev.site/typo3/" >/dev/null 2>&1 || {
    case "$?" in
      6) skip "ddev.site does not resolve here — no /etc/hosts entry (needs sudo)" ;;
    esac
  }

  run curl -sfI --max-time 20 "${url}"
  assert_failure
}

# TRYOUT_IMPL=rust: the Rust port answers `ddev tryout`. Its binaries are not
# part of the payload until the switch-over, so they come from a local build
# (tui/scripts/stage-bins.sh) — the host one and the container's alike.
use_rust_if_asked() {
  [ "${TRYOUT_IMPL:-}" = "rust" ] || return 0
  [ -f "${DIR}/tryout/bin/tryout-linux-x86_64" ] || fail "no Rust build — run tui/scripts/stage-bins.sh"
  mkdir -p .ddev/tryout/bin
  cp "${DIR}"/tryout/bin/tryout-* .ddev/tryout/bin/
  cp "${DIR}/tryout/tryout" .ddev/tryout/tryout
}

# Clone only what the test needs. `ddev start` runs the post-start hook, which
# clones Core, syncs the overlay, installs dependencies and sets up TYPO3.
addon_start() {
  run ddev add-on get "${DIR}"
  assert_success
  use_rust_if_asked
  run ddev start -y
  assert_success
}

# bats test_tags=lifecycle
@test "ddev start provisions a working TYPO3 from the Core git repository" {
  set -eu -o pipefail
  echo "# full lifecycle in ${TESTDIR} — clones TYPO3 Core, takes several minutes" >&3
  addon_start

  # Core was cloned by the post-start hook.
  assert_dir_exist "${TESTDIR}/typo3/sysext/core"

  # The overlay was generated from the sysexts actually on disk. It ships empty,
  # so a populated require block proves sync-composer.php ran before install.
  run bash -c "grep -c 'typo3/cms-' '${TESTDIR}/TYPO3-Instances/primary/composer.tryout.json'"
  assert_success
  [ "${output}" -gt 20 ]

  # The user's own composer.json is still absent — we never created one. (The
  # composer.json at the project root is Core's own; the instance's is the overlay.)
  assert_file_not_exist "${TESTDIR}/TYPO3-Instances/primary/composer.json"

  # Dependencies resolved through the path repository, as symlinks into the clone.
  assert_dir_exist "${TESTDIR}/TYPO3-Instances/primary/vendor/typo3/cms-core"
  assert_file_exist "${TESTDIR}/TYPO3-Instances/primary/vendor/bin/typo3"

  # TYPO3 was set up and answers on the backend.
  assert_file_exist "${TESTDIR}/TYPO3-Instances/primary/config/system/settings.php"
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"

  # The add-on provisions a frontend too (styleguide demo on 13.4+); it renders.
  assert_frontend_renders "${PROJNAME}.ddev.site" "${TESTDIR}/TYPO3-Instances/primary"

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

  run bash -c "cd '${TESTDIR}' && git branch --show-current"
  assert_output "13.4"

  # The primary's console lives in its instance, not at the root (the Core
  # checkout) where a bare `ddev exec` starts.
  run ddev tryout exec @primary vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 13.4"

  # theme-camino only exists on main/v14+, so the overlay must have dropped it.
  run grep -q 'typo3/theme-camino' "${TESTDIR}/TYPO3-Instances/primary/composer.tryout.json"
  assert_failure

  # The merge-plugin wiring must survive regeneration.
  run grep -q 'wikimedia/composer-merge-plugin' "${TESTDIR}/TYPO3-Instances/primary/composer.tryout.json"
  assert_success

  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"
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

  run bash -c "cd '${TESTDIR}' && git status --porcelain | wc -l | tr -d ' '"
  assert_output "0"
}

# bats test_tags=lifecycle
@test "a served worktree gets its own URL, PHP version and database" {
  set -eu -o pipefail
  addon_start

  run ddev tryout worktree add v13 13.4
  assert_success
  assert_dir_exist "${TESTDIR}/worktrees/v13"

  run ddev tryout worktree serve v13 --php 8.4
  assert_success
  run ddev restart -y
  assert_success

  # Its own tree, its own overlay, its own marker.
  assert_file_exist "${TESTDIR}/TYPO3-Instances/v13/composer.tryout.json"
  assert_file_exist "${TESTDIR}/TYPO3-Instances/v13/.tryout-site"
  assert_dir_exist "${TESTDIR}/TYPO3-Instances/v13/vendor"

  # The served site's overlay points at its own worktree and back at the shared
  # packages/ and the project's composer.json.
  run grep -q '\.\./\.\./worktrees/v13/typo3/sysext/\*' "${TESTDIR}/TYPO3-Instances/v13/composer.tryout.json"
  assert_success
  run grep -q '\.\./\.\./packages/\*' "${TESTDIR}/TYPO3-Instances/v13/composer.tryout.json"
  assert_success

  # Both sites answer, on different TYPO3 and PHP versions.
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://v13.${PROJNAME}.ddev.site/typo3/"

  run ddev tryout exec v13 vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 13.4"
  assert_output --partial "PHP 8.4"

  run ddev tryout worktree list
  assert_success
  assert_output --partial "v13"
  assert_output --partial "db_v13"

  # Each site must have its OWN database, populated by its own TYPO3 setup — a
  # misrouted site would still answer 200 while sharing the primary's tables.
  run ddev mysql -uroot -proot -N -e \
    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='db_v13';"
  assert_success
  refute_output "0"

  # Unserving drops the site but keeps the worktree and its git state.
  run ddev tryout worktree unserve v13
  assert_success
  assert_dir_not_exist "${TESTDIR}/TYPO3-Instances/v13"
  assert_dir_exist "${TESTDIR}/worktrees/v13"

  # ...and the hostname stops answering, while the primary is unaffected.
  run ddev restart -y
  assert_success
  assert_backend_gone "https://v13.${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"

  # The database survives unserve by design, so re-serving restores the site.
  run ddev tryout worktree serve v13
  assert_success
  run ddev restart -y
  assert_success
  assert_backend_loads "https://v13.${PROJNAME}.ddev.site/typo3/"
}

# bats test_tags=lifecycle
@test "several worktrees are served side by side, each on its own database" {
  set -eu -o pipefail
  addon_start

  run ddev tryout worktree add v13 13.4 --serve
  assert_success
  run ddev tryout worktree add v12 12.4 --serve
  assert_success
  run ddev restart -y
  assert_success

  # Three instances at once: the primary plus two served worktrees.
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://v13.${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://v12.${PROJNAME}.ddev.site/typo3/"

  # Each on its own Core, and its own database.
  run ddev tryout exec v13 vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 13.4"

  run ddev tryout exec v12 vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 12.4"

  # Separate databases, each with its own schema.
  run ddev mysql -uroot -proot -N -e \
    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='db_v13';"
  assert_success
  refute_output "0"
  run ddev mysql -uroot -proot -N -e \
    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='db_v12';"
  assert_success
  refute_output "0"

  # Dropping one leaves the others serving.
  run ddev tryout worktree unserve v12
  assert_success
  run ddev restart -y
  assert_success
  assert_backend_gone  "https://v12.${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://v13.${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"
}

# bats test_tags=lifecycle
@test "TRYOUT_PATCHES from the patch list is applied on start" {
  set -eu -o pipefail
  run ddev add-on get "${DIR}"
  assert_success
  use_rust_if_asked

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
  # The user's own composer.json lives beside the instance overlay, which merges
  # it — TYPO3-Instances/primary/composer.json. It must be created before the
  # add-on runs, so write it after config but the harness's addon_start does both;
  # create the dir and file here, then start.
  mkdir -p "${TESTDIR}/TYPO3-Instances/primary"
  cat > "${TESTDIR}/TYPO3-Instances/primary/composer.json" <<'JSON'
{
    "name": "acme/site",
    "require": { "psr/log": "^3.0" }
}
JSON
  addon_start

  # The user's requirement resolved through the merge-plugin include...
  assert_dir_exist "${TESTDIR}/TYPO3-Instances/primary/vendor/psr/log"
  # ...alongside Core, from the path repository.
  assert_dir_exist "${TESTDIR}/TYPO3-Instances/primary/vendor/typo3/cms-core"
  # ...and their file was never written to.
  run grep -q 'typo3/cms-core' "${TESTDIR}/TYPO3-Instances/primary/composer.json"
  assert_failure
}

# bats test_tags=lifecycle
@test "the whole layout survives add, use, serve and a restart" {
  set -eu -o pipefail
  # EVERY bug the layout change produced needed a real project to surface: a glob
  # that could not see the root checkout, a `cd` on a path that cannot exist, an
  # `ln -sfn` at the project root, a first-run guard on a moved path, a lister the
  # fix missed, a panel reading the wrong source, and a cache cleared for the wrong
  # site. The unit suite cannot build one. This walks the layout end to end and
  # asserts on `worktree list --plain`, the documented machine-readable contract.
  addon_start

  # The project root IS the Core clone, and nothing the add-on generates may show
  # up in `git status` — a Gerrit patch from here must carry none of it.
  run git -C "${TESTDIR}" rev-parse --abbrev-ref HEAD
  assert_success
  run bash -c "git -C '${TESTDIR}' status --porcelain | head -5"
  assert_output ""

  # The instance is in TYPO3-Instances/primary, and Core's own Build/ is untouched.
  assert_dir_exist "${TESTDIR}/TYPO3-Instances/primary/public"
  assert_dir_exist "${TESTDIR}/TYPO3-Instances/primary/vendor"
  run bash -c "ls '${TESTDIR}/Build' | grep -cE '^(public|vendor)$' || true"
  assert_output "0"

  # A fresh project lists exactly its root checkout, marked primary.
  run ddev tryout worktree list --plain
  assert_success
  assert_output --partial "← primary"

  run ddev tryout worktree add v13 13.4
  assert_success
  assert_dir_exist "${TESTDIR}/worktrees/v13"
  # Nested worktrees record RELATIVE metadata, which is what makes host and
  # container paths interchangeable.
  run cat "${TESTDIR}/worktrees/v13/.git"
  assert_output --partial "gitdir: ../../.git/worktrees/v13"

  # Both checkouts are listed — the root does not live under worktrees/, so a bare
  # glob loses it, and that is the bug this pins.
  run ddev tryout worktree list --plain
  assert_success
  assert_output --partial "v13"
  assert_output --partial "← primary"

  # `use` repoints the primary instance's overlay. It must not create a symlink:
  # CORE_DIR is the project root, and `ln -sfn` there links INSIDE the checkout.
  run ddev tryout worktree use v13
  assert_success
  run grep -q '\.\./\.\./worktrees/v13/typo3/sysext/\*' \
      "${TESTDIR}/TYPO3-Instances/primary/composer.tryout.json"
  assert_success
  run bash -c "find '${TESTDIR}' -maxdepth 1 -type l | head -1"
  assert_output ""

  # And back again, to the root checkout.
  run ddev tryout worktree use main
  assert_success
  run grep -q '\.\./\.\./typo3/sysext/\*' \
      "${TESTDIR}/TYPO3-Instances/primary/composer.tryout.json"
  assert_success

  # Each instance owns its database, and says so in its own settings.php — the
  # container-wide TYPO3_DB_DBNAME must not override it.
  run ddev tryout worktree serve v13
  assert_success
  run grep -q "'dbname' => 'db_v13'" \
      "${TESTDIR}/TYPO3-Instances/v13/config/system/settings.php"
  assert_success
  run ddev exec "cd /var/www/html/TYPO3-Instances/v13 && php -r '
    \$GLOBALS[\"TYPO3_CONF_VARS\"] = include \"config/system/settings.php\";
    include \"config/system/additional.php\";
    echo \$GLOBALS[\"TYPO3_CONF_VARS\"][\"DB\"][\"Connections\"][\"Default\"][\"dbname\"];'"
  assert_success
  assert_output --partial "db_v13"

  # A restart must not re-run `typo3 setup` against a populated database.
  run ddev restart -y
  assert_success
  refute_output --partial "contains already"

  # Still clean, after all of it.
  run bash -c "git -C '${TESTDIR}' status --porcelain | head -5"
  assert_output ""
}

# ─── Journeys: every command a user can type, the way a user runs them ──────
# One project per journey and many commands in a row, so each Core clone pays
# for as much of the surface as possible. tests/unit.bats checks that every
# verb, subcommand and flag appears somewhere in here or in test.bats.

# An open change on main, resolved live so the test does not rot.
open_change() {
  curl -s 'https://review.typo3.org/changes/?q=status:open+project:Packages/TYPO3.CMS+branch:main&n=1' \
    | tail -c +6 | sed -n 's/.*"_number": *\([0-9]*\).*/\1/p' | head -1
}

# How many tables a database holds.
table_count() {
  ddev mysql -uroot -proot -N -e \
    "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='$1';" 2>/dev/null | tr -dc '0-9'
}

# bats test_tags=lifecycle
@test "journey: the primary is inspected, patched, updated, rebuilt and wiped" {
  set -eu -o pipefail
  addon_start

  # The tools' contracts: one JSON value each, and nothing else on stdout.
  run bash -c "ddev tryout worktree branches --json | jq -e 'index(\"main\") != null'"
  assert_success
  run bash -c "ddev tryout worktree list --json | jq -e '.[0].primary == true and .[0].name == \"main\"'"
  assert_success
  run bash -c "ddev tryout patch --list --json | jq -e 'type == \"array\" and length > 0'"
  assert_success

  local change
  change="$(open_change)"
  [ -n "${change}" ]
  run ddev tryout patch "${change}"
  assert_success
  run ddev tryout status
  assert_output --partial "1 applied"

  # Update refuses a dirty tree and says how to get past it...
  echo "local edit" >> "${TESTDIR}/README.md"
  run ddev tryout download
  assert_failure
  assert_output --partial "download --reset"
  # ...and --reset drops the edit and the patch alike.
  run ddev tryout download --reset
  assert_success
  run ddev tryout status
  assert_output --partial "none applied"
  run bash -c "cd '${TESTDIR}' && git status --porcelain | wc -l | tr -d ' '"
  assert_output "0"
  run ddev tryout download
  assert_success

  # composer puts back a sysext taken out of the overlay by hand.
  local overlay="${TESTDIR}/TYPO3-Instances/primary/composer.tryout.json"
  sed -i.bak '/"typo3\/cms-backend"/d' "${overlay}" && rm -f "${overlay}.bak"
  run grep -q '"typo3/cms-backend"' "${overlay}"
  assert_failure
  run ddev tryout composer
  assert_success
  run grep -q '"typo3/cms-backend"' "${overlay}"
  assert_success

  # delete asks first; with nobody to ask it refuses and names the way past.
  run ddev tryout delete
  assert_failure
  assert_output --partial "--yes"
  local fileadmin="${TESTDIR}/TYPO3-Instances/primary/public/fileadmin"
  mkdir -p "${fileadmin}" && echo x > "${fileadmin}/tryout-marker.txt"
  run ddev tryout delete --yes
  assert_success
  assert_file_not_exist "${fileadmin}/tryout-marker.txt"
  [ "$(table_count db)" -gt 0 ]
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"
}

# bats test_tags=lifecycle
@test "journey: a served worktree is switched, reset, renamed, opened, wiped and removed" {
  set -eu -o pipefail
  addon_start

  # --serve creates a hostname, and the command restarts DDEV for it by itself.
  run ddev tryout worktree add side 13.4 --serve --php 8.4
  assert_success
  assert_backend_loads "https://side.${PROJNAME}.ddev.site/typo3/"

  run ddev tryout checkout 12.4 --site side
  assert_success
  run ddev tryout exec side vendor/bin/typo3 --version
  assert_success
  assert_output --partial "TYPO3 CMS 12.4"

  run ddev tryout reset side
  assert_success
  run ddev tryout download side --reset
  assert_success

  # A served worktree renamed moves its site: new hostname up, old one gone.
  run ddev tryout worktree rename side lts
  assert_success
  assert_dir_exist "${TESTDIR}/worktrees/lts"
  assert_dir_not_exist "${TESTDIR}/worktrees/side"
  assert_backend_loads "https://lts.${PROJNAME}.ddev.site/typo3/"
  assert_backend_gone "https://side.${PROJNAME}.ddev.site/typo3/"

  # launch hands the URL to the desktop's opener; a stand-in records it.
  local bin="${BATS_TEST_TMPDIR}/bin"
  mkdir -p "${bin}"
  printf '#!/bin/sh\necho "$1" > "%s/opened"\n' "${BATS_TEST_TMPDIR}" > "${bin}/open"
  cp "${bin}/open" "${bin}/xdg-open"
  chmod +x "${bin}/open" "${bin}/xdg-open"
  PATH="${bin}:${PATH}" run ddev tryout launch lts --backend
  assert_success
  run cat "${BATS_TEST_TMPDIR}/opened"
  assert_output "https://lts.${PROJNAME}.ddev.site/typo3/"

  # --all wipes every site, the served one included, and both come back fresh.
  run ddev tryout delete --all --yes
  assert_success
  assert_backend_loads "https://${PROJNAME}.ddev.site/typo3/"
  assert_backend_loads "https://lts.${PROJNAME}.ddev.site/typo3/"

  run ddev tryout worktree unserve lts --drop-db
  assert_success
  [ "$(table_count db_lts)" = "0" ]

  # remove asks first; --yes is the scripted answer, and it takes the directory.
  run ddev tryout worktree remove lts
  assert_failure
  assert_output --partial "--yes"
  run ddev tryout worktree remove lts --yes
  assert_success
  assert_dir_not_exist "${TESTDIR}/worktrees/lts"
  run bash -c "cd '${TESTDIR}' && git worktree list --porcelain"
  refute_output --partial "worktrees/lts"
}

# bats test_tags=lifecycle
@test "journey: contribution is set up, diagnosed and taken back out" {
  set -eu -o pipefail
  addon_start
  local user="tryout-e2e-user"

  # The account lookup finds nobody by this name and the SSH probe cannot
  # authenticate here; both only inform, so setup still completes.
  run ddev tryout cs setup "${user}"
  assert_success
  assert_output --partial "Contribution setup complete"
  assert_file_executable "${TESTDIR}/.git/hooks/commit-msg"
  assert_file_executable "${TESTDIR}/.git/hooks/pre-commit"
  run git -C "${TESTDIR}" config --get commit.template
  assert_output ".ddev/tryout/gitmessage.txt"
  run git -C "${TESTDIR}" remote get-url --push origin
  assert_output "ssh://${user}@review.typo3.org:29418/Packages/TYPO3.CMS"

  run ddev tryout cs doctor
  assert_success
  assert_output --partial "${user}"
  assert_output --partial "installed"
  assert_output --partial "Gerrit SSH"

  run ddev tryout cs uninstall
  assert_success
  assert_file_not_exist "${TESTDIR}/.git/hooks/commit-msg"
  run git -C "${TESTDIR}" remote get-url --push origin
  assert_output "$(git -C "${TESTDIR}" remote get-url origin)"

  run ddev tryout help
  assert_success
  assert_output --partial "Commands:"

  # No session was ever started: stop says so and succeeds.
  run ddev tryout ui stop
  assert_success
  assert_output --partial "No session running"
}

# bats test_tags=lifecycle
@test "serving works when DDEV has to write the hostname into /etc/hosts" {
  set -eu -o pipefail
  # With use_dns_when_possible off, DDEV never trusts DNS for *.ddev.site: every
  # hostname goes into /etc/hosts through `sudo ddev-hostname` — the route a
  # machine with DNS-rebinding protection takes, and the one that asks for a
  # password. It runs where sudo needs none (CI); anywhere else it would stop at
  # the prompt, which the TUI's own tests cover with a stand-in.
  sudo -n true 2>/dev/null || skip "needs passwordless sudo — DDEV edits /etc/hosts here"
  run ddev config --use-dns-when-possible=false
  assert_success
  addon_start

  run ddev tryout worktree add hosts 13.4 --serve
  assert_success
  run grep -F "hosts.${PROJNAME}.ddev.site" /etc/hosts
  assert_success
  assert_backend_loads "https://hosts.${PROJNAME}.ddev.site/typo3/"

  # A --php switch keeps the hostname set, so it applies in place — no restart,
  # and no second trip through sudo.
  run ddev tryout worktree serve hosts --php 8.4
  assert_success
  assert_output --partial "Applied without a restart"
  run ddev tryout exec hosts vendor/bin/typo3 --version
  assert_output --partial "PHP 8.4"
}
