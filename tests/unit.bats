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

  run names worktree use "''"
  assert_line "--force"
}

@test "completion lists the worktrees on disk" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"

  run names worktree use "''"
  assert_success
  assert_line "main"
  assert_line "v13"
}

@test "completion lists served sites for the commands that take one" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"

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

  run names worktree use "''"
  assert_success
  assert_line "--force"
}

# --- herdr integration ----------------------------------------------------
# `ddev tryout herdr` opens one herdr tab per Core worktree. herdr is an optional
# host tool, so these tests cover the pure helpers and the guard clauses only —
# real tab creation mutates a live session and is verified by hand.

@test "herdr agent names are sanitised to herdr's grammar" {
  # Worktree names allow uppercase and dots; agent names must match
  # [a-z][a-z0-9_-]{0,31}.
  set -eu -o pipefail
  run helper herdr_agent_name main
  assert_output "main"

  run helper herdr_agent_name v13
  assert_output "v13"

  run helper herdr_agent_name my.branch
  assert_output "my-branch"

  run helper herdr_agent_name Feature-X
  assert_output "feature-x"

  # Must start with a letter.
  run helper herdr_agent_name 13.4
  assert_output "x13-4"

  # And be at most 32 characters.
  run helper_eval 'herdr_agent_name a-very-long-worktree-name-that-goes-past-the-limit | wc -c'
  assert_output --partial "33"   # 32 chars + newline
}

@test "the herdr session is named after the DDEV project" {
  # Each project gets its own session, so two tryout projects never collide.
  set -eu -o pipefail
  run helper_eval 'DDEV_SITENAME=myproj herdr_session_name'
  assert_output "tryout-myproj"

  # setup() exports DDEV_SITENAME, so clear it in a child env rather than in-shell.
  run env -u DDEV_SITENAME bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    herdr_session_name
  "
  assert_output "tryout"
}

@test "herdr_cli puts --session before the subcommand" {
  # herdr SILENTLY IGNORES --session when it comes after the subcommand and talks to
  # the default session instead — so argument order is load-bearing, not cosmetic.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/bin"
  printf '#!/usr/bin/env bash\necho "ARGV: $*"\n' > "${FAKEROOT}/bin/herdr"
  chmod +x "${FAKEROOT}/bin/herdr"

  run env DDEV_SITENAME=myproj PATH="${FAKEROOT}/bin:${PATH}" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    herdr_cli workspace list
  "
  assert_success
  assert_output "ARGV: --session tryout-myproj workspace list"
}

@test "every herdr call goes through the session wrapper" {
  # A bare `herdr <subcommand>` would silently target the user's default session.
  # The only allowed raw call is the deliberate `nohup herdr --session ... server`.
  set -eu -o pipefail
  # Only look at executable lines: strip comments and echo/error strings first, so
  # help text and messages mentioning herdr do not register as calls.
  run bash -c "
    cat '${DIR}/tryout/functions.sh' '${DIR}/commands/host/tryout' \
      | grep -vE '^[[:space:]]*#' \
      | grep -vE '^[[:space:]]*(echo|printf|error|warn|info|success)\b' \
      | grep -E '(^|[^_[:alnum:]])herdr (workspace|pane|agent|tab|status|session) ' \
      | grep -v 'herdr_cli' \
      | grep -v 'herdr session attach' || true
  "
  assert_output ""
}

@test "attaching falls back to printing the command when it cannot attach" {
  # Three ways attaching is impossible: no controlling terminal (CI, a script), or
  # already inside herdr, which refuses to nest. Neither may hang or fail.
  set -eu -o pipefail

  # Inside herdr: say how to switch, do not try to nest.
  run env HERDR_ENV=1 DDEV_SITENAME=myproj bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    attach_herdr_session
  "
  assert_success
  assert_output --partial "already in herdr"
  assert_output --partial "tryout-myproj"

  # No controlling terminal: print the command. bats already runs without one.
  run env -u HERDR_ENV DDEV_SITENAME=myproj bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    attach_herdr_session
  "
  assert_success
  assert_output --partial "herdr session attach tryout-myproj"
}

@test "the herdr command reports a missing herdr binary" {
  set -eu -o pipefail
  run bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    export HERDR_ENV=1 HERDR_WORKSPACE_ID=w1
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    PATH=/nonexistent
    herdr_available
  "
  assert_failure
  assert_output --partial "herdr is not installed"
  assert_output --partial "herdr.dev"
}

@test "herdr workspace labels are namespaced" {
  # Workspace labels share one global sidebar with every other project, so a bare
  # worktree name would be ambiguous there.
  set -eu -o pipefail
  run helper herdr_workspace_label main
  assert_output "core-main"

  run helper herdr_workspace_label v13
  assert_output "core-v13"
}

@test "the herdr keybinding round-trips the config byte for byte" {
  # This edits a file outside .ddev/ that holds the user's own settings and that the
  # add-on's removal actions cannot reach, so setup/unsetup MUST be exact. Every
  # shape below broke a naive implementation during development.
  set -eu -o pipefail
  local case_name content
  for case_name in normal no-newline trailing-blanks many-blanks empty; do
    case "${case_name}" in
      normal)          content=$'a = 1\n[ui]\nx = 2\n' ;;
      no-newline)      content=$'a = 1' ;;
      trailing-blanks) content=$'a = 1\n\n' ;;
      many-blanks)     content=$'a = 1\n\n\n\n' ;;
      empty)           content='' ;;
    esac
    printf '%s' "${content}" > "${FAKEROOT}/cfg.toml"
    cp "${FAKEROOT}/cfg.toml" "${FAKEROOT}/cfg.before"

    run env DDEV_SITENAME=myproj HERDR_CONFIG="${FAKEROOT}/cfg.toml" bash -c "
      export DDEV_APPROOT='${FAKEROOT}'
      source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
      command -v herdr >/dev/null 2>&1 || exit 0
      herdr_setup_keys true >/dev/null 2>&1
      grep -q 'new_worktree' '${FAKEROOT}/cfg.toml' || exit 1
      herdr_unsetup_keys >/dev/null 2>&1
    "
    assert_success

    run diff "${FAKEROOT}/cfg.before" "${FAKEROOT}/cfg.toml"
    assert_success
  done
}

