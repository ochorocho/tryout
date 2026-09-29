#!/usr/bin/env bats

# Unit tests for what the add-on ships besides the tryout binary: the command
# and completion shims, the launcher, install.yaml and the config. They need no
# DDEV project and no containers and run in seconds:
#   bats ./tests/unit.bats
#
# The binary's own logic is tested in Rust (cd tui && cargo test); what a user
# does end to end is tests/test.bats and tests/lifecycle.bats.

setup() {
  set -eu -o pipefail
  TEST_BREW_PREFIX="$(brew --prefix 2>/dev/null || true)"
  export BATS_LIB_PATH="${BATS_LIB_PATH:-}:${TEST_BREW_PREFIX}/lib:/usr/lib/bats"
  bats_load_library bats-assert
  bats_load_library bats-file
  bats_load_library bats-support

  export DIR="$(cd "$(dirname "${BATS_TEST_FILENAME}")/.." >/dev/null 2>&1 && pwd)"
  export FAKEROOT="$(mktemp -d "${BATS_TMPDIR:-/tmp}/tryout-unit.XXXXXX")"
}

teardown() {
  set -eu -o pipefail
  [ -n "${FAKEROOT:-}" ] && rm -rf "${FAKEROOT}"
}

# The bash the add-on still ships: shims, the launcher, a compat script.
shipped_scripts() {
  printf '%s\n' "${DIR}/commands/host/tryout" "${DIR}/commands/host/autocomplete/tryout" \
    "${DIR}/tryout/tryout" "${DIR}"/tryout/*.sh
}

# Lay a payload out under FAKEROOT the way `ddev add-on get` does, with a fake
# binary for this machine that prints its arguments (and stderr noise).
fake_install() {
  mkdir -p "${FAKEROOT}/.ddev/commands/host/autocomplete" "${FAKEROOT}/.ddev/tryout/bin"
  cp "${DIR}/commands/host/tryout" "${FAKEROOT}/.ddev/commands/host/"
  cp "${DIR}/commands/host/autocomplete/tryout" "${FAKEROOT}/.ddev/commands/host/autocomplete/"
  cp "${DIR}/tryout/tryout" "${DIR}/tryout/tryout-php-fpm.sh" "${FAKEROOT}/.ddev/tryout/"
  local b
  for b in tryout-macos-universal tryout-linux-x86_64 tryout-linux-aarch64; do
    printf '#!/bin/sh\necho "%s $*"\necho noise >&2\nexit "${FAKE_EXIT:-0}"\n' "${b}" \
      > "${FAKEROOT}/.ddev/tryout/bin/${b}"
    chmod +x "${FAKEROOT}/.ddev/tryout/bin/${b}"
  done
}

@test "every shipped script parses, under the bash 3.2 macOS still ships too" {
  set -eu -o pipefail
  local f
  while IFS= read -r f; do
    run bash -n "${f}"
    assert_success
    if [ -x /bin/bash ]; then
      run /bin/bash -n "${f}"
      assert_success
    fi
  done < <(shipped_scripts)
}

@test "no GNU-only utilities are assumed" {
  # These break on macOS's BSD userland. `sed -i` needs a suffix to work on both.
  set -eu -o pipefail
  local all bad
  all="$(shipped_scripts | xargs cat; sed -n '/^pre_install_actions:/,$p' "${DIR}/install.yaml")"
  for bad in 'readlink -f' 'grep -P' 'sed -r ' 'stat -c' 'date -d'; do
    printf '%s\n' "${all}" | grep -vE '^[[:space:]]*#' | grep -qF -- "${bad}" \
      && fail "GNU-only: ${bad}"
  done
  printf '%s\n' "${all}" | grep -vE '^[[:space:]]*#' | grep -qE 'sed -i +[^.]' \
    && fail "a bare sed -i is GNU-only"
  :
}

@test "the command declares no AutocompleteTerms and no Flags header" {
  # AutocompleteTerms sets cobra's ValidArgs, which rejects any second argument
  # during completion, so the completion script never runs. Flags makes DDEV parse
  # flags itself and reject the ones it does not know, like `--php 8.2`.
  set -eu -o pipefail
  run grep -qE '^## (AutocompleteTerms|Flags):' "${DIR}/commands/host/tryout"
  assert_failure
}

@test "the completion script ships executable, marked, without CRLF" {
  # DDEV skips an autocomplete script containing \r\n, with only a warning.
  set -eu -o pipefail
  assert_file_executable "${DIR}/commands/host/autocomplete/tryout"
  run grep -q '#ddev-generated' "${DIR}/commands/host/autocomplete/tryout"
  assert_success
  run grep -qU $'\r' "${DIR}/commands/host/autocomplete/tryout"
  assert_failure
}

@test "the host command hands every argument to the binary, unchanged" {
  set -eu -o pipefail
  fake_install
  run env DDEV_APPROOT="${FAKEROOT}" bash "${FAKEROOT}/.ddev/commands/host/tryout" \
    exec v13 "config:set" "My Site" '$(x)'
  assert_success
  assert_line --partial 'exec v13 config:set My Site $(x)'
}

@test "completion finds its project from its own path and never fails or talks on stderr" {
  set -eu -o pipefail
  fake_install
  # DDEV gives it no DDEV_* variables and runs it from anywhere.
  run bash -c "cd / && env -u DDEV_APPROOT '${FAKEROOT}/.ddev/commands/host/autocomplete/tryout' tryout cs '' 2>&1"
  assert_success
  assert_output --regexp '__complete tryout cs'
  refute_output --partial noise
  # A binary that fails still leaves a clean exit 0.
  run bash -c "cd / && FAKE_EXIT=3 '${FAKEROOT}/.ddev/commands/host/autocomplete/tryout' tryout '' 2>&1"
  assert_success
  # No binary at all: nothing, and still 0.
  rm -f "${FAKEROOT}/.ddev/tryout/tryout"
  run bash -c "'${FAKEROOT}/.ddev/commands/host/autocomplete/tryout' tryout '' 2>&1"
  assert_success
  assert_output ""
}

@test "the launcher picks the build for the machine, and TRYOUT_BIN wins" {
  set -eu -o pipefail
  fake_install
  mkdir -p "${FAKEROOT}/fakebin"
  local os arch want
  for os in Darwin:arm64:tryout-macos-universal Darwin:x86_64:tryout-macos-universal \
            Linux:x86_64:tryout-linux-x86_64 Linux:aarch64:tryout-linux-aarch64 \
            Linux:arm64:tryout-linux-aarch64; do
    IFS=: read -r os arch want <<< "${os}"
    printf '#!/bin/sh\n[ "$1" = -s ] && echo %s || echo %s\n' "${os}" "${arch}" > "${FAKEROOT}/fakebin/uname"
    chmod +x "${FAKEROOT}/fakebin/uname"
    run env PATH="${FAKEROOT}/fakebin:${PATH}" "${FAKEROOT}/.ddev/tryout/tryout" status
    assert_success
    assert_line "${want} status"
  done
  run env TRYOUT_BIN=/bin/echo "${FAKEROOT}/.ddev/tryout/tryout" hello
  assert_output "hello"
}

@test "the launcher says what to do when this machine has no build" {
  set -eu -o pipefail
  fake_install
  rm -f "${FAKEROOT}"/.ddev/tryout/bin/*
  run "${FAKEROOT}/.ddev/tryout/tryout" status
  assert_equal "${status}" 69
  assert_output --partial "is missing"
  assert_output --partial "stage-bins.sh"
}

@test "the launcher restores an executable bit a file sync lost" {
  set -eu -o pipefail
  fake_install
  chmod -x "${FAKEROOT}"/.ddev/tryout/bin/*
  run "${FAKEROOT}/.ddev/tryout/tryout" status
  assert_success
}

@test "an FPM daemon from an older config reaches the binary's __fpm" {
  set -eu -o pipefail
  fake_install
  run bash "${FAKEROOT}/.ddev/tryout/tryout-php-fpm.sh" 8.2
  assert_success
  assert_line --partial "__fpm 8.2"
}

@test "install moves FPM daemons from the old script to the binary" {
  set -eu -o pipefail
  local action
  action="$(sed -n '/#ddev-description:Point PHP-FPM daemons at the tryout binary/,/^  - |/p' "${DIR}/install.yaml" \
    | sed '$d' | sed 's/^    //')"
  mkdir -p "${FAKEROOT}/ddev" && cd "${FAKEROOT}/ddev"
  printf 'web_extra_daemons:\n  - name: tryout-php-8.2\n    command: "bash /var/www/html/.ddev/tryout/tryout-php-fpm.sh 8.2"\n' > config.worktrees.yaml
  run bash -c "${action}"
  assert_success
  run grep -F 'command: "/var/www/html/.ddev/tryout/tryout __fpm 8.2"' config.worktrees.yaml
  assert_success
  assert_file_not_exist config.worktrees.yaml.tryout-bak
}

@test "the payload version is one number, and install records it" {
  set -eu -o pipefail
  run grep -cE '^[0-9]+$' "${DIR}/tryout/VERSION"
  assert_output "1"
  run grep -q '#ddev-generated' "${DIR}/tryout/VERSION"
  assert_success
  run grep -qF "grep -E '^[0-9]+\$' tryout/VERSION | head -1 > tryout/.version" "${DIR}/install.yaml"
  assert_success
}

@test "every PHP file the add-on ships is valid" {
  set -eu -o pipefail
  command -v php >/dev/null 2>&1 || skip 'php not available'
  local f
  for f in "${DIR}"/tryout/*.php; do
    run php -l "${f}"
    assert_success
  done
}

@test "the shipped overlay template is valid JSON and carries its marker" {
  set -eu -o pipefail
  command -v php >/dev/null 2>&1 || skip 'php not available'
  run php -r 'json_decode(file_get_contents($argv[1]), true, 512, JSON_THROW_ON_ERROR);' \
    "${DIR}/tryout/composer.tryout.json"
  assert_success
  run grep -q 'ddev-generated' "${DIR}/tryout/composer.tryout.json"
  assert_success
}

@test "install.yaml lists every file the add-on ships, and each exists" {
  set -eu -o pipefail
  local entry
  for entry in commands/host/tryout commands/host/autocomplete/tryout \
               config.tryout.yaml tryout web-build/Dockerfile.tryout; do
    run grep -qE "^  - ${entry}\$" "${DIR}/install.yaml"
    assert_success
    assert_exist "${DIR}/${entry}"
  done
}

@test "every file the add-on ships carries its marker, the binaries included" {
  # DDEV only updates or removes a file with it; the conformance checker demands it.
  set -eu -o pipefail
  local f
  while IFS= read -r f; do
    grep -q 'ddev-generated' "${f}" || fail "no #ddev-generated in ${f#"${DIR}/"}"
  done < <(find "${DIR}/tryout" "${DIR}/commands" -type f ! -name '.*'; \
           echo "${DIR}/config.tryout.yaml"; echo "${DIR}/web-build/Dockerfile.tryout")
}

@test "the post-start hook is the binary, run inside the web container" {
  set -eu -o pipefail
  run grep -qE '^    - exec: \.ddev/tryout/tryout __post-start$' "${DIR}/config.tryout.yaml"
  assert_success
}

@test "install refuses Windows and warns when this machine has no build" {
  set -eu -o pipefail
  run grep -q 'use WSL2' "${DIR}/install.yaml"
  assert_success
  run grep -q 'No tryout build for this machine' "${DIR}/install.yaml"
  assert_success
}

@test "install cleans away the bash implementation an older version shipped" {
  set -eu -o pipefail
  local f
  for f in functions.sh commands.sh tryout-container.sh post-start.sh sync-composer.php; do
    run grep -q "tryout/${f}" "${DIR}/install.yaml"
    assert_success
  done
  assert_file_not_exist "${DIR}/tryout/functions.sh"
}

@test "the web image builds a pinned git that supports relative worktree paths" {
  set -eu -o pipefail
  local f="${DIR}/web-build/Dockerfile.tryout"
  assert_file_exist "${f}"
  run grep -q '^#ddev-generated' "${f}"
  assert_success
  # Version and checksum are pinned, so an image never builds from a tarball
  # nobody has looked at.
  run grep -E '^ARG TRYOUT_GIT_VERSION=2\.(4[8-9]|[5-9][0-9])\.[0-9]+$' "${f}"
  assert_success
  run grep -E '^ARG TRYOUT_GIT_SHA256=[0-9a-f]{64}$' "${f}"
  assert_success
  run grep -q 'sha256sum -c' "${f}"
  assert_success
  # Build dependencies do not stay in the image.
  run grep -q -- '--auto-remove' "${f}"
  assert_success
  run grep -qE '^  - web-build/Dockerfile\.tryout$' "${DIR}/install.yaml"
  assert_success
}

@test "install notes a host git older than 2.48, and only notes it" {
  set -eu -o pipefail
  # The pre-install action, extracted the way DDEV would run it.
  sed -n '/^pre_install_actions:/,/^project_files:/p' "${DIR}/install.yaml" \
    | sed '1d;$d' | sed '1d' | sed 's/^    //' > "${FAKEROOT}/action.sh"
  mkdir -p "${FAKEROOT}/bin"
  printf '#!/bin/sh\necho "git version 2.39.5"\n' > "${FAKEROOT}/bin/git"
  chmod +x "${FAKEROOT}/bin/git"
  run env PATH="${FAKEROOT}/bin:${PATH}" bash "${FAKEROOT}/action.sh"
  assert_success
  assert_output --partial "predates relative worktree paths"
  printf '#!/bin/sh\necho "git version 2.50.1 (Apple Git-155)"\n' > "${FAKEROOT}/bin/git"
  run env PATH="${FAKEROOT}/bin:${PATH}" bash "${FAKEROOT}/action.sh"
  assert_success
  refute_output --partial "predates"
}

@test "composer installs a Core whose pinned packages have advisories, but never malware" {
  set -eu -o pipefail
  # Composer (2.9+) refuses packages with known advisories while RESOLVING. Older
  # and dev Cores pin such versions — 13.3 pins enshrined/svg-sanitize ^0.20.0 —
  # so serving one failed outright. tryout exists to run those Cores locally.
  run grep -xF '  - COMPOSER_POLICY_ADVISORIES_BLOCK=0' "${DIR}/config.tryout.yaml"
  assert_success
  # Only the advisories policy. The blanket switches also stop blocking malware.
  run bash -c "grep -v '^[[:space:]]*#' '${DIR}/config.tryout.yaml' \
    | grep -E 'COMPOSER_NO_(SECURITY_)?BLOCKING|COMPOSER_POLICY_MALWARE_BLOCK'"
  assert_failure
}

@test "uninstalling takes the served-site markers with it" {
  set -eu -o pipefail
  # .tryout-site IS the definition of "served" (site_is_served), so a marker left
  # behind makes a reinstalled add-on believe a site is served that has no vhost,
  # no grant and no daemon: worktree list prints a URL that 404s.
  local removal
  removal=$(sed -n '/^removal_actions:/,$p' "${DIR}/install.yaml")

  printf '%s' "${removal}" | grep -q 'TYPO3-Instances/\*/.tryout-site' \
    || fail "removal must take the served-site markers"
  printf '%s' "${removal}" | grep -q 'TYPO3-Instances/\..*settings.php' \
    || fail "removal must take the settings unserve saved"

  # The instance TREES stay: they hold a site's public/, vendor/ and content, which
  # is the user's, exactly as packages/ is.
  printf '%s' "${removal}" | grep -qE 'rm -rf .*TYPO3-Instances/\*[^/]' \
    && fail "an instance tree is user content; removal must not delete it"
  :
}

