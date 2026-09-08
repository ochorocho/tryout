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

# Wait for the reader, but only where there is one. With no terminal `read` hits
# EOF and returns at once, so the window would close before anything could be read
# — which looks exactly like the command never ran.
pause() {
    printf "\n  ${DIM}press any key to close${NC} "
    if [ -t 0 ]; then
        # ALWAYS bounded. Run inline in the panel's own pane, an unbounded read
        # holds that pane forever if the keystroke never comes — the panel stops
        # redrawing and the whole thing looks dead. 60s is long enough to read
        # the output and short enough to always come back.
        read -r -n1 -s -t 60 2>/dev/null || true
    else
        # Nobody to press a key: hold the output long enough to be read.
        sleep 5
    fi
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

# Tell the panel we actually started. It cannot find out any other way: herdr
# answers "ok" whether or not it had a UI to draw into, the verb travels in the
# environment (unreadable from outside on macOS), and the popup's parent is the
# herdr server rather than the panel — so neither pgrep nor ppid can identify it.
PANEL_STARTED_MARKER="${TMPDIR:-/tmp}/tryout-panel-started.$$"
if [ -n "${TRYOUT_PANEL_STARTED:-}" ]; then
    PANEL_STARTED_MARKER="${TRYOUT_PANEL_STARTED}"
    : > "${PANEL_STARTED_MARKER}" 2>/dev/null || true
fi

# The creator knew the project root; only work it out when started by hand.
APPROOT="${TRYOUT_PANEL_APPROOT:-}"
[ -n "${APPROOT}" ] || APPROOT="$(resolve_approot)" \
    || fail "No DDEV project here — the tryout panel only drives one from inside it."
[ -f "${APPROOT}/.ddev/tryout/functions.sh" ] \
    || fail "$(basename "${APPROOT}") is a DDEV project, but the tryout add-on is not installed in it."
command -v ddev >/dev/null 2>&1 || fail "ddev not found on the host."

cd "${APPROOT}" || fail "Cannot enter ${APPROOT}"

# The popup does not inherit the pane's colours: it comes up on a LIGHT ground,
# so `status` output that carries no colour of its own — the "Core:" labels — and
# anything dim is unreadable there. Set the terminal's DEFAULT foreground and
# background for the popup rather than an SGR pair: ESC[0m, which follows every
# coloured span in that output, resets TO these rather than away from them.
# OSC 111/110 hand the popup back the way it was found, whatever it was.
# Only in the popup. The inline fallback runs in the PANEL'S own pane, which
# already has the session's colours, and repainting that would be a regression.
# TRYOUT_PANEL_STARTED is set by the panel only when it opens a popup.
if [ -n "${TRYOUT_PANEL_STARTED:-}" ]; then
    printf '\033]11;#1e1e2e\033\\\033]10;#cdd6f4\033\\'
    printf '\033[2J\033[H'
    restore_colours() { printf '\033]111\033\\\033]110\033\\\033[0m'; }
    trap restore_colours EXIT INT TERM
fi

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
