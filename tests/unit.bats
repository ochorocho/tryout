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

# Lay the payload out under FAKEROOT the way `ddev add-on get` does, then run the
# completion script the way DDEV does: absolute path, argv = the command line,
# an empty word as the literal '', and no DDEV_* variables in the environment.
complete() {
  if [ ! -x "${FAKEROOT}/.ddev/commands/host/autocomplete/tryout" ]; then
    mkdir -p "${FAKEROOT}/.ddev/commands/host/autocomplete" "${FAKEROOT}/.ddev/tryout"
    cp "${DIR}/commands/host/autocomplete/tryout" "${FAKEROOT}/.ddev/commands/host/autocomplete/"
    cp "${DIR}/tryout/functions.sh" "${FAKEROOT}/.ddev/tryout/"
    chmod +x "${FAKEROOT}/.ddev/commands/host/autocomplete/tryout"
  fi
  # cd elsewhere on purpose: DDEV leaves the cwd wherever the user pressed TAB.
  (cd / && env -u DDEV_APPROOT -u DDEV_SITENAME \
    "${FAKEROOT}/.ddev/commands/host/autocomplete/tryout" tryout "$@")
}

# Candidates carry a TAB-separated description and free-text positions emit an
# `_activeHelp_` hint line; tests that care about the names alone go through this.
names() { complete "$@" | grep -v '^_activeHelp_ ' | cut -f1; }

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

@test "the primary site resolves to TYPO3-Instances/primary" {
  set -eu -o pipefail
  # The project root is Core's source tree, and Build/ is Core's OWN directory —
  # instances live under TYPO3-Instances/ instead, one per site, the primary
  # included. Core's composer.json is typo3/cms with no web-dir, so nothing would
  # put a docroot at the root even if we wanted one there.
  run helper site_dir
  assert_output "${FAKEROOT}/TYPO3-Instances/primary"
  run helper site_dir "@primary"
  assert_output "${FAKEROOT}/TYPO3-Instances/primary"
  run helper site_docroot
  assert_output "${FAKEROOT}/TYPO3-Instances/primary/public"
}

@test "a named site resolves under TYPO3-Instances/" {
  set -eu -o pipefail
  run helper site_dir v13
  assert_output "${FAKEROOT}/TYPO3-Instances/v13"
  run helper site_docroot v13
  assert_output "${FAKEROOT}/TYPO3-Instances/v13/public"
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

@test "the primary Core is the root checkout, a named one is its worktree" {
  set -eu -o pipefail
  # There is no symlink any more: the root clone IS the primary, and a served
  # site is nailed to its own nested worktree so it cannot follow the root's branch.
  run helper site_core_dir
  assert_output "${FAKEROOT}"
  run helper site_core_dir v13
  assert_output "${FAKEROOT}/worktrees/v13"
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
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"
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
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13" "${FAKEROOT}/TYPO3-Instances/v12" "${FAKEROOT}/TYPO3-Instances/scratch"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"
  printf 'php=8.3\n' > "${FAKEROOT}/TYPO3-Instances/v12/.tryout-site"

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
  for f in "${DIR}"/tryout/*.sh "${DIR}/commands/host/tryout" \
           "${DIR}/commands/host/autocomplete/tryout"; do
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
  for entry in commands/host/tryout commands/host/autocomplete/tryout \
               config.tryout.yaml tryout web-build/Dockerfile.tryout; do
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

# --- Tab-completion -------------------------------------------------------
# DDEV runs commands/host/autocomplete/tryout on every TAB, passing the command
# line as argv and reading candidates from stdout. See the script's header for
# the contract these tests pin down.

@test "the completion script ships executable" {
  set -eu -o pipefail
  assert_file_executable "${DIR}/commands/host/autocomplete/tryout"
  run grep -q '#ddev-generated' "${DIR}/commands/host/autocomplete/tryout"
  assert_success
}

@test "the completion script has no CRLF line endings" {
  # DDEV skips an autocomplete script containing \r\n, with only a warning.
  set -eu -o pipefail
  run grep -qU $'\r' "${DIR}/commands/host/autocomplete/tryout"
  assert_failure
}

@test "the command declares no AutocompleteTerms header" {
  # It would set cobra's ValidArgs, which then rejects any second argument during
  # completion — so the autocomplete script never runs and `ddev tryout cs <TAB>`
  # completes nothing. Verified against ddev v1.25.2 with `ddev __complete`.
  set -eu -o pipefail
  run grep -q '^## AutocompleteTerms:' "${DIR}/commands/host/tryout"
  assert_failure
}

@test "completion covers every command the dispatch case accepts" {
  set -eu -o pipefail
  local actions verb
  actions=$(sed -n '/^case "${ACTION}" in/,/^esac/p' "${DIR}/commands/host/tryout" \
    | sed -n 's/^    \([a-z|]*\)).*/\1/p' | tr '|' '\n' | grep -v '^\*$')
  [ -n "${actions}" ]

  run names "''"
  assert_success
  for verb in ${actions}; do
    assert_line "${verb}"
  done
}

@test "completion suggests the top-level commands" {
  set -eu -o pipefail
  run names "''"
  assert_success
  for verb in status download checkout composer patch worktree cs exec reset delete help; do
    assert_line "${verb}"
  done
}

@test "completion is position aware for cs and worktree" {
  set -eu -o pipefail
  run names cs "''"
  assert_success
  assert_line "setup"
  assert_line "doctor"
  assert_line "uninstall"
  # The top-level verbs must NOT come back here — that is the whole point of the
  # script over the flat AutocompleteTerms list.
  refute_line "download"

  run names worktree "''"
  assert_success
  for sub in add list use serve unserve remove; do
    assert_line "${sub}"
  done
  refute_line "status"
}

@test "completion offers the flags a subcommand actually parses" {
  set -eu -o pipefail
  run names download "''"
  assert_line "--reset"

  run names worktree unserve "''"
  assert_line "--drop-db"

  run names worktree remove "''"
  assert_line "--force"
}

@test "completion lists the worktrees on disk" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/worktrees/v13"

  # `use` switches the PRIMARY, so it never offers the primary itself — that is
  # the root checkout, whose name comes from its branch rather than from a
  # directory under worktrees/.
  run names worktree use "''"
  assert_success
  assert_line "v13"
  refute_line "main"

  # A verb that takes any worktree offers the root checkout too, exactly once
  # even when a worktrees/<primary> directory also exists.
  run names worktree serve "''"
  assert_success
  assert_line "main"
  assert_line "v13"
  [ "$(printf '%s\n' "${lines[@]}" | grep -c '^main$')" -eq 1 ] \
    || fail "the primary must be offered once, not once per source"
}

@test "completion lists served sites for the commands that take one" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"

  run names exec "''"
  assert_success
  assert_line "@primary"
  assert_line "v13"
}

@test "completion never writes to stderr and never fails" {
  # DDEV merges our stderr into the candidate list and drops every suggestion if
  # we exit non-zero, so both are silent breakages. Sweep every dispatch path.
  set -eu -o pipefail
  local args err
  for args in "''" "cs ''" "worktree ''" "worktree add foo ''" "worktree serve ''" \
              "checkout ''" "patch ''" "reset ''" "delete ''" "exec ''" \
              "status ''" "composer ''" "help ''" "download ''" "bogus ''"; do
    # shellcheck disable=SC2086
    err=$(complete ${args} 2>&1 >/dev/null)
    [ -z "${err}" ] || { echo "stderr for '${args}': ${err}"; false; }
    # shellcheck disable=SC2086
    complete ${args} >/dev/null
  done
}

@test "completion survives a missing functions.sh" {
  # A partial install must degrade to the static candidates, never break TAB.
  set -eu -o pipefail
  run names "''"
  assert_success
  rm -f "${FAKEROOT}/.ddev/tryout/functions.sh"

  # A verb that still HAS a flag — `use` has none now, and an empty candidate list
  # would pass this test without proving the script survived.
  run names worktree remove "''"
  assert_success
  assert_line "--force"
}

@test "no fractional read -t, which bash 3.2 rejects outright" {
  # `read -t 0.01` is bash 4+. On macOS's bash 3.2 it fails with "invalid timeout
  # specification" — so a drain written that way never runs, and every prompt after
  # a single-key read silently swallows the stale Enter and cancels. Parsing is not
  # enough to catch this: it is a runtime argument error.
  set -eu -o pipefail
  run bash -c "
    cat '${DIR}'/tryout/*.sh \
        '${DIR}/commands/host/tryout' \
      | grep -vE '^[[:space:]]*#' | grep -qE 'read .*-t +[0-9]*\\.[0-9]'
  "
  assert_failure

  # And bash 3.2 really does reject it, so the guard is not theoretical.
  if [ -x /bin/bash ]; then
    run /bin/bash -c 'read -r -t 0.01 x </dev/null'
    assert_failure
  fi
}

@test "every shipped script parses under bash 3.2, which macOS still ships" {
  set -eu -o pipefail
  [ -x /bin/bash ] || skip 'no /bin/bash'
  local f
  for f in "${DIR}"/tryout/*.sh \
           "${DIR}/commands/host/tryout" "${DIR}/commands/host/autocomplete/tryout"; do
    run /bin/bash -n "${f}"
    assert_success
  done
}

@test "no GNU-only utilities are assumed" {
  # These break on macOS's BSD userland. `sed -i` needs a suffix to work on both,
  # which is why the tests use sed -i.bak.
  set -eu -o pipefail
  local bad
  for bad in 'readlink -f' 'grep -P' 'sed -r ' 'stat -c' 'date -d'; do
    run bash -c "
      cat '${DIR}'/tryout/*.sh \
          '${DIR}/commands/host/tryout' '${DIR}/commands/host/autocomplete/tryout' \
        | grep -vE '^[[:space:]]*#' | grep -q -- '${bad}'
    "
    assert_failure
  done

  # A bare `sed -i` with no suffix is GNU-only; BSD would eat the next argument.
  run bash -c "
    cat '${DIR}'/tryout/*.sh '${DIR}/commands/host/tryout' \
      | grep -vE '^[[:space:]]*#' | grep -qE 'sed -i +[^.]'
  "
  assert_failure
}

@test "a served site's vhost tells PHP the request was TLS" {
  # Without `fastcgi_param HTTPS`, TYPO3 builds http:// URLs behind DDEV's TLS
  # terminator and its secure session cookie is never returned — the backend login
  # then fails with "Please activate Cookies" while the page still answers 200, so
  # no status-code check can catch it. The $ must reach nginx literally, which
  # means escaping it inside the generator's unquoted heredoc.
  set -eu -o pipefail
  run grep -q 'fastcgi_param HTTPS \\$fcgi_https;' "${DIR}/tryout/functions.sh"
  assert_success

  # An unescaped $fcgi_https would be eaten by the shell and emit
  # `fastcgi_param HTTPS ;`, which nginx rejects outright — taking the whole
  # container down, not just that site.
  run grep -q 'fastcgi_param HTTPS \$fcgi_https;' "${DIR}/tryout/functions.sh"
  assert_failure
}

@test "the default PHP comes from the branch's own constraint" {
  # Core states its requirement per branch — ^8.5 on main, ^8.2 on 13.4 — so the
  # project's PHP is the wrong default for a worktree on another branch. An upper
  # bound has to be honoured too: reading only the floor would hand a capped
  # branch a PHP it rejects.
  set -eu -o pipefail
  command -v php >/dev/null 2>&1 || skip 'php not available'

  mkdir -p "${FAKEROOT}/worktrees/capped" "${FAKEROOT}/worktrees/open" \
           "${FAKEROOT}/worktrees/nojson"
  printf '{"require":{"php":">=8.2 <8.4"}}' > "${FAKEROOT}/worktrees/capped/composer.json"
  printf '{"require":{"php":"^8.2"}}'       > "${FAKEROOT}/worktrees/open/composer.json"
  printf '{}'                               > "${FAKEROOT}/worktrees/nojson/composer.json"

  # Stub the container lookup: these are the versions the web image ships.
  run env DDEV_PHP_VERSION=8.5 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    available_php_versions() { printf '8.2\n8.3\n8.4\n8.5\n'; }
    printf '%s %s %s' \
      \"\$(best_php_for_worktree capped)\" \
      \"\$(best_php_for_worktree open)\" \
      \"\$(best_php_for_worktree nojson)\"
  "
  assert_success
  # capped stops at 8.3; open takes the highest; no constraint falls back.
  assert_output "8.3 8.5 8.5"
}

@test "the PHP constraint is read from require, not config.platform" {
  # A naive grep for "php" finds config.platform.php first — a pinned build
  # version, not the constraint.
  set -eu -o pipefail
  run grep -q 'require.*\]\["php"\]' "${DIR}/tryout/functions.sh"
  assert_success
}

@test "a failed serve does not leave the site marked as served" {
  # .tryout-site IS the definition of served, so writing it before the work made a
  # half-built site look real to worktree list and delete --all.
  set -eu -o pipefail
  run grep -q "trap \"rm -f '\${dir}/.tryout-site'\" RETURN" "${DIR}/tryout/functions.sh"
  assert_success
  # cleared only once the site really is built
  run grep -q 'trap - RETURN' "${DIR}/tryout/functions.sh"
  assert_success
}

@test "exec preserves argument boundaries through the container" {
  # `exec v13 typo3 config:set X "My Site"` must arrive as two arguments: the host
  # hands "$@" to the container dispatcher, which hands "$@" to site_exec, which
  # runs the binary directly — no sh -c re-parse anywhere.
  set -eu -o pipefail
  run grep -q 'bash /var/www/html/.ddev/tryout/tryout-container.sh "\$@"' "${DIR}/commands/host/tryout"
  assert_success
  run grep -q 'delegate exec "\${site}" "\$@"' "${DIR}/commands/host/tryout"
  assert_success
  run grep -q 'site_exec "\${site}" "\$@"' "${DIR}/tryout/commands.sh"
  assert_success
  run grep -q 'sh -c' "${DIR}/tryout/functions.sh"
  assert_failure

  # And site_exec itself: a fake php records what it received.
  mkdir -p "${FAKEROOT}/bin" "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"
  cat > "${FAKEROOT}/bin/php8.2" <<'FAKE'
#!/bin/sh
printf 'cwd=%s\n' "$(pwd)"
printf 'db=%s site=%s\n' "${TYPO3_DB_DBNAME}" "${TRYOUT_SITE}"
for a in "$@"; do printf 'arg=[%s]\n' "$a"; done
FAKE
  chmod +x "${FAKEROOT}/bin/php8.2"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' site_exec v13 vendor/bin/typo3 config:set X 'My Site'"
  assert_success
  assert_line "cwd=${FAKEROOT}/TYPO3-Instances/v13"
  assert_line "db=db_v13 site=v13"
  assert_line "arg=[vendor/bin/typo3]"
  assert_line "arg=[config:set]"
  assert_line "arg=[X]"
  assert_line "arg=[My Site]"
}

@test "a worktree can be renamed without touching its branch" {
  # The directory name and the branch are independent: renaming one must never
  # touch the other.
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'

  git -C "${FAKEROOT}" init -q .
  git -C "${FAKEROOT}" commit -q --allow-empty -m init
  git -C "${FAKEROOT}" worktree add -q "${FAKEROOT}/worktrees/wilie-wonka" -b wilie-wonka

  run helper_eval 'rename_core_worktree wilie-wonka experiment'
  assert_success

  assert_dir_exist "${FAKEROOT}/worktrees/experiment"
  assert_dir_not_exist "${FAKEROOT}/worktrees/wilie-wonka"

  # The branch is the point: it must survive the rename untouched.
  run bash -c "git -C '${FAKEROOT}/worktrees/experiment' branch --show-current"
  assert_output "wilie-wonka"
}

@test "rename refuses a name that is already taken" {
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'
  git -C "${FAKEROOT}" init -q .
  git -C "${FAKEROOT}" commit -q --allow-empty -m init
  git -C "${FAKEROOT}" worktree add -q "${FAKEROOT}/worktrees/a" -b a
  mkdir -p "${FAKEROOT}/worktrees/b"

  run helper_eval 'rename_core_worktree a b'
  assert_failure
  assert_output --partial "already exists"
  assert_dir_exist "${FAKEROOT}/worktrees/a"
}

@test "commands that take no arguments say so instead of ignoring them" {
  # `ddev tryout composer install` used to regenerate the overlay and say nothing
  # about `install` — a typo that looked like it worked. Worse, the name suggests
  # it runs Composer, which it does not.
  set -eu -o pipefail
  local c
  for c in status composer help; do
    run grep -qE "(reject_args ${c}|'${c}' takes no arguments)" "${DIR}/commands/host/tryout"
    assert_success
  done

  # composer points at the command the user probably wanted.
  run grep -q 'ddev composer' "${DIR}/commands/host/tryout"
  assert_success

  # help must receive its arguments to be able to reject them.
  run grep -q 'help)     cmd_help "$@"' "${DIR}/commands/host/tryout"
  assert_success
}

@test "completion works while a word is being typed, not just on an empty one" {
  # Every test here used to pass '' as the word being completed, so the partial
  # case went unexercised — and it was broken: the script read $2 as the verb, but
  # with `ddev tryout wor<TAB>` argv is `tryout wor`, so $2 IS the partial word.
  # It matched no case, printed nothing, and zsh fell back to file completion.
  # cobra filters candidates against the partial word itself, so returning the full
  # list is correct.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/worktrees/v13"

  run names wor
  assert_success
  assert_line "worktree"

  run names cs doc
  assert_success
  assert_line "doctor"

  run names worktree us
  assert_success
  assert_line "use"

  # `use` never offers the primary, so filter on a worktree that is not it.
  run names worktree use v1
  assert_success
  assert_line "v13"

  # An empty word must keep working too.
  run names "''"
  assert_success
  assert_line "worktree"
}

@test "completion describes every candidate or explains the free-text word" {
  # DDEV passes each line to cobra verbatim, so `value<TAB>description` renders
  # as two columns and `_activeHelp_ text` as a hint. A bare word would look like
  # a regression in zsh: no description beside it.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"
  local args line
  for args in "''" "worktree ''" "worktree add ''" "worktree add x ''" "worktree use ''" \
              "worktree serve ''" "worktree list ''" "cs ''" "cs setup ''" \
              "checkout ''" "patch ''" "exec ''" "exec v13 ''" \
              "delete ''" "download ''" "status ''"; do
    # shellcheck disable=SC2086
    while IFS= read -r line; do
      [ -n "${line}" ] || continue
      case "${line}" in
        "_activeHelp_ "?*) ;;
        *"	"?*) ;;
        *) echo "bare candidate for '${args}': ${line}"; false ;;
      esac
    done < <(complete ${args})
  done
}

@test "completion hints at free-text words instead of staying silent" {
  # DDEV always returns cobra's Default directive, so silence means the shell
  # lists files. A hint above them is the best we can do — so there must be one.
  set -eu -o pipefail
  run complete worktree add "''"
  assert_success
  assert_line --regexp '^_activeHelp_ name for the new worktree'

  run complete worktree rename main "''"
  assert_line --regexp '^_activeHelp_ new name for main'

  run complete cs setup "''"
  assert_line --regexp '^_activeHelp_ .*Gerrit username'

  run complete exec v13 "''"
  assert_line --regexp '^_activeHelp_ command to run in v13'
}

@test "completion offers --plain for worktree list" {
  set -eu -o pipefail
  run names worktree list "''"
  assert_success
  assert_line "--plain"
}

@test "completion offers serve only unserved worktrees and unserve only served ones" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/worktrees/v13" "${FAKEROOT}/worktrees/v12"
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"

  run names worktree serve "''"
  assert_success
  assert_line "main"
  assert_line "v12"
  refute_line "v13"

  run names worktree unserve "''"
  assert_success
  assert_line "v13"
  refute_line "main"
  refute_line "v12"

  # The served one says so, with its PHP version.
  run complete worktree unserve "''"
  assert_line --regexp $'^v13\t.*PHP 8\\.2'
}

@test "completion omits the primary from use and remove" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/worktrees/v13"

  run names worktree use "''"
  assert_success
  assert_line "v13"
  refute_line "main"

  run names worktree remove "''"
  refute_output --partial "main"

  # Elsewhere the primary is offered, and labelled.
  run complete worktree rename "''"
  assert_line --regexp $'^main\tprimary'
}

@test "completion does not offer a flag already on the line" {
  set -eu -o pipefail
  run names worktree add x --serve --
  assert_success
  refute_line "--serve"
  assert_line "--php"

  # refute_line cannot cope with empty output, and nothing is a valid answer here.
  run names download --reset "''"
  refute_output --partial "--reset"

  run names delete --all "''"
  refute_output --partial "--all"
  refute_output --partial "@primary"
  assert_line "--yes"
}

@test "completion shows only flags once a dash is typed" {
  set -eu -o pipefail
  # `remove`, not `use`: switching repoints the primary instance's overlay and
  # touches no checkout, so `use` has no flags left to offer.
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/worktrees/v13"
  run names worktree remove --
  assert_success
  assert_line "--force"
  refute_line "v13"
}

@test "completion offers PHP versions after --php" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/v13"
  run names worktree serve v13 --php "''"
  assert_success
  assert_line "8.2"
  assert_line "8.5"

  run names worktree add x --php "''"
  assert_line "8.4"
}

@test "completion offers the configured patch numbers" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/.ddev"
  printf 'web_environment:\n  - TRYOUT_PATCHES=56947,12345\n' > "${FAKEROOT}/.ddev/config.tryout-patches.yaml"
  run names patch "''"
  assert_success
  assert_line "56947"
  assert_line "12345"
}

@test "completion covers every worktree and cs subcommand the command dispatches" {
  set -eu -o pipefail
  local subs sub
  for verb in worktree cs; do
    # The first name of each case label; aliases (remove|rm) are deliberately
    # not offered — two spellings of one thing would only lengthen the list.
    subs=$(sed -n "/^cmd_${verb}()/,/^}/p" "${DIR}/commands/host/tryout" \
      | sed -n 's/^        \([a-z][a-z|-]*\)).*/\1/p' | cut -d'|' -f1)
    [ -n "${subs}" ]
    run names "${verb}" "''"
    assert_success
    for sub in ${subs}; do
      assert_line "${sub}"
    done
  done
}