@test "setup-keys keeps a single global block and backs the config up" {
  # The popups resolve the project from the cwd, so one block serves every
  # project. A second run from the same project changes nothing.
  set -eu -o pipefail
  printf 'a = 1\n' > "${FAKEROOT}/cfg.toml"
  mkdir -p "${FAKEROOT}/.ddev/tryout" && : > "${FAKEROOT}/.ddev/tryout/herdr-menu.sh"

  run env DDEV_SITENAME=myproj HERDR_CONFIG="${FAKEROOT}/cfg.toml" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    command -v herdr >/dev/null 2>&1 || skip 'herdr not installed'
    herdr_setup_keys true >/dev/null 2>&1
    herdr_setup_keys true 2>&1 | grep -c 'already in'
    grep -c '>>> tryout' '${FAKEROOT}/cfg.toml'
    grep -c 'tryout myproj' '${FAKEROOT}/cfg.toml' || true
  "
  assert_success
  # second run said "already in"; one block; and it carries no project name.
  assert_output $'1\n1\n0'

  # The backup must hold the pre-write content.
  run bash -c "cat '${FAKEROOT}'/cfg.toml.tryout-backup-*"
  assert_success
  assert_output "a = 1"
}

@test "setup-keys folds per-project blocks into the one global block" {
  # Older versions wrote "# >>> tryout <project> >>>" once per project, so two
  # projects bound the same three keys twice. Both legacy blocks must go, exactly
  # one global block must remain, and unsetup must still give the file back byte
  # for byte.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/.ddev/tryout" && : > "${FAKEROOT}/.ddev/tryout/herdr-menu.sh"
  local legacy proj
  {
    printf 'a = 1\n'
    for proj in oldproj otherproj; do
      printf '\n# >>> tryout %s >>> newline_added=false blank_added=true\n' "${proj}"
      printf '[[keys.command]]\nkey = "prefix+shift+t"\ncommand = "/gone/%s/.ddev/tryout/herdr-menu.sh"\n' "${proj}"
      printf '# <<< tryout %s <<<\n' "${proj}"
    done
  } > "${FAKEROOT}/cfg.toml"

  run env DDEV_SITENAME=myproj HERDR_CONFIG="${FAKEROOT}/cfg.toml" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    command -v herdr >/dev/null 2>&1 || skip 'herdr not installed'
    herdr_setup_keys true 2>&1 | grep -o 'Merged 2 tryout block'
    grep -c '>>> tryout' '${FAKEROOT}/cfg.toml'
    grep -c 'oldproj\|otherproj' '${FAKEROOT}/cfg.toml' || true
    grep -c '${FAKEROOT}/.ddev/tryout/herdr-menu.sh' '${FAKEROOT}/cfg.toml'
    herdr_unsetup_keys >/dev/null 2>&1
    cat '${FAKEROOT}/cfg.toml'
  "
  assert_success
  assert_output $'Merged 2 tryout block\n1\n0\n1\na = 1'
}

@test "setup-keys replaces a block whose project is gone" {
  # A block pointing at a deleted project is a dead key in every session.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/.ddev/tryout" && : > "${FAKEROOT}/.ddev/tryout/herdr-menu.sh"
  {
    printf 'a = 1\n\n# >>> tryout >>> newline_added=false blank_added=true\n'
    printf '[[keys.command]]\ncommand = "/gone/.ddev/tryout/herdr-menu.sh"\n# <<< tryout <<<\n'
  } > "${FAKEROOT}/cfg.toml"

  run env DDEV_SITENAME=myproj HERDR_CONFIG="${FAKEROOT}/cfg.toml" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    command -v herdr >/dev/null 2>&1 || skip 'herdr not installed'
    herdr_setup_keys true >/dev/null 2>&1
    grep -c '/gone/' '${FAKEROOT}/cfg.toml' || true
    grep -c '${FAKEROOT}/.ddev/tryout/herdr-menu.sh' '${FAKEROOT}/cfg.toml'
  "
  assert_success
  assert_output $'0\n1'
}

@test "the popup script trusts the cwd, not its own location" {
  # The key is bound globally, so \$0 always points at whichever project ran
  # setup-keys. Trusting it would create a worktree in a project the user is
  # nowhere near.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/.ddev/tryout"
  cp "${DIR}/tryout/herdr-new-worktree.sh" "${FAKEROOT}/.ddev/tryout/"
  chmod +x "${FAKEROOT}/.ddev/tryout/herdr-new-worktree.sh"

  # Run it from outside any project: it must refuse rather than resolve via \$0.
  run bash -c "cd / && printf '' | '${FAKEROOT}/.ddev/tryout/herdr-new-worktree.sh' 2>&1"
  assert_output --partial "No DDEV project here"
}

@test "the menu covers every command the dispatch case accepts" {
  # The menu IS the GUI — herdr allows no plugin menu entries — so a new subcommand
  # that never reaches it is invisible to anyone driving tryout from herdr.
  set -eu -o pipefail
  local actions verb
  actions=$(sed -n '/^case "${ACTION}" in/,/^esac/p' "${DIR}/commands/host/tryout" \
    | sed -n 's/^    \([a-z|]*\)).*/\1/p' | tr '|' '\n' | grep -v '^\*$')
  [ -n "${actions}" ]

  for verb in ${actions}; do
    grep -q -- "${verb}" "${DIR}/tryout/herdr-menu.sh" \
      || { echo "menu is missing '${verb}'"; false; }
  done
}

@test "the menu covers the worktree and cs subcommands too" {
  # Checking only the top-level verbs is what let `worktree adopt` ship without a
  # menu entry: `worktree` matched, the subcommand went unnoticed.
  set -eu -o pipefail
  local subs sub

  # worktree's own case labels, minus the aliases and help.
  subs=$(sed -n '/^cmd_worktree()/,/^}$/p' "${DIR}/commands/host/tryout" \
    | sed -n 's/^        \([a-z|]*\))$/\1/p' | tr '|' '\n' \
    | grep -vE '^(rm|help)$')
  [ -n "${subs}" ]
  for sub in ${subs}; do
    grep -qE "worktree ${sub}\b" "${DIR}/tryout/herdr-menu.sh" \
      || { echo "menu is missing 'worktree ${sub}'"; false; }
  done

  # cs: setup, doctor and uninstall must all be reachable.
  for sub in setup doctor uninstall; do
    grep -qE "cs ${sub}\b" "${DIR}/tryout/herdr-menu.sh" \
      || { echo "menu is missing 'cs ${sub}'"; false; }
  done
}

