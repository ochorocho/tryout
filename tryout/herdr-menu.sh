#!/usr/bin/env bash
#ddev-generated

# The tryout command menu, bound to prefix+shift+T by `ddev tryout herdr setup-keys`.
#
# herdr plugin actions cannot reach herdr's own menus (the manifest has no keys
# section, and actions are only invocable by keybinding or link click), so this popup
# IS the GUI. It is a session-modal terminal, so anything slow is pushed into a real
# pane instead — see run_in_pane.
#
# herdr's config is global: this key is bound in every session, so a run outside a
# tryout project must say so plainly rather than failing obscurely.

set -uo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'

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

# The CWD decides, not $0 — the key is bound globally, so $0 always points at
# whichever project ran setup-keys. Identical to herdr-new-worktree.sh.
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

    self="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
    from_self="${self%/.ddev/tryout}"
    if [ -n "${self}" ] && [ "${from_self}" != "${self}" ] \
       && [ -d "${from_self}/.ddev" ] && case "${PWD}/" in "${from_self}/"*) true ;; *) false ;; esac; then
        echo "${from_self}"
        return 0
    fi
}

APPROOT="$(resolve_approot)"
[ -n "${APPROOT}" ] || fail "No DDEV project here — this key only drives tryout inside one."
[ -f "${APPROOT}/.ddev/tryout/functions.sh" ] \
    || fail "$(basename "${APPROOT}") is a DDEV project, but the tryout add-on is not installed in it."
command -v ddev >/dev/null 2>&1 || fail "ddev not found on the host."

cd "${APPROOT}" || fail "Cannot enter ${APPROOT}"
PROJECT="$(basename "${APPROOT}")"

# --- running commands ------------------------------------------------------

# Instant commands print here; the popup stays until the user has read them.
run_here() {
    printf '\n'
    ddev tryout "$@"
    pause
    exit 0
}

# Anything that takes real time goes to a pane: the popup is modal, so a long
# command inside it blocks the whole session and cannot be watched alongside
# anything else. Falls back to running here when we are not inside herdr.
run_in_pane() {
    local cmd="ddev tryout $*"

    if [ "${HERDR_ENV:-}" != "1" ] || ! command -v herdr >/dev/null 2>&1; then
        run_here "$@"
    fi

    local pane
    pane=$(herdr pane split --direction down --cwd "${APPROOT}" --focus 2>/dev/null \
           | jq -r '.result.pane.pane_id // empty' 2>/dev/null)

    if [ -z "${pane}" ]; then
        run_here "$@"
    fi

    herdr pane run "${pane}" "${cmd}" >/dev/null 2>&1
    printf "\n${GREEN}✓${NC} running in a new pane: ${BOLD}%s${NC}\n" "${cmd}"
    exit 0
}

# Destructive commands name what they will affect and require it typed back.
confirm_destructive() {
    local what="$1" expect="$2" answer
    printf "\n${YELLOW}!${NC} %s\n" "${what}"
    printf "  type ${BOLD}%s${NC} to confirm: " "${expect}"
    read -r answer || return 1
    [ "${answer}" = "${expect}" ] || { printf "\n  cancelled\n"; pause; exit 0; }
}

# --- pickers ---------------------------------------------------------------

worktree_names() {
    local d
    for d in "${APPROOT}"/typo3-core-*; do
        [ -d "${d}" ] || continue
        basename "${d}" | sed 's/^typo3-core-//'
    done
}

