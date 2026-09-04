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
# Without jq the pane id cannot be parsed, and a multi-minute command would end up
# running inside this modal popup instead of a pane.
command -v jq >/dev/null 2>&1 || fail "jq not found on the host — needed to launch jobs."

cd "${APPROOT}" || fail "Cannot enter ${APPROOT}"
PROJECT="$(basename "${APPROOT}")"

# --- running commands ------------------------------------------------------

# One key. ESC and q both mean "back": ESC used to fall into the catch-all and
# close the whole popup from a submenu, which is not what anyone means by it.
KEY=""
read_key() {
    KEY=""
    # -s so the keypress is not echoed and, crucially, no Enter is left in the
    # buffer for the next `read -r` to swallow as an empty answer. The old drain
    # used `read -t 0.01`, which is bash 4+ only: on macOS's bash 3.2 it fails with
    # "invalid timeout specification", never drains, and every following prompt
    # silently cancels.
    read -r -n1 -s KEY
    # -s suppresses the echo, so show the choice back.
    case "${KEY}" in
        $'\e'|'') printf '\n' ;;
        *) printf '%s\n' "${KEY}" ;;
    esac
    case "${KEY}" in
        $'\e') KEY="q" ;;
    esac
}

# Instant commands run here and their output goes to the viewer, so it scrolls and
# ESC comes back to the menu.
run_and_show() {
    local out
    printf "\n  ${DIM}running…${NC}\n"
    out="$(ddev tryout "$@" 2>&1)"
    show_output "$*" "" "${out}"
}

# Slow commands go to a pane of their own, which keeps them watchable full-size and
# out of this modal popup. The menu stays open.
run_in_pane() {
    if [ "${HERDR_ENV:-}" != "1" ] || ! command -v herdr >/dev/null 2>&1; then
        run_and_show "$@"
        return 0
    fi

    # shellcheck disable=SC1090
    . "${APPROOT}/.ddev/tryout/functions.sh" >/dev/null 2>&1 || { run_and_show "$@"; return 0; }

    local id
    if id=$(start_tryout_job "$*" "$@") && [ -n "${id}" ]; then
        printf "\n${GREEN}✓${NC} started: ${BOLD}ddev tryout %s${NC}\n" "$*"
        printf "  ${DIM}o watch it   any other key back to the menu${NC} "
        local k=""
        read -r -n1 -s k
        case "${k}" in
            o|O) show_job_output "${id}" ;;
        esac
    else
        # Never claim success we did not verify — that was the old behaviour.
        printf "\n${YELLOW}!${NC} could not start a pane; running here instead\n"
        run_and_show "$@"
    fi
    return 0
}

# --- the output viewer -----------------------------------------------------
# A popup is a singleton, session-modal terminal with no pane id, so it cannot hold
# real herdr panes: this draws the view itself, full height, replacing the menu.

# show_output <title> <state> <text>
show_output() {
    local title="$1" state="$2" text="$3"
    local -a lines=()
    local top=0 rows key
    while IFS= read -r l; do lines+=("${l}"); done <<< "${text}"
    rows=$(( $(tput lines 2>/dev/null || echo 24) - 6 ))
    [ "${rows}" -lt 5 ] && rows=5

    while :; do
        printf '\033[H\033[2J'
        printf "  ${BOLD}%s${NC}  %b\n\n" "${title}" "${state}"
        local i=0
        while [ "${i}" -lt "${rows}" ]; do
            local n=$(( top + i ))
            [ "${n}" -ge "${#lines[@]}" ] && break
            printf '  %s\n' "${lines[$n]}"
            i=$(( i + 1 ))
        done
        printf "\n  ${DIM}↑↓/jk scroll  g/G top/bottom  r refresh  ESC back  (%s/%s)${NC} " \
            "$(( top + 1 ))" "${#lines[@]}"

        read_key
        case "${KEY}" in
            q)   return 0 ;;
            j)   [ $(( top + rows )) -lt "${#lines[@]}" ] && top=$(( top + 1 )) ;;
            k)   [ "${top}" -gt 0 ] && top=$(( top - 1 )) ;;
            g)   top=0 ;;
            G)   top=$(( ${#lines[@]} - rows )); [ "${top}" -lt 0 ] && top=0 ;;
            r)   return 2 ;;
            *)   [ $(( top + rows )) -lt "${#lines[@]}" ] && top=$(( top + rows )) ;;
        esac
    done
}