@test "the menu keeps destructive commands behind a confirmation" {
  set -eu -o pipefail
  # Each destructive path must call confirm_destructive before running anything.
  run grep -c 'confirm_destructive' "${DIR}/tryout/herdr-menu.sh"
  assert_success
  [ "${output}" -ge 3 ]

  # And the confirmation must compare against the typed name, not just prompt.
  run grep -q 'answer.*}" = "\${expect}' "${DIR}/tryout/herdr-menu.sh"
  assert_success
}

@test "the menu script trusts the cwd, not its own location" {
  # Same global-keybinding hazard as the new-worktree popup.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/.ddev/tryout"
  cp "${DIR}/tryout/herdr-menu.sh" "${FAKEROOT}/.ddev/tryout/"
  chmod +x "${FAKEROOT}/.ddev/tryout/herdr-menu.sh"

  run bash -c "cd / && printf '' | '${FAKEROOT}/.ddev/tryout/herdr-menu.sh' 2>&1"
  assert_output --partial "No DDEV project here"
}

@test "setup-keys binds the worktree, menu and dashboard keys" {
  set -eu -o pipefail
  printf 'a = 1\n' > "${FAKEROOT}/cfg.toml"

  run env DDEV_SITENAME=myproj HERDR_CONFIG="${FAKEROOT}/cfg.toml" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    command -v herdr >/dev/null 2>&1 || skip 'herdr not installed'
    herdr_setup_keys true >/dev/null 2>&1
    grep -c 'keys.command' '${FAKEROOT}/cfg.toml'
  "
  assert_success
  assert_output "3"

  run grep -q 'prefix+shift+t' "${FAKEROOT}/cfg.toml"
  assert_success
  run grep -q 'prefix+shift+d' "${FAKEROOT}/cfg.toml"
  assert_success
}

@test "the relocation hook never touches a worktree outside a tryout project" {
  # THE safety property: this hook fires for EVERY worktree herdr creates, on any
  # repository on the machine. Moving someone else's checkout would be data loss.
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'
  command -v jq  >/dev/null 2>&1 || skip 'jq not available'

  mkdir -p "${FAKEROOT}/other"
  git -C "${FAKEROOT}/other" init -q .
  git -C "${FAKEROOT}/other" commit -q --allow-empty -m init
  git -C "${FAKEROOT}/other" worktree add -q "${FAKEROOT}/other-wt" -b probe

  run env HERDR_PLUGIN_EVENT_JSON="$(jq -nc --arg p "${FAKEROOT}/other-wt" \
      '{worktree:{path:$p,branch:"probe"},workspace:{workspace_id:"w9"}}')" \
    bash "${DIR}/tryout/herdr-plugin/relocate.sh"
  assert_success

  # Still exactly where git put it.
  assert_dir_exist "${FAKEROOT}/other-wt"
}

@test "the relocation hook leaves a correctly-placed worktree alone" {
  set -eu -o pipefail
  command -v jq >/dev/null 2>&1 || skip 'jq not available'
  mkdir -p "${FAKEROOT}/.ddev/tryout" "${FAKEROOT}/typo3-core-v13"
  cp "${DIR}/tryout/functions.sh" "${FAKEROOT}/.ddev/tryout/"

  run env HERDR_PLUGIN_EVENT_JSON="$(jq -nc --arg p "${FAKEROOT}/typo3-core-v13" \
      '{worktree:{path:$p,branch:"13.4"},workspace:{workspace_id:"w9"}}')" \
    bash "${DIR}/tryout/herdr-plugin/relocate.sh"
  assert_success
  assert_dir_exist "${FAKEROOT}/typo3-core-v13"
}

@test "worktree names are derived from a branch and sanitised" {
  # herdr names its own checkouts worktree/<generated-slug>; tryout needs a name
  # validate_worktree_name accepts.
  set -eu -o pipefail
  run helper worktree_name_from_ref "worktree/brave-harbor-dc20"
  assert_output "brave-harbor-dc20"

  run helper worktree_name_from_ref "feature/foo"
  assert_output "feature-foo"

  run helper worktree_name_from_ref "typo3-core-v13"
  assert_output "v13"

  run helper worktree_name_from_ref "13.4"
  assert_output "13.4"
}

@test "foreign worktrees are the ones outside the project root" {
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'
  mkdir -p "${FAKEROOT}/typo3-core"
  git -C "${FAKEROOT}/typo3-core" init -q .
  git -C "${FAKEROOT}/typo3-core" commit -q --allow-empty -m init
  git -C "${FAKEROOT}/typo3-core" worktree add -q "${FAKEROOT}/typo3-core-inside" -b inside
  git -C "${FAKEROOT}/typo3-core" worktree add -q "${BATS_TMPDIR}/tryout-outside-$$" -b outside

  run helper_eval 'list_foreign_core_worktrees'
  assert_success
  refute_output --partial "typo3-core-inside"
  assert_output --partial "tryout-outside-$$"

  git -C "${FAKEROOT}/typo3-core" worktree remove --force "${BATS_TMPDIR}/tryout-outside-$$" || true
}

@test "the guard plugin manifest declares the worktree hook" {
  set -eu -o pipefail
  local m="${DIR}/tryout/herdr-plugin/herdr-plugin.toml"
  assert_file_exist "${m}"

  run grep -q 'id = "tryout.worktree-guard"' "${m}"
  assert_success
  run grep -q 'on = "worktree.created"' "${m}"
  assert_success
  run grep -q 'min_herdr_version' "${m}"
  assert_success
  run grep -q 'ddev-generated' "${m}"
  assert_success
}

@test "a job records its command, pane and exit code" {
  set -eu -o pipefail
  command -v jq >/dev/null 2>&1 || skip 'jq not available'
  mkdir -p "${FAKEROOT}/bin" "${FAKEROOT}/.ddev"
  cat > "${FAKEROOT}/bin/herdr" <<'STUB'
#!/usr/bin/env bash
case "$*" in
  *"pane split"*) echo '{"result":{"pane":{"pane_id":"w1:p9"}}}' ;;
  *"pane run"*)   printf '%s' "$*" > "${CAPTURE:-/dev/null}" ;;