@test "an instance's own settings.php decides its database, not the environment" {
  set -eu -o pipefail
  command -v php >/dev/null 2>&1 || skip 'php not available'
  # config.tryout.yaml sets TYPO3_DB_DBNAME=db for the WHOLE container, so taking
  # the environment first meant every instance reached without a vhost — every CLI
  # command — silently used the PRIMARY's database while its own settings.php said
  # otherwise. Served sites only looked right because their vhost injects the
  # correct name over HTTP; nothing injects it for the CLI.
  local add="${DIR}/tryout/additional.php"
  local dir="${FAKEROOT}/dbresolve"
  mkdir -p "${dir}"

  # Runs additional.php with a given pre-loaded dbname and environment, and prints
  # what the connection ends up on.
  resolve() { # <settings-dbname> <env-dbname>
    IS_DDEV_PROJECT=true TYPO3_DB_DBNAME="${2}" php -r "
      \$GLOBALS['TYPO3_CONF_VARS'] = ['DB' => ['Connections' => ['Default' => [
          'dbname' => '${1}',
      ]]]];
      include '${add}';
      echo \$GLOBALS['TYPO3_CONF_VARS']['DB']['Connections']['Default']['dbname'];
    "
  }

  # A served worktree keeps its own database even though the container-wide
  # environment says 'db'. This is the bug.
  run resolve "db_v13" "db"
  assert_output "db_v13"

  # The primary is unaffected: its settings.php says db and so does the result.
  run resolve "db" "db"
  assert_output "db"

  # No settings yet — the FIRST RUN. site_exec exports the right name for
  # `typo3 setup`, and it must still win, or serving a new worktree would install
  # it into the primary's database.
  run resolve "" "db_new"
  assert_output "db_new"

  # Nothing anywhere falls back to plain db rather than erroring out.
  run resolve "" ""
  assert_output "db"
}

