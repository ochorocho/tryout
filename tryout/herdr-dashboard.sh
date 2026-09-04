#!/usr/bin/env bash
#ddev-generated

# A live view of the project, opened by `ddev tryout herdr dashboard` and by
# prefix+shift+D.
#
# IT MUST NEVER BLOCK. This redraws on a timer in a pane the user is watching, so
# every read here has to be instant: local filesystem or local git only. One
# `git ls-remote`, `ddev exec` or SSH probe would freeze the whole thing. A unit
# test greps this file for those calls — see tests/unit.bats.
#
# It is also read-only. Every mutation goes through the menu (prefix+shift+T).

set -uo pipefail

REFRESH="${TRYOUT_DASHBOARD_REFRESH:-5}"

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

# The CWD decides, not $0 — the key is bound globally. Identical to the other popups.
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
[ -n "${APPROOT}" ] || fail "No DDEV project here — this key only shows a tryout project."
[ -f "${APPROOT}/.ddev/tryout/functions.sh" ] \
    || fail "$(basename "${APPROOT}") is a DDEV project, but the tryout add-on is not installed in it."

DDEV_APPROOT="${APPROOT}"; export DDEV_APPROOT
# shellcheck disable=SC1090
. "${APPROOT}/.ddev/tryout/functions.sh" >/dev/null 2>&1 || fail "Could not read the add-on helpers."

PROJECT="$(basename "${APPROOT}")"

# DDEV only exports DDEV_SITENAME to commands it runs itself; this popup is launched
# by herdr, so hostnames would render as ".ddev.site" without it. Read it from the
# project config, which is where ddev config wrote it.
if [ -z "${DDEV_SITENAME:-}" ]; then
    DDEV_SITENAME="$(sed -n 's/^name: *//p' "${APPROOT}/.ddev/config.yaml" 2>/dev/null | head -1 | tr -d '"'"'"'"')"
    [ -n "${DDEV_SITENAME}" ] || DDEV_SITENAME="${PROJECT}"
    export DDEV_SITENAME
fi

# --- the view --------------------------------------------------------------

render() {
    printf '\033[H\033[2J'
    printf "${BOLD}tryout${NC}  ${CYAN}%s${NC}%*s${DIM}%s${NC}\n\n" \
        "${PROJECT}" $(( 40 - ${#PROJECT} )) "" "$(date +%H:%M:%S)"

    if [ ! -d "${CORE_DIR}" ] && [ ! -L "${CORE_DIR}" ]; then
        printf "  ${RED}✗${NC} TYPO3 Core is not cloned\n"
        printf "    ${DIM}→ ddev tryout download${NC}\n\n"
        return
    fi

    # One snapshot, reused: cmd_status calls this twice, which doubles the git cost.
    local worktrees active
    worktrees="$(list_core_worktrees 2>/dev/null)"
    active="$(active_worktree_name 2>/dev/null)"

    if [ -n "${worktrees}" ]; then
        printf "  ${BOLD}Worktrees${NC}\n"
        printf '%s\n' "${worktrees}" | while IFS=$'\t' read -r name head branch dirty is_active; do
            local mark="  " state php db url
            [ -n "${is_active}" ] && mark="${GREEN}→${NC} "
            state="${dirty}"
            [ "${dirty}" = "dirty" ] && state="${YELLOW}dirty${NC}"
            if site_is_served "${name}" 2>/dev/null; then
                php="$(site_php_version "${name}" 2>/dev/null)"
                db="$(site_database "${name}" 2>/dev/null)"
                url="https://$(site_hostname "${name}" 2>/dev/null)"
            else
                php="-"; db="-"; url="${DIM}not served${NC}"
            fi
            printf "  %b%-12s %-10s %-12s %b\n" "${mark}" "${name}" "${head}" "${branch}" "${state}"
            printf "    ${DIM}%-6s %-12s${NC} %b\n" "${php}" "${db}" "${url}"
        done
        echo ""
    fi

    # Patches: ahead of the LOCAL origin ref. No fetch — this is a view, not a sync.
    local ahead="0"
    if git -C "${CORE_DIR}" rev-parse "origin/${BRANCH}" >/dev/null 2>&1; then
        ahead=$(git -C "${CORE_DIR}" rev-list --count "origin/${BRANCH}..HEAD" 2>/dev/null || echo 0)
    fi

    local composer="${RED}missing${NC}" typo3="${RED}not set up${NC}"
    [ -d "${PROJECT_ROOT}/vendor" ] && composer="${GREEN}installed${NC}"
    [ -f "${PROJECT_ROOT}/config/system/settings.php" ] && typo3="${GREEN}configured${NC}"

    printf "  ${BOLD}Patches${NC}   %-18s ${BOLD}Composer${NC}  %b\n" "${ahead} applied" "${composer}"
    printf "  ${BOLD}Active${NC}    %-18s ${BOLD}TYPO3${NC}     %b\n" "${active:-—}" "${typo3}"

    if vendor_core_mismatch 2>/dev/null; then
        printf "  ${YELLOW}!${NC} vendor/ was built from a different Core ${DIM}→ worktree use ${active}${NC}\n"
    fi
    local strays
    strays="$(list_foreign_core_worktrees 2>/dev/null | grep -c . || true)"
    [ "${strays:-0}" -gt 0 ] && \
        printf "  ${YELLOW}!${NC} %s checkout(s) outside the project ${DIM}→ worktree adopt${NC}\n" "${strays}"

    # Jobs
    local jobs
    jobs="$(tryout_jobs_status 2>/dev/null)"
    if [ -n "${jobs}" ]; then
        printf "\n  ${BOLD}Jobs${NC}\n"
        printf '%s\n' "${jobs}" | head -8 | while IFS=$'\t' read -r state cmd detail; do
            case "${state}" in
                ok)      printf "    ${GREEN}✓${NC} %-34s ${DIM}%s${NC}\n" "${cmd}" "${detail}" ;;
                failed)  printf "    ${RED}✗${NC} %-34s ${RED}%s${NC}\n" "${cmd}" "${detail}" ;;
                *)       printf "    ${CYAN}⟳${NC} %-34s ${DIM}running${NC}\n" "${cmd}" ;;
            esac
        done
    fi

    printf "\n  ${DIM}r refresh   q quit   (auto every %ss)${NC}\n" "${REFRESH}"
}

# --- loop ------------------------------------------------------------------
while :; do
    render
    read -r -n1 -t "${REFRESH}" key 2>/dev/null || key=""
    case "${key}" in
        q|Q) printf '\n'; exit 0 ;;
        *)   ;;
    esac
done