served_names() {
    local d
    [ -d "${APPROOT}/sites" ] || return 0
    for d in "${APPROOT}"/sites/*; do
        [ -d "${d}" ] && [ -f "${d}/.tryout-site" ] && basename "${d}"
    done
}

# Prompt for a name, showing what exists. Empty input aborts.
# The prompt and the option list go to STDERR on purpose: the caller captures
# stdout, so anything printed there would be swallowed into the name itself.
# NOT called in a $(...) subshell: an abort there could only exit the subshell, and
# the caller would carry on with an empty name. The answer lands in ASKED instead.
ASKED=""
ask_name() {
    local prompt="$1" options="$2"
    ASKED=""
    [ -n "${options}" ] && printf "  ${DIM}%s${NC}\n" "$(echo "${options}" | tr '\n' ' ')"
    printf "  %s: " "${prompt}"
    read -r ASKED || { printf "\n  cancelled\n"; pause; exit 0; }
    [ -n "${ASKED}" ] || { printf "\n  cancelled\n"; pause; exit 0; }
}

# --- menus -----------------------------------------------------------------

worktree_menu() {
    local key name
    printf "\n${BOLD}worktree${NC} — side-by-side Core checkouts\n\n"
    printf "  ${BOLD}1${NC} list          ${DIM}worktrees, served sites, URLs${NC}\n"
    printf "  ${BOLD}2${NC} add           ${DIM}new checkout${NC}\n"
    printf "  ${BOLD}3${NC} use           ${DIM}switch the primary Core${NC}\n"
    printf "  ${BOLD}4${NC} serve         ${DIM}own URL, PHP and database${NC}\n"
    printf "  ${BOLD}5${NC} unserve       ${DIM}drop the site, keep the worktree${NC}\n"
    printf "  ${BOLD}6${NC} remove        ${DIM}delete the checkout${NC}\n"
    printf "  ${BOLD}q${NC} back\n"
    printf "\n  choose: "
    read -r -n1 key
    # A single-key read leaves the Enter in the buffer; drain it so the next
    # `read -r` does not take it as an empty answer.
    read -r -t 0.01 _drain 2>/dev/null || true
    printf '\n'

    case "${key}" in
        1) run_here worktree list ;;
        2) printf '\n'
           ask_name "name" ""; name="${ASKED}"
           printf "  branch [current]: "; read -r branch || true
           run_in_pane worktree add "${name}" ${branch:+"${branch}"} ;;
        3) printf '\n'
           ask_name "worktree" "$(worktree_names)"; name="${ASKED}"
           run_in_pane worktree use "${name}" ;;
        4) printf '\n'
           ask_name "worktree" "$(worktree_names)"; name="${ASKED}"
           run_in_pane worktree serve "${name}" ;;
        5) printf '\n'
           ask_name "served site" "$(served_names)"; name="${ASKED}"
           run_in_pane worktree unserve "${name}" ;;
        6) printf '\n'
           ask_name "worktree" "$(worktree_names)"; name="${ASKED}"
           confirm_destructive "Removes the checkout typo3-core-${name} and any uncommitted work in it." "${name}"
           run_in_pane worktree remove "${name}" --force ;;
        *) exit 0 ;;
    esac
}

main_menu() {
    local key name
    printf "\n${BOLD}tryout${NC}  ${CYAN}%s${NC}\n\n" "${PROJECT}"
    printf "  ${BOLD}1${NC} status        ${DIM}Core, patches, packages, contrib${NC}\n"
    printf "  ${BOLD}2${NC} worktree…     ${DIM}checkouts, serving, URLs${NC}\n"
    printf "  ${BOLD}3${NC} herdr new     ${DIM}new worktree, opened here${NC}\n"
    printf "  ${BOLD}4${NC} patch         ${DIM}cherry-pick a Gerrit change${NC}\n"
    printf "  ${BOLD}5${NC} checkout      ${DIM}switch TYPO3 version${NC}\n"
    printf "  ${BOLD}6${NC} download      ${DIM}clone or update Core${NC}\n"
    printf "  ${BOLD}7${NC} composer      ${DIM}regenerate the overlay${NC}\n"
    printf "  ${BOLD}8${NC} cs doctor     ${DIM}check the Gerrit setup${NC}\n"
    printf "  ${BOLD}9${NC} reset         ${DIM}Core to its branch + rebuild${NC}\n"
    printf "  ${BOLD}0${NC} delete        ${DIM}wipe a site's DB + fileadmin${NC}\n"
    printf "  ${BOLD}e${NC} exec          ${DIM}run a command in a site${NC}\n"
    printf "  ${BOLD}h${NC} help          ${BOLD}q${NC} quit\n"
    printf "\n  choose: "
    read -r -n1 key
    # A single-key read leaves the Enter in the buffer; drain it so the next
    # `read -r` does not take it as an empty answer.
    read -r -t 0.01 _drain 2>/dev/null || true
    printf '\n'

    case "${key}" in
        1) run_here status ;;
        2) worktree_menu ;;
        3) run_in_pane herdr new ;;
        4) printf '\n  change-id [empty = all from config]: '
           read -r id || true
           run_in_pane patch ${id:+"${id}"} ;;
        5) printf '\n'
           ask_name "branch" ""; name="${ASKED}"
           run_in_pane checkout "${name}" ;;
        6) run_in_pane download ;;
        7) run_in_pane composer ;;
        8) run_here cs doctor ;;
        9) run_in_pane reset ;;
        0) printf '\n'
           ask_name "site" "$(served_names)"; name="${ASKED}"
           confirm_destructive "Wipes the database and fileadmin of '${name}'." "${name}"
           run_in_pane delete "${name}" ;;
        e) printf '\n'
           ask_name "site" "@primary $(served_names | tr '\n' ' ')"; name="${ASKED}"
           printf "  command: "
           read -r cmd || exit 0
           [ -n "${cmd}" ] || { printf "\n  cancelled\n"; pause; exit 0; }
           # shellcheck disable=SC2086 # the command is deliberately word-split
           run_in_pane exec "${name}" ${cmd} ;;
        h) run_here help ;;
        *) exit 0 ;;
    esac
}

main_menu