@test "every command a user can type runs somewhere against a real DDEV project" {
  set -eu -o pipefail
  # One pattern per verb, subcommand and flag. Each must match a `ddev tryout …`
  # line in the DDEV-backed suites; the journeys in lifecycle.bats exist so
  # that it does. A new verb or flag with no end-to-end test fails here.
  local patterns=(
    'status' 'help' 'composer' 'launch .*--backend' 'ui stop'
    'download$' 'download --reset' 'download [a-z0-9-]+ --reset'
    'checkout [0-9.]+' 'checkout .*--site'
    'patch "?\$\{?change' 'patch --list --json' 'patch [0-9]+'
    'reset$' 'reset [a-z0-9-]+'
    'delete$' 'delete --yes' 'delete --all'
    'exec [a-z@]'
    'cs setup' 'cs doctor' 'cs uninstall' 'cs help'
    'worktree add [a-z0-9-]+ [0-9.]+$' 'worktree add .*--serve' 'worktree add .*--php'
    'worktree list$' 'worktree list --plain' 'worktree list --json'
    'worktree branches --json'
    'worktree use' 'worktree serve [a-z0-9-]+ --php' 'worktree unserve [a-z0-9-]+$'
    'worktree unserve .*--drop-db' 'worktree rename'
    'worktree remove [a-z0-9-]+$' 'worktree remove .*--yes'
  )
  # Not covered on purpose: `ui` attaching needs a terminal to draw on; the TUI
  # is tested by its own Rust suite (pseudo-terminal and snapshot tests).
  local lines p missing=()
  lines="$(grep -hoE 'ddev tryout [^|;&)`]*' "${DIR}/tests/test.bats" "${DIR}/tests/lifecycle.bats" \
    | sed -e 's/["]*[[:space:]]*$//')"
  for p in "${patterns[@]}"; do
    printf '%s\n' "${lines}" | grep -qE "^ddev tryout ${p}" || missing+=("${p}")
  done
  [ ${#missing[@]} -eq 0 ] || fail "no end-to-end test runs: ${missing[*]}"
}