# Read a job's output out of the pane it runs in, and say so plainly when that pane
# has gone. `pane read` answers with a JSON error AND exit code 0 for a missing
# pane, so the text is the only reliable signal.
show_job_output() {
    local id="$1" pane cmd state text rc
    . "${APPROOT}/.ddev/tryout/functions.sh" >/dev/null 2>&1 || return 0
    pane="$(cat "${TRYOUT_JOBS_DIR}/${id}.pane" 2>/dev/null)"
    cmd="$(cat "${TRYOUT_JOBS_DIR}/${id}.cmd" 2>/dev/null)"

    while :; do
        if [ -f "${TRYOUT_JOBS_DIR}/${id}.rc" ]; then
            rc="$(cat "${TRYOUT_JOBS_DIR}/${id}.rc" 2>/dev/null)"
            if [ "${rc}" = "0" ]; then state="${GREEN}✓ finished${NC}"
            else state="${RED}✗ exit ${rc}${NC}"; fi
        else
            state="${CYAN}⟳ running${NC}"
        fi

        text="$(herdr_cli pane read "${pane}" --source recent-unwrapped --lines 400 2>&1)"
        case "${text}" in
            *'"code":"pane_not_found"'*|*'pane_not_found'*)
                text="  The pane this job ran in has been closed, so its output is gone."$'\n'"  The exit code above is still recorded." ;;
        esac

        show_output "${cmd}" "${state}" "${text}"
        [ $? -eq 2 ] || return 0     # 2 = refresh, anything else = back
    done
}

# Destructive commands name what they will affect and require it typed back.
confirm_destructive() {
    local what="$1" expect="$2" answer
    printf "\n${YELLOW}!${NC} %s\n" "${what}"
    printf "  type ${BOLD}%s${NC} to confirm: " "${expect}"
    read -r answer || return 1
    [ "${answer}" = "${expect}" ] || { printf "\n  cancelled\n"; pause; return 1; }
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
    read -r ASKED || { printf "\n  cancelled\n"; pause; return 1; }
    [ -n "${ASKED}" ] || { printf "\n  cancelled\n"; pause; return 1; }
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
    printf "  ${BOLD}7${NC} adopt         ${DIM}move stray checkouts into the project${NC}\n"
    printf "  ${BOLD}8${NC} rename        ${DIM}rename a checkout (branch untouched)${NC}\n"
    printf "  ${BOLD}q${NC} back\n"
    printf "\n  choose: "
    read_key
    key="${KEY}"

    case "${key}" in
        1) run_and_show worktree list ;;
        2) printf '\n'
           ask_name "name" "" || return 0; name="${ASKED}"
           printf "  branch [current]: "; read -r branch || true
           run_in_pane worktree add "${name}" ${branch:+"${branch}"} ;;
        3) printf '\n'
           ask_name "worktree" "$(worktree_names)" || return 0; name="${ASKED}"
           run_in_pane worktree use "${name}" ;;
        4) printf '\n'
           ask_name "worktree" "$(worktree_names)" || return 0; name="${ASKED}"
           run_in_pane worktree serve "${name}" ;;
        5) printf '\n'
           ask_name "served site" "$(served_names)" || return 0; name="${ASKED}"
           run_in_pane worktree unserve "${name}" ;;
        6) printf '\n'
           ask_name "worktree" "$(worktree_names)" || return 0; name="${ASKED}"
           confirm_destructive "Removes the checkout typo3-core-${name} and any uncommitted work in it." "${name}" || return 0
           run_in_pane worktree remove "${name}" --force ;;
        7) run_and_show worktree adopt ;;
        8) printf '\n'
           ask_name "worktree" "$(worktree_names)" || return 0; name="${ASKED}"
           printf "  new name: "
           read -r newname || return 0
           [ -n "${newname}" ] || { printf "\n  cancelled\n"; pause; return 0; }
           run_in_pane worktree rename "${name}" "${newname}" ;;
        *) return 0 ;;
    esac
}

