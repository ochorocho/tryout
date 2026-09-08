#!/usr/bin/env bash
#ddev-generated

# One command from the tryout panel, run in a herdr popup.
#
# Everything runs here, where there is a TTY and therefore gum: the verbs prompt
# with their own ask_* helpers rather than this script reimplementing any of them.
# The popup is session-modal, so a long verb does hold the session until it is
# done — accepted deliberately, because the alternative left a stray pane behind
# after every run.
#
# The panel is opened per project but this script is reached through a globally
# registered plugin action, so the project is resolved from the CWD, not from $0.

set -uo pipefail

RED='\033[0;31m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'

pause() {
    printf "\n  ${DIM}press any key to close${NC} "
    read -r -n1 -s 2>/dev/null || true
    printf '\n'
}

fail() {
    printf "${RED}✗${NC} %s\n" "$*"
    pause
    exit 1
}

# The CWD decides, not $0: a plugin action is registered for the whole machine, so
# $0 points at whichever checkout installed it. A popup inherits the focused pane's
# directory, which is the project the user is actually looking at.
resolve_approot() {
    local dir="${PWD}"
    while [ -n "${dir}" ] && [ "${dir}" != "/" ]; do
        if [ -d "${dir}/.ddev" ] && compgen -G "${dir}/.ddev/config*.yaml" >/dev/null 2>&1; then
            echo "${dir}"
            return 0
        fi
        dir="$(dirname "${dir}")"
    done
    return 1
}

# Positional when run directly (the no-herdr fallback); TRYOUT_PANEL_VERB when
# opened as a plugin pane, which takes no arguments of its own.
VERB="${1:-${TRYOUT_PANEL_VERB:-}}"
[ -n "${VERB}" ] || fail "No command given."

APPROOT="$(resolve_approot)" \
    || fail "No DDEV project here — the tryout panel only drives one from inside it."
[ -f "${APPROOT}/.ddev/tryout/functions.sh" ] \
    || fail "$(basename "${APPROOT}") is a DDEV project, but the tryout add-on is not installed in it."
command -v ddev >/dev/null 2>&1 || fail "ddev not found on the host."

cd "${APPROOT}" || fail "Cannot enter ${APPROOT}"

printf "\n  ${BOLD}ddev tryout %s${NC}\n\n" "${VERB}"

# Every verb runs right here, however long it takes. Splitting a pane for the slow
# ones left one behind after every run, which is the clutter the panel exists to
# avoid; and the long verbs stream their own [1/4] progress, so a popup showing
# them is a live log rather than a frozen box.
# shellcheck disable=SC2086
ddev tryout ${VERB}
rc=$?

[ "${rc}" -eq 0 ] || printf "\n${RED}✗${NC} exited ${rc}\n"
pause
exit "${rc}"
