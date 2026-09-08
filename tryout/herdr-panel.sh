#!/usr/bin/env bash
#ddev-generated

# The tryout command panel: a narrow herdr pane listing the commands worth reaching
# for, driven by click or keystroke.
#
# It runs INSIDE the pane it draws, so every herdr call here targets the session the
# user is in. That is why nothing goes through herdr_cli — that wrapper pins the
# `tryout-<project>` session the workspaces command owns, which is not this one.
#
# Mouse: herdr forwards clicks and wheel to a pane app that asks for them, so
# enabling SGR reporting is all that is needed. Right-click is NOT available — herdr
# keeps it for its own pane menu unless the user sets right_click_passthrough_modifier
# in their global config, so nothing here may depend on it.

set -uo pipefail

BOLD='\033[1m'; DIM='\033[2m'; CYAN='\033[0;36m'; NC='\033[0m'
REV='\033[7m'

# Commands worth a click. Deliberately no `delete` and no `worktree remove`: those
# destroy data and their confirmation needs the room a full shell gives it.
LABELS=(
    "status"
    "worktree list"
    "worktree add"
    "worktree serve"
    "worktree use"
    "checkout"
    "patch"
    "composer"
    "reset"
)
HINTS=(
    "project overview"
    "worktrees and sites"
    "new Core worktree"
    "give it its own URL"
    "point primary at it"
    "switch TYPO3 version"
    "apply a Gerrit change"
    "regenerate the overlay"
    "reset Core + rebuild"
)

FIRST_ROW=3          # screen row (1-based) of the first command
SEL=0

self_dir() { cd "$(dirname "$0")" 2>/dev/null && pwd; }
RUNNER="$(self_dir)/herdr-panel-run.sh"

mouse_on()  { printf '\033[?1000h\033[?1006h\033[?25l'; }
mouse_off() { printf '\033[?1006l\033[?1000l\033[?25h'; }

# Terminal state must always come back, however this exits — an abandoned mouse mode
# leaves the user's pane emitting escape codes into whatever they type next.
cleanup() {
    mouse_off
    printf '\033[2J\033[H'
}
trap cleanup EXIT INT TERM

render() {
    local i=0 row
    printf '\033[2J\033[H'
    printf "  ${BOLD}tryout${NC}\n\n"
    while [ "${i}" -lt "${#LABELS[@]}" ]; do
        row=$(( FIRST_ROW + i ))
        printf '\033[%d;1H' "${row}"
        if [ "${i}" -eq "${SEL}" ]; then
            printf "  ${REV} %-20s ${NC}" "${LABELS[${i}]}"
        else
            printf "   %-20s " "${LABELS[${i}]}"
        fi
        i=$(( i + 1 ))
    done
    printf '\033[%d;1H' "$(( FIRST_ROW + ${#LABELS[@]} + 1 ))"
    printf "  ${DIM}%s${NC}\n" "${HINTS[${SEL}]}"
    printf "\n  ${DIM}click or ↑↓ · enter runs · q quits${NC}"
}

# A click lands on a screen row; map it back to a list index. Anything outside the
# list is not an error, just not a selection.
row_to_index() {
    local row="$1" idx=$(( row - FIRST_ROW ))
    [ "${idx}" -ge 0 ] && [ "${idx}" -lt "${#LABELS[@]}" ] || return 1
    printf '%s' "${idx}"
}

run_selected() {
    local verb="${LABELS[${SEL}]}"

    # The popup is where the prompting happens: it is modal, so it can own the
    # screen while gum asks, and it closes back to this panel. A plugin pane takes
    # no positional arguments, so the verb rides in the environment.
    if [ "${HERDR_ENV:-}" = "1" ] && command -v herdr >/dev/null 2>&1 \
       && herdr plugin pane open --plugin ddev-tryout --entrypoint run \
            --cwd "${PWD}" --env "TRYOUT_PANEL_VERB=${verb}" >/dev/null 2>&1; then
        return 0
    fi

    # The plugin is not linked, so there is no popup to open. Run it here instead
    # of pretending one appeared — and hand the terminal over properly while it
    # runs, because the command prompts and its gum needs a normal screen.
    mouse_off
    printf '\033[2J\033[H'
    "${RUNNER}" "${verb}"
    # Back to the panel: without this the loop keeps drawing but the mouse is dead,
    # so every later click would go unnoticed.
    mouse_on
}

# --- input ------------------------------------------------------------------
# SGR mouse: ESC [ < btn ; col ; row (M press | m release). Read the tail of an
# escape sequence up to its final letter, then decide what it was.
# No timeout: bash 3.2 rejects a fractional `read -t` outright, and `-t 0` only
# tests readiness without consuming. An escape sequence arrives as one burst and
# ends in a letter, so the terminator is the boundary to read up to. A bare ESC
# (the user pressing Escape) is the one case that would block, so it is bounded by
# a whole-second timeout, which 3.2 does accept.
read_escape_tail() {
    local seq="" ch
    while IFS= read -rsn1 -t 1 ch; do
        seq="${seq}${ch}"
        case "${ch}" in
            [A-Za-z~]) break ;;
        esac
    done
    printf '%s' "${seq}"
}

handle_escape() {
    local seq="$1"
    local mouse='^\[<([0-9]+);([0-9]+);([0-9]+)([Mm])$'
    if [[ "${seq}" =~ $mouse ]]; then
        local btn="${BASH_REMATCH[1]}" row="${BASH_REMATCH[3]}" kind="${BASH_REMATCH[4]}"
        # Wheel up/down are buttons 64/65 and arrive as presses.
        case "${btn}" in
            64) [ "${SEL}" -gt 0 ] && SEL=$(( SEL - 1 )); return 0 ;;
            65) [ "${SEL}" -lt $(( ${#LABELS[@]} - 1 )) ] && SEL=$(( SEL + 1 )); return 0 ;;
        esac
        # Act on release, so a press-and-drag off the row does not fire.
        [ "${kind}" = "m" ] || return 0
        [ "${btn}" = "0" ] || return 0
        local idx
        idx="$(row_to_index "${row}")" || return 0
        SEL="${idx}"
        run_selected
        return 0
    fi
    case "${seq}" in
        "[A") [ "${SEL}" -gt 0 ] && SEL=$(( SEL - 1 )) ;;
        "[B") [ "${SEL}" -lt $(( ${#LABELS[@]} - 1 )) ] && SEL=$(( SEL + 1 )) ;;
    esac
    return 0
}

mouse_on

while :; do
    render
    IFS= read -rsn1 key || break
    case "${key}" in
        $'\033') handle_escape "$(read_escape_tail)" ;;
        ""|$'\n') run_selected ;;
        j) [ "${SEL}" -lt $(( ${#LABELS[@]} - 1 )) ] && SEL=$(( SEL + 1 )) ;;
        k) [ "${SEL}" -gt 0 ] && SEL=$(( SEL - 1 )) ;;
        q) break ;;
    esac
done