cs_menu() {
    local key user
    printf "\n${BOLD}cs${NC} — Gerrit contribution setup\n\n"
    printf "  ${BOLD}1${NC} doctor        ${DIM}check hooks, template, push URL${NC}\n"
    printf "  ${BOLD}2${NC} setup         ${DIM}install them (asks for your Gerrit user)${NC}\n"
    printf "  ${BOLD}3${NC} uninstall     ${DIM}remove hooks, reset the push URL${NC}\n"
    printf "  ${BOLD}q${NC} back\n"
    printf "\n  choose: "
    read_key
    key="${KEY}"

    case "${key}" in
        1) run_in_pane cs doctor ;;
        2) printf '\n  Gerrit user [empty = ask in the pane]: '
           read -r user || true
           # cs setup probes Gerrit over SSH, so it belongs in a pane.
           run_in_pane cs setup ${user:+"${user}"} ;;
        3) confirm_destructive "Removes the Gerrit hooks and resets origin's push URL." "uninstall" || return 0
           run_in_pane cs uninstall ;;
        *) return 0 ;;
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
    printf "  ${BOLD}8${NC} cs…           ${DIM}Gerrit contribution setup${NC}\n"
    printf "  ${BOLD}9${NC} reset         ${DIM}Core to its branch + rebuild${NC}\n"
    printf "  ${BOLD}0${NC} delete        ${DIM}wipe a site's DB + fileadmin${NC}\n"
    printf "  ${BOLD}e${NC} exec          ${DIM}run a command in a site${NC}\n"
    printf "  ${BOLD}j${NC} jobs          ${DIM}what is running, and what failed${NC}\n"
    printf "  ${BOLD}d${NC} dashboard     ${DIM}live project view${NC}\n"
    printf "  ${BOLD}h${NC} help          ${BOLD}q${NC} quit\n"
    printf "\n  choose: "
    read_key
    key="${KEY}"

    case "${key}" in
        1) run_and_show status ;;
        2) worktree_menu ;;
        3) run_in_pane herdr new ;;
        4) printf '\n  change-id [empty = all from config]: '
           read -r id || true
           run_in_pane patch ${id:+"${id}"} ;;
        5) printf '\n'
           ask_name "branch" "" || return 0; name="${ASKED}"
           confirm_destructive "Switching branch discards uncommitted work in Core." "${name}" || return 0
           run_in_pane checkout "${name}" ;;
        6) run_in_pane download ;;
        7) run_in_pane composer ;;
        8) cs_menu ;;
        9) confirm_destructive "Resets Core to its branch — uncommitted work is lost." "reset" || return 0
           run_in_pane reset ;;
        0) printf '\n'
           ask_name "site" "$(served_names)" || return 0; name="${ASKED}"
           confirm_destructive "Wipes the database and fileadmin of '${name}'." "${name}" || return 0
           run_in_pane delete "${name}" --yes ;;
        e) printf '\n'
           ask_name "site" "@primary $(served_names | tr '\n' ' ')" || return 0; name="${ASKED}"
           printf "  command: "
           read -r cmd || return 0
           [ -n "${cmd}" ] || { printf "\n  cancelled\n"; pause; return 0; }
           # shellcheck disable=SC2086 # the command is deliberately word-split
           run_in_pane exec "${name}" ${cmd} ;;
        j) printf '\n'
           # shellcheck disable=SC1090
           . "${APPROOT}/.ddev/tryout/functions.sh" >/dev/null 2>&1
           if [ -n "$(tryout_jobs_status 2>/dev/null)" ]; then
               tryout_jobs_status | while IFS=$'\t' read -r st cmd detail; do
                   case "${st}" in
                       ok)     printf "  ${GREEN}✓${NC} %-34s\n" "${cmd}" ;;
                       failed) printf "  ${RED}✗${NC} %-34s ${RED}%s${NC}\n" "${cmd}" "${detail}" ;;
                       *)      printf "  ${CYAN}⟳${NC} %-34s ${DIM}running${NC}\n" "${cmd}" ;;
                   esac
               done
           else
               printf "  ${DIM}no jobs yet${NC}\n"
           fi
           pause ;;
        d) "${APPROOT}/.ddev/tryout/herdr-dashboard.sh" ;;
        h) run_and_show help ;;
        q) return 1 ;;
        *) ;;
    esac
    return 0
}

# The menu is a session now, not a one-shot: run something, come back, run another.
while :; do
    main_menu || break
done
printf '\n'
