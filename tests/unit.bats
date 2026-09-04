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
               config.tryout.yaml tryout; do
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

  run complete "''"
  assert_success
  for verb in ${actions}; do
    assert_line "${verb}"
  done
}

@test "completion suggests the top-level commands" {
  set -eu -o pipefail
  run complete "''"
  assert_success
  for verb in status download checkout composer patch worktree cs exec reset delete help; do
    assert_line "${verb}"
  done
}

@test "completion is position aware for cs and worktree" {
  set -eu -o pipefail
  run complete cs "''"
  assert_success
  assert_line "setup"
  assert_line "doctor"
  assert_line "uninstall"
  # The top-level verbs must NOT come back here — that is the whole point of the
  # script over the flat AutocompleteTerms list.
  refute_line "download"

  run complete worktree "''"
  assert_success
  for sub in add list use serve unserve remove; do
    assert_line "${sub}"
  done
  refute_line "status"
}

@test "completion offers the flags a subcommand actually parses" {
  set -eu -o pipefail
  run complete download "''"
  assert_line "--reset"

  run complete worktree unserve "''"
  assert_line "--drop-db"

  run complete worktree use "''"
  assert_line "--force"
}

@test "completion lists the worktrees on disk" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"

  run complete worktree use "''"
  assert_success
  assert_line "main"
  assert_line "v13"
}

@test "completion lists served sites for the commands that take one" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/sites/v13"
  printf 'php=8.2\n' > "${FAKEROOT}/sites/v13/.tryout-site"

  run complete exec "''"
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
  run complete "''"
  assert_success
  rm -f "${FAKEROOT}/.ddev/tryout/functions.sh"

  run complete worktree use "''"
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

@test "setup-keys refuses to write a second block and backs the config up" {
  set -eu -o pipefail
  printf 'a = 1\n' > "${FAKEROOT}/cfg.toml"

  run env DDEV_SITENAME=myproj HERDR_CONFIG="${FAKEROOT}/cfg.toml" bash -c "
    export DDEV_APPROOT='${FAKEROOT}'
    source '${DIR}/tryout/functions.sh' >/dev/null 2>&1
    command -v herdr >/dev/null 2>&1 || skip 'herdr not installed'
    herdr_setup_keys true >/dev/null 2>&1
    herdr_setup_keys true >/dev/null 2>&1
    grep -c '>>> tryout' '${FAKEROOT}/cfg.toml'
  "
  assert_success
  assert_output "1"

  # The backup must hold the pre-write content.
  run bash -c "cat '${FAKEROOT}'/cfg.toml.tryout-backup-*"
  assert_success
  assert_output "a = 1"
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

  run complete herd
  assert_success
  assert_line "herdr"

  run complete cs doc
  assert_success
  assert_line "doctor"

  run complete worktree us
  assert_success
  assert_line "use"

  run complete worktree use ma
  assert_success
  assert_line "main"

  # An empty word must keep working too.
  run complete "''"
  assert_success
  assert_line "herdr"
}

@test "completion offers herdr, its worktrees and its flags" {
  set -eu -o pipefail
  mkdir -p "${FAKEROOT}/typo3-core-main" "${FAKEROOT}/typo3-core-v13"

  run complete "''"
  assert_success
  assert_line "herdr"

  run complete herdr "''"
  assert_success
  assert_line "main"
  assert_line "v13"
  assert_line "--no-agent"
  assert_line "--no-focus"
}