esac
exit 0
STUB
  chmod +x "${FAKEROOT}/bin/herdr"

  run env PATH="${FAKEROOT}/bin:${PATH}" CAPTURE="${FAKEROOT}/wrapper.txt" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    start_tryout_job 'worktree serve v13' worktree serve v13
  "
  assert_success
  local job_id="${output}"
  [ -n "${job_id}" ]

  assert_file_exist "${FAKEROOT}/.ddev/.tryout-jobs/${job_id}.cmd"
  assert_file_exist "${FAKEROOT}/.ddev/.tryout-jobs/${job_id}.pane"

  # The wrapper is what makes the outcome observable: it must record the exit code.
  run cat "${FAKEROOT}/wrapper.txt"
  assert_output --partial "${job_id}.rc"
  assert_output --partial "ddev tryout worktree serve v13"
  assert_output --partial "notification show"
}

@test "job status reports running, ok and failed" {
  set -eu -o pipefail
  local j="${FAKEROOT}/.ddev/.tryout-jobs"
  mkdir -p "${j}"
  printf 'worktree serve v13\n' > "${j}/1-a.cmd"

  run helper_eval 'tryout_jobs_status'
  assert_output --partial "running"

  printf '0' > "${j}/1-a.rc"
  run helper_eval 'tryout_jobs_status'
  assert_output --partial "ok"

  printf '1' > "${j}/1-a.rc"
  run helper_eval 'tryout_jobs_status'
  assert_output --partial "failed"
  assert_output --partial "exit 1"
}

@test "the dashboard only makes instant calls" {
  # It redraws on a timer in a pane the user is watching. One network or container
  # call would freeze it, so none may appear at all.
  set -eu -o pipefail
  local bad
  grep -vE '^[[:space:]]*#' "${DIR}/tryout/herdr-dashboard.sh" > "${FAKEROOT}/dash.code"
  for bad in 'ls-remote' 'ddev exec' 'ddev composer' 'ddev typo3' \
             'diagnose_gerrit_ssh' 'inspect_author_identity' \
             'detect_detached_base_branch' 'curl' 'ssh '; do
    run grep -q -- "${bad}" "${FAKEROOT}/dash.code"
    assert_failure
  done

  # `git ... fetch|pull|push` in any argument order — the inserted -C makes a plain
  # 'git fetch' substring match useless.
  run grep -qE 'git .*(fetch|pull|push|ls-remote)' "${FAKEROOT}/dash.code"
  assert_failure
}

@test "the dashboard never mutates the project" {
  set -eu -o pipefail
  local bad
  grep -vE '^[[:space:]]*#' "${DIR}/tryout/herdr-dashboard.sh" > "${FAKEROOT}/dash.code"
  for bad in 'rm -rf' 'git reset' 'git checkout' 'worktree add' 'worktree remove' \
             'worktree move' 'start_tryout_job'; do
    run grep -q -- "${bad}" "${FAKEROOT}/dash.code"
    assert_failure
  done
}

@test "the menu refuses to run without jq" {
  # Without jq the pane id cannot be parsed and a multi-minute command would run
  # inside the modal popup instead.
  set -eu -o pipefail
  run grep -q 'command -v jq' "${DIR}/tryout/herdr-menu.sh"
  assert_success
}

@test "every destructive menu entry asks first" {
  # reset and checkout both run `git reset --hard` + `clean -fd`.
  set -eu -o pipefail
  local m="${DIR}/tryout/herdr-menu.sh"
  run grep -c 'confirm_destructive' "${m}"
  assert_success
  [ "${output}" -ge 5 ]

  # delete must not prompt twice: the menu confirms, so the command is told not to.
  run grep -q 'run_in_pane delete .* --yes' "${m}"
  assert_success
}

@test "the menu treats ESC as back, not as quit" {
  # ESC used to fall into the catch-all and close the whole popup from a submenu.
  set -eu -o pipefail
  run grep -qE "\\\$'\\\\e'\\)" "${DIR}/tryout/herdr-menu.sh"
  assert_success

  # and every menu reads through the one helper, so the mapping applies everywhere
  run grep -c 'read_key' "${DIR}/tryout/herdr-menu.sh"
  assert_success
  [ "${output}" -ge 4 ]
}

@test "the menu loops instead of exiting after each action" {
  set -eu -o pipefail
  # The loop itself.
  run grep -qE 'while :; do' "${DIR}/tryout/herdr-menu.sh"
  assert_success
  run grep -q 'main_menu || break' "${DIR}/tryout/herdr-menu.sh"
  assert_success

  # The action helpers must return, not exit — that was the old one-shot shape.
  run bash -c "
    sed -n '/^run_and_show()/,/^}\$/p;/^run_in_pane()/,/^}\$/p' \
      '${DIR}/tryout/herdr-menu.sh' | grep -c 'exit 0' || true
  "
  assert_output "0"
}

@test "the output view reports a closed pane instead of raw JSON" {
  # `herdr pane read` answers with a JSON error AND exit code 0 when the pane is
  # gone, so the exit status cannot be trusted — the text is the only signal.
  set -eu -o pipefail
  run bash -c '
    text="{\"error\":{\"code\":\"pane_not_found\",\"message\":\"gone\"}}"
    case "${text}" in
        *pane_not_found*) echo detected ;;
        *) echo missed ;;
    esac
  '
  assert_output "detected"

  # and the menu actually implements that check
  run grep -q 'pane_not_found' "${DIR}/tryout/herdr-menu.sh"
  assert_success
  run grep -q 'has been closed' "${DIR}/tryout/herdr-menu.sh"
  assert_success
}

@test "the output view clamps scrolling at both ends" {
  set -eu -o pipefail
  # g goes to the top, G to the last screenful, and neither runs past the array.
  run grep -qE 'g\)\s+top=0' "${DIR}/tryout/herdr-menu.sh"
  assert_success
  run grep -q 'top}" -lt 0 ] && top=0' "${DIR}/tryout/herdr-menu.sh"
  assert_success
  run grep -qE 'top \+ rows \)\) -lt "\$\{#lines\[@\]\}"' "${DIR}/tryout/herdr-menu.sh"
  assert_success
}

@test "a missing herdr says how to install it, per platform" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/empty"

  # macOS gets brew; the universal installer is always offered because no Linux
  # distro packages herdr — an apt-get line would just fail.
  run env OSTYPE=darwin24 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    PATH='${FAKEROOT}/empty'
    herdr_available
  "
  assert_failure
  assert_output --partial "herdr is not installed"
  assert_output --partial "brew install herdr"
  assert_output --partial "herdr.dev/install.sh"

  run env OSTYPE=linux-gnu bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    PATH='${FAKEROOT}/empty'
    herdr_available
  "
  assert_failure
  assert_output --partial "herdr.dev/install.sh"
  # herdr is not in apt/dnf/pacman — never claim otherwise.
  refute_output --partial "apt-get install herdr"
}