@test "completion answers instantly" {
  # list_core_worktrees runs a `git status` per worktree; using it for names
  # once made a TAB cost a third of a second. Generous bound, coarse clock.
  set -eu -o pipefail
  local n start end
  for n in 1 2 3 4 5 6; do mkdir -p "${FAKEROOT}/worktrees/wt${n}"; done
  start=$(date +%s)
  complete worktree use "''" >/dev/null
  complete worktree rename "''" >/dev/null
  complete exec "''" >/dev/null
  end=$(date +%s)
  [ $(( end - start )) -le 1 ]
}

# --- gum presentation layer -------------------------------------------------
# These pin the two gum behaviours that would otherwise cause silent damage:
# spin must not swallow exit codes (the error handling depends on them), and
# choose reports success without a TTY while choosing nothing.

@test "ui_spin preserves the wrapped command's exit code" {
  set -eu -o pipefail

  # The whole error-handling layer branches on these codes; a spinner that
  # swallows them would report every failed serve/checkout as a success.
  run helper_eval 'ui_spin "working" true'
  assert_success

  run helper_eval 'ui_spin "working" false'
  assert_failure

  run helper_eval 'ui_spin "working" sh -c "exit 3"'
  [ "$status" -eq 3 ]
}

@test "ui_spin does not emit escape sequences when its output is captured" {
  set -eu -o pipefail

  # gum spin writes control characters to a non-TTY; the helper must run the
  # command plainly there, or captured output is corrupted.
  run helper_eval 'ui_spin "working" printf hello'
  assert_success
  assert_output --partial "hello"
  refute_output --partial $'\e['
}

@test "ui_choose fails without a TTY instead of reporting a bogus choice" {
  set -eu -o pipefail

  # gum choose exits 0 with no output when it cannot open a terminal. Trusting
  # that exit code would make a cancelled prompt look like a real answer.
  run helper_eval 'ui_choose "pick one" alpha beta </dev/null'
  assert_failure
  refute_output --partial "alpha"
}

@test "ui_choose falls back to a plain read when a name is piped in" {
  set -eu -o pipefail

  run helper_eval 'printf "beta\n" | ui_choose "pick one" alpha beta'
  assert_success
  assert_output --partial "beta"
}

@test "ui_table renders rows and degrades without gum" {
  set -eu -o pipefail

  run helper_eval 'printf "NAME,STATE\nmain,clean\n" | ui_table'
  assert_success
  assert_output --partial "NAME"
  assert_output --partial "main"
  assert_output --partial "clean"

  # With gum off the PATH the data must still come through, unbordered.
  run helper_eval 'printf "NAME,STATE\nmain,clean\n" | PATH=/usr/bin:/bin ui_table'
  assert_success
  assert_output --partial "main"
}

@test "worktree list --plain is the parseable contract, the default is cards" {
  set -eu -o pipefail

  # tests/e2e/login.spec.ts discovers served sites by regex over --plain. The
  # default card layout does not match it, so the flag must keep working.
  run grep -n "worktree', 'list', '--plain'" "${DIR}/tests/e2e/login.spec.ts"
  assert_success

  # The flag has to reach the list branch, not be swallowed as a worktree name:
  # the host hands `list` its arguments verbatim, and the container branches on it.
  run grep -q 'delegate worktree list "\$@"' "${DIR}/commands/host/tryout"
  assert_success
  run grep -c -- '--plain' "${DIR}/tryout/commands.sh"
  assert_success
}

@test "gum is optional: install notes its absence but does not refuse" {
  set -eu -o pipefail

  # gum only changes how output looks — every command works without it — so a
  # missing binary must never block installing the add-on.
  run grep -A2 'command -v gum' "${DIR}/install.yaml"
  assert_success
  assert_output --partial "gum"

  # The note has to say where to get it...
  run grep -q 'github.com/charmbracelet/gum' "${DIR}/install.yaml"
  assert_success

  # ...and must not exit: the git check above it does, this one may not.
  run helper_eval '
    block=$(awk "/command -v gum/,/^    fi$/" "'"${DIR}"'/install.yaml")
    printf "%s" "${block}" | grep -q "exit 1" && exit 1
    exit 0'
  assert_success
}

@test "every ui_ helper degrades to plain output when gum is absent" {
  set -eu -o pipefail

  # This is the path a user without gum actually gets, so it has to carry the
  # same information as the styled one — just unstyled.
  local nogum="PATH=/usr/bin:/bin:/usr/sbin:/sbin"

  run helper_eval "printf 'NAME,STATE\nmain,clean\n' | ${nogum} ui_table"
  assert_success
  assert_output --partial "NAME"
  assert_output --partial "main"
  assert_output --partial "clean"

  run helper_eval "printf 'Core: main\n' | ${nogum} ui_box 'Status'"
  assert_success
  assert_output --partial "Status"
  assert_output --partial "Core: main"

  # Exit codes still have to survive the un-spun path.
  run helper_eval "${nogum} ui_spin 'work' true"
  assert_success
  run helper_eval "${nogum} ui_spin 'work' false"
  assert_failure

  # And a piped answer still selects, without gum's chooser.
  run helper_eval "printf 'main\n' | ${nogum} ui_choose 'pick' benni main"
  assert_success
  assert_output --partial "main"

  # A confirm with no terminal is "could not ask" (2), never a yes — with or
  # without gum, and whatever is piped at it.
  run helper_eval "${nogum} ui_confirm 'sure?' </dev/null"
  assert_equal "${status}" 2
  run helper_eval "printf 'y\n' | ${nogum} ui_confirm 'sure?'"
  assert_equal "${status}" 2
}

@test "ui_confirm separates a declined answer from an unanswerable one" {
  set -eu -o pipefail

  # The three-way return is the whole point of the helper. A caller says
  # "Aborted." for a no and "pass --yes" for no-terminal, and cannot tell them
  # apart from gum, which exits 1 for both. 2 means nobody was there to ask.
  run helper_eval "ui_confirm 'sure?' </dev/null"
  assert_equal "${status}" 2
  # Nothing was asked, so nothing may be printed into whatever captured us.
  refute_output --partial "sure?"

  # A piped "y" is not a person answering: no terminal is still 2, never 0.
  # Anything else here would let a script wipe a database by accident.
  run helper_eval "printf 'y\n' | ui_confirm 'sure?'"
  assert_equal "${status}" 2
}

@test "ui_confirm defaults to no, so Enter never confirms" {
  set -eu -o pipefail
  command -v expect >/dev/null 2>&1 || skip "expect not installed"

  # gum preselects Yes unless --default=false, so a bare Enter used to confirm.
  # Every prompt here is [y/N]; Enter has to decline in BOTH renderings.
  local script="source '${DIR}/tryout/functions.sh'; rc=0; ui_confirm 'Delete this?' || rc=\$?; echo \"RC=\${rc}\""

  # gum branch.
  run expect -c "log_user 0
    set timeout 10
    spawn bash -c {${script}}
    expect -re {Delete this}
    send -- \"\r\"
    expect -re {RC=([0-9]+)} { puts \$expect_out(1,string) }"
  assert_output --partial "1"

  # plain branch, gum off PATH.
  run expect -c "log_user 0
    set timeout 10
    spawn bash -c {export PATH=/usr/bin:/bin:/usr/sbin:/sbin; ${script}}
    expect -re {\\[y/N\\]}
    send -- \"\r\"
    expect -re {RC=([0-9]+)} { puts \$expect_out(1,string) }"
  assert_output --partial "1"
}

@test "ui_confirm reads y as yes and an arrow key as no" {
  set -eu -o pipefail
  command -v expect >/dev/null 2>&1 || skip "expect not installed"

  # An arrow key arrives as ESC [ A. Stripping only the ESC would leave a
  # printable "[A"; a stray "y" in such a tail must never read as a yes.
  local script="source '${DIR}/tryout/functions.sh'; export PATH=/usr/bin:/bin:/usr/sbin:/sbin; rc=0; ui_confirm 'Delete this?' || rc=\$?; echo \"RC=\${rc}\""
  local drive="log_user 0
    set timeout 10
    spawn bash -c {${script}}
    expect -re {\\[y/N\\]}"

  run expect -c "${drive}
    send -- \"y\r\"
    expect -re {RC=([0-9]+)} { puts \$expect_out(1,string) }"
  assert_output --partial "0"

  run expect -c "${drive}
    send -- \"\033\[A\r\"
    expect -re {RC=([0-9]+)} { puts \$expect_out(1,string) }"
  assert_output --partial "1"
}

@test "confirmations go through ui_confirm, not a hand-rolled read" {
  set -eu -o pipefail

  # A raw `read -r -p` cannot tell a no from an empty room, and under `set -u`
  # the unset variable it leaves behind aborts the command with a bash error
  # instead of a usable message. That is what cmd_delete used to do.
  run grep -nE 'read -r -p.*\[y/N\]' "${DIR}/commands/host/tryout"
  assert_failure

  # And gum is optional, so a raw `gum confirm` would hard-break the no-gum path.
  run bash -c "grep -n 'gum confirm' '${DIR}/commands/host/tryout' '${DIR}/tryout/commands.sh'"
  assert_failure
}

@test "worktree remove always asks, because it deletes the directory" {
  set -eu -o pipefail

  # It deletes the checkout's directory outright — ~20k files that took minutes to
  # check out. git's refusal to drop a dirty tree is no longer the backstop, since
  # the removal deliberately forces past it, so the question is the only guard
  # left and it must be asked every time.
  local branch
  branch="$(awk '/^        remove\|rm\)/{f=1} f{print} f&&/^            ;;/{exit}' \
    "${DIR}/commands/host/tryout")"

  printf '%s' "${branch}" | grep -q 'ui_confirm' \
    || fail "worktree remove must confirm before an irreversible removal"

  # Grepping for the words alone would pass with the gate rewritten to `if false`.
  # Pin the condition: the ONLY thing that may skip it is an explicit --yes.
  local gate
  gate="$(printf '%s' "${branch}" | grep -n 'ui_confirm' | head -1 | cut -d: -f1)"
  gate="$(printf '%s' "${branch}" | sed -n "1,${gate}p" | grep -E '^\s*if .*; then$' | tail -1)"
  printf '%s' "${gate}" | grep -q 'wt_yes' \
    || fail "only an explicit --yes may skip the question, got: ${gate}"
  printf '%s' "${gate}" | grep -qE 'wt_force|site_is_served' \
    && fail "the question is unconditional now, not gated on what is lost: ${gate}"

  # And --yes is the host's own word: delegating it would fail the container's
  # argument parser.
  printf '%s' "${branch}" | grep -q -- '--yes|-y) wt_yes="true"' \
    || fail "--yes must be consumed on the host"
  printf '%s' "${branch}" | grep -q 'delegate worktree remove "${name}" ${wt_args' \
    || fail "the delegated arguments must be the filtered ones, not \"\$@\""
}

@test "removing a worktree really takes its directory" {
  set -eu -o pipefail
  # A Core checkout always carries untracked and ignored files — vendor/, var/,
  # Build/ — and a plain `git worktree remove` refuses on any of them ("contains
  # modified or untracked files"), leaving the directory behind after reporting
  # success. The user was already asked, and the question named the directory, so
  # the answer has to actually take it.
  local fn
  fn=$(sed -n '/^remove_core_worktree()/,/^}/p' "${DIR}/tryout/functions.sh")

  printf '%s' "${fn}" | grep -q 'local args=("worktree" "remove" "--force")' \
    || fail "without --force git leaves the directory whenever anything is untracked"

  # And a sweep afterwards, for what git would not delete itself.
  printf '%s' "${fn}" | grep -q 'rm -rf "${dir}"' \
    || fail "a directory git declined to remove must still go"
  # Scoped to the worktree's own path, never a bare or derived one.
  local bad
  bad=$(printf '%s' "${fn}" | grep -E '^\s*rm -rf' | grep -v '"\${dir}"' || true)
  [ -z "${bad}" ] || fail "unscoped removal: ${bad}"

  # It must be deregistered before the sweep, or rm -rf races git's own bookkeeping.
  local git_line rm_line
  git_line=$(printf '%s' "${fn}" | grep -n 'worktree prune' | head -1 | cut -d: -f1)
  rm_line=$(printf '%s' "${fn}" | grep -n 'rm -rf' | head -1 | cut -d: -f1)
  [ "${git_line}" -lt "${rm_line}" ] \
    || fail "prune the worktree before deleting what is left of it"
}

@test "have_tty tests stderr, not stdout, so the chooser survives \$(...)" {
  set -eu -o pipefail

  # Every prompt is read as x="$(ui_choose ...)", which makes stdout a pipe. A
  # have_tty that required [ -t 1 ] would silently disable gum's chooser in the
  # one place it is meant to run — and the plain read it fell back to returned
  # the arrow-key escape sequence as the "choice".
  run grep -A1 '^have_tty()' "${DIR}/tryout/functions.sh"
  assert_success
  refute_output --partial '-t 1'
  assert_output --partial '-t 2'
}

@test "the plain prompt never returns a control sequence as an answer" {
  set -eu -o pipefail

  # An arrow key at a plain `read` arrives as an escape sequence; treating it as
  # a worktree name would send a command off at a nonexistent target.
  run helper_eval 'printf "\033[B\n" | ui_choose "pick" alpha beta'
  assert_failure
}

# --- guided arguments ---------------------------------------------------------
# A command missing its argument asks for it; without a terminal it prints the
# usage line as before. These pin the helpers and the one trap that made every
# prompt invisible: gum draws on stderr.

@test "gum prompts do not redirect stderr, because that is where gum draws" {
  set -eu -o pipefail
  # ui_choose and ui_input once carried 2>/dev/null to hush "could not open
  # TTY"; the effect was a chooser the user could not see. have_tty guards the
  # no-terminal case instead.
  run grep -nE 'gum (choose|filter|input|confirm) .*2>/dev/null' "${DIR}/tryout/functions.sh"
  assert_failure
  # The same trap one level up: hushing an ask_* helper hides the gum UI it draws,
  # and its chooser answers nothing.
  run bash -c "grep -nE 'ask_(branch|worktree|site|text|patches)[^|]*2>/dev/null' \
    '${DIR}'/tryout/*.sh '${DIR}/commands/host/tryout'"
  assert_failure
}

@test "ask_worktree takes a piped answer and fails cleanly with none" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/worktrees/main" "${FAKEROOT}/worktrees/v13"

  run helper_eval 'printf "v13\n" | ask_worktree "which?" all'
  assert_success
  assert_output "v13"

  run helper_eval 'ask_worktree "which?" all </dev/null'
  assert_failure
  refute_output --partial "v13"
}

@test "ask_worktree always has at least the root checkout to offer" {
  set -eu -o pipefail
  # There is no "no worktrees yet" state any more: the project root IS a checkout,
  # so the list is never empty. With no terminal the picker still fails — that is
  # ui_choose's contract — but it does so without claiming there is nothing there.
  run helper_eval 'ask_worktree "which?" all </dev/null'
  assert_failure
  refute_output --partial "No worktree to choose from"
}

