#!/usr/bin/env bash
#ddev-generated

# Popup entry point for herdr's "new worktree" key (see `ddev tryout herdr setup-keys`).
#
# herdr runs this in a session-modal terminal from ITS OWN cwd, not the project, so
# the project has to be found here. herdr's config is global: this key is bound in
# every session, so a run outside a tryout project must say so plainly rather than
# failing obscurely.

set -uo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

# Leave the popup up long enough to read, whatever happened.
pause() {
    printf '\n  press any key to close '
    read -r -n1 -s 2>/dev/null || true
    printf '\n'
}

fail() {
    printf "${RED}✗${NC} %s\n" "$*"
    pause
    exit 1
}

# The CWD decides, not $0. The completion script resolves $0 first because DDEV only
# ever runs it for its own project — but this key is bound globally, so $0 always
# points at whichever project ran setup-keys. Trusting it would silently create a
# worktree in a project the user is nowhere near. Walk up from the cwd instead, and
# only fall back to $0 when herdr started us inside that same project anyway.
resolve_approot() {
    local dir self from_self
    dir="${PWD}"
    while [ -n "${dir}" ] && [ "${dir}" != "/" ]; do
        if [ -d "${dir}/.ddev" ] && compgen -G "${dir}/.ddev/config*.yaml" >/dev/null 2>&1; then
            echo "${dir}"
            return 0
        fi
        dir="$(dirname "${dir}")"
    done

    # Not under a project: accept our own project only if the cwd is inside it.
    self="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
    from_self="${self%/.ddev/tryout}"
    if [ -n "${self}" ] && [ "${from_self}" != "${self}" ] \
       && [ -d "${from_self}/.ddev" ] && case "${PWD}/" in "${from_self}/"*) true ;; *) false ;; esac; then
        echo "${from_self}"
        return 0
    fi
}

APPROOT="$(resolve_approot)"
[ -n "${APPROOT}" ] || fail "No DDEV project here — this key only creates TYPO3 Core worktrees inside one."
[ -f "${APPROOT}/.ddev/tryout/functions.sh" ] \
    || fail "$(basename "${APPROOT}") is a DDEV project, but the tryout add-on is not installed in it."

command -v ddev >/dev/null 2>&1 || fail "ddev not found on the host."

cd "${APPROOT}" || fail "Cannot enter ${APPROOT}"

printf "${BOLD}New TYPO3 Core worktree${NC}  ${CYAN}%s${NC}\n\n" "$(basename "${APPROOT}")"

printf "  Name: "
read -r name || exit 1
[ -n "${name}" ] || fail "No name given."

printf "  Branch [leave empty for the current one]: "
read -r branch || exit 1

printf '\n'
# ddev tryout does the real work: validation, git worktree add, and opening it in
# herdr. Everything this script knows about tryout lives behind that one call.
if ddev tryout herdr new "${name}" ${branch:+"${branch}"}; then
    printf "\n${GREEN}✓${NC} worktree '%s' is ready\n" "${name}"
    pause
else
    fail "Could not create worktree '${name}'"
fi