@test "a missing jq says how to install it, and needs no uname" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/onlyherdr"
  printf '#!/bin/sh\nexit 0\n' > "${FAKEROOT}/onlyherdr/herdr"
  chmod +x "${FAKEROOT}/onlyherdr/herdr"

  # No uname on this PATH: the platform must come from OSTYPE instead.
  run env OSTYPE=darwin24 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    PATH='${FAKEROOT}/onlyherdr'
    herdr_available
  "
  assert_failure
  assert_output --partial "jq is not installed"
  assert_output --partial "brew install jq"
  refute_output --partial "uname: command not found"

  run env OSTYPE=linux-gnu bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    PATH='${FAKEROOT}/onlyherdr'
    herdr_available
  "
  assert_failure
  assert_output --partial "jq is not installed"
}

@test "no fractional read -t, which bash 3.2 rejects outright" {
  # `read -t 0.01` is bash 4+. On macOS's bash 3.2 it fails with "invalid timeout
  # specification" — so a drain written that way never runs, and every prompt after
  # a single-key read silently swallows the stale Enter and cancels. Parsing is not
  # enough to catch this: it is a runtime argument error.
  set -eu -o pipefail
  run bash -c "
    cat '${DIR}'/tryout/*.sh '${DIR}'/tryout/herdr-plugin/*.sh \
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
  for f in "${DIR}"/tryout/*.sh "${DIR}"/tryout/herdr-plugin/*.sh \
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
      cat '${DIR}'/tryout/*.sh '${DIR}'/tryout/herdr-plugin/*.sh \
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

  mkdir -p "${FAKEROOT}/typo3-core-capped" "${FAKEROOT}/typo3-core-open" \
           "${FAKEROOT}/typo3-core-nojson"
  printf '{"require":{"php":">=8.2 <8.4"}}' > "${FAKEROOT}/typo3-core-capped/composer.json"
  printf '{"require":{"php":"^8.2"}}'       > "${FAKEROOT}/typo3-core-open/composer.json"
  printf '{}'                               > "${FAKEROOT}/typo3-core-nojson/composer.json"

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

@test "job ids do not collide within the same second" {
  # $$ is constant for the menu process and date is second-granular, so two jobs
  # launched together shared an id: the second overwrote the first's .rc, and a
  # FAILED job could be reported ok — the exact thing job tracking exists to stop.
  set -eu -o pipefail
  run grep -q 'date +%s)-\$\$-\${RANDOM}' "${DIR}/tryout/functions.sh"
  assert_success
  # and it retries rather than trusting RANDOM to be unique
  run grep -q 'while \[ -e "\${TRYOUT_JOBS_DIR}/\${id}.cmd" \]' "${DIR}/tryout/functions.sh"
  assert_success
}

@test "a failed serve does not leave the site marked as served" {
  # .tryout-site IS the definition of served, so writing it before the work made a
  # half-built site look real to worktree list, the dashboard and delete --all.
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
  mkdir -p "${FAKEROOT}/bin" "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"
  cat > "${FAKEROOT}/bin/php8.2" <<'FAKE'
#!/bin/sh
printf 'cwd=%s\n' "$(pwd)"
printf 'db=%s site=%s\n' "${TYPO3_DB_DBNAME}" "${TRYOUT_SITE}"
for a in "$@"; do printf 'arg=[%s]\n' "$a"; done
FAKE
  chmod +x "${FAKEROOT}/bin/php8.2"
  run helper_eval "PATH='${FAKEROOT}/bin:${PATH}' site_exec v13 vendor/bin/typo3 config:set X 'My Site'"
  assert_success
  assert_line "cwd=${FAKEROOT}/sites/v13"
  assert_line "db=db_v13 site=v13"
  assert_line "arg=[vendor/bin/typo3]"
  assert_line "arg=[config:set]"
  assert_line "arg=[X]"
  assert_line "arg=[My Site]"
}

@test "a worktree can be renamed without touching its branch" {
  # herdr's own New-worktree action names a checkout after the branch it invents
  # (worktree/wilie-wonka), and adopt derives one the same way. That is a starting
  # point, not a commitment: the directory name and the branch are independent.
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'

  git -C "${FAKEROOT}" init -q .
  git -C "${FAKEROOT}" commit -q --allow-empty -m init
  mkdir -p "${FAKEROOT}/typo3-core"
  git -C "${FAKEROOT}" worktree add -q "${FAKEROOT}/typo3-core-wilie-wonka" -b wilie-wonka

  run helper_eval 'rename_core_worktree wilie-wonka experiment'
  assert_success

  assert_dir_exist "${FAKEROOT}/typo3-core-experiment"
  assert_dir_not_exist "${FAKEROOT}/typo3-core-wilie-wonka"

  # The branch is the point: it must survive the rename untouched.
  run bash -c "git -C '${FAKEROOT}/typo3-core-experiment' branch --show-current"
  assert_output "wilie-wonka"
}

@test "rename refuses a name that is already taken" {
  set -eu -o pipefail
  command -v git >/dev/null 2>&1 || skip 'git not available'
  git -C "${FAKEROOT}" init -q .
  git -C "${FAKEROOT}" commit -q --allow-empty -m init
  git -C "${FAKEROOT}" worktree add -q "${FAKEROOT}/typo3-core-a" -b a
  mkdir -p "${FAKEROOT}/typo3-core-b"

  run helper_eval 'rename_core_worktree a b'
  assert_failure
  assert_output --partial "already exists"
  assert_dir_exist "${FAKEROOT}/typo3-core-a"
}

@test "adopt takes an optional name so the branch does not dictate the path" {
  set -eu -o pipefail
  run grep -q 'adopt \[<path> \[<name>\]\]' "${DIR}/commands/host/tryout"
  assert_success
  # and rename is offered alongside it
  run grep -q 'rename <old> <new>' "${DIR}/commands/host/tryout"
  assert_success
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
  # with `ddev tryout herd<TAB>` argv is `tryout herd`, so $2 IS the partial word.
  # It matched no case, printed nothing, and zsh fell back to file completion.
  # cobra filters candidates against the partial word itself, so returning the full
  # list is correct.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"

  run names herd
  assert_success
  assert_line "herdr"

  run names cs doc
  assert_success
  assert_line "doctor"

  run names worktree us
  assert_success
  assert_line "use"

  run names worktree use ma
  assert_success
  assert_line "main"

  # An empty word must keep working too.
  run names "''"
  assert_success
  assert_line "herdr"
}

@test "completion offers herdr, its worktrees and its flags" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"

  run names "''"
  assert_success
  assert_line "herdr"

  run names herdr "''"
  assert_success
  assert_line "main"
  assert_line "v13"
  assert_line "--no-agent"
  assert_line "--no-focus"
}

@test "completion describes every candidate or explains the free-text word" {
  # DDEV passes each line to cobra verbatim, so `value<TAB>description` renders
  # as two columns and `_activeHelp_ text` as a hint. A bare word would look like
  # a regression in zsh: no description beside it.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"
  local args line
  for args in "''" "worktree ''" "worktree add ''" "worktree add x ''" "worktree use ''" \
              "worktree serve ''" "worktree list ''" "cs ''" "cs setup ''" "herdr ''" \
              "herdr new ''" "checkout ''" "patch ''" "exec ''" "exec v13 ''" \
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
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13" "${FAKEROOT}/typo3-core-v12"
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"

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
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"

  run names worktree use "''"
  assert_success
  assert_line "v13"
  refute_line "main"

  run names worktree remove "''"
  refute_output --partial "main"

  # Elsewhere the primary is offered, and labelled.
  run complete herdr "''"
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
  mkdir -p "${FAKEROOT}/typo3-core-main"
  run names worktree use --
  assert_success
  assert_line "--force"
  refute_line "main"
}

@test "completion offers PHP versions after --php" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-v13"
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

@test "completion covers every worktree, cs and herdr subcommand the command dispatches" {
  set -eu -o pipefail
  local subs sub
  for verb in worktree cs herdr; do
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
  for n in 1 2 3 4 5 6; do mkdir -p "${FAKEROOT}/typo3-core-wt${n}"; done
  start=$(date +%s)
  complete worktree use "''" >/dev/null
  complete herdr "''" >/dev/null
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

@test "worktree list --plain is the parseable contract, the default is a table" {
  set -eu -o pipefail

  # tests/e2e/login.spec.ts discovers served sites by regex over --plain. The
  # bordered default table does not match it, so the flag must keep working.
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

@test "the herdr menu loads functions.sh outside a ddev command" {
  set -eu -o pipefail

  # functions.sh derives PROJECT_ROOT from DDEV_APPROOT, which DDEV exports only
  # to its own commands. The menu is launched by herdr, so without setting it the
  # library aborts under `set -u` and the popup dies with no output at all.
  run grep -q 'export DDEV_APPROOT=' "${DIR}/tryout/herdr-menu.sh"
  assert_success

  # And it must be set before the library is sourced, not after.
  local export_line source_line
  export_line=$(grep -n 'export DDEV_APPROOT=' "${DIR}/tryout/herdr-menu.sh" | head -1 | cut -d: -f1)
  source_line=$(grep -n '^\. "\${APPROOT}/.ddev/tryout/functions.sh"' "${DIR}/tryout/herdr-menu.sh" | head -1 | cut -d: -f1)
  [ -n "${export_line}" ] && [ -n "${source_line}" ]
  [ "${export_line}" -lt "${source_line}" ]
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
  run grep -nE 'gum (choose|filter|input) .*2>/dev/null' "${DIR}/tryout/functions.sh"
  assert_failure
}

@test "core_worktree_names filters by primary and served state" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13" "${FAKEROOT}/typo3-core-v12" "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"

  run helper core_worktree_names all
  assert_line "main"; assert_line "v13"; assert_line "v12"

  run helper core_worktree_names nonprimary
  refute_line "main"; assert_line "v13"

  run helper core_worktree_names served
  assert_output "v13"

  run helper core_worktree_names unserved
  assert_line "main"; assert_line "v12"; refute_line "v13"
}

@test "ask_worktree takes a piped answer and fails cleanly with none" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"

  run helper_eval 'printf "v13\n" | ask_worktree "which?" all'
  assert_success
  assert_output "v13"

  run helper_eval 'ask_worktree "which?" all </dev/null'
  assert_failure
  refute_output --partial "v13"
}

@test "ask_worktree explains when there is nothing to choose from" {
  set -eu -o pipefail
  run helper_eval 'ask_worktree "which?" all </dev/null'
  assert_failure
  assert_output --partial "No worktree to choose from"
  assert_output --partial "worktree add"
}

@test "ask_branch puts main first and legacy refs last" {
  set -eu -o pipefail
  # A fake list_local_core_branches; ask_branch only orders what it gets.
  run helper_eval '
    list_local_core_branches() { printf "%s\n" 9.5 TYPO3_8-7 13.4 main 14.3 12.4; }
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

  mkdir -p "${FAKEROOT}/typo3-core" "${FAKEROOT}/typo3-core-v13"
  printf '{"require":{"php":"^8.5"}}' > "${FAKEROOT}/typo3-core/composer.json"
  printf '{"require":{"php":"^8.2"}}' > "${FAKEROOT}/typo3-core-v13/composer.json"

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
    check_php_for_core '${FAKEROOT}/typo3-core-v13' 8.1 v13
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

  mkdir -p "${FAKEROOT}/typo3-core" "${FAKEROOT}/typo3-core-bare"
  printf '{"require":{"php":"^8.5"}}' > "${FAKEROOT}/typo3-core/composer.json"
  printf '{}' > "${FAKEROOT}/typo3-core-bare/composer.json"

  run env DDEV_PHP_VERSION=8.5 bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    check_php_for_core && check_php_for_core '${FAKEROOT}/typo3-core-bare' 8.1 \
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

@test "the single Core checkout of a fresh project opens in herdr under its branch" {
  # Before any worktree exists there is only typo3-core/ itself. It must resolve
  # to that directory under the name the worktree layout gives it later, so the
  # herdr label survives the migration; on the symlink layout the name maps to
  # typo3-core-<name> as before.
  set -eu -o pipefail
  git init -q -b main "${FAKEROOT}/typo3-core"

  run bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    printf '%s|%s|%s\n' \"\$(plain_core_name)\" \"\$(herdr_checkout_dir main)\" \"\$(herdr_checkout_dir v13)\"
  "
  assert_success
  assert_output "main|${FAKEROOT}/typo3-core|${FAKEROOT}/typo3-core-v13"

  # A branch name that is no valid worktree name falls back to the default.
  git -C "${FAKEROOT}/typo3-core" checkout -q -b 'feature/x' 2>/dev/null
  run bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    plain_core_name
  "
  assert_success
  assert_output "main"

  # The symlink layout: the plain name is empty and lookups go to typo3-core-<name>.
  mv "${FAKEROOT}/typo3-core" "${FAKEROOT}/typo3-core-main"
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  run bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    printf '[%s]%s' \"\$(plain_core_name)\" \"\$(herdr_checkout_dir main)\"
  "
  assert_success
  assert_output "[]${FAKEROOT}/typo3-core-main"
}

@test "the herdr command no longer turns a single-checkout project away" {
  set -eu -o pipefail
  run grep -q 'This project has a single Core checkout, not worktrees' "${DIR}/commands/host/tryout"
  assert_failure
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
# tryout's git work runs inside the web container while editors, herdr and the
# dashboard read the same checkouts on the host. Worktree metadata records paths,
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
  local main="${FAKEROOT}/typo3-core-main" wt="${FAKEROOT}/typo3-core-x"
  git init -q "${main}"
  git -C "${main}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  git -C "${main}" worktree add -q --detach "${wt}" HEAD
  # The shape a worktree has when the OTHER side created it.
  printf 'gitdir: /var/www/html/typo3-core-main/.git/worktrees/typo3-core-x\n' > "${wt}/.git"
  printf '/var/www/html/typo3-core-x/.git\n' > "${main}/.git/worktrees/typo3-core-x/gitdir"
  run git -C "${wt}" status --short
  assert_failure

  run helper ensure_relative_worktree_paths
  assert_success
  run git -C "${main}" config --get worktree.useRelativePaths
  assert_output "true"
  run cat "${wt}/.git"
  assert_output "gitdir: ../typo3-core-main/.git/worktrees/typo3-core-x"
  run git -C "${wt}" status --short
  assert_success
  # A worktree added afterwards is relative from the start.
  git -C "${main}" worktree add -q --detach "${FAKEROOT}/typo3-core-y" HEAD
  run cat "${FAKEROOT}/typo3-core-y/.git"
  assert_output "gitdir: ../typo3-core-main/.git/worktrees/typo3-core-y"
}

@test "every clone and worktree operation goes through ensure_relative_worktree_paths" {
  set -eu -o pipefail
  # A worktree made without it is absolute and breaks on the other side.
  local fn body
  for fn in add_core_worktree migrate_core_to_worktree_layout; do
    body=$(sed -n "/^${fn}() {/,/^}/p" "${DIR}/tryout/functions.sh")
    printf '%s\n' "${body}" | grep -q 'ensure_relative_worktree_paths' \
      || fail "${fn} does not call ensure_relative_worktree_paths"
  done
  # And a fresh clone is configured before anything else happens to it.
  local file clone ensure
  for file in "${DIR}/tryout/post-start.sh" "${DIR}/tryout/commands.sh"; do
    [ -f "${file}" ] || continue
    clone=$(grep -nE '^[[:space:]]*(if ! )?git clone ' "${file}" | head -1 | cut -d: -f1)
    [ -n "${clone}" ] || continue
    ensure=$(awk -v from="${clone}" 'NR > from && /ensure_relative_worktree_paths/ { print NR; exit }' "${file}")
    [ -n "${ensure}" ] || fail "${file}: git clone at line ${clone} is not followed by ensure_relative_worktree_paths"
  done
  # add_core_worktree refuses on a git that cannot write relative paths.
  body=$(sed -n "/^add_core_worktree() {/,/^}/p" "${DIR}/tryout/functions.sh")
  printf '%s\n' "${body}" | grep -q 'git_supports_relative_worktrees' \
    || fail "add_core_worktree does not check the git version"
}

# --- host / container split ---------------------------------------------------
# commands/host/tryout is the entry point: it owns the terminal (prompts, gum,
# herdr) and hands every container-safe verb to tryout-container.sh through ONE
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
      herdr|help) continue ;;    # host by nature: the terminal, and static text
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
  # `worktree add x && cd typo3-core-x` on the host must not race Mutagen.
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
  printf '%s\n' "${host}" | grep -q -- '--site "\$1"' || fail "host does not translate patch <id> <site>"
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
  mkdir -p "${FAKEROOT}/sites/v13" "${FAKEROOT}/sites/v12"
  printf 'php=8.4\n' > "${FAKEROOT}/sites/v13/.tryout-site"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v12/.tryout-site"

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
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.4\n' > "${FAKEROOT}/sites/v13/.tryout-site"
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
  local main="${FAKEROOT}/typo3-core-main" wt="${FAKEROOT}/typo3-core-v13"
  git init -q "${main}"
  git -C "${main}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  git -C "${main}" worktree add -q --detach "${wt}" HEAD
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.4\n' > "${FAKEROOT}/sites/v13/.tryout-site"

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
  local main="${FAKEROOT}/typo3-core-main" wt="${FAKEROOT}/typo3-core-v13"
  git init -q "${main}"
  git -C "${main}" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  git -C "${main}" worktree add -q --detach "${wt}" HEAD
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.4\n' > "${FAKEROOT}/sites/v13/.tryout-site"

  # served: only v13. unserved and nonprimary: only main / only what is left.
  run helper worktree_labels served
  assert_output --partial "v13"
  refute_output --partial "primary"

  run helper worktree_labels nonprimary
  assert_output --partial "v13"

  run helper worktree_labels unserved
  refute_output --partial "v13"
}

@test "worktree adopt offers the strays instead of taking all of them" {
  set -eu -o pipefail
  local body
  body=$(awk '/^        adopt\)/,/^            ;;/' "${DIR}/commands/host/tryout")
  [ -n "${body}" ] || fail "no adopt branch"
  printf '%s\n' "${body}" | grep -q 'ui_choose_multi' \
    || fail "adopt does not offer a multi-select"
  # --dry-run still just reports, and an explicit path still bypasses the picker.
  printf '%s\n' "${body}" | grep -q 'dry="true"' || fail "--dry-run is gone"
  printf '%s\n' "${body}" | grep -q 'one_path' || fail "the explicit-path form is gone"
}

@test "every worktree subcommand that names a worktree can pick one" {
  set -eu -o pipefail
  local sub body
  for sub in "use" "remove|rm" "serve" "unserve" "rename" "adopt"; do
    body=$(awk -v pat="        ${sub})" 'index($0, pat) == 1, /^            ;;/' \
      "${DIR}/commands/host/tryout")
    [ -n "${body}" ] || fail "no '${sub}' branch"
    printf '%s\n' "${body}" | grep -qE 'ask_worktree|ui_choose_multi' \
      || fail "'${sub}' never offers a worktree list"
  done
}

# --- the installed copy going stale -----------------------------------------
# `ddev add-on get` copies the payload once and never refreshes it, so a project
# installed before a change keeps the old command AND the old completion script.
# Completion then still works but offers the older feature set, which is
# indistinguishable from "autocomplete is broken".

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

# --- keeping herdr in step with the worktrees -------------------------------
# `ddev tryout herdr` used to only ever ADD: it opened a workspace per worktree
# and skipped the ones already open. Nothing closed a workspace whose worktree
# had been removed — its pane sat in a directory that no longer existed.

# A herdr stub that answers `workspace list` with the given JSON and logs every
# other call, so a test can see exactly what was closed.
fake_herdr() {
  mkdir -p "${FAKEROOT}/bin"
  printf '%s' "$1" > "${FAKEROOT}/ws.json"
  cat > "${FAKEROOT}/bin/herdr" <<FAKE
#!/usr/bin/env bash
# Drop the leading "--session <name>" the wrapper adds.
[ "\$1" = "--session" ] && shift 2
if [ "\$1" = "workspace" ] && [ "\$2" = "list" ]; then
  cat "${FAKEROOT}/ws.json"
  exit 0
fi
echo "CALL: \$*" >> "${FAKEROOT}/calls.log"
FAKE
  chmod +x "${FAKEROOT}/bin/herdr"
  : > "${FAKEROOT}/calls.log"
}

@test "a workspace whose worktree is gone is an orphan; one still on disk is not" {
  set -eu -o pipefail
  # v13 exists on disk, v12 does not.
  mkdir -p "${FAKEROOT}/typo3-core-v13"
  ln -s typo3-core-v13 "${FAKEROOT}/typo3-core"
  fake_herdr '{"result":{"workspaces":[
    {"workspace_id":"w1","label":"core-v13"},
    {"workspace_id":"w2","label":"core-v12"}
  ]}}'

  run env DDEV_SITENAME=myproj PATH="${FAKEROOT}/bin:${PATH}" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    herdr_orphan_workspaces
  "
  assert_success
  assert_output --partial "w2"
  assert_output --partial "core-v12"
  refute_output --partial "core-v13"
}

@test "workspaces that are not ours are never touched, however stale they look" {
  # The session is per project, but a user may have opened anything in it. Only
  # the core-<name> label marks a workspace as one this command manages.
  set -eu -o pipefail
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  fake_herdr '{"result":{"workspaces":[
    {"workspace_id":"w1","label":"my-notes"},
    {"workspace_id":"w2","label":"typo3-core-main"},
    {"workspace_id":"w3","label":"core-gone"}
  ]}}'

  run env DDEV_SITENAME=myproj PATH="${FAKEROOT}/bin:${PATH}" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    herdr_orphan_workspaces
  "
  assert_success
  assert_output --partial "core-gone"
  refute_output --partial "my-notes"
  # Labelled like a directory, but not our scheme — leave it alone.
  refute_output --partial "typo3-core-main"
}

@test "close_orphan_workspaces closes each orphan once, by id, and says so" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main"
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  fake_herdr '{"result":{"workspaces":[
    {"workspace_id":"w1","label":"core-main"},
    {"workspace_id":"w2","label":"core-v12"},
    {"workspace_id":"w3","label":"core-bugfix"}
  ]}}'

  run env DDEV_SITENAME=myproj PATH="${FAKEROOT}/bin:${PATH}" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    close_orphan_workspaces
  "
  assert_success
  assert_output --partial "core-v12"
  assert_output --partial "core-bugfix"

  run cat "${FAKEROOT}/calls.log"
  assert_line "CALL: workspace close w2"
  assert_line "CALL: workspace close w3"
  # main is still on disk, so it is left open.
  refute_output --partial "close w1"
}

@test "nothing is closed when every worktree is still there" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main"
  ln -s typo3-core-main "${FAKEROOT}/typo3-core"
  fake_herdr '{"result":{"workspaces":[{"workspace_id":"w1","label":"core-main"}]}}'

  run env DDEV_SITENAME=myproj PATH="${FAKEROOT}/bin:${PATH}" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    close_orphan_workspaces
  "
  assert_success
  run cat "${FAKEROOT}/calls.log"
  assert_output ""
}

@test "a single-clone project's one workspace is never an orphan" {
  # Before any worktree exists, typo3-core/ is a plain clone and the workspace is
  # labelled after its branch — herdr_checkout_dir resolves that to typo3-core/.
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core"
  git init -q "${FAKEROOT}/typo3-core"
  git -C "${FAKEROOT}/typo3-core" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
  git -C "${FAKEROOT}/typo3-core" branch -M main
  fake_herdr '{"result":{"workspaces":[{"workspace_id":"w1","label":"core-main"}]}}'

  run env DDEV_SITENAME=myproj PATH="${FAKEROOT}/bin:${PATH}" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    herdr_orphan_workspaces
  "
  assert_success
  assert_output ""
}

@test "herdr syncs on a bare run, and leaves other workspaces alone when named" {
  set -eu -o pipefail
  local body
  body=$(sed -n '/^cmd_herdr() {/,/^}$/p' "${DIR}/commands/host/tryout")
  [ -n "${body}" ] || fail "no cmd_herdr()"
  printf '%s\n' "${body}" | grep -q 'close_orphan_workspaces' \
    || fail "cmd_herdr never closes orphaned workspaces"
  # Guarded on no worktree having been named: a targeted command must not close
  # workspaces that have nothing to do with it.
  printf '%s\n' "${body}" | grep -q '\[ -z "\${only}" \] && close_orphan_workspaces' \
    || fail "the cleanup is not guarded by an empty \${only}"
  # …and it runs after the open loop, so a rename settles in one command.
  local open_at close_at
  open_at=$(printf '%s\n' "${body}" | grep -n 'open_worktree_in_herdr' | head -1 | cut -d: -f1)
  close_at=$(printf '%s\n' "${body}" | grep -n 'close_orphan_workspaces' | head -1 | cut -d: -f1)
  [ "${open_at}" -lt "${close_at}" ] || fail "orphans are closed before the open loop"
}