@test "ask_branch puts main first and legacy refs last" {
  set -eu -o pipefail
  # A fake list_local_core_branches; ask_branch only orders what it gets.
  run helper_eval '
    list_local_core_branches() { printf "%s\n" 9.5 TYPO3_8-7 13.4 main 14.3 12.4; }
    ensure_core_branch_refs() { :; }
    ui_choose() { shift; printf "%s\n" "$@"; }
    ask_branch "which?"'
  assert_success
  assert_line --index 0 "main"
  assert_line --index 1 "14.3"
  assert_line --index 2 "13.4"
  assert_line --index 3 "12.4"
  assert_line --index 4 "9.5"
  assert_line --index 5 "TYPO3_8-7"
}

@test "explain_missing prints the usage line only where nobody could answer" {
  set -eu -o pipefail
  run helper_eval 'explain_missing "ddev tryout worktree use <name>" </dev/null'
  assert_success
  assert_output --partial "Usage: ddev tryout worktree use <name>"
}

@test "every command that used to fail on a missing argument now asks first" {
  set -eu -o pipefail
  # The old shape was a one-line usage error; each of those sites must reach an
  # ask_* helper before it gives up.
  run grep -cE 'ask_(worktree|site|branch|text) ' "${DIR}/commands/host/tryout"
  assert_success
  [ "${output}" -ge 10 ]
  run grep -E '\[ -z "\$\{name\}" \] && \{ error "Usage' "${DIR}/commands/host/tryout"
  assert_failure
}

@test "ui_spin decides on stderr, the stream gum draws on" {
  set -eu -o pipefail
  # DDEV pipes a host command's stdout, always; a spinner gated on stdout being
  # a terminal never showed under `ddev tryout`.
  run grep -A3 '^ui_spin()' "${DIR}/tryout/functions.sh"
  run grep -nE 'have_gum && \[ -t 2 \]' "${DIR}/tryout/functions.sh"
  assert_success
}

@test "the post-start hook runs inside the web container" {
  # The clone, the patches and composer all happen in there now, so there is
  # nothing to flush to the container and no host git involved.
  set -eu -o pipefail
  run grep -E '^    - exec: bash \.ddev/tryout/post-start\.sh$' "${DIR}/config.tryout.yaml"
  assert_success
  run grep -q 'exec-host' "${DIR}/config.tryout.yaml"
  assert_failure
  run grep -q 'sync_to_container' "${DIR}/tryout/post-start.sh"
  assert_failure
}

@test "a PHP that cannot run the Core on disk is rejected before composer runs" {
  # Composer reports the same mismatch, but as a resolver trace followed by our
  # hint to re-download — which is not the fix. The check names the constraint,
  # the version in use, and the command that changes it.
  set -eu -o pipefail
  command -v php >/dev/null 2>&1 || skip 'php not available'

  mkdir -p "${FAKEROOT}/worktrees/v13"
  printf '{"require":{"php":"^8.5"}}' > "${FAKEROOT}/composer.json"
  printf '{"require":{"php":"^8.2"}}' > "${FAKEROOT}/worktrees/v13/composer.json"

  # The project on 8.4 against a main Core: refused, with the concrete fix.
  run env DDEV_PHP_VERSION=8.4 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    available_php_versions() { printf '8.2\n8.3\n8.4\n8.5\n'; }
    check_php_for_core
  "
  assert_failure
  assert_output --partial 'requires PHP ^8.5'
  assert_output --partial 'the project runs PHP 8.4'
  assert_output --partial 'ddev config --php-version=8.5 && ddev restart'

  # A served site gets its own remedy, not the project-wide one.
  run env DDEV_PHP_VERSION=8.5 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    available_php_versions() { printf '8.2\n8.3\n8.4\n8.5\n'; }
    check_php_for_core '${FAKEROOT}/worktrees/v13' 8.1 v13
  "
  assert_failure
  assert_output --partial "site 'v13' runs PHP 8.1"
  assert_output --partial 'ddev tryout worktree serve v13 --php 8.5'
}

@test "a PHP the Core accepts passes the check, and so does an unreadable constraint" {
  # No composer.json yet (fresh project before the clone) or no constraint in
  # it: Composer is the authority then, so the check must not block.
  set -eu -o pipefail
  command -v php >/dev/null 2>&1 || skip 'php not available'

  mkdir -p "${FAKEROOT}/worktrees/bare"
  printf '{"require":{"php":"^8.5"}}' > "${FAKEROOT}/composer.json"
  printf '{}' > "${FAKEROOT}/worktrees/bare/composer.json"

  run env DDEV_PHP_VERSION=8.5 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    check_php_for_core && check_php_for_core '${FAKEROOT}/worktrees/bare' 8.1 \
      && check_php_for_core '${FAKEROOT}/nowhere' 8.1 && echo passed
  "
  assert_success
  assert_output "passed"
}

@test "every composer install is preceded by the PHP check" {
  # post-start.sh, rebuild_typo3 and serve_worktree are the places Composer
  # resolves Core; each must ask first, or the resolver trace is what the user
  # sees. The guard has to sit in the same function as the install (or anywhere
  # above it in the flat post-start script).
  set -eu -o pipefail
  local file install check
  for file in "${DIR}/tryout/post-start.sh" "${DIR}/tryout/functions.sh"; do
    grep -vE '^[[:space:]]*#' "${file}" > "${FAKEROOT}/code"
    while IFS= read -r install; do
      # The nearest check_php_for_core above the install that is not separated
      # from it by a function boundary.
      check=$(awk -v to="${install}" '
        /^[a-z_]+\(\) \{/ { fn = NR }
        /check_php_for_core/ && NR < to { n = NR; nfn = fn }
        NR == to { print (n && nfn == fn) ? n : ""; exit }' "${FAKEROOT}/code")
      [ -n "${check}" ] || fail "${file}: composer install at code line ${install} has no check_php_for_core in its function"
    done <<LINES
$(grep -nE 'composer install' "${FAKEROOT}/code" | cut -d: -f1)
LINES
  done
}

@test "the extra php-fpm socket lives under /run/php/, in daemon and vhost alike" {
  # The daemon runs as the web user and /run is root-owned on some providers
  # (Colima): a bind there fails with "Permission denied" and every request to the
  # served site is a 502. /run/php/ is where the stock php-fpm writes its pid, so
  # it is writable everywhere. Both sides must agree on the path, or nginx passes
  # requests to a socket nobody listens on.
  set -eu -o pipefail
  run grep -E '^SOCKET=' "${DIR}/tryout/tryout-php-fpm.sh"
  assert_output 'SOCKET="${RUN_DIR}/php-fpm-${VERSION}.sock"'
  run grep -E '^RUN_DIR=' "${DIR}/tryout/tryout-php-fpm.sh"
  assert_output 'RUN_DIR="/run/php"'
  run grep -E '^pid = ' "${DIR}/tryout/tryout-php-fpm.sh"
  assert_output 'pid = ${RUN_DIR}/php-fpm-${VERSION}.pid'

  run grep -E '^[[:space:]]*sock="/run/php/php-fpm-\$\{php\}\.sock"' "${DIR}/tryout/functions.sh"
  assert_success

  # Nothing may bind straight under /run/ any more.
  run grep -E '"/run/php-fpm-' "${DIR}/tryout/tryout-php-fpm.sh" "${DIR}/tryout/functions.sh"
  assert_failure
}

# A stand-in for tryout-php-fpm.sh: records the launch, writes its pid the way an
# FPM master does once its socket is bound, and stays alive like one.
fake_fpm_launcher() { # $1=root
  mkdir -p "$1/.ddev/tryout" "$1/run"
  cat > "$1/.ddev/tryout/tryout-php-fpm.sh" <<EOF
echo "launched \$1" >> "$1/launches"
echo \$\$ > "$1/run/php-fpm-\$1.pid"
exec sleep 30
EOF
}

@test "a --php switch starts its FPM master at once, not on the next restart" {
  set -eu -o pipefail
  # DDEV bakes web_extra_daemons into the web IMAGE, so a version that was not
  # declared when the container started has no master until a restart rebuilds
  # it — and the vhost, reloaded in place, passes requests to a dead socket.
  local root="${FAKEROOT}/fpm"
  fake_fpm_launcher "${root}"
  fpm_eval() {
    helper_eval "PROJECT_ROOT='${root}'; TRYOUT_FPM_RUN_DIR='${root}/run'
      DDEV_PHP_VERSION=8.5; $1" 2>&1
  }

  run fpm_eval "ensure_php_fpm_running 8.3"
  assert_success
  run cat "${root}/launches"
  assert_output "launched 8.3"

  # Already running: no second master, which would steal the first one's socket.
  run fpm_eval "ensure_php_fpm_running 8.3"
  assert_success
  run grep -c launched "${root}/launches"
  assert_output "1"

  # The project's own version is DDEV's php-fpm; never start one for it.
  run fpm_eval "ensure_php_fpm_running 8.5"
  assert_success
  run grep -c "launched 8.5" "${root}/launches"
  assert_output "0"

  # A pid file left by a dead master is not a running one.
  kill "$(cat "${root}/run/php-fpm-8.3.pid")"
  echo 999999 > "${root}/run/php-fpm-8.3.pid"
  run fpm_eval "ensure_php_fpm_running 8.3"
  assert_success
  run grep -c "launched 8.3" "${root}/launches"
  assert_output "2"
  kill "$(cat "${root}/run/php-fpm-8.3.pid")" 2>/dev/null || true
}

@test "serve starts the site's FPM master before it reloads the webserver" {
  set -eu -o pipefail
  local fn
  fn=$(sed -n '/^serve_worktree()/,/^}/p' "${DIR}/tryout/functions.sh" | grep -v '^[[:space:]]*#')
  printf '%s' "${fn}" | grep -q 'ensure_php_fpm_running "\${php}"' \
    || fail "serve_worktree never starts the FPM master for its PHP"
  # Before the reload: a reload that routes to a socket nobody listens on is a 502.
  local start apply
  start=$(printf '%s\n' "${fn}" | grep -n 'ensure_php_fpm_running' | head -1 | cut -d: -f1)
  apply=$(printf '%s\n' "${fn}" | grep -n 'apply_site_config' | head -1 | cut -d: -f1)
  [ "${start}" -lt "${apply}" ] || fail "the master must be up before the reload"
}

@test "switching Core drops vendor/ before the rebuild" {
  # Composer loads the plugins already in vendor/ before resolving, so a Core
  # switch across majors (class-alias-loader v2 -> v1) dies in the loaded
  # plugin's hook and leaves the site on 500. Both switch paths must wipe first,
  # and in the container — a host-side rm races Mutagen.
  set -eu -o pipefail
  local fn body
  for fn in use_core_worktree ctr_checkout; do
    body=$(sed -n "/^${fn}() {/,/^}/p" "${DIR}/tryout/functions.sh" "${DIR}/tryout/commands.sh")
    [ -n "${body}" ] || fail "no function ${fn}"
    printf '%s\n' "${body}" | grep -q 'wipe_site_vendor' \
      || fail "${fn} does not call wipe_site_vendor"
    # The wipe comes before the rebuild.
    [ "$(printf '%s\n' "${body}" | grep -n 'wipe_site_vendor' | head -1 | cut -d: -f1)" \
      -lt "$(printf '%s\n' "${body}" | grep -n 'rebuild_typo3' | tail -1 | cut -d: -f1)" ] \
      || fail "${fn} rebuilds before wiping"
  done
  run grep -E 'rm -rf "\$\(site_vendor "\$\{name\}"\)"' "${DIR}/tryout/functions.sh"
  assert_success
}

# --- git in the container, relative worktree paths --------------------------
# tryout's git work runs inside the web container while editors and the
# completion read the same checkouts on the host. Worktree metadata records paths,
# and an absolute path is right on one side only; relative paths (git >= 2.48)
# are what let both sides share one worktree.

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

@test "git_supports_relative_worktrees reads the version, not the platform" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/bin"
  printf '#!/bin/sh\necho "git version 2.47.3"\n' > "${FAKEROOT}/bin/git"
  chmod +x "${FAKEROOT}/bin/git"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' git_supports_relative_worktrees"
  assert_failure
  printf '#!/bin/sh\necho "git version 2.48.0"\n' > "${FAKEROOT}/bin/git"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' git_supports_relative_worktrees"
  assert_success
  printf '#!/bin/sh\necho "git version 2.50.1 (Apple Git-155)"\n' > "${FAKEROOT}/bin/git"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' git_supports_relative_worktrees"
  assert_success
}

@test "ensure_relative_worktree_paths converts a worktree recorded with absolute paths" {
  set -eu -o pipefail
  helper git_supports_relative_worktrees || skip "host git < 2.48"
  # The ROOT is the clone; worktrees are nested inside it under worktrees/<name>.
  local main="${FAKEROOT}" wt="${FAKEROOT}/worktrees/x"
  git init -q "${main}"
  git -C "${main}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  git -C "${main}" worktree add -q --detach "${wt}" HEAD
  # The shape a worktree has when the OTHER side created it: absolute container
  # paths, which the host cannot follow.
  printf 'gitdir: /var/www/html/.git/worktrees/x\n' > "${wt}/.git"
  printf '/var/www/html/worktrees/x/.git\n' > "${main}/.git/worktrees/x/gitdir"
  run git -C "${wt}" status --short
  assert_failure

  run helper ensure_relative_worktree_paths
  assert_success
  run git -C "${main}" config --get worktree.useRelativePaths
  assert_output "true"
  # Nested worktrees are two levels down, so the pointer back is ../../.git/...
  run cat "${wt}/.git"
  assert_output "gitdir: ../../.git/worktrees/x"
  run git -C "${wt}" status --short
  assert_success
  # A worktree added afterwards is relative from the start.
  git -C "${main}" worktree add -q --detach "${FAKEROOT}/worktrees/y" HEAD
  run cat "${FAKEROOT}/worktrees/y/.git"
  assert_output "gitdir: ../../.git/worktrees/y"
}

@test "every clone and worktree operation goes through ensure_relative_worktree_paths" {
  set -eu -o pipefail
  # A worktree made without it is absolute and breaks on the other side.
  local fn body
  # migrate_core_to_worktree_layout is a no-op now (the root clone IS the layout),
  # so only the function that actually creates a worktree has to configure paths.
  for fn in add_core_worktree; do
    body=$(sed -n "/^${fn}() {/,/^}/p" "${DIR}/tryout/functions.sh")
    printf '%s\n' "${body}" | grep -q 'ensure_relative_worktree_paths' \
      || fail "${fn} does not call ensure_relative_worktree_paths"
  done
  # And a fresh clone is configured before anything else happens to it. The clone
  # goes through clone_core_into_root now — `git clone` cannot be used at all,
  # because the project root always already holds .ddev/ and clone refuses a
  # non-empty target.
  local file clone ensure
  for file in "${DIR}/tryout/post-start.sh" "${DIR}/tryout/commands.sh"; do
    [ -f "${file}" ] || continue
    clone=$(grep -nE 'clone_core_into_root' "${file}" | head -1 | cut -d: -f1)
    [ -n "${clone}" ] || continue
    ensure=$(awk -v from="${clone}" 'NR > from && /ensure_relative_worktree_paths/ { print NR; exit }' "${file}")
    [ -n "${ensure}" ] || fail "${file}: the clone at line ${clone} is not followed by ensure_relative_worktree_paths"
  done
  # Nothing may use plain `git clone` on the project root.
  for file in "${DIR}/tryout/post-start.sh" "${DIR}/tryout/commands.sh"; do
    grep -qE '^[[:space:]]*(if ! )?git clone .*CORE_DIR|PROJECT_ROOT' "${file}" \
      && grep -qE '^[[:space:]]*(if ! )?git clone ' "${file}" \
      && fail "${file}: git clone refuses a non-empty root; use clone_core_into_root"
  done
  # The clone writes the excludes, or the checkout is dirty from the first start.
  body=$(sed -n "/^clone_core_into_root() {/,/^}/p" "${DIR}/tryout/functions.sh")
  printf '%s\n' "${body}" | grep -q 'ensure_core_excludes' \
    || fail "clone_core_into_root must write .git/info/exclude"
  # add_core_worktree refuses on a git that cannot write relative paths.
  body=$(sed -n "/^add_core_worktree() {/,/^}/p" "${DIR}/tryout/functions.sh")
  printf '%s\n' "${body}" | grep -q 'git_supports_relative_worktrees' \
    || fail "add_core_worktree does not check the git version"
}

# --- host / container split ---------------------------------------------------
# commands/host/tryout is the entry point: it owns the terminal (prompts, gum)
# and hands every container-safe verb to tryout-container.sh through ONE
# `ddev exec`. Inside, git, composer, php and the database clients are the
# container's own, so nothing in there may call `ddev` back.

@test "container-side code never shells out to ddev" {
  set -eu -o pipefail
  local f
  for f in tryout/functions.sh tryout/commands.sh tryout/tryout-container.sh tryout/post-start.sh; do
    run bash -c "
      grep -vE '^[[:space:]]*#' '${DIR}/${f}' \
        | grep -E '(^[[:space:]]*|\\\$\\(|&& |\\|\\| |; )(if ! |! )?ddev (exec|composer|typo3|php|mysql|mutagen)( |\$)'
    "
    [ -z "${output}" ] || fail "${f} calls ddev: ${output}"
  done
}

@test "the container dispatcher covers every verb the host delegates" {
  set -eu -o pipefail
  local host_verbs ctr_verbs verb
  host_verbs=$(sed -n '/^case "${ACTION}" in/,/^esac/p' "${DIR}/commands/host/tryout" \
    | sed -n 's/^    \([a-z|]*\)).*/\1/p' | tr '|' '\n' | grep -v '^\*$')
  ctr_verbs=$(sed -n '/^case "${ACTION}" in/,/^esac/p' "${DIR}/tryout/tryout-container.sh" \
    | sed -n 's/^    \([a-z|]*\)).*/\1/p' | tr '|' '\n' | grep -v '^\*$')
  [ -n "${ctr_verbs}" ]
  for verb in ${host_verbs}; do
    case "${verb}" in
      launch|help) continue ;;  # host by nature: a browser, static text
    esac
    printf '%s\n' "${ctr_verbs}" | grep -qx "${verb}" || fail "container dispatcher lacks '${verb}'"
  done
  run grep -q '^export TRYOUT_IN_CONTAINER=1' "${DIR}/tryout/tryout-container.sh"
  assert_success
  run grep -q 'tryout/commands.sh' "${DIR}/tryout/tryout-container.sh"
  assert_success
}

