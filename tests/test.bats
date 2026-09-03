setup() {
  set -eu -o pipefail
  export DIR="$( cd "$( dirname "$BATS_TEST_FILENAME" )/.." >/dev/null 2>&1 && pwd )"
  export PROJNAME="test-tryout"
  export TESTDIR=~/tmp/${PROJNAME}
  mkdir -p "${TESTDIR}"
  export DDEV_NONINTERACTIVE=true
  ddev delete -Oy "${PROJNAME}" >/dev/null 2>&1 || true
  cd "${TESTDIR}"
  ddev config --project-name="${PROJNAME}" --project-type=typo3 \
    --docroot=public --php-version=8.5 >/dev/null
}

teardown() {
  set -eu -o pipefail
  ddev delete -Oy "${PROJNAME}" >/dev/null 2>&1 || true
  [ -n "${TESTDIR:-}" ] && rm -rf "${TESTDIR}"
}

# The add-on ships only .ddev-scoped payload plus a few guarded project-root files.
health_checks() {
  # Payload landed where DDEV puts project_files.
  [ -f .ddev/commands/host/tryout ]
  [ -f .ddev/tryout/functions.sh ]
  [ -f .ddev/config.tryout.yaml ]

  # Project-root files the post_install_actions copy out of .ddev/tryout/.
  [ -f composer.tryout.json ]
  [ -f config/system/additional.php ]
  [ -d packages ]

  # The add-on must never create the project's own composer.json.
  [ ! -f composer.json ]

  # Everything shipped is marked, so DDEV may update and remove it.
  grep -q '#ddev-generated' .ddev/commands/host/tryout
  grep -q 'ddev-generated' composer.tryout.json

  # The command is registered and runs. Capture first: `grep -q` exits on the
  # first match and would break the pipe under `set -o pipefail`.
  local help_output
  help_output="$(ddev tryout help)"
  echo "${help_output}" | grep -q 'TYPO3 development toolkit'
  echo "${help_output}" | grep -q 'cs \[setup|doctor\]'
}

@test "install from directory" {
  set -eu -o pipefail
  cd "${TESTDIR}"
  echo "# ddev add-on get ${DIR} in $(pwd)" >&3
  ddev add-on get "${DIR}"
  health_checks
}

@test "install from release" {
  set -eu -o pipefail
  cd "${TESTDIR}" || ( printf "unable to cd to ${TESTDIR}\n" && exit 1 )
  echo "# ddev add-on get bmack/tryout with project ${PROJNAME} in $(pwd)" >&3
  ddev add-on get bmack/tryout
  health_checks
}

@test "an existing composer.json is never rewritten" {
  set -eu -o pipefail
  cd "${TESTDIR}"
  cat > composer.json <<'JSON'
{ "name": "acme/site", "require": { "psr/log": "^3.0" } }
JSON
  cp composer.json /tmp/${PROJNAME}-composer-before.json
  ddev add-on get "${DIR}"
  diff composer.json /tmp/${PROJNAME}-composer-before.json
  rm -f /tmp/${PROJNAME}-composer-before.json
}

@test "reinstall does not clobber a file the user took ownership of" {
  set -eu -o pipefail
  cd "${TESTDIR}"
  ddev add-on get "${DIR}"
  # Dropping the marker is the documented way to take ownership.
  sed -i.bak 's/#ddev-generated/#user-owned/' config/system/additional.php
  echo '// my own change' >> config/system/additional.php
  ddev add-on get "${DIR}"
  grep -q 'my own change' config/system/additional.php
}

@test "remove takes its files back out" {
  set -eu -o pipefail
  cd "${TESTDIR}"
  ddev add-on get "${DIR}"
  ddev add-on remove tryout
  [ ! -d .ddev/tryout ]
  [ ! -f .ddev/commands/host/tryout ]
  [ ! -f .ddev/config.tryout.yaml ]
  [ ! -f composer.tryout.json ]
  [ ! -f config/system/additional.php ]
}
