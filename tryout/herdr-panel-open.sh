#!/usr/bin/env bash
#ddev-generated

# Dock the tryout panel, focus it if it is already there, close it if it is the pane
# you are standing in. Reached from `ddev tryout panel` and from the plugin action.
#
# Like herdr-panel.sh, every herdr call here targets the session the user is in, so
# none of them go through herdr_cli — that wrapper pins the tryout-<project> session
# the workspaces command owns.

set -uo pipefail

PANE_LABEL="tryout"
LOCK_DIR="${TMPDIR:-/tmp}/ddev-tryout-panel.lock"
LOCK_STALE_SECS=30
DOCK_RATIO="0.78"   # herdr clamps to 0.1-0.9; the panel is the right-hand sliver

err() { printf '\033[0;31m✗\033[0m %s\n' "$*" >&2; }

command -v herdr >/dev/null 2>&1 || { err "herdr is not installed on the host"; exit 1; }
command -v jq >/dev/null 2>&1 || { err "jq is not installed on the host"; exit 1; }
[ "${HERDR_ENV:-}" = "1" ] || { err "Run this from inside herdr."; exit 1; }

# Two focus events can arrive while a split is still being set up; without a lock
# each would see no panel and open one. mkdir is the atomic primitive everywhere.
take_lock() {
    local i=0
    while [ "${i}" -lt 20 ]; do
        if mkdir "${LOCK_DIR}" 2>/dev/null; then
            trap 'rmdir "${LOCK_DIR}" 2>/dev/null || true' EXIT INT TERM
            return 0
        fi
        # A crashed run must not wedge the panel forever. `find -mtime` is in POSIX
        # and answers on both userlands, unlike stat, whose format flag differs.
        local stale=""
        if [ -d "${LOCK_DIR}" ]; then
            stale=$(find "${LOCK_DIR}" -maxdepth 0 -mmin "+$(( (LOCK_STALE_SECS + 59) / 60 ))" 2>/dev/null)
        fi
        if [ -n "${stale}" ]; then
            rmdir "${LOCK_DIR}" 2>/dev/null || true
            continue
        fi
        sleep 0.25
        i=$(( i + 1 ))
    done
    return 1
}

panes_json() { herdr pane list 2>/dev/null; }

current_tab() {
    printf '%s' "${HERDR_TAB_ID:-}"
}

# Our pane in THIS tab, if it is there. A label alone is not proof: it survives a
# herdr server restart while the process behind it does not, so a labelled pane
# whose shell is gone is a corpse to be replaced, not focused.
find_panel_pane() {
    local tab="$1"
    panes_json | jq -r --arg label "${PANE_LABEL}" --arg tab "${tab}" '
        .result.panes[]
        | select(.label == $label)
        | select($tab == "" or .tab_id == $tab)
        | .pane_id' 2>/dev/null | head -n1
}

pane_is_alive() {
    local id="$1" info
    # --pane, not a positional: `pane process-info <id>` is rejected outright.
    info=$(herdr pane process-info --pane "${id}" 2>/dev/null) || return 1
    [ -n "${info}" ] || return 1
    printf '%s' "${info}" | jq -e '.result != null' >/dev/null 2>&1
}

# The pane this command was invoked from. NEVER the UI-focused one: herdr's own
# guidance is that "omitting a target may use the UI-focused pane, which can belong
# to the user or another client" — and it does. Docking against it put the panel in
# whichever workspace happened to hold focus, which is how it went missing.
# DDEV passes the caller's environment through to a host command, so HERDR_PANE_ID
# is present and identifies the right pane even while another client holds focus.
calling_pane() {
    printf '%s' "${HERDR_PANE_ID:-}"
}

open_panel() {
    local cwd="$1" target new
    target="$(calling_pane)"
    [ -n "${target}" ] || {
        err "herdr did not say which pane this ran from (HERDR_PANE_ID)"
        err "  → run 'ddev tryout panel' from a pane inside herdr"
        return 1
    }

    # Always split right; the panel is the narrow half. A left dock would need a
    # pane swap afterwards, which is a setting this does not offer yet.
    new=$(herdr pane split "${target}" --direction right --ratio "${DOCK_RATIO}" \
            --no-focus --cwd "${cwd}" 2>/dev/null \
          | jq -r '.result.pane.pane_id // empty' 2>/dev/null)
    if [ -z "${new}" ]; then
        err "Could not split a pane for the panel"
        return 1
    fi

    herdr pane rename "${new}" "${PANE_LABEL}" >/dev/null 2>&1 || true
    herdr pane run "${new}" bash "${cwd}/.ddev/tryout/herdr-panel.sh" >/dev/null 2>&1 \
        || { err "Could not start the panel in its pane"; return 1; }
    return 0
}

APPROOT="${DDEV_APPROOT:-${PWD}}"
[ -f "${APPROOT}/.ddev/tryout/herdr-panel.sh" ] \
    || { err "The tryout add-on is not installed in ${APPROOT}"; exit 1; }

take_lock || { err "Another panel operation is in progress"; exit 1; }

TAB="$(current_tab)"
EXISTING="$(find_panel_pane "${TAB}")"

if [ -n "${EXISTING}" ]; then
    if pane_is_alive "${EXISTING}"; then
        # Toggle: running this from the panel itself means close it.
        if [ "${EXISTING}" = "$(calling_pane)" ]; then
            herdr pane close "${EXISTING}" >/dev/null 2>&1
            exit 0
        fi
        # Otherwise it is open in this tab already. herdr 0.9 has no
        # focus-a-pane-by-id — `pane focus` only moves in a DIRECTION, and
        # `--current` is meaningless here because a DDEV host command is not
        # itself a pane. The panel is the right-hand half of the split, so step
        # right from the caller; if that lands elsewhere, say where it is rather
        # than moving focus somewhere unexpected.
        if ! herdr pane focus --pane "$(calling_pane)" --direction right >/dev/null 2>&1; then
            printf 'The tryout panel is already open in this tab (%s).\n' "${EXISTING}"
        fi
        exit 0
    fi
    # A corpse from a restarted server: the label outlived the process. Take it
    # out before opening a new one, or the next run would focus a dead pane.
    herdr pane close "${EXISTING}" >/dev/null 2>&1 || true
fi

open_panel "${APPROOT}"