@test "verbs that need Core are refused on the host before the container is asked" {
  # `ddev tryout patch 1` before ddev start must still say "TYPO3 Core not found"
  # with a next step — not a ddev error about a stopped project.
  set -eu -o pipefail
  local fn body rc dl
  for fn in cmd_patch cmd_reset cmd_checkout cmd_composer cmd_worktree cmd_cs; do
    body=$(sed -n "/^${fn}() {/,/^}/p" "${DIR}/commands/host/tryout")
    [ -n "${body}" ] || fail "no function ${fn}"
    rc=$(printf '%s\n' "${body}" | grep -n 'require_core' | head -1 | cut -d: -f1)
    dl=$(printf '%s\n' "${body}" | grep -n 'delegate ' | head -1 | cut -d: -f1)
    [ -n "${rc}" ] || fail "${fn} never calls require_core"
    [ -n "${dl}" ] || fail "${fn} never delegates"
    [ "${rc}" -lt "${dl}" ] || fail "${fn} delegates before require_core"
  done
  # status stays useful with the containers down: the not-cloned answer is local.
  run helper_eval 'source "${DIR}/commands/host/tryout" 2>/dev/null; true'
}

@test "the host forwards the caller's environment to the container" {
  set -eu -o pipefail
  local body
  body=$(sed -n '/^delegate() {/,/^}/p' "${DIR}/commands/host/tryout")
  [ -n "${body}" ] || fail "no delegate()"
  printf '%s\n' "${body}" | grep -q 'ddev exec' || fail "delegate does not use ddev exec"
  printf '%s\n' "${body}" | grep -q 'TRYOUT_BRANCH' || fail "TRYOUT_BRANCH is not forwarded"
  printf '%s\n' "${body}" | grep -q 'TRYOUT_GERRIT_USER' || fail "TRYOUT_GERRIT_USER is not forwarded"
}

@test "database helpers reach the db service from inside the web container" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/bin"
  printf '#!/bin/sh\nprintf "mysql %%s\\n" "$*"\n' > "${FAKEROOT}/bin/mysql"
  printf '#!/bin/sh\nprintf "psql %%s\\n" "$*"\n' > "${FAKEROOT}/bin/psql"
  chmod +x "${FAKEROOT}/bin/mysql" "${FAKEROOT}/bin/psql"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' db_root_sql 'SELECT 1'"
  assert_success
  assert_output "mysql -h db -uroot -proot -e SELECT 1"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' DDEV_DATABASE=postgres:16 db_root_sql 'SELECT 1'"
  assert_success
  assert_output "psql -h db -U db -d postgres -tAc SELECT 1"
}

@test "the SSH hint names ddev auth ssh inside the container, ssh-add on the host" {
  # The container's probe talks to ddev-ssh-agent, the host's to the host agent;
  # the next step differs, and the wrong one sends people to the wrong keychain.
  set -eu -o pipefail
  run helper_eval 'CS_SSH_REASON=no-agent-key; TRYOUT_IN_CONTAINER=1 gerrit_ssh_hint'
  assert_output --partial "ddev auth ssh"
  run helper_eval 'CS_SSH_REASON=no-agent-key; TRYOUT_IN_CONTAINER= gerrit_ssh_hint'
  assert_output --partial "ssh-add"
  # And the host adds its own line after the container's doctor report.
  run grep -c 'host_gerrit_ssh_report' "${DIR}/commands/host/tryout"
  assert_success
  [ "${output}" -ge 2 ]
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

@test "shell functions are never run through env" {
  # `env VAR=x run_typo3 …` looks for a binary called run_typo3 and fails with
  # "No such file or directory" — which is how the first-run TYPO3 setup broke
  # once it moved into the container. A variable prefix is the right shape.
  set -eu -o pipefail
  run bash -c "grep -nE '\benv +([A-Z_]+=[^ ]* +)+(run_|site_exec|db_root_sql|ctr_|cmd_)' \
    '${DIR}'/tryout/*.sh '${DIR}/commands/host/tryout'"
  [ -z "${output}" ] || fail "shell function run through env: ${output}"
}

@test "the host is brought up to date after a verb that changes files" {
  # `worktree add x && cd worktrees/x` on the host must not race Mutagen.
  set -eu -o pipefail
  local body
  body=$(sed -n '/^delegate() {/,/^}/p' "${DIR}/commands/host/tryout")
  printf '%s\n' "${body}" | grep -q 'flush_mutagen' || fail "delegate never flushes"
  # …and the read-only verbs are exempt, so status stays instant.
  printf '%s\n' "${body}" | grep -q '"status "\*|"exec "\*' || fail "status/exec are not exempt"
  printf '%s\n' "${body}" | grep -q '"worktree list"\*' || fail "worktree list is not exempt"
}

# --- browsing Gerrit patches ------------------------------------------------
# `ddev tryout patch` with no argument lists the open changes for the branch in
# use and lets the user pick one or several. The list itself is fetched in the
# container (curl + jq); the picker runs on the host, where gum and the terminal
# are.

@test "list-patches parses a Gerrit change listing into pickable rows" {
  set -eu -o pipefail
  local script="${DIR}/tryout/list-patches.sh"
  assert_file_exist "${script}"
  # A fake curl serving the captured payload, XSSI prefix and all.
  mkdir -p "${FAKEROOT}/bin"
  cat > "${FAKEROOT}/bin/curl" <<FAKE
#!/bin/sh
cat "${DIR}/tests/fixtures-gerrit-changes.json"
FAKE
  chmod +x "${FAKEROOT}/bin/curl"

  run env PATH="${FAKEROOT}/bin:${PATH}" bash "${script}" https://review.typo3.org main
  assert_success
  # number<TAB>subject<TAB>owner<TAB>scores — one row per change.
  local first
  first="$(printf '%s\n' "${output}" | head -1)"
  printf '%s' "${first}" | grep -qE '^[0-9]+	' || fail "no change number in: ${first}"
  [ "$(printf '%s' "${first}" | awk -F'\t' '{print NF}')" -eq 4 ] \
    || fail "expected 4 tab-separated fields, got: ${first}"
  assert_output --partial "whitespace module"
  # The owner is a name, not a JSON blob.
  printf '%s' "${first}" | cut -f3 | grep -qE '^[A-Za-zÀ-ÿ. -]+$' \
    || fail "owner column is not a name: $(printf '%s' "${first}" | cut -f3)"
}

@test "list-patches signals fetch and parse failures the way the other resolvers do" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/bin"
  printf '#!/bin/sh\nexit 22\n' > "${FAKEROOT}/bin/curl"
  chmod +x "${FAKEROOT}/bin/curl"
  run env PATH="${FAKEROOT}/bin:${PATH}" bash "${DIR}/tryout/list-patches.sh" https://x main
  [ "${status}" -eq 2 ] || fail "expected exit 2 on a fetch failure, got ${status}"

  printf '#!/bin/sh\nprintf "not json\\n"\n' > "${FAKEROOT}/bin/curl"
  run env PATH="${FAKEROOT}/bin:${PATH}" bash "${DIR}/tryout/list-patches.sh" https://x main
  [ "${status}" -eq 3 ] || fail "expected exit 3 on a parse failure, got ${status}"
}

@test "ui_choose_multi returns every picked line and fails on a cancel" {
  set -eu -o pipefail
  # Without a TTY the plain path reads piped answers, one per line, and an empty
  # answer is a cancel — the same contract ui_choose follows.
  run bash -c "printf 'a\nb\n' | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; ui_choose_multi 'Pick' one two three; }"
  assert_success
  assert_output --partial "a"
  assert_output --partial "b"

  run bash -c "printf '\n' | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; ui_choose_multi 'Pick' one two; }"
  assert_failure
}

@test "the multi picker uses gum choose --no-limit and never redirects its screen" {
  set -eu -o pipefail
  local body
  body=$(sed -n '/^ui_choose_multi() {/,/^}/p' "${DIR}/tryout/functions.sh")
  [ -n "${body}" ] || fail "no ui_choose_multi()"
  printf '%s\n' "${body}" | grep -q -- '--no-limit' || fail "not a multi-select"
  # gum draws on stderr; a 2>/dev/null here gives an invisible prompt.
  if printf '%s\n' "${body}" | grep -q 'gum choose.*2>/dev/null'; then
    fail "gum's screen is redirected"
  fi
  # Same TTY guard as ui_choose: gum exits 0 with no output when it has none.
  printf '%s\n' "${body}" | grep -q 'have_gum && have_tty' || fail "no TTY guard"
}

@test "a bare patch command offers the list before giving up" {
  set -eu -o pipefail
  local body
  body=$(sed -n '/^cmd_patch() {/,/^}/p' "${DIR}/commands/host/tryout")
  [ -n "${body}" ] || fail "no cmd_patch()"
  printf '%s\n' "${body}" | grep -q 'pick_patches' \
    || fail "cmd_patch never offers the patch list"
  # The fetch is separate from the pick: a spinner must not run over a chooser.
  printf '%s\n' "${body}" | grep -q 'fetch_open_patches' || fail "no fetch step"
  # And it still delegates the actual work.
  printf '%s\n' "${body}" | grep -q 'delegate patch' || fail "cmd_patch does not delegate"
}

@test "picked patch numbers are appended to the patch list, once each" {
  set -eu -o pipefail
  local f="${FAKEROOT}/.ddev/config.tryout-patches.yaml"
  mkdir -p "${FAKEROOT}/.ddev"

  # An empty list, the shape a fresh install has.
  cat > "${f}" <<'YAML'
# Gerrit Patches
#
# Example: TRYOUT_PATCHES=56947,12345

web_environment:
  - TRYOUT_PATCHES=
YAML
  run helper_eval "persist_patches 95074 95671"
  assert_success
  run grep -c '^# Gerrit Patches' "${f}"
  assert_output "1"
  run grep '  - TRYOUT_PATCHES=' "${f}"
  assert_output "  - TRYOUT_PATCHES=95074,95671"

  # A populated list gains only what is new, in order, with no duplicate.
  run helper_eval "persist_patches 95671 12345"
  assert_success
  run grep '  - TRYOUT_PATCHES=' "${f}"
  assert_output "  - TRYOUT_PATCHES=95074,95671,12345"

  # The rest of the file is preserved verbatim.
  run grep -c 'Example: TRYOUT_PATCHES=56947,12345' "${f}"
  assert_output "1"
}

@test "persisting refuses politely when the patch list is not there" {
  set -eu -o pipefail
  run helper_eval "persist_patches 1"
  assert_failure
}

@test "several picked patches are applied in order and rebuilt once" {
  # A rebuild is a full composer install; doing it per patch would run it three
  # times for a three-patch pick.
  set -eu -o pipefail
  local body
  body=$(sed -n '/^ctr_patch() {/,/^}/p' "${DIR}/tryout/commands.sh")
  [ -n "${body}" ] || fail "no ctr_patch()"
  printf '%s\n' "${body}" | grep -q 'for id in "${ids\[@\]}"' || fail "patches are not applied in a loop"
  # Exactly one rebuild call in the multi-patch branch.
  [ "$(printf '%s\n' "${body}" | sed -n '/for id in/,/^    else/p' | grep -c 'rebuild_typo3')" -eq 1 ] \
    || fail "expected one rebuild for the whole batch"
}

@test "the site moved to a flag so change numbers can be positional" {
  set -eu -o pipefail
  local ctr host
  ctr=$(sed -n '/^ctr_patch() {/,/^}/p' "${DIR}/tryout/commands.sh")
  printf '%s\n' "${ctr}" | grep -q -- '--site)' || fail "container does not accept --site"
  # And the old two-argument form still reaches it.
  host=$(sed -n '/^cmd_patch() {/,/^}/p' "${DIR}/commands/host/tryout")
  # The id-carrying delegate specifically: a site given after the change number
  # must still reach the container as --site, or it lands in the id slot.
  printf '%s\n' "${host}" | grep -q -- 'delegate patch ${site:+--site "${site}"} "${id}"' \
    || fail "host does not translate patch <id> <site> into --site"
  # And --site is accepted from the user too: a caller may know the site but not
  # the change number, and positionally the id comes first.
  printf '%s\n' "${host}" | grep -q -- '--site)' \
    || fail "host does not accept --site"
}

@test "the picker is skipped when a patch list is configured or there is no terminal" {
  # `ddev start` and any scripted call must keep applying TRYOUT_PATCHES rather
  # than opening a chooser nobody can answer.
  set -eu -o pipefail
  local body
  body=$(sed -n '/^cmd_patch() {/,/^}/p' "${DIR}/commands/host/tryout")
  printf '%s\n' "${body}" | grep -q 'TRYOUT_PATCHES' || fail "a configured list does not short-circuit"
  printf '%s\n' "${body}" | grep -q '! have_tty' || fail "no TTY guard before the picker"
}

@test "the patch list is fetched under a spinner, and picked without one" {
  # gum spin and gum choose both own the screen; running the chooser inside the
  # spinner's pipeline makes the prompt unusable.
  set -eu -o pipefail
  local fetch pick
  fetch=$(sed -n '/^fetch_open_patches() {/,/^}/p' "${DIR}/tryout/functions.sh")
  pick=$(sed -n '/^pick_patches() {/,/^}/p' "${DIR}/tryout/functions.sh")
  [ -n "${fetch}" ] || fail "no fetch_open_patches()"
  [ -n "${pick}" ] || fail "no pick_patches()"
  printf '%s\n' "${fetch}" | grep -q 'ui_spin' || fail "the fetch does not spin"
  if printf '%s\n' "${pick}" | grep -q 'ui_spin'; then fail "the picker spins"; fi
  printf '%s\n' "${pick}" | grep -q 'ui_choose_multi' || fail "the picker does not choose"
  # An unreachable Gerrit is a failure, not an empty list.
  printf '%s\n' "${fetch}" | grep -q 'return 1' || fail "a failed fetch is not signalled"
}

@test "pick_patches turns TSV rows into labels and returns the numbers" {
  set -eu -o pipefail
  # Rows arrive on stdin; without gum the chooser reads the answer from the same
  # stream, so a picked label can follow the rows.
  # Rows are arguments, so stdin stays free for the no-gum chooser's answer.
  run bash -c "printf '95074   [BUGFIX] Something\n' \
    | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; \
        pick_patches 'Pick' \
          \"\$(printf '95074\\t[BUGFIX] Something\\tBenni Mack\\tCR+2 V+2')\" \
          \"\$(printf '95671\\t[BUGFIX] Other\\tOli Bartsch\\tV+1')\"; }"
  assert_success
  assert_output "95074"
}

@test "the picker takes its rows as arguments, leaving stdin for the answer" {
  # Reading rows from stdin would swallow the very input the no-gum chooser needs.
  set -eu -o pipefail
  local body
  body=$(sed -n '/^pick_patches() {/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s\n' "${body}" | grep -qE 'for row in "\$@"' || fail "rows are not arguments"
  if printf '%s\n' "${body}" | grep -qE 'while IFS= read -r row'; then
    fail "pick_patches consumes stdin"
  fi
}

@test "the picker leaves the key hints to gum" {
  # gum 2.0 toggles with x, not space, and draws its own footer saying so; a
  # hint of ours in the header would just be a second place to get it wrong.
  set -eu -o pipefail
  if grep -rn 'space to select' "${DIR}/commands/host/tryout" "${DIR}/tryout/functions.sh"; then
    fail "a hard-coded toggle key is spelled out in a prompt"
  fi
}

@test "patch labels line up even with non-ASCII subjects and owners" {
  # printf's %-Ns pads by BYTES: a "…" or an accented name silently shortens its
  # column and pushes everything after it out of line.
  set -eu -o pipefail
  run helper_eval 'printf "[%s]" "$(pad_display "abc" 6)"'
  assert_output "[abc   ]"
  # Three bytes, one column: the padding must still reach six.
  run helper_eval 'printf "[%s]" "$(pad_display "a…c" 6)"'
  assert_output "[a…c   ]"
  run helper_eval 'printf "[%s]" "$(pad_display "Frédéric" 10)"'
  assert_output "[Frédéric  ]"
  # Longer than the field: left alone, never truncated mid-character.
  run helper_eval 'printf "[%s]" "$(pad_display "abcdefgh" 4)"'
  assert_output "[abcdefgh]"

  # And the renderer uses it rather than printf padding.
  local body
  body=$(sed -n '/^pick_patches() {/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s\n' "${body}" | grep -q 'pad_display' || fail "pick_patches pads by bytes"
}

@test "list-patches truncates with ASCII so the columns stay byte-aligned" {
  set -eu -o pipefail
  # The code, not the comment that explains why.
  if grep -vE '^[[:space:]]*(#|//)' "${DIR}/tryout/list-patches.sh" | grep -q '…'; then
    fail "a multi-byte ellipsis in the truncation shortens the padded column"
  fi
}

@test "the persist prompt names each change, not just its number" {
  # "Add 93838 to your patch list?" tells the user nothing about what 93838 is,
  # and it is about to be written into a config file they keep.
  set -eu -o pipefail
  run helper_eval "describe_patches '95347 93838' \
    \"\$(printf '95347\\t[TASK] Skip database setup\\tWouter Wolters\\tV+1')\" \
    \"\$(printf '93838\\t[FEATURE] Translate forms\\tJosua Vogel\\tV+1')\""
  assert_success
  assert_line "95347 - [TASK] Skip database setup"
  assert_line "93838 - [FEATURE] Translate forms"

  # A number nobody listed still gets a line rather than vanishing.
  run helper_eval "describe_patches '11111' \
    \"\$(printf '95347\\t[TASK] Something\\tX\\tV+1')\""
  assert_success
  assert_output "11111"

  # And the prompt uses it.
  local body
  body=$(sed -n '/^cmd_patch() {/,/^}/p' "${DIR}/commands/host/tryout")
  printf '%s\n' "${body}" | grep -q 'describe_patches' \
    || fail "the confirmation still lists bare numbers"
}

# --- picking a site ---------------------------------------------------------

@test "ask_site shows what each site is, and returns the name behind it" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13" "${FAKEROOT}/TYPO3-Instances/v12"
  printf 'php=8.4\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"
  printf 'php=8.2\n' > "${FAKEROOT}/TYPO3-Instances/v12/.tryout-site"

  # The label carries the URL and PHP version; the answer is the bare name.
  run bash -c "printf 'v13          https://v13.unitproj.ddev.site  PHP 8.4\n' \
    | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; ask_site 'Which?'; }"
  assert_success
  assert_output "v13"

  # The primary is offered as "primary" but answers with the sentinel, which is
  # what site_is_primary understands.
  run bash -c "printf 'primary       https://unitproj.ddev.site\n' \
    | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; \
        DDEV_PRIMARY_URL=https://unitproj.ddev.site ask_site 'Which?'; }"
  assert_success
  assert_output "@primary"
}

@test "ask_site offers the extra entries it is given, unchanged" {
  # delete uses this for --all: wiping everything is a pick, not a flag to recall.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.4\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"
  run bash -c "printf -- '--all         every site\n' \
    | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; ask_site 'Which?' '--all         every site'; }"
  assert_success
  assert_output -- "--all         every site"

  # Typing just the name works too: the caller matches on the first word.
  run bash -c "printf -- '--all\n' \
    | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; ask_site 'Which?' '--all         every site'; }"
  assert_success
  assert_output -- "--all"
}

@test "exec, reset and delete all ask which site when there are several" {
  set -eu -o pipefail
  local fn body
  for fn in cmd_exec cmd_reset cmd_delete; do
    body=$(sed -n "/^${fn}() {/,/^}/p" "${DIR}/commands/host/tryout")
    [ -n "${body}" ] || fail "no ${fn}"
    printf '%s\n' "${body}" | grep -q 'ask_site' || fail "${fn} never offers a site list"
    printf '%s\n' "${body}" | grep -q 'explain_missing' \
      || fail "${fn} does not fall through to the usage line"
  done

  # reset and delete only ask once something else is served — a single-site
  # project keeps working without a prompt.
  for fn in cmd_reset cmd_delete; do
    body=$(sed -n "/^${fn}() {/,/^}/p" "${DIR}/commands/host/tryout")
    printf '%s\n' "${body}" | grep -q 'served_site_names' \
      || fail "${fn} asks even with no served site"
  done
}

@test "a picked primary does not reach the container as a sentinel" {
  # "@primary" is the host's word for the default; ctr_delete parses positional
  # arguments and would take it for a site name.
  set -eu -o pipefail
  local body
  body=$(sed -n '/^cmd_delete() {/,/^}/p' "${DIR}/commands/host/tryout")
  printf '%s\n' "${body}" | grep -q 'site_is_primary "${target}" && target=""' \
    || fail "the primary sentinel is forwarded verbatim"
}

# --- picking a worktree -----------------------------------------------------

@test "ask_worktree labels each worktree with its branch, HEAD and state" {
  set -eu -o pipefail
  local main="${FAKEROOT}/worktrees/main" wt="${FAKEROOT}/worktrees/v13"
  git init -q "${main}"
  git -C "${main}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  git -C "${main}" worktree add -q --detach "${wt}" HEAD
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.4\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"

  # The rendered rows carry branch, HEAD, state and what the site is.
  run helper worktree_labels all
  assert_success
  assert_output --partial "main"
  assert_output --partial "primary"
  assert_output --partial "v13"
  assert_output --partial "(detached)"
  assert_output --partial "PHP 8.4"

  # …but the answer is still the bare name the callers pass on.
  run bash -c "printf 'v13\n' | { source '${DIR}/tryout/functions.sh' >/dev/null 2>&1; ask_worktree 'Which?'; }"
  assert_success
  assert_output "v13"
}

@test "ask_worktree keeps filtering by primary and served state" {
  set -eu -o pipefail
  local main="${FAKEROOT}/worktrees/main" wt="${FAKEROOT}/worktrees/v13"
  git init -q "${main}"
  git -C "${main}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  git -C "${main}" worktree add -q --detach "${wt}" HEAD
  mkdir -p "${FAKEROOT}/TYPO3-Instances/v13"
  printf 'php=8.4\n' > "${FAKEROOT}/TYPO3-Instances/v13/.tryout-site"

  # served: only v13. unserved and nonprimary: only main / only what is left.
  run helper worktree_labels served
  assert_output --partial "v13"
  refute_output --partial "primary"

  run helper worktree_labels nonprimary
  assert_output --partial "v13"

  run helper worktree_labels unserved
  refute_output --partial "v13"
}

@test "every worktree subcommand that names a worktree can pick one" {
  set -eu -o pipefail
  local sub body
  for sub in "use" "remove|rm" "serve" "unserve" "rename"; do
    body=$(awk -v pat="        ${sub})" 'index($0, pat) == 1, /^            ;;/' \
      "${DIR}/commands/host/tryout")
    [ -n "${body}" ] || fail "no '${sub}' branch"
    printf '%s\n' "${body}" | grep -q 'ask_worktree' \
      || fail "'${sub}' never offers a worktree list"
  done
}

# --- the installed copy going stale -----------------------------------------
# `ddev add-on get` copies the payload once and never refreshes it, so a project
# installed before a change keeps the old command AND the old completion script.
# Completion then still works but offers the older feature set, which is
# indistinguishable from "autocomplete is broken".

@test "status output states its own colour instead of inheriting one" {
  set -eu -o pipefail
  # A popup or embedded terminal need not share the pane's colours, so text with
  # none of its own — the "Core:" labels and their values — can come out
  # unreadable. Stating the colour fixes it for every theme and background.
  local body
  body=$(sed -n '/^ctr_status_body()/,/^}/p' "${DIR}/tryout/commands.sh")

  # Every line the status prints says what colour it is.
  local bare
  bare=$(printf '%s' "${body}" | grep -c 'echo -e "' || true)
  [ "${bare}" -gt 0 ] || fail "no status output found"
  printf '%s' "${body}" | grep 'echo -e "' | grep -v '${TEXT}' \
    && fail "a status line inherits the terminal's colour instead of stating one"

  # The icons return TO that colour rather than resetting to nothing, or the text
  # after them on the same line goes bare again.
  printf '%s' "${body}" | grep -q 'local OK="${GREEN}✓${TEXT}"' \
    || fail "the OK icon must return to the text colour, not reset"
}

@test "a new worktree is always created detached, with no local branch" {
  set -eu -o pipefail
  # Detached at origin/<base>, always. A local branch bought nothing and cost a
  # lot: git allows one worktree per branch, so two off the same base collided; a
  # removed worktree left its branch behind, blocking re-creation; and composer's
  # branch-alias forced the branch to be named after the base, never the worktree.
  # A detached HEAD has none of those failure modes, and composer still resolves
  # the version from the checked-out commit.
  local fn
  fn=$(sed -n '/^add_core_worktree()/,/^}/p' "${DIR}/tryout/functions.sh")

  # Exactly one worktree-add, and it detaches.
  printf '%s' "${fn}" | grep -q 'worktree add --detach "${dir}" "origin/${branch}"' \
    || fail "creation must be a detached checkout at origin/<base>"
  # No branch is ever created or reset.
  ! printf '%s' "${fn}" | grep -q -- '-B ' \
    || fail "a detached worktree must never create a branch"
  ! printf '%s' "${fn}" | grep -q 'attach=' \
    || fail "there is no attach mode any more"
  ! printf '%s' "${fn}" | grep -q 'refs/heads/' \
    || fail "creation must not touch any local branch ref"

  # --detach on the command line is accepted but does nothing (it is the default).
  run grep -q -- '--detach)  ;;' "${DIR}/tryout/commands.sh"
  assert_success
  # The call site no longer passes an attach argument.
  run grep -q 'add_core_worktree "${name}" "${branch:-${BRANCH}}" ||' "${DIR}/tryout/commands.sh"
  assert_success
}

@test "download updates the worktree it is given, not always the primary" {
  set -eu -o pipefail
  # It took no site at all, so it only ever updated the primary — the one thing
  # you cannot use it for when the work is in a worktree.
  local fn
  fn=$(sed -n '/^ctr_download()/,/^}/p' "${DIR}/tryout/commands.sh")

  printf '%s' "${fn}" | grep -q 'CORE_DIR="$(site_core_dir "${site}")"' \
    || fail "download must resolve the named site's checkout"
  # The base comes from the recorded upstream; the branch name is the worktree's.
  printf '%s' "${fn}" | grep -q 'rev-parse --abbrev-ref' \
    || fail "the base must come from the tracked upstream"
  # Rebase, never merge: Gerrit takes one commit with one Change-Id.
  printf '%s' "${fn}" | grep -q 'pull --rebase' \
    || fail "the update must rebase"
  # A detached checkout has nothing to rebase, and must say so rather than fail
  # obscurely on a branch that is not there.
  printf '%s' "${fn}" | grep -q 'Detached checkout' \
    || fail "a detached worktree must be told why it cannot update"
}

@test "download rebuilds the site it updated, not the primary" {
  set -eu -o pipefail
  # It resolved CORE_DIR for the named worktree but then called the two helpers
  # bare, so both defaulted to the primary: `download jiiha` pulled jiiha's Core
  # and ran composer install against the site whose vendor tree had not moved.
  local fn
  fn=$(sed -n '/^ctr_download()/,/^}/p' "${DIR}/tryout/commands.sh")

  printf '%s' "${fn}" | grep -q 'reset_core_to_main "${site}"' \
    || fail "the reset must drop the cache of the site it reset"
  printf '%s' "${fn}" | grep -q 'rebuild_typo3 "${site}"' \
    || fail "the rebuild must target the site that was updated"
  printf '%s' "${fn}" | grep -qE '^\s*(reset_core_to_main|rebuild_typo3)\s*$' \
    && fail "a bare call here silently rebuilds the primary"

  # Every hint must name the same site, or following it hits the primary's Core.
  printf '%s' "${fn}" | grep -q 'ddev tryout download --reset' \
    && fail "a --reset hint without the site points at the wrong checkout"
  printf '%s' "${fn}" | grep -q 'site_is_primary "${site}" || site_arg=' \
    || fail "the hints need a site suffix built once"
}

@test "reset moves the branch it is on, never checks out the base" {
  set -eu -o pipefail
  # BRANCH is the BASE a worktree tracks, while the worktree carries a branch of
  # its own name. Checking out the base here fails twice over: another worktree
  # already holds it ("'main' is already used by worktree at …"), and creating it
  # fails because the branch exists. `reset --hard` moves whatever is checked
  # out — a branch or a detached HEAD — which is what was wanted all along.
  local fn
  fn=$(sed -n '/^reset_core_to_main()/,/^}/p' "${DIR}/tryout/functions.sh")

  printf '%s' "${fn}" | grep -q 'reset --hard "origin/${BRANCH}"' \
    || fail "reset must move the current branch to the base tip"
  printf '%s' "${fn}" | grep -qE 'checkout (-b )?"\$\{BRANCH\}"' \
    && fail "reset must not check out the base branch"

  # Behaviour: a branch that is not the base survives the reset and lands on it.
  run bash -c "
    set -euo pipefail
    d=\$(mktemp -d); u=\$(mktemp -d)
    git init -q -b main \"\${u}\"; git -C \"\${u}\" commit -q --allow-empty -m base
    git clone -q \"\${u}\" \"\${d}\" 2>/dev/null
    git -C \"\${d}\" checkout -q -b feature
    git -C \"\${d}\" commit -q --allow-empty -m mine
    git -C \"\${d}\" reset --hard origin/main >/dev/null 2>&1
    echo \"on=\$(git -C \"\${d}\" branch --show-current) at=\$(git -C \"\${d}\" log --oneline -1 --format=%s)\"
    rm -rf \"\${d}\" \"\${u}\"
  "
  assert_success
  assert_output "on=feature at=base"
}

@test "a worktree with no recorded upstream still updates" {
  set -eu -o pipefail
  # `rev-parse --abbrev-ref @{upstream}` EXITS 128 when a branch has no upstream,
  # and the container runs under `set -e` — so reading it without guarding the
  # failure killed the command outright. Every worktree created before tracking
  # was recorded hits this, which is most of them on an existing project.
  local fn
  fn=$(sed -n '/^ctr_download()/,/^}/p' "${DIR}/tryout/commands.sh")
  printf '%s' "${fn}" | grep -q "abbrev-ref '@{upstream}' 2>/dev/null || true" \
    || fail "reading a missing upstream must not abort under set -e"
  # And the prefix strip must not be a pipe: pipefail would resurrect the failure.
  printf '%s' "${fn}" | grep -q 'abbrev-ref.*| *sed' \
    && fail "piping rev-parse reintroduces its exit code under pipefail"

  # Behaviour: a branch with no upstream leaves BRANCH empty and carries on.
  run bash -c "
    set -euo pipefail
    d=\$(mktemp -d)
    git init -q -b feature \"\${d}\" 2>/dev/null
    git -C \"\${d}\" commit -q --allow-empty -m x
    B=\"\$(git -C \"\${d}\" rev-parse --abbrev-ref '@{upstream}' 2>/dev/null || true)\"
    B=\"\${B#origin/}\"
    echo \"reached with BRANCH=[\${B}]\"
    rm -rf \"\${d}\"
  "
  assert_success
  assert_output --partial "reached with BRANCH=[]"
}

@test "worktree_name_for_path names the worktree a path sits in" {
  set -eu -o pipefail
  # `launch` reads the cwd with this. The subtleties are easy to get wrong:
  # /private/var, and that worktrees/ lives inside the root checkout.
  local root
  root="$(mktemp -d)"
  mkdir -p "${root}/worktrees/jiiha/Build" "${root}/Build" "${root}/packages"

  run bash -c "
    set -euo pipefail
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    PROJECT_ROOT='${root}'
    plain_core_name() { echo plainclone; }
    p() { printf '%s ' \"\$(worktree_name_for_path \"\$1\" || echo none)\"; }
    p '${root}/worktrees/jiiha'
    p '${root}/worktrees/jiiha/Build'
    p '${root}'
    p '${root}/Build'
    p '${root}/worktrees'
    echo
  "
  assert_success
  # The root IS the primary checkout, so a path anywhere under it that is not in
  # worktrees/ belongs to the primary — packages/ and Build/ included. worktrees/
  # is the container of worktrees, not one itself.
  # in-wt | subdir | root | Build | container
  assert_output "jiiha jiiha plainclone plainclone none "
  rm -rf "${root}"
}

@test "launch opens the worktree you are standing in" {
  set -eu -o pipefail
  local fn
  fn=$(sed -n '/^cmd_launch()/,/^}/p' "${DIR}/commands/host/tryout")
  [ -n "${fn}" ] || fail "no cmd_launch"

  # The cwd is the whole point: a bare `launch` inside a checkout must not ask.
  printf '%s' "${fn}" | grep -q 'worktree_name_for_path "${PWD}"' \
    || fail "launch must resolve the worktree from the cwd"
  # The active worktree IS the primary, and only the primary knows its own URL
  # (a non-standard port lives in DDEV_PRIMARY_URL, not in the hostname).
  printf '%s' "${fn}" | grep -q 'active_worktree_name' \
    || fail "the active worktree must resolve to the primary site"
  printf '%s' "${fn}" | grep -q 'DDEV_PRIMARY_URL' \
    || fail "the primary URL must come from DDEV, not be rebuilt"
  # An unserved checkout has no URL; opening some other site's would be worse
  # than refusing, so it refuses with the two ways to give it one.
  printf '%s' "${fn}" | grep -q 'is not served' \
    || fail "an unserved worktree must be refused, not silently redirected"
  # And outside a worktree it asks, then falls through to the usage line.
  printf '%s' "${fn}" | grep -q 'ask_site' \
    || fail "launch must ask when the cwd answers nothing"
  printf '%s' "${fn}" | grep -q 'explain_missing' \
    || fail "a cancelled pick must explain instead of failing bare"
}

@test "launch takes the active worktree by name, not just @primary" {
  set -eu -o pipefail
  # The active worktree has no sites/<name>/ marker — it IS the primary — so
  # looking it up by name reads as "not served", yet it is the name anyone sees
  # in `worktree list` and types.
  local fn
  fn=$(sed -n '/^cmd_launch()/,/^}/p' "${DIR}/commands/host/tryout")
  printf '%s' "${fn}" | grep -q 'active_worktree_name 2>/dev/null.*site="${PRIMARY_SITE}"' \
    || fail "the active worktree's name must resolve to the primary site"
}

@test "launch is host-only and never reaches the container" {
  set -eu -o pipefail
  # There is no browser in the web container, and nothing here needs one — so
  # unlike almost every other verb, launch does its work on the host.
  local fn
  fn=$(sed -n '/^cmd_launch()/,/^}/p' "${DIR}/commands/host/tryout")
  printf '%s' "${fn}" | grep -q 'delegate ' \
    && fail "launch must not delegate: the container has no browser"

  # open_url must not try either, and must still print the URL when it cannot open.
  local ou
  ou=$(sed -n '/^open_url()/,/^}/p' "${DIR}/tryout/functions.sh")
  [ -n "${ou}" ] || fail "no open_url"
  printf '%s' "${ou}" | grep -q 'TRYOUT_IN_CONTAINER' \
    || fail "open_url must not spawn an opener inside the container"
  printf '%s' "${ou}" | grep -q 'xdg-open' \
    || fail "open_url must handle Linux"
  printf '%s' "${ou}" | grep -q 'darwin\*|Darwin' \
    || fail "open_url must handle macOS"

  # No opener is not a failure: the URL is the useful part.
  run bash -c "
    set -euo pipefail
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    TRYOUT_IN_CONTAINER=1 open_url https://example.ddev.site
  "
  assert_success
  assert_output --partial "https://example.ddev.site"
}

@test "nginx gets a server-name hash big enough for the longest hostname" {
  set -eu -o pipefail
  # nginx hashes every server_name into fixed-size buckets and REFUSES TO START
  # when one does not fit:
  #   [emerg] could not build server_names_hash, you should increase
  #           server_names_hash_bucket_size: 64
  # That takes the whole web container down — every site, not just the long one.
  # A 49-character name is what broke it: jochen-super-duper on a project called
  # typo3-worktree-test. Neither name is unreasonable.
  local root="${FAKEROOT}/hash"
  mkdir -p "${root}/.ddev/nginx_full"

  gen() { # hostnames (short form) -> the bucket size written
    helper_eval "
      PROJECT_ROOT='${root}'
      write_server_names_hash $(printf "'%s' " "$@")
      grep -o 'bucket_size [0-9]*' '${root}/.ddev/nginx_full/tryout-server-names-hash.conf' 2>/dev/null
    " 2>/dev/null
  }

  # Short names keep the nginx default; there is no reason to inflate it.
  run gen "benni.small"
  assert_success
  assert_output "bucket_size 64"

  # The name that actually broke it must land above 64.
  run gen "jochen-super-duper.typo3-worktree-test"
  assert_success
  assert_output "bucket_size 128"

  # The longest of the set decides, not the first or the last.
  run gen "a.b" "jochen-super-duper.typo3-worktree-test" "c.d"
  assert_success
  assert_output "bucket_size 128"

  # It is a power of two, as nginx wants.
  local size
  size="$(gen "a-very-long-worktree-name-indeed.and-a-long-project-name-too" \
          | sed 's/bucket_size //')"
  [ -n "${size}" ] || fail "no size for a very long name"
  case "${size}" in
    64|128|256|512|1024) ;;
    *) fail "bucket size ${size} is not a power of two nginx would take" ;;
  esac

  # With nothing served there is no hash to size, and the file goes — otherwise it
  # would outlive the sites it was written for.
  helper_eval "PROJECT_ROOT='${root}'; write_server_names_hash ''" 2>/dev/null
  [ ! -f "${root}/.ddev/nginx_full/tryout-server-names-hash.conf" ] \
    || fail "the hash config must go when nothing is served"
}

@test "the hash config is written wherever the hostnames are, and removed with them" {
  set -eu -o pipefail
  # It has to be regenerated by the same function that collects the hostnames, or
  # serving a worktree with a long name writes a vhost nginx then chokes on.
  local fn
  fn=$(sed -n '/^write_worktree_config()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')
  printf '%s' "${fn}" | grep -q 'write_server_names_hash' \
    || fail "the hash must be resized whenever the served set changes"

  # It lives at http scope, not in a server block: DDEV includes nginx_full/ there
  # — its own nginx-site.conf carries a `map`, which is http-only too.
  local path
  path=$(sed -n '/^site_hash_config_file()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${path}" | grep -q 'nginx_full' \
    || fail "the hash config belongs where DDEV includes http-level config"

  # And uninstalling takes it with the vhosts. The existing glob is tryout-site-*,
  # which does not match this file.
  run grep -q 'tryout-server-names-hash.conf' "${DIR}/install.yaml"
  assert_success
}

@test "the overlay requires exactly the sysexts on disk, nothing by branch name" {
  set -eu -o pipefail
  # typo3/theme-camino used to be appended whenever the branch looked like main or
  # v14+. On 13.4 there is no typo3/sysext/theme_camino, so the entry pointed a
  # path repository at a directory that does not exist:
  #   Source path "…/worktrees/main/typo3/sysext/theme_camino" is not found
  # which fails composer install and with it the whole checkout. A detached
  # worktree made it worse: the fallback read EXT:core's branch-alias, got "main",
  # and added camino to a 13.4 tree. The sysexts present decide, and nothing else.
  command -v php >/dev/null || skip "php not available"

  # A fake Core: two sysexts, no theme_camino — a 13.4-shaped tree.
  local core="${FAKEROOT}/core13" proj="${FAKEROOT}/proj13"
  mkdir -p "${core}/typo3/sysext/core" "${core}/typo3/sysext/backend" "${proj}"
  printf '{"name":"typo3/cms-core","extra":{"branch-alias":{"dev-main":"13.4.x-dev"}}}\n' \
    > "${core}/typo3/sysext/core/composer.json"
  printf '{"name":"typo3/cms-backend"}\n' > "${core}/typo3/sysext/backend/composer.json"
  printf '{"name":"x/y","require":{"typo3/theme-camino":"@dev","acme/own":"^1.0"}}\n' \
    > "${proj}/composer.tryout.json"

  run env PROJECT_ROOT="${proj}" TRYOUT_CORE_DIR="${core}" \
      php "${DIR}/tryout/sync-composer.php"
  assert_success

  local req
  req=$(python3 -c "
import json; print(' '.join(sorted(json.load(open('${proj}/composer.tryout.json'))['require'])))")
  # Not required, because it is not there.
  printf '%s' "${req}" | grep -q 'typo3/theme-camino' \
    && fail "camino required on a tree that has no theme_camino: ${req}"
  # The sysexts that ARE there, and the user's own package, all survive.
  printf '%s' "${req}" | grep -q 'typo3/cms-core' || fail "cms-core missing: ${req}"
  printf '%s' "${req}" | grep -q 'typo3/cms-backend' || fail "cms-backend missing: ${req}"
  printf '%s' "${req}" | grep -q 'acme/own' || fail "custom package dropped: ${req}"

  # And where the sysext IS present, the ordinary scan picks it up — so main keeps
  # camino without anything special-casing it.
  local core14="${FAKEROOT}/core14" proj14="${FAKEROOT}/proj14"
  mkdir -p "${core14}/typo3/sysext/core" "${core14}/typo3/sysext/theme_camino" "${proj14}"
  printf '{"name":"typo3/cms-core"}\n' > "${core14}/typo3/sysext/core/composer.json"
  printf '{"name":"typo3/theme-camino"}\n' > "${core14}/typo3/sysext/theme_camino/composer.json"
  printf '{"name":"x/y","require":{}}\n' > "${proj14}/composer.tryout.json"

  run env PROJECT_ROOT="${proj14}" TRYOUT_CORE_DIR="${core14}" \
      php "${DIR}/tryout/sync-composer.php"
  assert_success
  run python3 -c "
import json; print('typo3/theme-camino' in json.load(open('${proj14}/composer.tryout.json'))['require'])"
  assert_output "True"
}

@test "worktree add takes its name from the loop, never from a flag" {
  set -eu -o pipefail
  # Reading $1 as the name BEFORE stripping flags made a leading flag the name —
  # non-empty, so the prompt below never fired, and it was delegated as the
  # worktree name; git then failed on it in the container.
  local body
  body=$(sed -n '/^cmd_worktree()/,/^}/p' "${DIR}/commands/host/tryout" \
         | sed -n '/^        add)/,/^            ;;/p' | grep -v '^[[:space:]]*#')
  [ -n "${body}" ] || fail "no worktree add branch"

  printf '%s' "${body}" | grep -q 'local name="${1:-}"' \
    && fail "the name must not be read before the flags are stripped"
  printf '%s' "${body}" | grep -q 'if \[ -z "${name}" \]; then name="$1"' \
    || fail "the first non-flag word is the name, the second the branch"
  # An unknown flag is an error, not a worktree called "-x".
  printf '%s' "${body}" | grep -q -- '-\*)' \
    || fail "unknown flags must be rejected"
  # And the prompt is still reachable for a bare invocation.
  printf '%s' "${body}" | grep -q 'ask_text' \
    || fail "a missing name must still be asked for"
}

@test "a worktree name can never start with a hyphen" {
  set -eu -o pipefail
  # It becomes a directory and a git ref — neither takes a leading hyphen — and
  # it is exactly what a mis-parsed flag looks like. The old regex
  # ^[A-Za-z0-9._-]+$ accepted "--serve" quite happily.
  local n
  for n in --serve -x - --php; do
    run helper_eval "validate_worktree_name '${n}'"
    assert_failure
  done
  # Real names still pass.
  for n in bugfix-12345 v13.4 main my_tree a; do
    run helper_eval "validate_worktree_name '${n}'"
    assert_success
  done
}

@test "re-serving a worktree keeps the database it still has" {
  set -eu -o pipefail
  # unserve deletes the site but KEEPS the database unless --drop-db was asked
  # for, and the only "already set up" signal was settings.php — which went with
  # the site. So serve ran a fresh `typo3 setup` against a populated database and
  # TYPO3 refused: "The selected database contains already 146 tables." --force
  # does not help; the table check happens first, at database selection.
  local fn un
  fn=$(sed -n '/^setup_site_typo3()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')
  un=$(sed -n '/^unserve_worktree()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  # The database is the second signal, since it can outlive the site.
  printf '%s' "${fn}" | grep -q 'site_database_has_tables' \
    || fail "a populated database means the site is already set up"
  # And the saved settings go back, rather than the setup running.
  printf '%s' "${fn}" | grep -q 'site_saved_settings' \
    || fail "the preserved settings.php must be restored"

  # Tables but NO saved settings must REFUSE, not fall through to `typo3 setup` —
  # which is guaranteed to fail on a populated database ("contains already N
  # tables"). The block that warns and the setup that follows are separated by a
  # `return 1`, so setup never runs on a database it cannot use.
  #
  # Pin it by position: the has-tables block must contain a `return 1` before the
  # `typo3 setup` line. Strip to the has-tables block and check it returns.
  local has_tables_block
  has_tables_block=$(printf '%s' "${fn}" \
    | sed -n '/site_database_has_tables/,/vendor\/bin\/typo3 setup/p')
  printf '%s' "${has_tables_block}" | grep -q 'return 1' \
    || fail "tables with no saved settings must refuse before running setup"


  # unserve preserves it only when the database survives — dropping the database
  # makes the old settings meaningless.
  printf '%s' "${un}" | grep -q 'site_saved_settings' \
    || fail "unserve must preserve settings.php"
  printf '%s' "${un}" | grep -q 'keep_db.*= "true"' \
    || fail "preserve only when the database is kept"

  # It is kept OUTSIDE the site dir, which unserve rm -rf's.
  local path
  path=$(sed -n '/^site_saved_settings()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${path}" | grep -q 'SITES_DIR}/\.' \
    || fail "the saved copy must not live inside the directory unserve deletes"

  # Behaviour: the table count is read from the site's OWN database.
  local q
  q=$(sed -n '/^site_database_has_tables()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${q}" | grep -q 'site_database' \
    || fail "the count must be for this site's database"
  printf '%s' "${q}" | grep -q 'db_is_postgres' \
    || fail "postgres counts its tables differently"
}

@test "nothing pays for a dirty check it does not print" {
  set -eu -o pipefail
  # list_core_worktrees runs two `git diff` calls per worktree. A TYPO3 Core
  # checkout is ~20k tracked files, so on a Mutagen project that is ~3.8s per
  # worktree cold. status read the answer into a variable and never printed it.
  local status_fn
  status_fn=$(sed -n '/^ctr_status_body()/,/^}/p' "${DIR}/tryout/commands.sh" \
              | grep -v '^[[:space:]]*#')

  # status prints name, branch and head — never the dirty flag.
  printf '%s' "${status_fn}" | grep -qE '\blist_core_worktrees\b' \
    && fail "status must not pay for a column it does not show"
  printf '%s' "${status_fn}" | grep -q 'list_core_worktrees_fast' \
    || fail "status must use the cheap lister"

  # And it asks once: the count and the loop want the same rows.
  local calls
  calls=$(printf '%s\n' "${status_fn}" | grep -c 'list_core_worktrees_fast' || true)
  [ "${calls}" -eq 1 ] \
    || fail "status calls its lister ${calls} times; once is enough"
}

@test "worktree list keeps the STATE column it is asked for" {
  set -eu -o pipefail
  # It is the one caller that PRINTS dirtiness, it is run by hand, and --plain is
  # a documented contract (NAME HEAD BRANCH STATE PHP DB URL) that
  # tests/e2e/login.spec.ts parses. It keeps the expensive lister.
  local fn
  fn=$(sed -n '/^ctr_worktree()/,/^}/p' "${DIR}/tryout/commands.sh" \
       | grep -v '^[[:space:]]*#')
  printf '%s' "${fn}" | grep -qE '\blist_core_worktrees\b' \
    || fail "worktree list needs the dirty column"
  # The PRINTED field, not just the name: `${dirty}` also appears in the `read`
  # that unpacks the row, so a grep for it passes even when the column is gone.
  printf '%s' "${fn}" | grep -qE 'printf.*\$\{dirty\}|"\$\{branch\}" "\$\{dirty\}"' \
    || fail "the STATE column must still be printed"
}

@test "the two worktree listers agree on their shared fields" {
  set -eu -o pipefail
  # Same rows, one column fewer — so a caller reading positionally cannot have a
  # field shift under it. Both must resolve the active worktree the same way too,
  # since both mark it in the last field.
  command -v git >/dev/null || skip "git not available"
  local root="${FAKEROOT}/listers"
  mkdir -p "${root}"
  local w
  for w in alpha beta; do
    git init -q -b "br-${w}" "${root}/worktrees/${w}"
    git -C "${root}/worktrees/${w}" commit -q --allow-empty -m x
  done
  ln -s worktrees/alpha "${root}/typo3-core"

  emit() { # $1=function
    helper_eval "
      PROJECT_ROOT='${root}'
      CORE_DIR='${root}/typo3-core'
      CORE_WORKTREE_PREFIX='${root}/worktrees/'
      $1
    " 2>/dev/null
  }

  local slow fast
  slow="$(emit list_core_worktrees)"
  fast="$(emit list_core_worktrees_fast)"
  [ -n "${slow}" ] || fail "no rows from list_core_worktrees"
  [ -n "${fast}" ] || fail "no rows from list_core_worktrees_fast"

  # Same worktrees, same order.
  [ "$(printf '%s\n' "${slow}" | cut -f1)" = "$(printf '%s\n' "${fast}" | cut -f1)" ] \
    || fail "the two listers disagree on which worktrees exist"
  # name, head, branch identical; the fast one simply stops there.
  [ "$(printf '%s\n' "${slow}" | cut -f1,2,3)" = "$(printf '%s\n' "${fast}" | cut -f1,2,3)" ] \
    || fail "the shared fields must match"
  # The active marker is the LAST field in both — 5th vs 4th.
  [ "$(printf '%s\n' "${slow}" | cut -f5)" = "$(printf '%s\n' "${fast}" | cut -f4)" ] \
    || fail "both must mark the active worktree in their last field"
}

# A Core-shaped fixture: a bare origin with main, a root clone on main, and a
# detached worktree under worktrees/<name> carrying commits on top of origin/main.
make_card_fixture() { # $1=root
  local root="$1" seed="$1/seed"
  git init -q -b main "${seed}"
  git -C "${seed}" -c user.name=t -c user.email=t@t commit -q --allow-empty -m "[TASK] Base"
  git clone -q --bare "${seed}" "${root}/origin.git"
  git clone -q "${root}/origin.git" "${root}/core"
  # As install does: nested worktrees are kept out of the root's status.
  echo "/worktrees/" >> "${root}/core/.git/info/exclude"
  git -C "${root}/core" worktree add -q --detach "${root}/core/worktrees/feat" origin/main
  local wt="${root}/core/worktrees/feat"
  git -C "${wt}" -c user.name=t -c user.email=t@t commit -q --allow-empty -m "[BUGFIX] One"
  echo x > "${wt}/tracked"
  git -C "${wt}" add tracked
  git -C "${wt}" -c user.name=t -c user.email=t@t commit -q -m "[BUGFIX] Fix page tree drag"
  echo y > "${wt}/tracked"
  echo z > "${wt}/new"
}

card_eval() { # $1=root $2=code
  helper_eval "
    PROJECT_ROOT='$1/core'
    CORE_DIR='$1/core'
    CORE_WORKTREE_PREFIX='$1/core/worktrees/'
    SITES_DIR='$1/sites'
    $2
  " 2>/dev/null
}

@test "worktree_change_summary counts changes and calls a non-checkout clean" {
  set -eu -o pipefail
  command -v git >/dev/null || skip "git not available"
  local root="${FAKEROOT}/summary"
  make_card_fixture "${root}"

  run card_eval "${root}" "worktree_change_summary '${root}/core'"
  assert_output "clean"
  run card_eval "${root}" "worktree_change_summary '${root}/core/worktrees/feat'"
  assert_output "1 modified, 1 untracked"
  # "Cannot look" is not "has changes" — see core_worktree_is_dirty.
  mkdir -p "${root}/not-a-repo"
  run card_eval "${root}" "worktree_change_summary '${root}/not-a-repo'"
  assert_output "clean"
}

@test "detect_detached_base_branch answers for the checkout it is given" {
  set -eu -o pipefail
  command -v git >/dev/null || skip "git not available"
  local root="${FAKEROOT}/base"
  make_card_fixture "${root}"
  # A release branch that contains only the root's HEAD, not the worktree's.
  git -C "${root}/core" push -q origin HEAD:refs/heads/14.3
  git -C "${root}/core" fetch -q origin

  run card_eval "${root}" "detect_detached_base_branch '${root}/core/worktrees/feat'"
  assert_output "main"
  run card_eval "${root}" "detect_detached_base_branch"
  assert_output "14.3"
}

@test "worktree list renders a card per worktree with base, patches and changes" {
  set -eu -o pipefail
  command -v git >/dev/null || skip "git not available"
  local root="${FAKEROOT}/card"
  make_card_fixture "${root}"
  local strip='s/\x1b\[[0-9;]*m//g'

  run card_eval "${root}" "worktree_card feat \"\$(git -C '${root}/core/worktrees/feat' rev-parse --short HEAD)\" '(detached)' ''"
  assert_success
  output="$(printf '%s' "${output}" | sed "${strip}")"
  assert_output --partial "○ feat"
  assert_output --partial "detached from main @"
  assert_output --partial "2 patches on top · [BUGFIX] Fix page tree drag"
  assert_output --partial "1 modified, 1 untracked"
  assert_output --partial "not served"
  assert_output --partial "→ ddev tryout worktree serve feat"
  refute_output --partial "primary"

  # The active row is the primary: marked, and given the project URL.
  run card_eval "${root}" "DDEV_PRIMARY_URL=https://p.ddev.site DDEV_PHP_VERSION=8.4
    worktree_card main \"\$(git -C '${root}/core' rev-parse --short HEAD)\" main active"
  output="$(printf '%s' "${output}" | sed "${strip}")"
  assert_output --partial "● main"
  assert_output --partial "← primary"
  assert_output --partial "main @"
  assert_output --partial "[TASK] Base"
  assert_output --partial "clean"
  assert_output --partial "https://p.ddev.site"
  assert_output --partial "PHP 8.4 · db"
  refute_output --partial "patches on top"
}

@test "ui_table keeps its rows where neither gum nor column exists" {
  set -eu -o pipefail
  # The web image has neither. `column … || cat` used to hand cat a stdin the
  # pipeline had already drained, so worktree list printed an empty table.
  run helper_eval 'have_gum() { return 1; }; column() { return 127; }
    printf "NAME,STATE\nmain,clean\n" | ui_table'
  assert_success
  assert_output --partial "NAME"
  assert_output --partial "main"
  assert_output --partial "clean"
}

@test "checkout takes --site, so the branch can still be asked for" {
  set -eu -o pipefail
  # A caller may know the site but not the branch, and positionally the branch
  # comes first — hence a flag.
  run bash -c "
    set -- --site benni 13.4
    args=(); site=''
    while [ \$# -gt 0 ]; do
      case \"\$1\" in
        --site)   site=\"\${2:-}\"; shift 2 || shift ;;
        --site=*) site=\"\${1#--site=}\"; shift ;;
        *)        args+=(\"\$1\"); shift ;;
      esac
    done
    set -- \${args[@]+\"\${args[@]}\"}
    echo \"branch=\${1:-} site=\${site}\"
  "
  assert_output "branch=13.4 site=benni"

  # The old positional form still works, or every existing invocation breaks.
  run grep -n 'local target_branch="\${1:-}"' "${DIR}/commands/host/tryout"
  assert_success
}

@test "the payload carries a version, and install records it" {
  set -eu -o pipefail
  run helper_eval 'printf "%s" "${TRYOUT_VERSION}"'
  assert_success
  [ -n "${output}" ] || fail "TRYOUT_VERSION is empty"
  # A plain integer, bumped by hand: nothing here can read git at install time.
  printf '%s' "${output}" | grep -qE '^[0-9]+$' \
    || fail "TRYOUT_VERSION should be an integer, got '${output}'"

  # install.yaml stamps the same value into the installed tree.
  run grep -q 'tryout/.version' "${DIR}/install.yaml"
  assert_success
  # …and removal takes it out again.
  local removal
  removal=$(sed -n '/^removal_actions:/,$p' "${DIR}/install.yaml")
  printf '%s\n' "${removal}" | grep -q '.version' || fail "removal leaves .version behind"
}

@test "addon_is_stale compares the installed stamp against the payload" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/.ddev/tryout"

  # No stamp at all: an install predating the marker. Not stale — we cannot
  # know, and crying wolf on every old project would train people to ignore it.
  run helper addon_is_stale
  assert_failure

  # Same version: current.
  helper_eval 'printf "%s\n" "${TRYOUT_VERSION}"' > "${FAKEROOT}/.ddev/tryout/.version"
  run helper addon_is_stale
  assert_failure

  # Older stamp: stale.
  printf '0\n' > "${FAKEROOT}/.ddev/tryout/.version"
  run helper addon_is_stale
  assert_success

  # Newer stamp than the running code — the command itself is the old copy.
  # Still worth reporting: something is out of step either way.
  printf '99999\n' > "${FAKEROOT}/.ddev/tryout/.version"
  run helper addon_is_stale
  assert_success
}

@test "status warns about a stale install, on the host, before delegating" {
  set -eu -o pipefail
  local body
  body=$(sed -n '/^cmd_status() {/,/^}/p' "${DIR}/commands/host/tryout")
  [ -n "${body}" ] || fail "no cmd_status()"
  printf '%s\n' "${body}" | grep -q 'addon_is_stale' \
    || fail "status never checks whether the install is stale"
  # It must come before the delegate, or a stopped project never shows it.
  local check delegate
  check=$(printf '%s\n' "${body}" | grep -n 'addon_is_stale' | head -1 | cut -d: -f1)
  delegate=$(printf '%s\n' "${body}" | grep -n 'delegate status' | head -1 | cut -d: -f1)
  [ -n "${check}" ] && [ -n "${delegate}" ] || fail "cannot locate both"
  [ "${check}" -lt "${delegate}" ] || fail "the staleness check runs after delegating"
  # And it names the command that fixes it.
  printf '%s\n' "${body}" | grep -q 'add-on get' || fail "no next step given"
}

# --- creating a worktree ----------------------------------------------------
# Every route must end at <project>/worktrees/<name>, and must ask which branch
# the checkout is based on rather than silently taking whatever Core is on.

@test "no herdr integration is left in the shipped payload" {
  set -eu -o pipefail
  # Removed whole; its replacement comes later. A half-removed helper or flag would
  # be a command that fails. install.yaml is exempt: its cleanup action names the
  # files earlier versions shipped, so upgraded projects lose them.
  run grep -rliE 'herdr|HERDR_' "${DIR}/commands" "${DIR}/tryout" "${DIR}/web-build"
  assert_failure
  run grep -E '^[[:space:]]*(cp|ln) .*herdr' "${DIR}/install.yaml"
  assert_failure
}

@test "the branch is asked for even when the name came in as an argument" {
  set -eu -o pipefail
  # Naming a worktree says nothing about which branch it sits on. `worktree add
  # <name>` passes a name and no branch, so a prompt nested inside
  # `if [ -z "$name" ]` would leave it silently based on whatever Core is out.
  run helper_eval '
    have_tty() { return 0; }
    ask_branch() { printf "13.4"; }
    BRANCH=main
    ASKED_BRANCH=""
    ask_new_worktree_branch "usage" && printf "%s" "${ASKED_BRANCH}"'
  assert_success
  assert_output "13.4"
}

@test "an explicitly given branch is not asked about again" {
  set -eu -o pipefail
  run helper_eval '
    have_tty() { return 0; }
    ask_branch() { printf "13.4"; }
    BRANCH=main
    ASKED_BRANCH="12.4"
    ask_new_worktree_branch "usage" && printf "%s" "${ASKED_BRANCH}"'
  assert_success
  assert_output "12.4"
}

@test "with no terminal the branch falls back to BRANCH instead of blocking" {
  set -eu -o pipefail
  # ddev start and any script must never stop here. ui_choose returns non-zero
  # without a TTY, so asking unconditionally would abort a non-interactive add.
  run helper_eval '
    have_tty() { return 1; }
    ask_branch() { printf "should-not-be-called"; return 1; }
    BRANCH=14.3
    ASKED_BRANCH=""
    ask_new_worktree_branch "usage" && printf "%s" "${ASKED_BRANCH}"'
  assert_success
  assert_output "14.3"
}

@test "cancelling the branch picker stops the command" {
  set -eu -o pipefail
  run helper_eval '
    have_tty() { return 0; }
    ask_branch() { return 1; }
    BRANCH=main
    ASKED_BRANCH=""
    ask_new_worktree_branch "ddev tryout worktree add <name> [<branch>]"'
  assert_failure
  # A cancel is not a usage error: explain_missing says so only without a terminal.
  refute_output --partial "Usage:"
}

@test "worktree add asks for the branch outside the name check" {
  set -eu -o pipefail
  # The regression this guards: `ask_branch` sitting inside `if [ -z "${name}" ]`,
  # so `worktree add <name>` never asked and silently took whatever Core was out.
  local f="${DIR}/commands/host/tryout"
  run grep -c 'ask_new_worktree_branch' "${f}"
  assert_success
  assert_output "1"
  local guard
  guard=$(awk '/^        add\)/,/^            ;;/' "${f}" | awk '/if \[ -z /,/^            fi$/')
  [ -n "${guard}" ] || fail "could not find the name guard in worktree add"
  if printf '%s\n' "${guard}" | grep -q 'ask_new_worktree_branch'; then
    fail "worktree add asks for the branch only when the name is missing"
  fi
}

@test "patch asks which site first, and lists that site's branch" {
  # The site decides which Core checkout is patched, and therefore which branch's
  # open changes are worth showing: offering main's changes for a 13.4 site is
  # simply the wrong list. So the site is resolved BEFORE the fetch.
  set -eu -o pipefail
  local body site_at fetch_at
  body=$(sed -n '/^cmd_patch() {/,/^}$/p' "${DIR}/commands/host/tryout")
  [ -n "${body}" ] || fail "no cmd_patch()"

  printf '%s\n' "${body}" | grep -q 'ask_site' || fail "patch never asks for a site"
  site_at=$(printf '%s\n' "${body}" | grep -n 'ask_site' | head -1 | cut -d: -f1)
  fetch_at=$(printf '%s\n' "${body}" | grep -n 'fetch_open_patches' | head -1 | cut -d: -f1)
  [ -n "${fetch_at}" ] || fail "no fetch"
  [ "${site_at}" -lt "${fetch_at}" ] \
    || fail "the site must be chosen before the changes are fetched"

  # A named site still skips the question, and an explicit change number is
  # applied without one either.
  printf '%s\n' "${body}" | grep -q 'served_site_names' \
    || fail "patch asks even when nothing else is served"
}

@test "the branch listed for a served site is that site's own, not the primary's" {
  set -eu -o pipefail
  local body
  body=$(sed -n '/^cmd_patch() {/,/^}$/p' "${DIR}/commands/host/tryout")
  # Without this a 13.4 worktree would be offered main's open changes.
  printf '%s\n' "${body}" | grep -q 'site_core_dir' \
    || fail "the site's own Core checkout is never consulted"
  printf '%s\n' "${body}" | grep -q 'detect_detached_base_branch' \
    || fail "a served site's base branch is never detected"
}

@test "a reload only replaces a restart when the hostname set is unchanged" {
  set -eu -o pipefail
  # DDEV derives the Traefik routing rule, $VIRTUAL_HOST and the certificate SANs
  # from additional_hostnames. All three go stale exactly when the served set
  # changes — and none of them can be refreshed from inside the container. So the
  # gate is the hostname set, and nothing else.
  local root="${FAKEROOT}/reloadgate"
  mkdir -p "${root}/TYPO3-Instances/keep" "${root}/TYPO3-Instances/gone"
  printf 'php=8.3\n' > "${root}/TYPO3-Instances/keep/.tryout-site"
  printf 'php=8.3\n' > "${root}/TYPO3-Instances/gone/.tryout-site"

  # DDEV_APPROOT, not PROJECT_ROOT: helper_eval sources functions.sh BEFORE it runs
  # this, so every derived path is already fixed by then. The environment is the
  # only lever that reaches the assignments themselves.
  snap() {
    ( export DDEV_APPROOT="${root}" DDEV_SITENAME='proj'
      helper_eval 'served_hostname_set' ) 2>/dev/null
  }

  local before after
  before="$(snap)"
  # Re-serving an existing site changes nothing about the set: same path taken.
  after="$(snap)"
  [ "${before}" = "${after}" ] || fail "an unchanged set must compare equal"

  # Removing one changes it, so the restart stays required.
  rm -f "${root}/TYPO3-Instances/gone/.tryout-site"
  after="$(snap)"
  [ "${before}" != "${after}" ] || fail "dropping a site must change the set"

  # And it is order-independent: the comparison is on a sorted set, not on the
  # order the directories happen to be read in.
  printf '%s\n' "${before}" | sort -c \
    || fail "the snapshot must be sorted, or two equal sets can compare different"
}

@test "the vhost sync clears our stale copies but never DDEV's own" {
  set -eu -o pipefail
  # sites-enabled is a COPY made by DDEV's /start.sh, not a mount. unserve removes
  # a vhost from nginx_full/, and without clearing the copy the dead site keeps
  # serving. The clear must be scoped: DDEV's nginx-site.conf lives in the same
  # directory, and taking it out would break every site in the project.
  local fn
  fn=$(sed -n '/^sync_and_reload_webserver()/,/^}/p' "${DIR}/tryout/functions.sh")

  printf '%s' "${fn}" | grep -q 'rm -f "${dst}"/tryout-site-\*\.conf' \
    || fail "stale per-site vhosts must be cleared, or unserve leaves them serving"
  printf '%s' "${fn}" | grep -q 'tryout-server-names-hash.conf' \
    || fail "the hash config is ours too and must be cleared with the vhosts"

  # Never a blanket wipe of the directory: that is DDEV's, not ours.
  printf '%s' "${fn}" | grep -qE 'rm -rf "\$\{dst\}"( |$)' \
    && fail "the enabled dir holds DDEV's own config; only our prefix may be removed"

  # Every removal is scoped to a tryout- prefix.
  local bad
  bad=$(printf '%s' "${fn}" | grep -E '^\s*rm ' | grep -v 'tryout-' || true)
  [ -z "${bad}" ] || fail "unscoped removal in the sync: ${bad}"
}

@test "the config is validated before the running webserver is reloaded" {
  set -eu -o pipefail
  # An invalid config makes nginx refuse to START, taking down every site in the
  # project — the server_names_hash_bucket_size trap. Validating first means a bad
  # generated vhost leaves the running config untouched, which is strictly safer
  # than the restart this replaces: that one only finds out once it is already down.
  local fn
  fn=$(sed -n '/^sync_and_reload_webserver()/,/^}/p' "${DIR}/tryout/functions.sh")

  local check_line reload_line
  check_line=$(printf '%s' "${fn}" | grep -n 'webserver_config_is_valid' | head -1 | cut -d: -f1)
  # grep -w and a tail: the function's OWN name contains reload_webserver, and
  # matching line 1 would make the ordering check pass no matter what.
  reload_line=$(printf '%s' "${fn}" | grep -n '^[[:space:]]*reload_webserver' | head -1 | cut -d: -f1)
  [ -n "${check_line}" ] || fail "the config must be validated before a reload"
  [ -n "${reload_line}" ] || fail "the sync must actually reload"
  [ "${check_line}" -lt "${reload_line}" ] \
    || fail "validation must come BEFORE the reload, or a bad config still lands"

  # And a failed check must abort rather than carry on.
  printf '%s' "${fn}" | grep -q 'webserver_config_is_valid || return 1' \
    || fail "a config that would not load must stop the reload"
}

@test "the reload keeps the webserver up instead of restarting the container" {
  set -eu -o pipefail
  # nginx is a supervisord program in the web image and there is no /run/nginx.pid,
  # so HUP goes through supervisorctl. The point of the whole change is not
  # bouncing the container, so a restart verb here would defeat it.
  local fn
  fn=$(sed -n '/^reload_webserver()/,/^}/p' "${DIR}/tryout/functions.sh")

  printf '%s' "${fn}" | grep -q 'supervisorctl signal HUP nginx' \
    || fail "nginx reloads by HUP through supervisord"
  printf '%s' "${fn}" | grep -q 'apachectl -k graceful' \
    || fail "the apache branch must reload too; apache-fpm is the default type"

  printf '%s' "${fn}" | grep -qE 'supervisorctl (restart|stop|start)' \
    && fail "restarting the process drops connections the reload exists to keep"
  :
}

@test "nothing container-side calls ddev to apply a config change" {
  set -eu -o pipefail
  # ddev is only a stub inside the web image. The reload path runs there, so it
  # must use the container's own tools.
  local block
  block=$(sed -n '/^# --- Applying a config change without ddev restart ---/,/^# Create the site.s database/p' \
          "${DIR}/tryout/functions.sh" | grep -v '^[[:space:]]*#')
  [ -n "${block}" ] || fail "could not locate the reload block"
  printf '%s' "${block}" | grep -qE '(^|[^-[:alnum:]])ddev ' \
    && fail "the reload path must not shell out to ddev"
  :
}

@test "both webserver types are handled wherever the reload branches" {
  set -eu -o pipefail
  # site_vhost_file already defaults to apache-fpm, so a branch that only knew
  # nginx would silently do nothing on an apache project — the site would look
  # served and never answer.
  local f
  for f in site_conf_source_dir site_conf_enabled_dir webserver_config_is_valid reload_webserver; do
    local fn
    fn=$(sed -n "/^${f}()/,/^}/p" "${DIR}/tryout/functions.sh")
    [ -n "${fn}" ] || fail "${f} is missing"
    printf '%s' "${fn}" | grep -q 'nginx\*)' \
      || fail "${f} must branch on the nginx case"
    printf '%s' "${fn}" | grep -q '\*)' \
      || fail "${f} needs an apache fallback, which is the default webserver type"
  done
}

@test "serve reloads in place, unserve still asks for the restart it needs" {
  set -eu -o pipefail
  # Removing a site always shrinks the hostname set, so unserve can never take the
  # fast path — but it must still drop the vhost, or the site keeps answering on a
  # hostname with nothing behind it.
  local fn
  fn=$(sed -n '/^serve_worktree()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${fn}" | grep -q 'hosts_before="\$(served_hostname_set)"' \
    || fail "serve must snapshot the hostname set before it writes the marker"
  printf '%s' "${fn}" | grep -q 'apply_site_config "\${hosts_before}"' \
    || fail "serve must decide its path from that snapshot"

  # The snapshot has to be taken before the marker is written, or the site being
  # served is already in it and the set always looks unchanged.
  local snap_line marker_line
  snap_line=$(printf '%s' "${fn}" | grep -n 'served_hostname_set' | head -1 | cut -d: -f1)
  marker_line=$(printf '%s' "${fn}" | grep -n "printf 'php=" | head -1 | cut -d: -f1)
  [ -n "${marker_line}" ] || fail "serve no longer writes the .tryout-site marker?"
  [ "${snap_line}" -lt "${marker_line}" ] \
    || fail "snapshot after the marker would always compare equal"

  # unserve still drops the vhost immediately, or the dead site keeps answering.
  # It no longer tells the user to restart: the HOST does that itself, and `ddev`
  # does not work in here anyway.
  fn=$(sed -n '/^unserve_worktree()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${fn}" | grep -q 'sync_and_reload_webserver' \
    || fail "unserve must drop the vhost now, or the dead site keeps serving"
  printf '%s' "${fn}" | grep -q "ddev restart" \
    && fail "the container must not ask for a restart the host already runs"
  :
}

@test "the reload copies the vhost the generator just wrote, not the host's copy" {
  set -eu -o pipefail
  # The generator writes under PROJECT_ROOT/.ddev — on a Mutagen project the
  # container's volume. /mnt/ddev_config is a bind mount of the HOST's .ddev, which
  # has it only once Mutagen has synced back, so a reload from there applied the
  # PREVIOUS vhost: `serve x --php 8.3` reported success while nginx kept passing
  # to the 8.4 socket. Observed on a real project.
  local t
  for t in nginx-fpm apache-fpm; do
    run helper_eval "PROJECT_ROOT=/p; DDEV_WEBSERVER_TYPE=${t}; site_conf_source_dir"
    assert_output --partial "/p/.ddev/"
  done
  run grep -n '/mnt/ddev_config/' "${DIR}/tryout/functions.sh"
  refute_output --regexp 'echo "/mnt/ddev_config'
}

@test "the vhosts are copied back by name, never by globbing the source" {
  set -eu -o pipefail
  # On a Mutagen project the host's deletion of a vhost has not necessarily
  # reached /mnt/ddev_config by the time unserve calls the sync. A glob over the
  # source would then faithfully copy back the file that was just removed, and the
  # unserved site would keep answering on a hostname with nothing behind it —
  # observed on a real project before this was fixed. served_site_names reads the
  # markers, which ARE the definition of served and are right in the container
  # whatever the file sync has caught up on.
  local fn
  fn=$(sed -n '/^sync_and_reload_webserver()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  printf '%s' "${fn}" | grep -q 'for name in \$(served_site_names)' \
    || fail "the copy must be driven by the served set, not by the source directory"

  # The glob is the bug: cp "${src}"/*.conf would restore a just-deleted vhost.
  printf '%s' "${fn}" | grep -qE 'cp "\$\{src\}"/\*' \
    && fail "globbing the source re-copies vhosts that unserve just removed"

  # Every copy names a single file.
  local copies
  copies=$(printf '%s' "${fn}" | grep -E '^\s*cp ' || true)
  [ -n "${copies}" ] || fail "the sync must copy something"
  printf '%s' "${copies}" | grep -q '\*' \
    && fail "a wildcard copy defeats the point: ${copies}"
  :
}

@test "the restart runs on the host, and only when the hostname set moved" {
  set -eu -o pipefail
  # DDEV owns the Traefik rule, $VIRTUAL_HOST and the certificate SANs, all keyed
  # on additional_hostnames — none refreshable from inside the container. So a
  # hostname that just appeared or went needs a restart. An UNCHANGED set does not:
  # serve_worktree already reloaded the webserver in place, and a restart costs
  # minutes on a Mutagen project for nothing.
  local fn
  fn=$(sed -n '/^restart_if_hosts_changed()/,/^}/p' "${DIR}/commands/host/tryout")
  [ -n "${fn}" ] || fail "the host has no restart helper"

  printf '%s' "${fn}" | grep -q 'ddev restart' \
    || fail "it has to actually restart, not just advise one"
  # The gate: same set, no restart.
  printf '%s' "${fn}" | grep -q '\[ "${before}" != "${after}" \] || return 0' \
    || fail "an unchanged hostname set must not pay for a restart"
  # And an opt-out, so serving several worktrees can restart once at the end.
  printf '%s' "${fn}" | grep -q 'TRYOUT_NO_RESTART' \
    || fail "--no-restart must be honoured"

  # It lives on the HOST: ddev is only a stub in the web image.
  grep -q '^restart_if_hosts_changed()' "${DIR}/commands/host/tryout" \
    || fail "the restart helper belongs in the host command"
  local f
  for f in functions.sh commands.sh; do
    run grep -q 'restart_if_hosts_changed' "${DIR}/tryout/${f}"
    assert_failure   # container-side files must not carry it
  done
}

@test "every verb that moves a hostname snapshots before delegating" {
  set -eu -o pipefail
  # The snapshot has to be taken BEFORE the container writes or removes the
  # .tryout-site marker, or the set always compares equal and nothing restarts.
  # serve, unserve and rename all move it; `add` does too, but only with --serve.
  local host="${DIR}/commands/host/tryout"
  local verb
  for verb in serve unserve; do
    local block
    block=$(sed -n "/^        ${verb})\$/,/^            ;;/p" "${host}")
    [ -n "${block}" ] || fail "no ${verb} dispatch block"
    printf '%s' "${block}" | grep -q 'served_hostname_set' \
      || fail "${verb} must snapshot the hostname set"
    printf '%s' "${block}" | grep -q 'restart_if_hosts_changed' \
      || fail "${verb} must restart when that set moved"

    # Snapshot first, delegate second: reversed, the marker is already written.
    local snap_line del_line
    snap_line=$(printf '%s' "${block}" | grep -n 'served_hostname_set' | head -1 | cut -d: -f1)
    del_line=$(printf '%s' "${block}" | grep -n 'delegate ' | head -1 | cut -d: -f1)
    [ "${snap_line}" -lt "${del_line}" ] \
      || fail "${verb} snapshots after the work, so the set always looks unchanged"
  done

  # --no-restart is the host's own word; delegating it would fail the container's
  # argument parser, so it must be stripped before the call.
  local sblock
  sblock=$(sed -n "/^        serve)\$/,/^            ;;/p" "${host}")
  printf '%s' "${sblock}" | grep -q -- '--no-restart) TRYOUT_NO_RESTART=1' \
    || fail "--no-restart must be consumed on the host, not delegated"
}

@test "removing a worktree takes its branch, unless that would lose work" {
  set -eu -o pipefail
  # The branch outlives the worktree, and add_core_worktree refuses a name whose
  # branch exists — so a leftover blocks re-creating a worktree of the same name,
  # with an error about a branch nobody is thinking about ("A branch 'jochen'
  # already exists" when the folder is plainly gone). Removing it closes that.
  local fn
  fn=$(sed -n '/^remove_core_worktree()/,/^}/p' "${DIR}/tryout/functions.sh")

  printf '%s' "${fn}" | grep -q 'branch "${del}" "${name}"' \
    || fail "remove must delete the worktree's branch too"

  # -d, not -D: git refuses -d on a branch holding unmerged work, and that refusal
  # IS the guard against an unpushed commit vanishing with the checkout. A bare -D
  # would destroy it silently.
  printf '%s' "${fn}" | grep -q 'local del="-d"' \
    || fail "the default must be -d, so git can refuse to lose unmerged work"
  printf '%s' "${fn}" | grep -q '\[ "${force}" = "true" \] && del="-D"' \
    || fail "--force is the only way to -D"

  # A refusal must be reported, not swallowed: the branch is still there and the
  # user has to know why.
  printf '%s' "${fn}" | grep -q "not merged" \
    || fail "a kept branch must say why it was kept"
}

@test "the Gerrit port probe does not use bash /dev/tcp" {
  set -eu -o pipefail
  # macOS SIGKILLs a shell that opens /dev/tcp to an EXTERNAL host — verified:
  # 127.0.0.1 works, any outside address dies with rc=137. The subshell's death
  # printed "Killed: 9" straight to the user's terminal, and diagnose_gerrit_ssh
  # read the failure as "the port is unreachable" — on a machine where it was
  # reachable, one line after an authenticated call to that very host and port.
  local fn
  fn=$(sed -n '/^diagnose_gerrit_ssh()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  printf '%s' "${fn}" | grep -q '/dev/tcp' \
    && fail "/dev/tcp is killed by macOS for external hosts; use nc"

  # nc -z -w takes the same flags on the BSD nc macOS ships and on GNU nc.
  # -G would be BSD-only, which the portability rules forbid.
  printf '%s' "${fn}" | grep -q 'nc -z -w' \
    || fail "the reachability probe must use a portable, quiet nc"
  printf '%s' "${fn}" | grep -q 'nc .*-G ' \
    && fail "-G is BSD-only; -w works on both"

  # No nc must not mean a false verdict: step 3 opens a real connection to the
  # same host and port, so skipping is safe and guessing is not.
  printf '%s' "${fn}" | grep -q 'command -v nc' \
    || fail "the probe must be skipped where nc is absent, not assumed to fail"
  :
}

@test "the Gerrit SSH verdict comes from SSH, not from what an agent holds" {
  set -eu -o pipefail
  # An `ssh-add -l` gate AHEAD of the auth probe answers a different question and
  # used to return early on its answer: a host whose agent is empty but which
  # authenticates from a key on disk — ~/.ssh/id_rsa offered and accepted by
  # Gerrit, the ordinary case — was told "no-agent-key" and pointed at an ssh-add
  # it did not need, right after `ddev auth ssh` had correctly filled the
  # CONTAINER's agent. Ask the thing being reported on, then explain.
  local fn
  fn=$(sed -n '/^diagnose_gerrit_ssh()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  local auth_line agent_line
  auth_line=$(printf '%s' "${fn}" | grep -n 'gerrit version' | head -1 | cut -d: -f1)
  agent_line=$(printf '%s' "${fn}" | grep -n 'ssh-add -l' | head -1 | cut -d: -f1)
  [ -n "${auth_line}" ] || fail "the auth probe is gone"
  [ -n "${agent_line}" ] || fail "the agent check is gone; it still classifies a failure"
  [ "${auth_line}" -lt "${agent_line}" ] \
    || fail "ssh-add before the auth probe is the bug: it short-circuits a working host"

  # no-agent-key must mean no identity ANYWHERE — an empty agent alone is not it,
  # because ssh authenticates from ~/.ssh/id_* without one.
  printf '%s' "${fn}" | grep -q 'id_\*' \
    || fail "an on-disk key must count, or an empty agent is misreported"
  printf '%s' "${fn}" | grep -q 'case "${k}" in \*.pub) continue ;; esac' \
    || fail "a .pub file is not a usable identity"

  # Both reasons must survive: they have different fixes.
  printf '%s' "${fn}" | grep -q 'CS_SSH_REASON="no-agent-key"' \
    || fail "the container's real case must still be diagnosable"
  printf '%s' "${fn}" | grep -q 'CS_SSH_REASON="denied"' \
    || fail "a key that Gerrit refuses is a different fix from having none"
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

@test "a path that is not a checkout is not dirty" {
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'
  # `! git diff --quiet` turns "cannot look" into "has changes": git fails on a
  # path that is not a repository, --quiet returns non-zero, and the negation
  # reports dirty. That is how `worktree use` came to refuse a clean project —
  # it tested worktrees/<primary>, which never exists because the primary IS the
  # root checkout.
  local repo="${FAKEROOT}/dirtycheck"
  git init -q "${repo}"
  git -C "${repo}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init

  run helper core_worktree_is_dirty "${FAKEROOT}/no-such-directory"
  assert_failure
  run helper core_worktree_is_dirty "${FAKEROOT}"
  assert_failure   # exists, but is not a checkout

  # A real change still reports dirty, or the guard would be useless.
  echo change > "${repo}/file"
  git -C "${repo}" add file
  run helper core_worktree_is_dirty "${repo}"
  assert_success
}

@test "the active Core comes from the primary overlay, not the root's branch" {
  set -eu -o pipefail
  # `worktree use` repoints the primary instance's sysext path repository. The
  # root's branch would keep answering for the root even after that switch, so
  # everything asking "which Core is served" has to read the overlay.
  local root="${FAKEROOT}/activecore"
  mkdir -p "${root}/TYPO3-Instances/primary"

  overlay() { # <sysext-url>
    printf '{"repositories":[{"type":"path","url":"../../packages/*"},{"type":"path","url":"%s"}]}\n' \
      "$1" > "${root}/TYPO3-Instances/primary/composer.tryout.json"
  }
  active() {
    ( export DDEV_APPROOT="${root}"
      helper_eval 'plain_core_name() { echo rootbranch; }; active_worktree_name' )
  }

  # Pointing at a worktree names that worktree.
  overlay "../../worktrees/v13/typo3/sysext/*"
  run active
  assert_output "v13"

  # Pointing at the root names the root, whose name is its branch.
  overlay "../../typo3/sysext/*"
  run active
  assert_output "rootbranch"

  # No overlay at all falls back the same way rather than erroring.
  rm -f "${root}/TYPO3-Instances/primary/composer.tryout.json"
  run active
  assert_output "rootbranch"
}

@test "switching the active Core never writes a symlink" {
  set -eu -o pipefail
  # CORE_DIR is the project root now, and `ln -sfn <target> <existing-dir>` does
  # not replace a directory — it creates a link INSIDE it. The old set_active_core
  # would have littered the Core working tree with a stray symlink and switched
  # nothing; only the false dirty-check was stopping it.
  local fn
  fn=$(sed -n '/^set_active_core()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  printf '%s' "${fn}" | grep -q 'ln -s' \
    && fail "set_active_core must not symlink: CORE_DIR is the project root"
  printf '%s' "${fn}" | grep -q 'use-core.php' \
    || fail "switching must rewrite the primary overlay"
}

@test "post-start finds the primary's settings through site_dir, not the project root" {
  set -eu -o pipefail
  # The "first time only" guard tested PROJECT_ROOT/config/system/settings.php.
  # That path stopped existing when instances moved under TYPO3-Instances/, so the
  # guard never found the file and ran `typo3 setup` on EVERY start — which TYPO3
  # then refused with "The selected database contains already N tables".
  local f="${DIR}/tryout/post-start.sh"

  grep -qE '\$\{PROJECT_ROOT\}/config/system' "${f}" \
    && fail "the instance is not at the project root; resolve it through site_dir"

  # It reuses the function that already knows where an instance lives — and that
  # already handles the populated-database case the error was really about.
  grep -q 'setup_site_typo3 "\${PRIMARY_SITE}"' "${f}" \
    || fail "post-start must set the primary up through setup_site_typo3"

  # No second copy of the setup invocation: one place decides how TYPO3 is set up.
  local calls
  calls=$(grep -c 'typo3 setup --no-interaction' "${f}" || true)
  [ "${calls}" -eq 0 ] \
    || fail "post-start duplicates the setup call instead of delegating: ${calls}"
}

@test "a site's cache is cleared per site, never always the primary's" {
  set -eu -o pipefail
  # INSTANCE_DIR is always TYPO3-Instances/primary, but ctr_checkout runs this on
  # the served-site arm too — so `checkout <branch> --site v13` wiped the PRIMARY's
  # cache and left v13's stale. reset_core_to_main already does it per site.
  local fn
  fn=$(sed -n '/^ctr_checkout()/,/^}/p' "${DIR}/tryout/commands.sh" \
       | grep -v '^[[:space:]]*#')

  printf '%s' "${fn}" | grep -qE 'rm -rf "\$\{INSTANCE_DIR\}/var/cache"' \
    && fail "checkout must clear the cache of the site it switched, not the primary's"
  printf '%s' "${fn}" | grep -q 'site_dir "${site}")/var/cache' \
    || fail "the cache path must be resolved through site_dir"
}

@test "site_core_dir follows worktree use for the primary" {
  set -eu -o pipefail
  # The primary serves whatever its overlay points at, which `worktree use` moves.
  # Returning CORE_DIR answers "the root" after a switch — latent today, since every
  # caller happens to be on a non-primary branch, and a trap for the first one that
  # is not.
  local fn
  fn=$(sed -n '/^site_core_dir()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${fn}" | grep -q 'active_core_dir' \
    || fail "the primary's Core is the active one, not CORE_DIR"
  printf '%s' "${fn}" | grep -qE 'echo "\$\{CORE_DIR\}"' \
    && fail "CORE_DIR is only the root; it does not follow worktree use"
  :
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

@test "a detached worktree still resolves against composer's branch alias" {
  set -eu -o pipefail
  # This used to require a local branch named after the BASE (dev-main → 13.4.x-dev),
  # because composer derives a path repository's version from the branch name. A
  # detached HEAD at origin/<base> resolves the same way — composer describes the
  # checked-out commit against the base — so no branch is created, and the whole
  # class of branch-name collisions is gone with it.
  local fn
  fn=$(sed -n '/^add_core_worktree()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  printf '%s' "${fn}" | grep -q 'worktree add --detach "${dir}" "origin/${branch}"' \
    || fail "creation must detach at origin/<base>, which composer can resolve"
  ! printf '%s' "${fn}" | grep -q -- '-B ' \
    || fail "no local branch may be created"
  ! printf '%s' "${fn}" | grep -q 'it is checked out in' \
    || fail "there is no branch to collide, so no collision message"
}

@test "the branch picker has a list to offer after a one-branch clone" {
  set -eu -o pipefail
  # clone_core_into_root fetches ONE branch (`fetch --depth 1 origin <branch>`),
  # which writes ONE remote-tracking ref — so refs/remotes/origin/ held only `main`
  # and every picker built on list_local_core_branches could offer nothing else.
  # Measured on a clean clone: 1 ref instead of 43.
  local fn
  fn=$(sed -n '/^ask_branch()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${fn}" | grep -q 'ensure_core_branch_refs' \
    || fail "the picker must make sure the list exists before offering it"

  fn=$(sed -n '/^ensure_core_branch_refs()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')
  # All heads, and still shallow: the tips are the list, the history is not wanted.
  printf '%s' "${fn}" | grep -q "refs/heads/\*:refs/remotes/origin/\*" \
    || fail "it must fetch every head, not one"
  printf '%s' "${fn}" | grep -q -- '--depth 1' \
    || fail "the tips are enough; full history would cost minutes"

  # Gated on the ref COUNT, so it runs once and cannot go stale the way a marker
  # file would — and so `ddev start` keeps paying only for the one branch it needs.
  printf '%s' "${fn}" | grep -qE 'for-each-ref refs/remotes/origin' \
    || fail "the guard must count the refs it is there to provide"
  printf '%s' "${fn}" | grep -qE '\-gt 1 \] && return 0' \
    || fail "more than one ref means the list is already there"
}

@test "completion never waits on the network for branches" {
  set -eu -o pipefail
  # DDEV runs the completion script on every TAB and drops all candidates if it
  # exits non-zero; a network call there would stall the shell. It shares
  # list_local_core_branches with the picker but must not share the fetch.
  run grep -q 'ensure_core_branch_refs' "${DIR}/commands/host/autocomplete/tryout"
  assert_failure
  run grep -qE '^[^#]*git (ls-remote|fetch)' "${DIR}/commands/host/autocomplete/tryout"
  assert_failure
}

@test "setup_site_frontend probes for styleguide, gates DB writes on the generate, and is version-agnostic" {
  set -eu -o pipefail
  # A bare typo3 setup builds only the backend, so the site provisions a frontend
  # with EXT:styleguide's generator where it exists (13.4+). It must decide by
  # PROBING for the command, never by matching a version number, and it must never
  # touch the database unless the generate actually succeeded.
  local fn
  fn=$(sed -n '/^setup_site_frontend()/,/^}/p' "${DIR}/tryout/functions.sh" \
       | grep -v '^[[:space:]]*#')

  # Capability probe, not a version check.
  printf '%s' "${fn}" | grep -q 'help styleguide:generate' \
    || fail "the frontend step must probe for the styleguide command"
  printf '%s' "${fn}" | grep -qE '1[234]\.4|version.*=' \
    && fail "the choice must be made by probing, not by matching a version"

  # No generator -> return cleanly, no DB write. The early return sits before any
  # generate or db_site_sql.
  local skip_line gen_line db_line
  skip_line=$(printf '%s' "${fn}" | grep -n 'return 0' | head -1 | cut -d: -f1)
  gen_line=$(printf '%s' "${fn}" | grep -n 'styleguide:generate frontend --create' | head -1 | cut -d: -f1)
  db_line=$(printf '%s' "${fn}" | grep -n 'db_site_sql' | head -1 | cut -d: -f1)
  [ -n "${gen_line}" ] || fail "it must run the styleguide frontend generator"
  [ -n "${db_line}" ] || fail "it must reveal the hidden styleguide root"
  [ "${skip_line}" -lt "${gen_line}" ] \
    || fail "a missing generator must return before generating"
  [ "${gen_line}" -lt "${db_line}" ] \
    || fail "the DB write must come after — and be gated on — the generate"

  # The unhide targets the site's OWN database and exactly styleguide's own root.
  printf '%s' "${fn}" | grep -q 'db_site_sql "${db}"' \
    || fail "the unhide must run against the site's own database, not the default"
  printf '%s' "${fn}" | grep -q "tx_styleguide_containsdemo='tx_styleguide_frontend_root'" \
    || fail "the unhide must match styleguide's root marker, nothing broader"
}

@test "db_site_sql targets a named database on both engines" {
  set -eu -o pipefail
  # db_root_sql connects to the server default (postgres / no db), right for CREATE
  # DATABASE but wrong for a site's own tables — every served site has its own db.
  local fn
  fn=$(sed -n '/^db_site_sql()/,/^}/p' "${DIR}/tryout/functions.sh")
  printf '%s' "${fn}" | grep -q 'psql -h db -U db -d "${db}"' \
    || fail "postgres must connect to the named database"
  printf '%s' "${fn}" | grep -q 'mysql -h db -uroot -proot -D "${db}"' \
    || fail "mysql must select the named database"
}
