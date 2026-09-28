#!/usr/bin/env bash
#ddev-generated

# Shared functions for TYPO3 tryout DDEV commands.
# Source this file: source "${DDEV_APPROOT}/.ddev/tryout/functions.sh"

# The project root IS the TYPO3 Core clone. Core's own .gitignore has carried
# /.ddev/* since 2018 (10a9e0ee805), so DDEV living here is a shape Core expects;
# everything else the add-on generates is kept out of `git status` by
# .git/info/exclude, which is local to the clone and never reaches a patch.
PROJECT_ROOT="${DDEV_APPROOT}"
CORE_DIR="${PROJECT_ROOT}"
CORE_GIT_DIR="${CORE_DIR}/.git"

# Instances are built under TYPO3-Instances/, never at the root and never in
# Build/: the root is Core's source tree, and Build/ is Core's OWN directory —
# Gruntfile.js, phpstan/, Sources/, ~700 tracked files — so an instance there
# interleaves build output with Core's build tooling. TYPO3-Instances/ is a name
# Core does not use, so one exclude line covers all of it.
#
# Core's composer.json is typo3/cms, a library with no web-dir, so nothing would
# create a docroot on its own; the overlay sets web-dir/vendor-dir per instance.
INSTANCES_DIR="${PROJECT_ROOT}/TYPO3-Instances"
# The instance served at the project URL. A plain name, not the branch: it goes in
# the user's .ddev/config.yaml as the docroot, which must not change when the root
# checkout switches branch.
PRIMARY_INSTANCE="primary"
INSTANCE_DIR="${INSTANCES_DIR}/${PRIMARY_INSTANCE}"
# shellcheck disable=SC2034 # used by post-start.sh and commands/host/tryout
CORE_REPO="https://github.com/typo3/typo3.git"
GERRIT_REMOTE="https://review.typo3.org/Packages/TYPO3.CMS"
GERRIT_API="https://review.typo3.org"
GERRIT_URL="https://review.typo3.org/c/Packages/TYPO3.CMS/+/"
GERRIT_SSH_HOST="review.typo3.org"
GERRIT_SSH_PORT="29418"
GERRIT_PROJECT="Packages/TYPO3.CMS"
COMMIT_TEMPLATE_SRC="${PROJECT_ROOT}/.ddev/tryout/gitmessage.txt"

# Payload version. `ddev add-on get` copies these files once and never refreshes
# them, so a project installed before a change keeps the old command and the old
# completion script — which still work, but offer the previous feature set. That
# is indistinguishable from a broken install, so the number is stamped into
# .ddev/tryout/.version at install time and cmd_status compares the two.
#
# BUMP THIS whenever a change alters what a user sees: a new verb, a new flag, a
# new completion candidate. It is a plain integer because nothing at install time
# can read git — a local `ddev add-on get <dir>` records no version of its own.
TRYOUT_VERSION=40

# Core worktrees live INSIDE the clone, under worktrees/<name>. Nested worktrees
# keep relative metadata on both pointers (worktrees/<n>/.git -> ../../.git/... and
# .git/worktrees/<n>/gitdir -> ../../../worktrees/<n>/.git), which is what keeps
# host and container paths interchangeable. See `ddev tryout worktree`.
WORKTREES_DIR="${PROJECT_ROOT}/worktrees"
CORE_WORKTREE_PREFIX="${WORKTREES_DIR}/"
DEFAULT_CORE_WORKTREE="main"

# Served sites live beside the primary, one directory per instance, so every site
# has the same shape: TYPO3-Instances/<name>/{public,vendor,config,var}. The
# primary is simply the one called "primary". See `ddev tryout worktree serve`.
SITES_DIR="${INSTANCES_DIR}"
PRIMARY_SITE="@primary"
WORKTREE_CONFIG="${PROJECT_ROOT}/.ddev/config.worktrees.yaml"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
# Ordinary text, stated rather than inherited. Output that carries no colour of
# its own takes the terminal's default foreground, which a popup or embedded
# terminal need not share — labels and values came out unreadable there. 37 is the
# basic ANSI white every theme maps to something legible on its own background.
TEXT='\033[37m'
NC='\033[0m'

# --- Output helpers ---
info()    { echo -e "${CYAN}==>${NC} $*"; }
success() { echo -e "${GREEN}==>${NC} $*"; }
warn()    { echo -e "${YELLOW}==>${NC} $*"; }
error()   { echo -e "${RED}✗${NC} $*" >&2; }

# --- host / container split ---------------------------------------------------
# The work of every container-safe verb runs INSIDE the web container: the host
# command resolves prompts, then hands the verb to tryout-container.sh, which
# exports this flag and sources commands.sh. The helpers below that build, query
# the database or talk to Gerrit are written for the container and call the
# tools directly — never `ddev …`, which is a stub in there. Path helpers, git
# reads and the ui code run on either side.
in_container() { [ "${TRYOUT_IN_CONTAINER:-}" = "1" ]; }

# A shipped script by name, wherever the payload was installed.
tryout_script() { echo "${PROJECT_ROOT}/.ddev/tryout/$1"; }

# Composer reads COMPOSER=composer.tryout.json from the container environment
# (config.tryout.yaml), exactly as `ddev composer` did.
run_composer() { (cd "${INSTANCE_DIR}" && composer "$@"); }
run_typo3()    { (cd "${INSTANCE_DIR}" && vendor/bin/typo3 "$@"); }

db_is_postgres() { [[ "${DDEV_DATABASE:-mariadb}" == postgres* ]]; }

# One SQL statement as the database superuser, against the db service. DDEV's
# root/root (MariaDB, MySQL) and db/db (Postgres, a superuser there) are reachable
# from the web container by the service name.
db_root_sql() {
    if db_is_postgres; then
        PGPASSWORD=db psql -h db -U db -d postgres -tAc "$1"
    else
        mysql -h db -uroot -proot -e "$1"
    fi
}

# Run SQL against a SPECIFIC database, root-owned. db_root_sql connects to the
# server's default (postgres / no db), which is right for CREATE DATABASE but not
# for touching a site's own tables — every served site has its own db.
db_site_sql() {
    local db="$1" sql="$2"
    if db_is_postgres; then
        PGPASSWORD=db psql -h db -U db -d "${db}" -tAc "${sql}"
    else
        mysql -h db -uroot -proot -D "${db}" -e "${sql}"
    fi
}

# --- gum-backed presentation ------------------------------------------------
# gum (https://github.com/charmbracelet/gum) is OPTIONAL and host-only: the
# container never has it. Everything goes through these wrappers rather than
# calling gum directly, because two of its behaviours have to be handled in ONE
# place:
#
#   1. `gum choose` and friends need a controlling terminal. Without one they
#      print "could not open TTY" AND EXIT 0 — so the exit code lies, and the
#      output is the only reliable signal.
#   2. `gum spin` writes escape sequences when its stdout is not a terminal, so
#      it must never wrap a command whose output is captured.

have_gum() { command -v gum >/dev/null 2>&1; }

# An interactive prompt is always read as `x="$(ui_choose ...)"`, so stdout is a
# pipe by definition — testing it would disable the chooser everywhere. gum draws
# its UI on stderr, so stdin and stderr are what must be terminals.
have_tty() { [ -t 0 ] && [ -t 2 ]; }

# A bordered block. Content on stdin.
ui_box() {
    local title="${1:-}"
    if have_gum; then
        if [ -n "${title}" ]; then
            gum style --border rounded --padding "0 1" --border-foreground 244                 "$(gum style --bold "${title}")" "$(cat)"
        else
            gum style --border rounded --padding "0 1" --border-foreground 244 "$(cat)"
        fi
    else
        [ -n "${title}" ] && echo -e "${BOLD}${title}${NC}"
        cat
    fi
}

# A table. CSV on stdin, first line the header.
# stdin is read once up front: a fallback chained with `||` would otherwise get a
# stream the failed attempt already drained — which is how the web image, with
# neither gum nor column, printed an empty table.
ui_table() {
    local csv
    csv="$(cat)"
    if have_gum; then
        printf '%s\n' "${csv}" | gum table --print --separator "," 2>/dev/null && return 0
    fi
    # Fallback: readable columns without the borders.
    printf '%s\n' "${csv}" | column -t -s ',' 2>/dev/null && return 0
    printf '%s\n' "${csv}" | awk -F, '
        { for (i = 1; i <= NF; i++) { cell[NR, i] = $i; if (length($i) > w[i]) w[i] = length($i) }
          if (NF > nf) nf = NF }
        END { for (r = 1; r <= NR; r++) { line = ""
                for (i = 1; i <= nf; i++) line = line sprintf("%-" w[i] + 2 "s", cell[r, i])
                sub(/ +$/, "", line); print line } }'
}

# Pick one of the arguments. Echoes the choice; empty means cancelled.
# NEVER trust gum's exit code here — see the note above.
ui_choose() {
    local prompt="$1"; shift
    [ $# -gt 0 ] || return 1

    if have_gum && have_tty; then
        local picked
        # Never redirect stderr: that is where gum draws the list.
        # `choose`, never `filter` — filter cannot be cancelled with ESC.
        picked="$(printf '%s\n' "$@" | gum choose --header "${prompt}" --height 15)"
        # An empty answer means no TTY or a cancel; either way, nothing was chosen.
        [ -n "${picked}" ] && { printf '%s' "${picked}"; return 0; }
        return 1
    fi

    # A piped answer still works; the prompt is only drawn where it can be seen,
    # or it lands in whatever is capturing this.
    if [ -t 2 ]; then
        printf '  %s\n' "$*" >&2
        printf '  %s: ' "${prompt}" >&2
    fi
    local answer=""
    read -r answer || return 1
    # An arrow key arrives as a full escape sequence (ESC [ B). Dropping the ESC
    # alone would leave a printable "[B" that reads like a typed name.
    answer="$(printf '%s' "${answer}" \
        | sed $'s/\033\[[0-9;]*[A-Za-z]//g; s/\033[NOP]*[A-Za-z]//g' \
        | tr -d '\000-\037')"
    [ -n "${answer}" ] || return 1
    printf '%s' "${answer}"
}

# Pick SEVERAL of the arguments, one per line. Echoes the choices; a cancel or an
# empty pick returns 1. Same two gum rules as ui_choose: its screen is stderr and
# never redirected, and its exit code is not to be trusted — the output is.
ui_choose_multi() {
    local prompt="$1"; shift
    [ $# -gt 0 ] || return 1

    if have_gum && have_tty; then
        local picked
        # Do NOT spell the toggle key out in a prompt: gum draws its own footer
        # ("x toggle • enter submit" in gum 2.0), and a hint of ours would be one
        # more thing to get wrong when gum rebinds it.
        picked="$(printf '%s\n' "$@" \
            | gum choose --no-limit --header "${prompt}" --height 15)"
        [ -n "${picked}" ] && { printf '%s\n' "${picked}"; return 0; }
        return 1
    fi

    # No gum, or no terminal for it: read a line per pick until EOF or a blank.
    if [ -t 2 ]; then
        printf '  %s\n' "$@" >&2
        printf '  %s (one per line, blank to finish): ' "${prompt}" >&2
    fi
    local answer="" got=""
    while IFS= read -r answer; do
        answer="$(printf '%s' "${answer}" | tr -d '\000-\037')"
        [ -n "${answer}" ] || break
        got="${got}${answer}"$'\n'
    done
    [ -n "${got}" ] || return 1
    printf '%s' "${got}"
}

# Free text. Echoes the answer; empty means cancelled.
ui_input() {
    local prompt="$1" placeholder="${2:-}"
    if have_gum && have_tty; then
        local v
        # stderr is gum's screen — see ui_choose.
        v="$(gum input --header "${prompt}" --placeholder "${placeholder}")"
        [ -n "${v}" ] && { printf '%s' "${v}"; return 0; }
        return 1
    fi

    if [ -t 2 ]; then printf '  %s: ' "${prompt}" >&2; fi
    local answer=""
    read -r answer || return 1
    [ -n "${answer}" ] || return 1
    printf '%s' "${answer}"
}

# Confirm a destructive action.
#   0 confirmed   1 declined (no, ESC, Ctrl-C)   2 no terminal to ask on
# 1 and 2 must stay apart: callers say "Aborted." for one, "pass --yes" for the
# other. gum conflates them — it exits 1 either way — hence the have_tty gate.
ui_confirm() {
    local prompt="$1"
    have_tty || return 2

    if have_gum; then
        # rc, not `if`: a bare `gum confirm` under `set -e` kills the script.
        local rc=0
        # --default=false, or gum preselects Yes and a bare Enter confirms.
        gum confirm --default=false --affirmative "Yes" --negative "No" "${prompt}" || rc=$?
        [ "${rc}" -eq 0 ] && return 0
        return 1
    fi

    # stderr, not stdout: DDEV pipes a host command's stdout, always.
    printf '  %s [y/N] ' "${prompt}" >&2
    local answer=""
    read -r answer || return 1
    # Strip escape sequences — see ui_choose; a stray "y" in one would confirm.
    answer="$(printf '%s' "${answer}" \
        | sed $'s/\033\[[0-9;]*[A-Za-z]//g; s/\033[NOP]*[A-Za-z]//g' \
        | tr -d '\000-\037')"
    case "${answer}" in [Yy]|[Yy][Ee][Ss]) return 0 ;; esac
    return 1
}

# Run a command behind a spinner, preserving its exit code — the error handling
# throughout this file depends on those codes surviving. Only spins on a terminal:
# piped, gum would emit escape sequences into whatever is reading.
ui_spin() {
    local title="$1"; shift
    # gum draws the spinner on stderr, so that is the stream that must be a
    # terminal. Not stdout: DDEV pipes a host command's stdout, always.
    if have_gum && [ -t 2 ]; then
        gum spin --spinner dot --title "${title}" --show-error -- "$@"
        return $?
    fi
    if [ -t 2 ]; then info "${title}" >&2; fi
    "$@"
}

# --- Guided arguments -------------------------------------------------------
# A command run without its argument asks for it instead of failing — when
# someone is there to answer. Each ask_* helper prints the answer; on a cancel
# or with no terminal it prints nothing and returns 1, so the caller falls
# through to explain_missing, which shows the usage line only where nobody
# could have answered a prompt.

explain_missing() {
    local usage="$1"
    if have_tty; then
        warn "Cancelled"
    else
        error "Usage: ${usage}"
    fi
}

# One rendered row per worktree, filtered by <mode> (all, nonprimary, served,
# unserved): "<name>  <branch>  <head>  <state>  <what it serves>".
#
# This runs a `git status` per worktree — fine for a picker opened by hand, once,
# since a list of bare names does not say which branch is which. Never call it
# from completion, which globs the directories instead.
worktree_labels() {
    local mode="${1:-all}" name head branch dirty active site
    while IFS=$'\t' read -r name head branch dirty active; do
        [ -n "${name}" ] || continue
        case "${mode}" in
            nonprimary) [ -n "${active}" ] && continue ;;
            served)     site_is_served "${name}" || continue ;;
            unserved)   site_is_served "${name}" && continue ;;
        esac
        site=""
        if [ -n "${active}" ]; then
            site="← primary"
        elif site_is_served "${name}"; then
            site="PHP $(site_php_version "${name}")"
        fi
        printf '%s  %s  %s  %s  %s\n' \
            "$(pad_display "${name}" 14)" "$(pad_display "${branch}" 20)" \
            "${head}" "$(pad_display "${dirty}" 5)" "${site}"
    done < <(list_core_worktrees)
}

# Pick a worktree. Shows what each one is; answers with its bare name.
ask_worktree() {
    local prompt="$1" mode="${2:-all}" labels=() l picked
    while IFS= read -r l; do [ -n "${l}" ] && labels+=("${l}"); done < <(worktree_labels "${mode}")
    if [ ${#labels[@]} -eq 0 ]; then
        error "No worktree to choose from"
        error "  → ddev tryout worktree add <name> [<branch>]"
        return 1
    fi
    picked="$(ui_choose "${prompt}" "${labels[@]}")" || return 1
    # Column one is the name, whether the answer is a picked row or a typed name.
    printf '%s' "${picked%% *}"
}

# Pick a site. Echoes the site NAME — the "@primary" sentinel for the primary, so
# callers keep passing what site_is_primary understands — while the list shows a
# readable label: what each site is, on which PHP, at which URL.
#
# extra… are further entries offered verbatim below the sites (delete uses this
# for "--all"); picking one echoes it unchanged.
ask_site() {
    local prompt="$1"; shift
    local names=("${PRIMARY_SITE}") labels=() n i=0
    while IFS= read -r n; do [ -n "${n}" ] && names+=("${n}"); done < <(served_site_names)

    for n in "${names[@]}"; do
        if site_is_primary "${n}"; then
            labels+=("$(printf '%s  %s' "$(pad_display "primary" 12)" "${DDEV_PRIMARY_URL:-}")")
        else
            labels+=("$(printf '%s  https://%s  %s' "$(pad_display "${n}" 12)" \
                "$(site_hostname "${n}")" "PHP $(site_php_version "${n}")")")
        fi
    done
    for n in "$@"; do labels+=("${n}"); done

    local picked
    picked="$(ui_choose "${prompt}" "${labels[@]}")" || return 1

    # Map the label back to the name it stands for; anything extra is itself.
    for i in "${!labels[@]}"; do
        if [ "${labels[${i}]}" = "${picked}" ]; then
            [ "${i}" -lt "${#names[@]}" ] && { printf '%s' "${names[${i}]}"; return 0; }
            printf '%s' "${picked}"; return 0
        fi
    done

    # Not an exact label: either a name typed at the no-gum prompt, or a label
    # whose spacing does not match byte for byte. The first word decides — it is
    # the site name in every label this function builds.
    picked="${picked%% *}"
    [ "${picked}" = "primary" ] && { printf '%s' "${PRIMARY_SITE}"; return 0; }
    printf '%s' "${picked}"
}

# Local refs, ordered by usefulness: main, releases newest first, the pre-9
# TYPO3_x-y refs last. checkout fetches afterwards anyway.
# "<number> - <subject>" for each of the space-separated change numbers in $1,
# looked up in the TSV rows that follow. A number with no row — a hand-typed one —
# still gets a line, just without a subject.
describe_patches() {
    local wanted="$1"; shift
    local id row n s rest
    for id in ${wanted}; do
        s=""
        for row in "$@"; do
            # All the way to `rest`: a two-variable read would put every
            # remaining column into the subject, owner and scores included.
            IFS=$'\t' read -r n s rest <<<"${row}"
            [ "${n}" = "${id}" ] && break
            s=""
        done
        if [ -n "${s}" ]; then
            printf '%s - %s\n' "${id}" "${s}"
        else
            printf '%s\n' "${id}"
        fi
    done
}

# Pad a string to <width> COLUMNS. printf's %-Ns counts bytes, so a name like
# "Frédéric" or a "…" would leave the column short and shift everything after it.
pad_display() {
    local str="$1" width="$2" len
    len=${#str}                       # bash counts characters here, not bytes
    if [ "${len}" -ge "${width}" ]; then
        printf '%s' "${str}"
        return
    fi
    printf '%s%*s' "${str}" "$((width - len))" ""
}

# The open Gerrit changes for a branch, one TSV row each, under a spinner.
# Fails when Gerrit cannot be reached or answers nothing.
#
# The listing is fetched in the container (curl and jq live there); picking from
# it happens on the host, where gum and the terminal are — the host/container
# split in two functions rather than one, because a spinner and an interactive
# chooser cannot share a pipeline.
fetch_open_patches() {
    local branch="${1:-${BRANCH}}" limit="${2:-50}" out=""
    local label="${branch}"
    [ "${label}" = "-" ] && label="every branch"
    out="$(ui_spin "Fetching open changes for ${label}" \
        bash "$(tryout_script list-patches.sh)" "${GERRIT_API}" "${branch}" "${limit}")" || return 1
    [ -n "${out}" ] || return 1
    printf '%s\n' "${out}"
}

# Pick one or several changes. The rows come as arguments, NOT on stdin: without
# gum the chooser reads the answer from stdin, and a function that had consumed
# stdin to build its list would leave nothing for it to read.
#
# Echoes the chosen change numbers, one per line; nothing on a cancel.
pick_patches() {
    local prompt="${1:-Apply which changes?}"; shift
    local row n s o sc labels=()

    # What the user picks from is a rendered line; the number is column one, so
    # it survives the round trip without a second lookup.
    for row in "$@"; do
        [ -n "${row}" ] || continue
        IFS=$'\t' read -r n s o sc <<<"${row}"
        # pad_display, not printf %-Ns: printf counts BYTES, so one accented
        # letter in an owner's name would shift that row's last column.
        labels+=("$(printf '%-7s %s %s %s' "${n}" \
            "$(pad_display "${s}" 68)" "$(pad_display "${o}" 18)" "${sc}")")
    done
    [ ${#labels[@]} -gt 0 ] || return 1

    ui_choose_multi "${prompt}" "${labels[@]}" | awk 'NF {print $1}'
}

# Append change numbers to TRYOUT_PATCHES in the user's patch list, keeping the
# rest of the file — comments included — exactly as it was. Numbers already
# there are not added twice.
persist_patches() {
    local file="${PROJECT_ROOT}/.ddev/config.tryout-patches.yaml"
    [ -f "${file}" ] || {
        error "No patch list at .ddev/config.tryout-patches.yaml"
        return 1
    }
    [ $# -gt 0 ] || return 0

    local current new_list="" n
    current=$(sed -n 's/^ *- *TRYOUT_PATCHES=//p' "${file}" | head -1 | tr -d '[:space:]')
    new_list="${current}"
    for n in "$@"; do
        case ",${new_list}," in
            *",${n},"*) continue ;;     # already listed
        esac
        [ -n "${new_list}" ] && new_list="${new_list},${n}" || new_list="${n}"
    done
    [ "${new_list}" = "${current}" ] && return 0

    # sed -i needs a suffix to work on both BSD and GNU; the backup goes away
    # again immediately.
    sed -i.tryout-bak "s|^\( *- *\)TRYOUT_PATCHES=.*|\1TRYOUT_PATCHES=${new_list}|" "${file}"
    rm -f "${file}.tryout-bak"
    success "Patch list is now: ${new_list}"
}

ask_branch() {
    local prompt="$1" branches=(main) b
    # The list has to exist before it can be offered.
    ensure_core_branch_refs || true
    while IFS= read -r b; do
        [ -n "${b}" ] && [ "${b}" != "main" ] && branches+=("${b}")
    done < <(list_local_core_branches | sort -rV | awk '/^[0-9]/{print; next}{l=l $0 "\n"} END{printf "%s", l}')
    ui_choose "${prompt}" "${branches[@]}"
}

ask_text() { ui_input "$1" "${2:-}"; }

# The branch a NEW worktree is based on, resolved into ASKED_BRANCH.
#
# Every creation route asks this, whether or not the name arrived as an argument:
# naming a worktree says nothing about which branch it should sit on, and quietly
# taking ${BRANCH} bases it on whatever Core happens to be checked out.
#
# Three outcomes, and the difference matters:
#   already set  → keep it (an explicit `worktree add x 13.4` is not a question)
#   no terminal  → ${BRANCH}, silently. `ddev start` and scripts must not block.
#   cancelled    → non-zero, so the caller stops. explain_missing says "Cancelled".
#
# The answer lands in a variable rather than on stdout so a caller can tell a cancel
# apart from an empty pick without inspecting $? through a command substitution.
ASKED_BRANCH=""
ask_new_worktree_branch() {
    local usage="${1:-ddev tryout worktree add <name> [<branch>]}"

    [ -n "${ASKED_BRANCH}" ] && return 0
    if ! have_tty; then
        ASKED_BRANCH="${BRANCH}"
        return 0
    fi

    ASKED_BRANCH="$(ask_branch "Based on which branch?")" || {
        ASKED_BRANCH=""
        explain_missing "${usage}"
        return 1
    }
    [ -n "${ASKED_BRANCH}" ] || { explain_missing "${usage}"; return 1; }
}

# In a linked worktree .git is a file pointing at the shared object store, so
# resolve it to the common dir — hooks and config live there, not per worktree.
# Must run before branch detection, which needs a usable git dir.
if [ -f "${CORE_GIT_DIR}" ]; then
    CORE_GIT_DIR="$(git -C "${CORE_DIR}" rev-parse --path-format=absolute --git-common-dir 2>/dev/null \
        || git -C "${CORE_DIR}" rev-parse --git-common-dir)"

    if [ ! -d "$CORE_GIT_DIR" ]; then
        error "Could not detect valid TYPO3 Git directory, git rev-parse --git-common-dir returned '$CORE_GIT_DIR'"
        exit 1
    fi
fi

# For a detached HEAD, derive the base branch from the remote branches containing
# it. A release branch wins over main: when a commit sits on both, it is the more
# specific answer, and the newest release branch is the likeliest base. Only when
# nothing but main contains it do we say main. Answers for <dir>, default the root.
detect_detached_base_branch() {
    local dir="${1:-${CORE_DIR}}" refs found
    # Drop origin/HEAD, which for-each-ref reports as a bare "origin".
    refs=$(git -C "${dir}" for-each-ref --format='%(refname:short)' \
               --contains HEAD refs/remotes/origin 2>/dev/null \
           | sed 's|^origin/||' \
           | grep -Ex 'main|[0-9]+\.[0-9]+')

    found=$(echo "${refs}" | grep -Ex '[0-9]+\.[0-9]+' | sort -V | tail -1)
    [ -z "${found}" ] && found=$(echo "${refs}" | grep -Fx main)
    echo "${found:-main}"
}

# Resolve the active branch: use TRYOUT_BRANCH env if set, otherwise detect from
# the Core clone, falling back to "main".
# Note -e, not -d: in a worktree .git is a file. And `branch --show-current`
# exits 0 with empty output on a detached HEAD, so test the value, not $?.
if [ -n "${TRYOUT_BRANCH:-}" ]; then
    BRANCH="${TRYOUT_BRANCH}"
elif [ -e "${CORE_GIT_DIR}" ]; then
    BRANCH=$(git -C "${CORE_DIR}" branch --show-current 2>/dev/null || true)
    [ -z "${BRANCH}" ] && BRANCH="$(detect_detached_base_branch)"
else
    BRANCH="main"
fi

# --- Git helpers ---

ensure_gerrit_remote() {
    if ! git -C "${CORE_DIR}" remote get-url gerrit >/dev/null 2>&1; then
        info "Adding Gerrit remote..."
        git -C "${CORE_DIR}" remote add gerrit "${GERRIT_REMOTE}"
    fi
}

require_core() {
    if [ ! -d "${CORE_GIT_DIR}" ] && [ ! -f "${CORE_GIT_DIR}" ]; then
        error "TYPO3 Core not found — the project root is not a git checkout"
        error "  → Run: ddev tryout download"
        exit 1
    fi
}

# Drop a site's vendor/ before Composer installs against a different Core.
#
# Composer loads the plugins already in vendor/ before resolving. Switching Core
# majors changes typo3/class-alias-loader (v2 on main, v1 on 13.4), and the loaded
# v2 plugin then runs its pre-autoload-dump hook against the v1 code it just
# installed: "Class TYPO3\ClassAliasLoader\IncludeFile\SuffixToken not found",
# exit 1, and the site answers 500 until vendor/ is wiped by hand. The lock is
# sync-composer.php's job.
wipe_site_vendor() {
    local name="${1:-${PRIMARY_SITE}}" rel=""
    site_is_primary "${name}" || rel="TYPO3-Instances/${name}/"
    info "Removing ${rel}vendor/ — Core changed, a stale install cannot be updated in place"
    rm -rf "$(site_vendor "${name}")" \
        || { error "Could not remove ${rel}vendor/"; return 1; }
}

# Rebuild a site. Called with no argument it targets the primary, exactly as before,
# so the existing call sites keep their behaviour; pass a served site name to rebuild
# that one under its own PHP, composer root and database.
rebuild_typo3() {
    local name="${1:-${PRIMARY_SITE}}"

    if site_is_primary "${name}"; then
        check_php_for_core || return 1
        info "Running composer install..."
        run_composer install || { error "Composer install failed"; return 1; }
        info "Running extension:setup..."
        run_typo3 extension:setup 2>/dev/null || true
        info "Flushing caches..."
        rm -rf "${INSTANCE_DIR}/var/cache"/* 2>/dev/null || true
        run_typo3 cache:flush 2>/dev/null || true
        success "Rebuild complete"
        return
    fi

    local php
    php=$(site_php_version "${name}")
    check_php_for_core "$(site_core_dir "${name}")" "${php}" "${name}" || return 1
    info "Running composer install for '${name}' on PHP ${php}..."
    "php${php}" /usr/local/bin/composer install \
        --working-dir="$(site_dir "${name}")" --no-interaction \
        || { error "Composer install failed for ${name}"; return 1; }
    info "Running extension:setup for '${name}'..."
    site_exec "${name}" vendor/bin/typo3 extension:setup >/dev/null 2>&1 || true
    info "Flushing caches for '${name}'..."
    rm -rf "$(site_dir "${name}")/var/cache"/* 2>/dev/null || true
    site_exec "${name}" vendor/bin/typo3 cache:flush >/dev/null 2>&1 || true
    success "Rebuild complete for '${name}'"
}

reset_core_to_main() {
    git -C "${CORE_DIR}" fetch origin
    # Whatever branch this worktree is on, stay on it and move it to the base's
    # tip. It must NOT try to check out ${BRANCH}: that is the BASE a worktree
    # tracks, and a worktree carries a branch of its own name — so checking out
    # `main` here either collides with the worktree that holds it, or fails
    # outright because the branch already exists. Only a detached checkout, which
    # has no branch to move, resets in place.
    git -C "${CORE_DIR}" reset --hard "origin/${BRANCH}"
    git -C "${CORE_DIR}" clean -fd
    # ${1} names the site whose cache to drop; defaults to the primary's.
    rm -rf "$(site_dir "${1:-${PRIMARY_SITE}}")/var/cache"/* 2>/dev/null || true
}

# ─────────────────────────────────────────────────────────────────────
# Core worktrees
#
# The project root IS the primary Core checkout; worktrees are nested under
# worktrees/<name>. An instance points at the sysexts of the checkout it serves,
# primary always follows the root checkout and a served site is nailed to its own
# worktree. Switching the root's branch MUST be followed by a rebuild, because
# composer resolves the sysext paths when it writes vendor/ — see use_core_worktree.
# ─────────────────────────────────────────────────────────────────────

core_worktree_dir() { echo "${CORE_WORKTREE_PREFIX}$1"; }

# Where a checkout name lives: worktrees/<name>, or the project root itself for
# the primary, which has no directory under worktrees/ and is named after its
# branch.
core_checkout_dir() {
    local name="$1"
    if [ ! -d "$(core_worktree_dir "${name}")" ] && [ "${name}" = "$(plain_core_name)" ]; then
        echo "${CORE_DIR}"
    else
        core_worktree_dir "${name}"
    fi
}

# Clone Core INTO an existing directory. `git clone` refuses a non-empty target,
# and the target always is non-empty here: `ddev config` writes .ddev/ before the
# add-on ever runs. So do what clone does, by hand — init, fetch the one branch,
# check it out. Same result, no emptiness requirement.
#
# `checkout -f` because the worktree is not empty either: .ddev/ is sitting there
# and git would otherwise refuse to overwrite nothing at all. Nothing of the
# user's is at risk — the branch's files cannot collide with .ddev/, which Core
# has ignored since 2018.
clone_core_into_root() {
    local branch="${1:-${BRANCH}}"
    info "Fetching TYPO3 Core (${branch})..."
    git -C "${PROJECT_ROOT}" init -q 2>/dev/null || { error "git init failed in ${PROJECT_ROOT}"; return 1; }
    git -C "${PROJECT_ROOT}" remote add origin "${CORE_REPO}" 2>/dev/null ||         git -C "${PROJECT_ROOT}" remote set-url origin "${CORE_REPO}"
    git -C "${PROJECT_ROOT}" remote add gerrit "${GERRIT_REMOTE}" 2>/dev/null || true
    if ! git -C "${PROJECT_ROOT}" fetch --depth 1 origin "${branch}"; then
        error "Failed to fetch ${branch} from ${CORE_REPO}"
        return 1
    fi
    # A shallow fetch keeps the first start quick; unshallow later if history is
    # wanted (`git fetch --unshallow`). Gerrit patching only needs the tip plus
    # the change ref it fetches on demand.
    if ! git -C "${PROJECT_ROOT}" checkout -f -B "${branch}" FETCH_HEAD; then
        error "Failed to check out ${branch}"
        return 1
    fi
    git -C "${PROJECT_ROOT}" branch --set-upstream-to="origin/${branch}" "${branch}" >/dev/null 2>&1 || true
    ensure_core_excludes
    return 0
}

# Keep everything the add-on generates out of `git status`, without touching a
# tracked file. .git/info/exclude is local to the clone, is never committed and
# so never reaches a Gerrit patch — which is exactly what it is for. It lives in
# the SHARED .git, so one write also covers every worktree, present and future.
#
# Idempotent: each line is added only when missing, so it is safe to call after
# every clone and every `worktree add`.
ensure_core_excludes() {
    local f="${CORE_GIT_DIR}/info/exclude" e
    # In a worktree .git is a file; the real dir is the common one.
    if [ -f "${CORE_GIT_DIR}" ]; then
        f="$(git -C "${PROJECT_ROOT}" rev-parse --git-common-dir 2>/dev/null)/info/exclude"
    fi
    [ -n "${f}" ] || return 0
    mkdir -p "$(dirname "${f}")" 2>/dev/null || return 0
    [ -f "${f}" ] || : > "${f}"
    for e in \
        "/.ddev/" \
        "/worktrees/" \
        "/TYPO3-Instances/" \
        "/packages/"
    do
        grep -qxF "${e}" "${f}" 2>/dev/null || printf '%s\n' "${e}" >> "${f}"
    done
}

# git >= 2.48 can record worktree metadata with RELATIVE paths. That is what lets
# the container (which does the git work) and the host (editors, the
# completion) share one worktree: an absolute path is right on one side only.
git_supports_relative_worktrees() {
    local v major minor
    v=$(git --version 2>/dev/null | sed -n 's/^git version \([0-9][0-9]*\.[0-9][0-9]*\).*/\1/p')
    [ -n "${v}" ] || return 1
    major="${v%%.*}"; minor="${v#*.}"
    [ "${major}" -gt 2 ] || { [ "${major}" -eq 2 ] && [ "${minor}" -ge 48 ]; }
}

# Configure the Core repo to write relative worktree paths, and rewrite whatever
# is there already. `git worktree repair <path>` re-derives both pointers from the
# worktree's directory name, so it also mends a worktree the other side created
# with absolute paths that do not exist here. Idempotent and cheap; every place
# that clones, migrates or adds a worktree calls it.
ensure_relative_worktree_paths() {
    local main_dir dir
    git_supports_relative_worktrees || return 0
    main_dir=$(main_core_worktree_dir)
    [ -n "${main_dir}" ] || main_dir="${CORE_DIR}"
    [ -e "${main_dir}/.git" ] || return 0
    git -C "${main_dir}" config worktree.useRelativePaths true 2>/dev/null || return 0
    for dir in "${CORE_WORKTREE_PREFIX}"*; do
        [ -d "${dir}" ] || continue
        [ "$(cd "${dir}" && pwd -P)" = "$(cd "${main_dir}" && pwd -P)" ] && continue
        git -C "${main_dir}" worktree repair "${dir}" >/dev/null 2>&1 || true
    done
}

# A name becomes a directory, so keep it strictly harmless (no slashes, no ..).
validate_worktree_name() {
    local name="${1:-}"
    if [ -z "${name}" ]; then
        error "Missing worktree name"
        error "  → ddev tryout worktree add <name> [<branch>]"
        return 1
    fi
    # A leading hyphen is what a mis-parsed flag looks like — a flag once reached
    # here as a NAME and passed. It is also unusable as a directory or a git
    # branch, so it is never a real name.
    case "${name}" in
        -*) error "Invalid worktree name '${name}' (cannot start with '-')"
            error "  → ddev tryout worktree add <name> [<branch>]"
            return 1 ;;
    esac
    if ! echo "${name}" | grep -Eq '^[A-Za-z0-9._-]+$'; then
        error "Invalid worktree name '${name}' (allowed: letters, digits, . _ -)"
        return 1
    fi
    if [ "${name}" = "." ] || [ "${name}" = ".." ]; then
        error "Invalid worktree name '${name}'"
        return 1
    fi
}

# Was: "is typo3-core a symlink to a sibling worktree?". The root clone IS the
# primary now and never a symlink, so the question became "does this project have
# worktrees at all?" — which is what every caller actually wanted to know.
core_is_symlinked() { [ -d "${WORKTREES_DIR}" ]; }

# Name of the active worktree, empty on the legacy plain-clone layout.
# Hand a URL to the desktop's browser. Host-only: the container has no browser,
# and DDEV runs host commands with a real environment, so `open`/`xdg-open` are
# there to be found.
#
# Output goes nowhere: xdg-open is chatty on some desktops and prints straight
# over the line we just wrote. The URL is echoed regardless, so a headless box
# (or a container shell) still leaves the user something to click.
open_url() {
    local url="${1:-}" opener=""
    [ -n "${url}" ] || return 1

    case "${OSTYPE:-$(uname -s 2>/dev/null)}" in
        darwin*|Darwin) opener="open" ;;
        *)              opener="xdg-open" ;;
    esac

    if [ "${TRYOUT_IN_CONTAINER:-}" != "1" ] && command -v "${opener}" >/dev/null 2>&1; then
        if "${opener}" "${url}" >/dev/null 2>&1; then
            success "Opened ${url}"
            return 0
        fi
    fi

    # No opener, or it refused. Not an error: the URL is the useful part.
    info "Open: ${url}"
}

# The Core worktree a path belongs to, or nothing if it is not in one.
#
# Anything INSIDE a worktree counts, which is what a cwd needs: you run
# `ddev tryout launch` from wherever you happen to be in the checkout.
#
# The path is resolved first: on macOS the project is reached through /var while
# other tools report /private/var, and a plain prefix test would match neither.
worktree_name_for_path() {
    local path="${1:-}" root real name=""
    [ -n "${path}" ] || return 1
    root="$(cd "${PROJECT_ROOT}" 2>/dev/null && pwd -P)" || return 1
    real="$(cd "${path}" 2>/dev/null && pwd -P)" || return 1

    # Order is load-bearing: worktrees/ lives INSIDE the root checkout, so the
    # nested arm has to be tested before the root arm, or every worktree would be
    # claimed by the root.
    case "${real}" in
        "${root}/worktrees/"*)  name="${real#"${root}/worktrees/"}"
                                name="${name%%/*}" ;;
        "${root}/worktrees")    return 1 ;;   # the container, not a worktree
        "${root}"|"${root}/"*)  name="$(plain_core_name)" ;;
        *) return 1 ;;
    esac

    [ -n "${name}" ] || return 1
    printf '%s' "${name}"
}

# The active Core is the ROOT checkout, always — there is no symlink to move. Its
# name is its branch, the same answer plain_core_name gives, so the two agree and
# `worktree use` becomes a checkout rather than a relink.
# Which checkout the PRIMARY instance serves. Read from the overlay, because that
# is what `worktree use` moves — the root's branch would answer for the root even
# after the primary was repointed at a worktree.
#
# grep + sed rather than jq: jq is not required on the host, and this runs on
# the dashboard path.
active_worktree_name() {
    local url name
    url=$(grep -oE '\.\./\.\./(worktrees/[A-Za-z0-9._-]+/)?typo3/sysext' \
            "${INSTANCE_DIR}/composer.tryout.json" 2>/dev/null | head -1)
    case "${url}" in
        */worktrees/*)
            name=$(printf '%s' "${url}" | sed -E 's|.*/worktrees/([^/]+)/.*|\1|')
            [ -n "${name}" ] && { printf '%s' "${name}"; return 0; }
            ;;
    esac
    # No overlay yet, or it names the root checkout: the root is what is served,
    # and its name is its branch.
    plain_core_name
}

# The checkout the primary instance currently serves. What anything syncing or
# reading "the active Core" wants — CORE_DIR is only right while the primary has
# not been repointed.
active_core_dir() {
    core_checkout_dir "$(active_worktree_name)"
}

# The worktree that owns the object store; git lists it first. worktree add must
# run against a real worktree, and it can never be removed.
main_core_worktree_dir() {
    git -C "${CORE_DIR}" worktree list --porcelain 2>/dev/null \
        | awk '/^worktree /{print substr($0,10); exit}'
}

# Uncommitted changes in a checkout. A path that is not a git checkout is NOT
# dirty: git fails there, `--quiet` returns non-zero, and the negation below would
# otherwise turn "cannot look" into "has changes" — which is how `worktree use`
# came to report a clean project as dirty and could never run at all.
core_worktree_is_dirty() {
    local dir="$1"
    git -C "${dir}" rev-parse --git-dir >/dev/null 2>&1 || return 1
    ! git -C "${dir}" diff --quiet 2>/dev/null \
        || ! git -C "${dir}" diff --cached --quiet 2>/dev/null
}

# Nothing to migrate: the root clone IS the primary checkout and worktrees hang
# off it, so there is no plain-clone-to-symlink step left. Kept as a no-op because
# `worktree add` calls it; it only ensures worktrees/ exists.
# The name the ROOT checkout goes by: its branch, falling back to the default.
# There is no directory under worktrees/ naming it, so anything keyed on that name
# has to come from here. It survives the
# move to the worktree layout. Empty on the symlink layout.
plain_core_name() {
    local name
    name=$(git -C "${CORE_DIR}" branch --show-current 2>/dev/null || true)
    [ -z "${name}" ] && name="${DEFAULT_CORE_WORKTREE}"
    validate_worktree_name "${name}" >/dev/null 2>&1 || name="${DEFAULT_CORE_WORKTREE}"
    echo "${name}"
}

# Nothing to migrate any more: the root clone IS the primary checkout and the
# worktrees hang off it, so there is no plain-clone-to-symlink step to perform.
# Kept as a no-op because `worktree add` calls it, and making every caller test
# for a layout that no longer varies would be noise.
migrate_core_to_worktree_layout() {
    mkdir -p "${WORKTREES_DIR}" 2>/dev/null || true
    return 0
}

# Create a sibling worktree, always DETACHED at origin/<base>.
#
# No local branch is created, deliberately. A branch bought nothing here and cost
# a great deal: git allows one worktree per branch, so two worktrees off the same
# base collided ("A branch 'main' already exists"); a removed worktree left its
# branch behind, blocking re-creation of the same name; and composer derives a
# path repository's version from the branch name, so the branch HAD to be named
# after the base (dev-main → 13.4.x-dev), never the worktree. All of that is gone
# with a detached HEAD: nothing to collide, nothing left behind, and composer
# resolves the version from the checked-out commit's description against the base.
#
# Gerrit does not care: pushes go to refs/for/<branch> from HEAD, never from a
# local branch. Work that must survive is pushed, not kept on a local ref.
add_core_worktree() {
    local name="$1" branch="${2:-${BRANCH}}" dir
    validate_worktree_name "${name}" || return 1
    dir=$(core_worktree_dir "${name}")

    if [ -e "${dir}" ]; then
        error "Worktree '${name}' already exists at $(basename "${dir}")"
        error "  → ddev tryout worktree use ${name}"
        return 1
    fi

    # An absolute-path worktree would be unreadable on the other side of the
    # container boundary; refuse rather than create one.
    if ! git_supports_relative_worktrees; then
        error "git $(git --version 2>/dev/null | awk '{print $3}') cannot write relative worktree paths (needs 2.48+)"
        error "  → ddev restart   (rebuilds the web image with the add-on's git)"
        return 1
    fi

    local main_dir
    main_dir=$(main_core_worktree_dir)
    [ -z "${main_dir}" ] && main_dir="${CORE_DIR}"
    ensure_relative_worktree_paths

    info "Fetching origin..."
    git -C "${main_dir}" fetch origin || { error "Fetch failed"; return 1; }

    if ! git -C "${main_dir}" rev-parse --verify --quiet "origin/${branch}" >/dev/null; then
        error "Branch '${branch}' does not exist on origin"
        error "  → ddev tryout checkout   (lists available branches)"
        return 1
    fi

    info "Creating worktree '${name}' at origin/${branch}..."
    git -C "${main_dir}" worktree add --detach "${dir}" "origin/${branch}" || return 1
    success "Worktree '${name}' created"
}

# Point the PRIMARY instance at a Core checkout. An empty name means the root.
#
# NOT a symlink any more. This used to move typo3-core -> typo3-core-<name>, but
# CORE_DIR is the project root now, and `ln -sfn <target> <existing-dir>` does not
# replace a directory — it creates a link INSIDE it, which would have quietly
# littered the Core working tree with a stray symlink and switched nothing.
# The overlay's sysext path repository is the pointer instead.
set_active_core() {
    local name="${1:-}"
    [ "${name}" = "$(plain_core_name)" ] && [ ! -d "$(core_worktree_dir "${name}")" ] && name=""
    php "$(tryout_script use-core.php)" "${name}" >/dev/null
}

# Switch the active Core. The rebuild is mandatory, never optional: Composer
# binds vendor/ to the resolved real path, so without it the site silently keeps
# serving the previous Core.
use_core_worktree() {
    local name="$1" dir
    validate_worktree_name "${name}" || return 1
    # core_checkout_dir, not core_worktree_dir: the ROOT checkout is a valid
    # target and has no directory under worktrees/, so switching BACK to it would
    # otherwise be refused as "No worktree".
    dir=$(core_checkout_dir "${name}")

    if [ ! -d "${dir}" ]; then
        error "No worktree '${name}'"
        error "  → ddev tryout worktree list"
        return 1
    fi

    # No dirty check any more, and --force has nothing left to force. Switching
    # rewrites the PRIMARY INSTANCE's path repository; it does not touch any
    # checkout, so there is no working tree to hide and nothing to lose. The old
    # guard existed because `use` moved the typo3-core symlink out from under the
    # project root.
    local active
    active=$(active_worktree_name)
    [ "${active}" = "${name}" ] && info "'${name}' is already active — rebuilding anyway"

    set_active_core "${name}"
    success "Active Core: ${name}"

    # Sysext sets differ between versions, so regenerate before installing —
    # against the checkout just switched TO, not the root.
    info "Syncing composer.tryout.json..."
    env PROJECT_ROOT="${INSTANCE_DIR}" TRYOUT_CORE_DIR="$(active_core_dir)" \
        php "$(tryout_script sync-composer.php)" || warn "composer sync had warnings"
    wipe_site_vendor || return 1
    rebuild_typo3
}

remove_core_worktree() {
    local name="$1" force="${2:-false}" dir
    validate_worktree_name "${name}" || return 1
    dir=$(core_worktree_dir "${name}")

    if [ ! -d "${dir}" ]; then
        error "No worktree '${name}'"
        return 1
    fi
    if [ "$(active_worktree_name)" = "${name}" ]; then
        error "Cannot remove the active worktree '${name}'"
        error "  → ddev tryout worktree use <other>   first"
        return 1
    fi

    local main_dir
    main_dir=$(main_core_worktree_dir)
    if [ "${dir}" = "${main_dir}" ]; then
        error "Cannot remove '${name}': it owns the shared git object store"
        return 1
    fi

    # --force twice, deliberately. A Core checkout always carries untracked and
    # ignored files — vendor/, var/, Build/ — and plain `git worktree remove`
    # refuses outright on any of them ("contains modified or untracked files"),
    # leaving the directory behind after saying it removed the worktree. The user
    # has already been asked on the host, and the question named the directory, so
    # the answer must actually take it. A second --force also drops a worktree
    # whose branch is not merged.
    local args=("worktree" "remove" "--force")
    [ "${force}" = "true" ] && args+=("--force")
    if ! git -C "${CORE_DIR}" "${args[@]}" "${dir}"; then
        error "Failed to remove worktree '${name}'"
        error "  → git -C ${CORE_DIR} worktree remove --force ${dir}"
        return 1
    fi
    git -C "${CORE_DIR}" worktree prune 2>/dev/null || true

    # git leaves the directory when anything in it was not its own to delete — a
    # root-owned file a container wrote, say. The worktree is deregistered by now,
    # so what is left is a plain directory nothing refers to: take it, or the next
    # `worktree add <same name>` fails on a path that already exists.
    if [ -d "${dir}" ]; then
        rm -rf "${dir}" 2>/dev/null || true
        [ -d "${dir}" ] && warn "Could not delete ${dir} — remove it by hand"
    fi

    # The branch outlives the worktree, and add_core_worktree refuses a name whose
    # branch already exists — so leaving it behind blocks re-creating a worktree of
    # the same name, with an error about a branch the user never thinks about
    # ("A branch 'jochen' already exists" after the folder is plainly gone).
    #
    # `git branch -d`, never -D unless asked: git itself refuses to delete a branch
    # holding work that is not merged, which is precisely the check that keeps an
    # unpushed commit from disappearing with the checkout. So a spent branch goes
    # quietly, and one with work on it stays and says why.
    local main_dir_b
    main_dir_b=$(main_core_worktree_dir)
    [ -z "${main_dir_b}" ] && main_dir_b="${CORE_DIR}"
    if git -C "${main_dir_b}" show-ref -q --verify "refs/heads/${name}"; then
        local del="-d"
        [ "${force}" = "true" ] && del="-D"
        if git -C "${main_dir_b}" branch "${del}" "${name}" >/dev/null 2>&1; then
            success "Removed worktree '${name}', its directory and its branch"
            return 0
        fi
        warn "Branch '${name}' kept — it has commits that are not merged"
        echo -e "  ${DIM}→ ddev tryout worktree remove ${name} --force   (delete it too)${NC}"
        echo -e "  ${DIM}→ or: git -C ${main_dir_b} branch -D ${name}${NC}"
    fi
    success "Removed worktree '${name}' and its directory"
}

# Give a worktree a different directory name. The BRANCH is never touched.
#
# The name reaches further than the checkout, so all of it moves together: a
# served site's tree, its vhost and its database name.
rename_core_worktree() {
    local old="$1" new="$2" old_dir new_dir was_active="false" was_served="false" php=""
    validate_worktree_name "${old}" || return 1
    validate_worktree_name "${new}" || return 1
    [ "${old}" = "${new}" ] && { info "'${old}' is already called that"; return 0; }

    old_dir="$(core_worktree_dir "${old}")"
    new_dir="$(core_worktree_dir "${new}")"

    [ -d "${old_dir}" ] || { error "No worktree '${old}'"; return 1; }
    [ -e "${new_dir}" ] && { error "'${new}' already exists"; return 1; }

    [ "$(active_worktree_name)" = "${old}" ] && was_active="true"
    if site_is_served "${old}" 2>/dev/null; then
        was_served="true"
        php="$(site_php_version "${old}")"
    fi

    # A served site owns a database and a vhost keyed on the old name. Drop the site
    # first — keeping its database — then rebuild it under the new one.
    if [ "${was_served}" = "true" ]; then
        info "Unserving '${old}' so it can be re-served as '${new}'..."
        unserve_worktree "${old}" "true" >/dev/null 2>&1 || true
    fi

    # git worktree move, never mv: it rewrites the metadata on both sides.
    if ! git -C "${old_dir}" worktree move "${old_dir}" "${new_dir}" 2>/dev/null; then
        error "Could not move ${old_dir} (uncommitted changes, or it is locked?)"
        return 1
    fi

    [ "${was_active}" = "true" ] && set_active_core "${new}"
    success "Renamed worktree '${old}' to '${new}'"

    if [ "${was_served}" = "true" ]; then
        info "Re-serving as '${new}' on PHP ${php}..."
        # The hostname changes with the name, so DDEV has to register the new one —
        # and drop the old, which it only does on a restart.
        serve_worktree "${new}" "${php}" || {
            error "The worktree was renamed, but re-serving failed"
            error "  → ddev tryout worktree serve ${new}"
            return 1
        }
    fi
}

# Emit "name<TAB>head<TAB>branch<TAB>dirty<TAB>active" per worktree.
# Same rows as list_core_worktrees, minus the dirty column — and minus the two
# `git diff` calls per worktree that produce it. A TYPO3 Core checkout is ~20k
# tracked files, so on a Mutagen project that check costs seconds per worktree
# cold; anything that does not PRINT the answer must not pay for it.
#
# Deliberately a separate function rather than a flag: the caller's choice is
# visible at the call site, and the field list differs, so a positional reader
# cannot silently shift a column.
# Emit one row for the ROOT checkout, which is the primary and does not live under
# worktrees/ — so neither lister's glob can see it. Shared by both so they cannot
# drift.
emit_root_worktree_row() {
    local active="$1" dirty="${2:-}" name head branch
    name=$(plain_core_name)
    [ -n "${name}" ] || return 0
    # A worktrees/<name> of the same name would be listed by the glob as well.
    [ -d "$(core_worktree_dir "${name}")" ] && return 0
    head=$(git -C "${CORE_DIR}" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    branch=$(git -C "${CORE_DIR}" branch --show-current 2>/dev/null || true)
    [ -z "${branch}" ] && branch="(detached)"
    if [ -n "${dirty}" ]; then
        printf '%s\t%s\t%s\t%s\t%s\n' "${name}" "${head}" "${branch}" \
            "$(core_worktree_is_dirty "${CORE_DIR}" && echo dirty || echo clean)" \
            "$([ "${name}" = "${active}" ] && echo active || echo "")"
    else
        printf '%s\t%s\t%s\t%s\n' "${name}" "${head}" "${branch}" \
            "$([ "${name}" = "${active}" ] && echo active || echo "")"
    fi
}

list_core_worktrees_fast() {
    local active dir name head branch
    active=$(active_worktree_name)
    emit_root_worktree_row "${active}"
    for dir in "${CORE_WORKTREE_PREFIX}"*; do
        [ -d "${dir}" ] || continue
        name="${dir#"${CORE_WORKTREE_PREFIX}"}"
        head=$(git -C "${dir}" rev-parse --short HEAD 2>/dev/null || echo "unknown")
        branch=$(git -C "${dir}" branch --show-current 2>/dev/null || true)
        [ -z "${branch}" ] && branch="(detached)"
        printf '%s\t%s\t%s\t%s\n' "${name}" "${head}" "${branch}" \
            "$([ "${name}" = "${active}" ] && echo active || echo "")"
    done
}

# With the dirty column, and the per-worktree cost that comes with it. For
# `worktree list`, which prints it — see list_core_worktrees_fast for the rest.
list_core_worktrees() {
    local active dir name head branch dirty
    active=$(active_worktree_name)
    emit_root_worktree_row "${active}" withdirty
    for dir in "${CORE_WORKTREE_PREFIX}"*; do
        [ -d "${dir}" ] || continue
        name="${dir#"${CORE_WORKTREE_PREFIX}"}"
        head=$(git -C "${dir}" rev-parse --short HEAD 2>/dev/null || echo "unknown")
        branch=$(git -C "${dir}" branch --show-current 2>/dev/null || true)
        [ -z "${branch}" ] && branch="(detached)"
        dirty="clean"
        core_worktree_is_dirty "${dir}" && dirty="dirty"
        printf '%s\t%s\t%s\t%s\t%s\n' "${name}" "${head}" "${branch}" "${dirty}" \
            "$([ "${name}" = "${active}" ] && echo active || echo "")"
    done
}

# Uncommitted work in a checkout, in words: "clean", "3 modified", "1 untracked",
# or both. Like core_worktree_is_dirty, a path that is not a checkout is clean —
# "cannot look" must never read as "has changes".
worktree_change_summary() {
    local dir="$1" status modified untracked out=""
    git -C "${dir}" rev-parse --git-dir >/dev/null 2>&1 || { echo "clean"; return 0; }
    status="$(git -C "${dir}" status --porcelain 2>/dev/null || true)"
    untracked=$(printf '%s\n' "${status}" | grep -c '^??' || true)
    modified=$(printf '%s\n' "${status}" | grep -c '^[^?]' || true)
    [ "${modified}" -gt 0 ] && out="${modified} modified"
    [ "${untracked}" -gt 0 ] && out="${out:+${out}, }${untracked} untracked"
    echo "${out:-clean}"
}

# One `worktree list` card: where the checkout stands, what is on it, what it
# serves. Takes a list_core_worktrees_fast row. Every git read here is per
# worktree, which is fine for a command run by hand and wrong anywhere else.
worktree_card() {
    local name="$1" head="$2" branch="$3" active="$4"
    local dir base upstream count subject age changes url php db
    dir="$(core_checkout_dir "${name}")"

    if [ -n "${active}" ]; then
        echo -e "${CYAN}●${NC} ${BOLD}${TEXT}${name}${NC}  ${CYAN}← primary${NC}"
    else
        echo -e "${TEXT}○${NC} ${BOLD}${TEXT}${name}${NC}"
    fi

    # Worktrees are created detached, so the base comes from the remote branches
    # that contain HEAD; an attached checkout is its own answer.
    if [ "${branch}" = "(detached)" ]; then
        base="$(detect_detached_base_branch "${dir}")"
        upstream="origin/${base}"
        branch="detached from ${base}"
    else
        upstream="$(git -C "${dir}" rev-parse --abbrev-ref '@{upstream}' 2>/dev/null || true)"
        [ -n "${upstream}" ] || upstream="origin/${branch}"
    fi
    age="$(git -C "${dir}" log -1 --format=%cr 2>/dev/null || true)"
    echo -e "  ${TEXT}${branch} ${DIM}${TEXT}@${NC} ${TEXT}${head}${age:+ ${DIM}${TEXT}· ${age}}${NC}"

    # Commits on top of the base ARE the applied patches — the measure `status`
    # reports.
    count="$(git -C "${dir}" rev-list --count "${upstream}..HEAD" 2>/dev/null || echo 0)"
    subject="$(git -C "${dir}" log -1 --format=%s 2>/dev/null || true)"
    # DDEV exports COLUMNS=0, so there is no width to fit; 70 keeps it on a line.
    [ "${#subject}" -gt 70 ] && subject="${subject:0:69}…"
    if [ "${count}" -gt 0 ] 2>/dev/null; then
        local noun="patches"
        [ "${count}" -eq 1 ] && noun="patch"
        echo -e "  ${YELLOW}${count} ${noun} on top${NC} ${DIM}${TEXT}·${NC} ${TEXT}${subject}${NC}"
    elif [ -n "${subject}" ]; then
        echo -e "  ${DIM}${TEXT}${subject}${NC}"
    fi

    changes="$(worktree_change_summary "${dir}")"
    if [ "${changes}" = "clean" ]; then changes="${GREEN}clean${NC}"; else changes="${YELLOW}${changes}${NC}"; fi

    if site_is_served "${name}"; then
        url="https://$(site_hostname "${name}")"
        php="$(site_php_version "${name}")"
        db="$(site_database "${name}")"
    elif [ -n "${active}" ]; then
        url="${DDEV_PRIMARY_URL:-}"
        php="${DDEV_PHP_VERSION:-}"
        db="db"
    fi
    if [ -n "${url:-}${php:-}" ]; then
        echo -e "  ${changes}"
        [ -n "${url:-}" ] && echo -e "  ${CYAN}${url}${NC}"
        echo -e "  ${DIM}${TEXT}PHP ${php:--} · ${db}${NC}"
    else
        echo -e "  ${changes} ${DIM}${TEXT}· not served${NC}"
        echo -e "    ${DIM}${TEXT}→ ddev tryout worktree serve ${name}${NC}"
    fi
}

# Branch names on origin, one per line, version-sorted. The remote form asks
# origin and so needs the network; the local form reads refs already fetched,
# which is what tab-completion uses so a TAB never blocks.
list_remote_core_branches() {
    git -C "${CORE_DIR}" ls-remote --heads origin 2>/dev/null \
        | awk -F/ '{print $NF}' \
        | sort -V
}

list_local_core_branches() {
    git -C "${CORE_DIR}" for-each-ref --format='%(refname:strip=3)' refs/remotes/origin 2>/dev/null \
        | grep -vx 'HEAD' \
        | sort -V
}

# Make sure the branch LIST is on disk before something offers it to the user.
#
# The clone fetches one branch (`fetch --depth 1 origin <branch>`), because that is
# all a working instance needs and it keeps the first start quick. But a
# single-branch fetch writes a single remote-tracking ref, so `refs/remotes/origin/`
# holds only `main` and every picker built on list_local_core_branches could offer
# nothing else.
#
# So fetch the tips on FIRST NEED instead of at install: ~70s and ~180MB once, for
# users who actually switch branches, rather than on every `ddev start`. Detected by
# the ref count rather than a marker file — the refs are the thing we need, so
# counting them cannot go stale.
#
# NEVER call this from the completion script: a TAB must not wait on the network.
ensure_core_branch_refs() {
    local n
    n=$(git -C "${CORE_DIR}" for-each-ref refs/remotes/origin 2>/dev/null | wc -l | tr -d ' ')
    [ "${n:-0}" -gt 1 ] && return 0

    # stderr: ask_branch returns the picked branch on STDOUT, so a notice there
    # would be read back as part of the answer — the same reason ui_confirm prompts
    # to stderr.
    info "Fetching the branch list (once; the clone only carried one branch)..." >&2
    if ! git -C "${CORE_DIR}" fetch --depth 1 origin '+refs/heads/*:refs/remotes/origin/*' \
           >/dev/null 2>&1; then
        warn "Could not fetch the branch list — only the current branch is offered" >&2
        return 1
    fi
    return 0
}

# True when the installed payload is not the one this code came from. Runs on the
# host, cheaply: two file reads and a string compare.
#
# A missing stamp means an install that predates the marker. That is NOT reported
# as stale — we cannot tell how old it is, and warning on every such project would
# train people to ignore the line.
addon_is_stale() {
    local f="${PROJECT_ROOT}/.ddev/tryout/.version" installed
    [ -f "${f}" ] || return 1
    installed="$(tr -d '[:space:]' < "${f}" 2>/dev/null)"
    [ -n "${installed}" ] || return 1
    [ "${installed}" != "${TRYOUT_VERSION}" ]
}

# True when vendor/ was built from a different Core than the active one. This is
# the silent failure mode of a symlink swap without a reinstall.
vendor_core_mismatch() {
    local link resolved active
    link="${INSTANCE_DIR}/vendor/typo3/cms-core"
    [ -L "${link}" ] || return 1
    active=$(active_worktree_name)
    [ -n "${active}" ] || return 1
    resolved=$(cd "$(dirname "${link}")" && cd "$(readlink "${link}")" 2>/dev/null && pwd -P) || return 1
    # The PRIMARY is the root checkout, which has no directory under worktrees/ —
    # asking core_worktree_dir for it names a path that does not exist, and the cd
    # below then printed an error on every `status`.
    local expected
    expected="$(core_checkout_dir "${active}")"
    expected="$(cd "${expected}" 2>/dev/null && pwd -P)" || return 1
    case "${resolved}" in
        "${expected}"/*) return 1 ;;
        *) return 0 ;;
    esac
}

# ─────────────────────────────────────────────────────────────────────
# Served sites
#
# The primary site is asymmetric on purpose: it stays at the project root so every
# existing command, composer.json and single-Core install keeps working untouched.
# The primary/extra distinction lives in these four path helpers ONLY — callers pass
# a site name and never branch themselves.
# ─────────────────────────────────────────────────────────────────────

site_is_primary() { [ "${1:-}" = "${PRIMARY_SITE}" ] || [ -z "${1:-}" ]; }

# Root of a site's TYPO3 instance (composer root).
# The PRIMARY site is TYPO3-Instances/primary — the root is Core's source, and the
# every instance is a sibling under TYPO3-Instances/, the primary included.
site_dir() {
    if site_is_primary "${1:-}"; then echo "${INSTANCE_DIR}"; else echo "${INSTANCES_DIR}/$1"; fi
}

site_docroot() { echo "$(site_dir "${1:-}")/public"; }
site_vendor()  { echo "$(site_dir "${1:-}")/vendor"; }

# The Core checkout a site serves. For the PRIMARY that is whatever its overlay
# points at, which `worktree use` moves — not CORE_DIR, which is only the root and
# would answer "the root" after a switch. A served site is nailed to its own
# worktree, so it cannot follow `use`.
site_core_dir() {
    if site_is_primary "${1:-}"; then active_core_dir; else core_worktree_dir "$1"; fi
}

# Hostname: <name>.<project>.ddev.site for extras, the bare project URL for primary.
# DDEV appends .ddev.site itself, so additional_hostnames gets the un-suffixed form.
site_hostname_short() {
    site_is_primary "${1:-}" && { echo "${DDEV_SITENAME:-}"; return; }
    echo "$1.${DDEV_SITENAME:-}"
}
site_hostname() { echo "$(site_hostname_short "${1:-}").ddev.site"; }

# Database name. The primary keeps plain `db` so existing installs are untouched.
site_database() {
    site_is_primary "${1:-}" && { echo "db"; return; }
    # printf, not echo: tr -c would turn echo's trailing newline into an underscore.
    echo "db_$(printf '%s' "$1" | tr -c '[:alnum:]_' '_')"
}

# A site is "served" when its generated marker exists (extras only).
site_is_served() {
    site_is_primary "${1:-}" && return 0
    [ -f "$(site_dir "$1")/.tryout-site" ]
}

# PHP version a site runs, from its marker; falls back to the project default.
site_php_version() {
    local f
    site_is_primary "${1:-}" && { echo "${DDEV_PHP_VERSION:-}"; return; }
    f="$(site_dir "$1")/.tryout-site"
    [ -f "${f}" ] && grep -E '^php=' "${f}" | head -1 | cut -d= -f2 || echo "${DDEV_PHP_VERSION:-}"
}

# Names of all served extra sites (primary excluded).
# The EXTRA sites, never the primary. It is a sibling of theirs under
# TYPO3-Instances/ now, so the glob sees it — and site_is_served answers true for
# any name that resolves to the primary, so without this skip the primary would
# be listed as one of its own extra sites and every hostname comparison would
# count it twice.
served_site_names() {
    [ -d "${SITES_DIR}" ] || return 0
    local d name
    for d in "${SITES_DIR}"/*; do
        [ -d "${d}" ] || continue
        name="$(basename "${d}")"
        [ "${name}" = "${PRIMARY_INSTANCE}" ] && continue
        site_is_served "${name}" && echo "${name}"
    done
}

# --- Site generators ---

# Where a site's generated vhost goes, per webserver_type. DDEV copies both dirs
# into the container; ours deliberately omit the #ddev-generated marker so DDEV
# never overwrites them, and are prefixed so they cannot collide with its own
# apache-site.conf / nginx-site.conf.
site_vhost_file() {
    local name="$1"
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in
        nginx*) echo "${PROJECT_ROOT}/.ddev/nginx_full/tryout-site-${name}.conf" ;;
        *)      echo "${PROJECT_ROOT}/.ddev/apache/tryout-site-${name}.conf" ;;
    esac
}

# The http-level config that sizes nginx's server-name hash. Separate from the
# per-site vhosts because server_names_hash_bucket_size belongs in `http`, and a
# vhost file is a `server` block. DDEV includes everything in nginx_full/ at http
# scope — its own nginx-site.conf carries a `map` there, which is http-only too.
site_hash_config_file() {
    echo "${PROJECT_ROOT}/.ddev/nginx_full/tryout-server-names-hash.conf"
}

# Where the FPM master for a PHP version listens: DDEV's own for the project's
# version, ours otherwise. Must match tryout-php-fpm.sh, which binds under
# /run/php/ because /run is root-owned on some providers.
site_fpm_socket() {
    local php="$1" sock
    sock="/run/php/php-fpm-${php}.sock"
    [ "${php}" = "${DDEV_PHP_VERSION:-}" ] && sock="/run/php-fpm.sock"
    echo "${sock}"
}

# Start the FPM master for a non-default PHP version unless one is running.
#
# config.worktrees.yaml declares one per version as a web_extra_daemon, but DDEV
# bakes those into the web IMAGE: a version that was not declared when the
# container started has no master until `ddev restart` rebuilds it, and a vhost
# reloaded in place would pass every request to a socket nobody listens on. So
# start it here, detached — it outlives `ddev exec`, reparented to PID 1 — and
# let the next restart hand it to supervisord. Never a second master beside a
# live one: tryout-php-fpm.sh removes the socket before binding, so it would
# steal the first one's.
#
# "Running" is a live pid in the pid file, which FPM writes only once its socket
# is bound — so it doubles as the readiness signal. Container-side only.
ensure_php_fpm_running() {
    local php="$1" run_dir="${TRYOUT_FPM_RUN_DIR:-/run/php}" pid_file pid log i
    [ "${php}" = "${DDEV_PHP_VERSION:-}" ] && return 0
    pid_file="${run_dir}/php-fpm-${php}.pid"
    fpm_pid_alive() {
        pid="$(cat "${pid_file}" 2>/dev/null || true)"
        [ -n "${pid}" ] && kill -0 "${pid}" 2>/dev/null
    }
    fpm_pid_alive && return 0

    rm -f "${pid_file}"
    log="${TMPDIR:-/tmp}/tryout-php-fpm-${php}.log"
    info "Starting PHP ${php} FPM..."
    # setsid detaches it from the exec's session; nohup alone covers a system
    # without it (the unit suite runs on macOS).
    local detach=()
    command -v setsid >/dev/null 2>&1 && detach=(setsid)
    ${detach[@]+"${detach[@]}"} nohup bash "$(tryout_script tryout-php-fpm.sh)" "${php}" \
        >"${log}" 2>&1 </dev/null &
    # Whole seconds: bash 3.2 rejects a fractional sleep-by-read, and FPM is up in
    # about two.
    for i in 1 2 3 4 5 6 7 8 9 10; do
        fpm_pid_alive && return 0
        sleep 1
    done
    error "PHP ${php} FPM did not start:"
    tail -5 "${log}" >&2 2>/dev/null || true
    error "  → ddev restart"
    return 1
}

generate_site_vhost() {
    local name="$1" php="$2" file docroot host db sock
    file=$(site_vhost_file "${name}")
    docroot="/var/www/html/TYPO3-Instances/${name}/public"
    host=$(site_hostname "${name}")
    db=$(site_database "${name}")
    sock="$(site_fpm_socket "${php}")"

    mkdir -p "$(dirname "${file}")"
    if [ "${DDEV_WEBSERVER_TYPE:-apache-fpm}" = "nginx-fpm" ]; then
        cat > "${file}" <<NGINX_EOF
# Generated by ddev tryout worktree serve ${name} — edits will be overwritten.
server {
    listen 80;
    listen 443 ssl;
    http2 on;
    server_name ${host};
    ssl_certificate /etc/ssl/certs/master.crt;
    ssl_certificate_key /etc/ssl/certs/master.key;
    root ${docroot};
    index index.php index.html;

    location / {
        try_files \$uri \$uri/ /index.php\$is_args\$args;
    }
    location ~ [^/]\.php(/|\$) {
        try_files \$uri =404;
        fastcgi_split_path_info ^(.+?\.php)(/.*)\$;
        include fastcgi_params;
        fastcgi_param SCRIPT_FILENAME \$document_root\$fastcgi_script_name;
        fastcgi_param PATH_INFO \$fastcgi_path_info;
        fastcgi_param TRYOUT_SITE ${name};
        fastcgi_param TYPO3_DB_DBNAME ${db};
        # Without this PHP never learns the request arrived over TLS, so TYPO3
        # builds http:// URLs and its secure session cookie is never sent back —
        # the backend login then fails with "Please activate Cookies". DDEV's own
        # vhost sets the same parameter.
        fastcgi_param HTTPS \$fcgi_https;
        fastcgi_pass unix:${sock};
    }
}
NGINX_EOF
    else
        cat > "${file}" <<APACHE_EOF
# Generated by ddev tryout worktree serve ${name} — edits will be overwritten.
<VirtualHost *:80 *:443>
    ServerName ${host}
    DocumentRoot ${docroot}

    SSLEngine on
    SSLCertificateFile /etc/ssl/certs/master.crt
    SSLCertificateKeyFile /etc/ssl/certs/master.key

    SetEnvIf X-Forwarded-Proto "https" HTTPS=on
    SetEnv TRYOUT_SITE ${name}
    SetEnv TYPO3_DB_DBNAME ${db}

    <Directory "${docroot}/">
        AllowOverride All
        Require all granted
    </Directory>

    # Overrides the global conf-enabled/php*-fpm.conf handler: sites-enabled is
    # read later and a FilesMatch inside a VirtualHost is more specific.
    <FilesMatch ".+\.ph(?:ar|p|tml)\$">
        SetHandler "proxy:unix:${sock}|fcgi://localhost"
    </FilesMatch>

    ErrorLog /dev/stdout
    CustomLog /dev/stdout combined
</VirtualHost>
APACHE_EOF
    fi
}

# Rewrite .ddev/config.worktrees.yaml from the served sites: hostnames (so mkcert
# covers them) plus one supervised daemon per extra PHP version.
write_worktree_config() {
    local names hosts=() phps=() name php
    names=$(served_site_names)

    for name in ${names}; do
        hosts+=("$(site_hostname_short "${name}")")
        php=$(site_php_version "${name}")
        [ -n "${php}" ] && [ "${php}" != "${DDEV_PHP_VERSION:-}" ] && phps+=("${php}")
    done

    write_server_names_hash "${hosts[@]:-}"

    if [ ${#hosts[@]} -eq 0 ]; then
        rm -f "${WORKTREE_CONFIG}"
        return 0
    fi

    {
        echo "#ddev-silent-no-warn"
        echo "# Generated by ddev tryout worktree serve — do not edit."
        echo "# Hostnames must be registered here so mkcert includes them in the cert."
        echo ""
        echo "additional_hostnames:"
        printf '  - %s\n' "${hosts[@]}"
        # De-duplicate: several sites may share one PHP version.
        if [ ${#phps[@]} -gt 0 ]; then
            echo ""
            echo "web_extra_daemons:"
            printf '%s\n' "${phps[@]}" | sort -u | while read -r v; do
                echo "  - name: tryout-php-${v}"
                echo "    command: \"bash /var/www/html/.ddev/tryout/tryout-php-fpm.sh ${v}\""
                echo "    directory: /var/www/html"
            done
        fi
    } > "${WORKTREE_CONFIG}"
}

# nginx hashes every server_name into buckets of a fixed size, and refuses to
# START when a name does not fit:
#
#   [emerg] could not build server_names_hash, you should increase
#           server_names_hash_bucket_size: 64
#
# which takes the whole web container down — every site, not just the long one.
# The default 64 holds names of roughly 50 characters, and
# `<worktree>.<project>.ddev.site` passes that with a project and a worktree name
# of ordinary length. So size it from the longest name actually served, rounded up
# to a power of two as nginx wants, and never below the 64 it already uses.
write_server_names_hash() {
    local file longest=0 h n
    file="$(site_hash_config_file)"

    for h in "$@"; do
        [ -n "${h}" ] || continue
        # The short form here is what write_worktree_config collects; DDEV appends
        # .ddev.site to reach the name nginx actually hashes.
        n=$(( ${#h} + 11 ))
        [ "${n}" -gt "${longest}" ] && longest="${n}"
    done

    if [ "${longest}" -eq 0 ]; then
        rm -f "${file}" 2>/dev/null || true
        return 0
    fi

    # A bucket holds the name plus nginx's own per-entry overhead and padding, so
    # the usable room is well short of the bucket size. Measured, not guessed: a
    # 49-character name is what broke the default 64 in the first place, which puts
    # the overhead near 16. Allow 24 to stay clear of it, and double from 64 as the
    # nginx docs advise — a bigger bucket costs a few bytes of memory and nothing
    # else, while one too small refuses to start the whole web container.
    local size=64
    while [ "${size}" -lt $(( longest + 24 )) ]; do
        size=$(( size * 2 ))
    done

    mkdir -p "$(dirname "${file}")"
    cat > "${file}" <<HASH_EOF
#ddev-generated
# Generated by ddev tryout worktree serve — do not edit.
# Sized for the longest served hostname (${longest} chars). Without this nginx
# refuses to start once a name outgrows the default bucket, taking every site
# with it.
server_names_hash_bucket_size ${size};
HASH_EOF
}

# --- Applying a config change without ddev restart ---

# Snapshot of the served hostnames, newline-separated and sorted. The gate for
# the fast path: DDEV derives the Traefik routing rule, $VIRTUAL_HOST and the
# certificate SANs from additional_hostnames, so all three go stale exactly when
# this set changes — and only then is a restart unavoidable.
served_hostname_set() {
    local name
    for name in $(served_site_names); do
        site_hostname_short "${name}"
    done | sort
}

# Where the webserver reads its vhosts from, and how it is reloaded. DDEV's
# /start.sh copies /mnt/ddev_config/{nginx_full,apache} into these at container
# start — a COPY, not a mount, which is why a freshly written vhost is visible in
# the container yet not in effect.
#
# The reload copies from the container's OWN .ddev, not /mnt/ddev_config: that is
# a bind mount of the host's .ddev, and on a Mutagen project the vhost the
# generator just wrote reaches it only after a sync back — so a reload from there
# applied the PREVIOUS vhost. Without Mutagen both are the same directory.
site_conf_source_dir() {
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in
        nginx*) echo "${PROJECT_ROOT}/.ddev/nginx_full" ;;
        *)      echo "${PROJECT_ROOT}/.ddev/apache" ;;
    esac
}

site_conf_enabled_dir() {
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in
        nginx*) echo "/etc/nginx/sites-enabled" ;;
        *)      echo "/etc/apache2/sites-enabled" ;;
    esac
}

# Validate the webserver config as it would be loaded. Errors are the caller's to
# report, so stderr is captured rather than shown.
webserver_config_is_valid() {
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in
        nginx*) nginx -t >/dev/null 2>&1 ;;
        *)      apachectl configtest >/dev/null 2>&1 ;;
    esac
}

# Reload in place, keeping the master process. nginx is a supervisord program in
# the web image, so HUP through supervisorctl reaches it without a pid file (there
# is none at /run/nginx.pid) and without restarting the container.
reload_webserver() {
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in
        nginx*) supervisorctl signal HUP nginx >/dev/null 2>&1 ;;
        *)      apachectl -k graceful >/dev/null 2>&1 ;;
    esac
}

# Copy the generated vhosts into the running webserver and reload it, so a change
# takes effect without `ddev restart`. Container-side only.
#
# Returns non-zero without touching the running config if the result would not
# load — the caller then falls back to advising a restart. That ordering matters:
# an invalid config makes nginx refuse to START, which takes down every site in
# the project, not just the one being changed (this is the
# server_names_hash_bucket_size trap). Validating first makes this strictly safer
# than the restart it replaces, which only discovers the problem once the
# container is already down.
sync_and_reload_webserver() {
    local src dst
    src="$(site_conf_source_dir)"
    dst="$(site_conf_enabled_dir)"

    [ -d "${src}" ] || return 1
    [ -d "${dst}" ] || return 1

    # Clear our own stale copies first: unserve removes a vhost from the source,
    # and a plain cp would leave the old one serving. Scoped to the tryout-site-
    # prefix — DDEV's own nginx-site.conf lives in the same directory.
    rm -f "${dst}"/tryout-site-*.conf 2>/dev/null || true
    rm -f "${dst}"/tryout-server-names-hash.conf 2>/dev/null || true

    # Copy back per served site, by name — never a glob over the source. The
    # source used to be /mnt/ddev_config, where a deletion made on the host had
    # not necessarily arrived yet, so a glob restored the vhost unserve had just
    # removed. It reads the container's own copy now, but the markers are still
    # the definition of "served", so they drive the copy.
    local name f
    for name in $(served_site_names); do
        f="${src}/tryout-site-${name}.conf"
        [ -f "${f}" ] || continue
        cp "${f}" "${dst}/" 2>/dev/null || true
    done
    if [ -f "${src}/tryout-server-names-hash.conf" ]; then
        cp "${src}/tryout-server-names-hash.conf" "${dst}/" 2>/dev/null || true
    fi

    # DDEV's own config is copied at container start and never removed above, so
    # it needs no restoring here.

    webserver_config_is_valid || return 1
    reload_webserver || return 1
}

# Apply the site config that has just been written. Prints its own outcome, since
# what the user must do next differs per path.
#
#   hostname set changed → a restart is unavoidable: DDEV owns the Traefik routing
#                          rule and the certificate, both keyed on
#                          additional_hostnames, and neither can be refreshed from
#                          in here.
#   hostname set same    → reload the webserver in place.
#
# `before` is the hostname set captured before the change was written.
apply_site_config() {
    local before="$1" after
    after="$(served_hostname_set)"

    if [ "${before}" != "${after}" ]; then
        return 1
    fi

    in_container || return 1
    sync_and_reload_webserver || return 1
}

# Create the site's database and grant the DDEV db user access.
ensure_site_database() {
    local db
    db=$(site_database "$1")
    [ "${db}" = "db" ] && return 0
    info "Ensuring database ${db}..."
    if db_is_postgres; then
        # No IF NOT EXISTS for CREATE DATABASE in Postgres: look first.
        if ! db_root_sql "SELECT 1 FROM pg_database WHERE datname='${db}'" | grep -q 1; then
            db_root_sql "CREATE DATABASE \"${db}\"" >/dev/null \
                || { error "Failed to create database ${db}"; return 1; }
        fi
    else
        db_root_sql "CREATE DATABASE IF NOT EXISTS \`${db}\`; GRANT ALL ON \`${db}\`.* TO 'db'@'%';" \
            || { error "Failed to create database ${db}"; return 1; }
    fi
}

# Run a command in a site's context: its PHP version, its composer root, its
# database. Without this every caller has to remember the php<version> binary and
# the TYPO3_DB_DBNAME the vhost would otherwise inject.
site_exec() {
    local name="$1"; shift
    local php dir db bin="php"
    php=$(site_php_version "${name}")
    dir=$(site_dir "${name}")
    db=$(site_database "${name}")
    [ -n "${php}" ] && [ "${php}" != "${DDEV_PHP_VERSION:-}" ] && bin="php${php}"

    # The binary runs directly with "$@": no shell in between, so an argument
    # with spaces — `config:set X "My Site"` — arrives as one argument.
    # shellcheck disable=SC2086 # TRYOUT_EXTRA_ENV is deliberately word-split
    (cd "${dir}" && env TYPO3_DB_DBNAME="${db}" TRYOUT_SITE="${name}" ${TRYOUT_EXTRA_ENV:-} "${bin}" "$@")
}

# First-run TYPO3 setup for a served site, mirroring what post-start.sh does for
# the primary but against that site's docroot, vendor and database.
setup_site_typo3() {
    local name="$1" db php driver server_type
    site_is_served "${name}" || { error "Site '${name}' is not served"; return 1; }
    db=$(site_database "${name}")
    php=$(site_php_version "${name}")

    if [ -f "$(site_dir "${name}")/config/system/settings.php" ]; then
        info "Site '${name}' already configured"
        return 0
    fi

    # No settings.php, but the database may still hold the install: unserve keeps
    # it unless --drop-db was asked for. Put the saved settings back and skip the
    # setup, so an unserve/serve round trip keeps the content and the logins.
    if site_database_has_tables "${name}"; then
        local saved; saved="$(site_saved_settings "${name}")"
        if [ -f "${saved}" ]; then
            mkdir -p "$(site_dir "${name}")/config/system"
            if cp "${saved}" "$(site_dir "${name}")/config/system/settings.php"; then
                success "Site '${name}' restored — existing database kept"
                return 0
            fi
        fi
        # Tables but nothing to restore: a site unserved before this existed, or a
        # database from somewhere else. TYPO3's own setup refuses a populated
        # database ("contains already N tables"), so running it here is guaranteed
        # to fail — stop with the fix instead of letting setup emit its bare error.
        error "Database $(site_database "${name}") already holds an install, and there is"
        error "no saved settings.php for '${name}' to go with it."
        error "  → ddev tryout worktree unserve ${name} --drop-db   then serve again"
        return 1
    fi

    driver="mysqli"
    db_is_postgres && driver="postgres"
    server_type="other"
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in apache*) server_type="apache" ;; esac

    info "Running TYPO3 setup for '${name}' (db ${db}, PHP ${php})..."
    TRYOUT_EXTRA_ENV="TYPO3_DB_DRIVER=${driver}" \
        site_exec "${name}" vendor/bin/typo3 setup --no-interaction --force "--server-type=${server_type}" \
        || { error "TYPO3 setup failed for ${name}"; return 1; }
    success "Site '${name}' set up"

    # A bare `typo3 setup` builds only the backend, so / would 404. Give the site a
    # rendered frontend too — only after a REAL first setup: the early returns above
    # (already configured, restored from a kept database) skip this, and their
    # frontend is already there.
    setup_site_frontend "${name}"
}

# Give a freshly set-up site a frontend that renders at its URL.
#
# EXT:styleguide's `frontend` generator (13.4+) is the richer option — a full demo
# page tree with content. 12.4's styleguide has no CLI generator, so there it falls
# back to TYPO3's own `setup --create-site`, a plain Home page. Which one is decided
# by probing for the command, never by a version number.
#
# Best-effort throughout: the backend already works, so a frontend that will not
# generate is a warning, never a failure of serve/start. Every DB write is gated on
# the generate having succeeded, so a probe miss cannot touch a working install.
setup_site_frontend() {
    local name="$1" db
    db=$(site_database "${name}")

    # EXT:styleguide's frontend generator (13.4+) builds a full demo page tree with
    # content and its own site configuration. 12.4's styleguide has no CLI
    # generator, so there the frontend is left unprovisioned and the backend is all
    # a bare instance serves — TYPO3's own `setup --create-site` was tried and
    # produces a site 12.4 renders as "Page Not Found", so it is not a usable
    # fallback. The choice is made by probing for the command, never a version.
    #
    # Best-effort: the backend already works, so a frontend that will not generate
    # is a note, never a failure of serve/start. Every DB write is gated on the
    # generate having succeeded, so a probe miss cannot touch a working install.
    if ! site_exec "${name}" vendor/bin/typo3 help styleguide:generate >/dev/null 2>&1; then
        info "No frontend generator for this TYPO3 version — '${name}' serves the backend only"
        return 0
    fi

    info "Generating a styleguide demo frontend for '${name}'..."
    if ! site_exec "${name}" vendor/bin/typo3 styleguide:generate frontend --create >/dev/null 2>&1; then
        warn "styleguide frontend generation failed for '${name}' — backend still works"
        return 0
    fi

    # The generator makes the site root HIDDEN (no CLI flag to change that), so an
    # anonymous visitor gets a 404 until it is unhidden. The marker column
    # identifies exactly styleguide's own root, so nothing else is touched — and
    # against the site's OWN database, not the server default.
    db_site_sql "${db}" \
        "UPDATE pages SET hidden=0 WHERE is_siteroot=1 AND tx_styleguide_containsdemo='tx_styleguide_frontend_root'" \
        >/dev/null 2>&1 || warn "Could not reveal the styleguide frontend page for '${name}'"
    site_exec "${name}" vendor/bin/typo3 cache:flush >/dev/null 2>&1 || true
    success "Frontend ready for '${name}' (styleguide demo)"
}

# What `delete` is about to destroy, one line per site. The host prints it before
# asking for confirmation; the container prints it when it has to ask itself.
delete_warning() {
    local target="$1"; shift
    echo ""
    echo -e "${YELLOW}${BOLD}Warning:${NC} this destroys data for:"
    local s label
    for s in "$@"; do
        label="${s}"
        site_is_primary "${s}" && label="primary"
        echo -e "  ${BOLD}${label}${NC} — database $(site_database "${s}"), $(site_docroot "${s}")/fileadmin, settings.php"
    done
    if [ -z "${target}" ] && [ -n "$(served_site_names)" ]; then
        echo -e "  ${DIM}served sites are untouched — use 'delete <site>' or 'delete --all'${NC}"
    fi
    echo ""
}

# Wipe one site back to a fresh TYPO3 install: its own database, its own
# fileadmin, its own settings.php. Works for the primary and for served sites.
delete_site() {
    local name="$1" db docroot dir driver server_type
    db=$(site_database "${name}")
    docroot=$(site_docroot "${name}")
    dir=$(site_dir "${name}")

    info "[1/4] Recreating database ${db}..."
    if db_is_postgres; then
        db_root_sql "DROP DATABASE IF EXISTS \"${db}\"" >/dev/null 2>&1 || true
        db_root_sql "CREATE DATABASE \"${db}\"" >/dev/null \
            || { error "Failed to reset database ${db}"; return 1; }
    else
        # Re-grant: DROP removes the privileges along with the schema.
        db_root_sql "DROP DATABASE IF EXISTS \`${db}\`; CREATE DATABASE \`${db}\`; GRANT ALL ON \`${db}\`.* TO 'db'@'%';" \
            || { error "Failed to reset database ${db}"; return 1; }
    fi
    success "Database ${db} recreated"

    info "[2/4] Clearing ${docroot#"${PROJECT_ROOT}/"}/fileadmin..."
    [ -d "${docroot}/fileadmin" ] && find "${docroot}/fileadmin" -mindepth 1 -delete 2>/dev/null || true
    success "fileadmin cleared"

    info "[3/4] Removing settings.php..."
    rm -f "${dir}/config/system/settings.php"
    success "Configuration removed"

    driver="mysqli"
    db_is_postgres && driver="postgres"
    server_type="other"
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in apache*) server_type="apache" ;; esac

    info "[4/4] Running TYPO3 setup + extension:setup..."
    TRYOUT_EXTRA_ENV="TYPO3_DB_DRIVER=${driver}" \
        site_exec "${name}" vendor/bin/typo3 setup --no-interaction --force "--server-type=${server_type}" \
        || { error "TYPO3 setup failed for ${name}"; return 1; }
    site_exec "${name}" vendor/bin/typo3 extension:setup >/dev/null 2>&1 || warn "extension:setup had warnings"
    site_exec "${name}" vendor/bin/typo3 cache:flush >/dev/null 2>&1 || warn "cache:flush had warnings"
    success "Setup complete"
}

# PHP versions the web container actually provides, low to high. Read from the
# image rather than hardcoded: DDEV adds versions over time and does not validate
# --php-version, so a stale list here would silently pick a PHP that is not there.
available_php_versions() {
    ls /usr/bin/php8.* 2>/dev/null \
        | sed 's|.*/php||' \
        | grep -E '^8\.[0-9]+$' \
        | sort -V
}

# The PHP constraint a Core checkout states, from require.php in its composer.json.
# Empty when there is none or the file cannot be read; that is the caller's cue to
# fall back rather than guess.
core_php_constraint() {
    local file="$1"
    [ -f "${file}" ] || return 0
    # require.php specifically — a naive grep for "php" finds config.platform.php
    # first, which is a pinned build version, not the constraint.
    php -r '
        $f = $argv[1];
        $d = json_decode(@file_get_contents($f), true);
        echo is_array($d) ? ($d["require"]["php"] ?? "") : "";
    ' "${file}" 2>/dev/null || true
}

# Does PHP <version> (major.minor) satisfy <constraint>? Exit 0 = yes, 1 = no.
#
# Let PHP judge each candidate against the constraint. Core uses "^8.x", but an
# upper bound like ">=8.2 <8.4" has to be honoured too — reading only the floor
# would hand a capped branch a PHP it rejects.
php_satisfies() {
    local constraint="$1" version="$2"
    php -r '
        $c = $argv[1]; $v = $argv[2] . ".0"; $ok = true;
        // Split on whitespace and commas: every clause must hold.
        foreach (preg_split("/[\s,]+/", trim($c), -1, PREG_SPLIT_NO_EMPTY) as $part) {
            if (preg_match("/^\^(\d+)\.(\d+)/", $part, $m)) {
                // ^8.2 means >=8.2 and <9.0
                $ok = $ok && version_compare($v, "{$m[1]}.{$m[2]}.0", ">=")
                          && version_compare($v, ($m[1] + 1) . ".0.0", "<");
            } elseif (preg_match("/^(>=|<=|>|<|=)?\s*(\d+(?:\.\d+){0,2})$/", $part, $m)) {
                $op = $m[1] ?: ">=";
                $ok = $ok && version_compare($v, $m[2], $op === "=" ? "==" : $op);
            }
        }
        exit($ok ? 0 : 1);
    ' "${constraint}" "${version}" 2>/dev/null
}

# The PHP versions the web container provides that satisfy <constraint>, ascending.
matching_php_versions() {
    local constraint="$1" v
    while IFS= read -r v; do
        [ -n "${v}" ] || continue
        php_satisfies "${constraint}" "${v}" && echo "${v}"
    done <<EOF
$(available_php_versions)
EOF
}

# The highest available PHP a worktree's Core will accept.
#
# Core states its requirement per branch — "^8.5" on main, "^8.2" on 13.4 — so the
# project's own PHP is the wrong default for a worktree on another branch. Falls
# back to the project version when the constraint cannot be read, which keeps a
# missing or unparsable composer.json from blocking a serve.
best_php_for_worktree() {
    local name="$1" constraint best
    constraint="$(core_php_constraint "$(core_worktree_dir "${name}")/composer.json")"
    [ -n "${constraint}" ] || { echo "${DDEV_PHP_VERSION:-8.5}"; return; }
    best="$(matching_php_versions "${constraint}" | tail -1)"
    [ -n "${best}" ] || best="${DDEV_PHP_VERSION:-8.5}"
    echo "${best}"
}

# Fail fast when a site's PHP cannot run the Core it is built on.
#
# Composer reports the same mismatch, but as a resolver trace ("your php version
# (8.4.20) does not satisfy that requirement") followed by our hint to re-download,
# which is not the fix. Say what Core wants, what the site has, and the command
# that changes it. Skips when the constraint cannot be read — no host php, no
# composer.json yet — because Composer is the authority then.
check_php_for_core() {
    local dir="${1:-${CORE_DIR}}" php="${2:-${DDEV_PHP_VERSION:-}}" site="${3:-}"
    local constraint branch best
    [ -n "${php}" ] || return 0
    constraint="$(core_php_constraint "${dir}/composer.json")"
    [ -n "${constraint}" ] || return 0
    command -v php >/dev/null 2>&1 || return 0
    php_satisfies "${constraint}" "${php}" && return 0

    branch=$(git -C "${dir}" branch --show-current 2>/dev/null)
    error "TYPO3 Core${branch:+ (${branch})} requires PHP ${constraint}, but $([ -n "${site}" ] && echo "site '${site}' runs" || echo "the project runs") PHP ${php}"
    best="$(matching_php_versions "${constraint}" | tail -1)"
    if [ -n "${site}" ]; then
        error "  → ddev tryout worktree serve ${site} --php ${best:-<version>}"
    else
        error "  → ddev config --php-version=${best:-<version>} && ddev restart"
    fi
    error "  → or switch Core to a branch this PHP can run: ddev tryout checkout <branch>"
    return 1
}

# --- Serve / unserve ---

# Build a site's own composer.json from the root one, repointing the Core path repo
# at that worktree.
generate_site_composer() {
    local name="$1" php="${2:-}"
    info "Generating TYPO3-Instances/${name}/composer.tryout.json..."
    php "$(tryout_script site-composer.php)" "${name}" "${php}" >/dev/null \
        || { error "Failed to generate the Composer overlay for ${name}"; return 1; }
}

# Make a worktree into a live site: own tree, composer.json, DB, vhost and daemon.
serve_worktree() {
    local name="$1" php="${2:-}" dir hosts_before
    validate_worktree_name "${name}" || return 1
    site_is_primary "${name}" && { error "'${name}' is reserved"; return 1; }

    # Captured before the .tryout-site marker exists, so it reflects what DDEV
    # last registered. Re-serving an already-served site leaves it unchanged,
    # which is what lets the reload replace the restart.
    hosts_before="$(served_hostname_set)"

    if [ ! -d "$(core_worktree_dir "${name}")" ]; then
        error "No worktree '${name}'"
        error "  → ddev tryout worktree add ${name} <branch>"
        return 1
    fi

    # No --php given: take the highest the branch's Core will accept, not the
    # project's version — a 13.4 worktree in an 8.5 project needs its own answer.
    if [ -z "${php}" ]; then
        php="$(best_php_for_worktree "${name}")"
        info "PHP ${php} (highest this Core accepts; --php overrides)"
    fi
    # An explicit --php can still be one this Core rejects. Refuse before a
    # database and vhost exist for a site that could never install.
    check_php_for_core "$(core_worktree_dir "${name}")" "${php}" "${name}" || return 1
    dir=$(site_dir "${name}")
    mkdir -p "${dir}/config/system" "${dir}/var"

    # The marker IS the definition of "served" (site_is_served), so writing it up
    # front makes a half-built site look real to worktree list,
    # delete --all and write_worktree_config. Remove it if we do not get to the end.
    printf 'php=%s\n' "${php}" > "${dir}/.tryout-site"
    # shellcheck disable=SC2064 # expand dir/name now, not when the trap fires
    trap "rm -f '${dir}/.tryout-site'" RETURN

    # TYPO3 loads config/system/additional.php relative to its OWN root, so a
    # served site would otherwise miss the DDEV overrides — including
    # trustedHostsPattern, without which its hostname is rejected outright.
    # Symlink rather than copy so there stays one source of truth.
    # Three levels up: system -> config -> <name>, then into primary/. The count
    # is load-bearing — it was four in the old sites/<name>/ layout.
    ln -sfn ../../../primary/config/system/additional.php "${dir}/config/system/additional.php"

    generate_site_composer "${name}" "${php}" || return 1

    # Sysext set is version-specific, so sync against this worktree.
    info "Syncing TYPO3-Instances/${name}/composer.tryout.json with its Core sysexts..."
    env PROJECT_ROOT="${dir}" TRYOUT_CORE_DIR="$(core_worktree_dir "${name}")" \
        php "$(tryout_script sync-composer.php)" \
        || { error "composer sync failed for ${name}"; return 1; }

    ensure_site_database "${name}" || return 1

    # Run composer under the site's own PHP so the lock file and the generated
    # platform_check match what its vhost will actually serve.
    info "Installing dependencies for ${name} on PHP ${php} (this takes a moment)..."
    "php${php}" /usr/local/bin/composer install \
        --working-dir="${dir}" --no-interaction \
        || { error "composer install failed for ${name}"; return 1; }

    generate_site_vhost "${name}" "${php}"
    write_worktree_config

    setup_site_typo3 "${name}" || return 1

    trap - RETURN    # got to the end: the marker stands
    success "Site '${name}' prepared — PHP ${php}, db $(site_database "${name}")"
    # Before the reload, which would otherwise route to a socket nobody listens on.
    if in_container; then
        ensure_php_fpm_running "${php}" || return 1
    fi
    # The HOST restarts when the hostname set changed — it can see the same
    # markers and `ddev` only works out there. So report what happened here and
    # leave the next step to it, rather than telling the user to do a thing that
    # is already being done.
    if apply_site_config "${hosts_before}"; then
        success "Applied without a restart."
    else
        info "New hostname $(site_hostname "${name}") — it needs DDEV restarted."
    fi
    echo -e "  ${DIM}then: https://$(site_hostname "${name}")/typo3/  (admin / Password.1)${NC}"
}

# Remove the site but keep the worktree and its git state.
# Where a site's settings.php is kept while the site itself is gone. Beside the
# site dir, never inside it: unserve does `rm -rf` on that directory.
site_saved_settings() { echo "${SITES_DIR}/.${1}.settings.php"; }

# Does this site's database already hold a TYPO3 install?
site_database_has_tables() {
    local db count
    db=$(site_database "$1")
    if db_is_postgres; then
        count=$(db_root_sql "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'" 2>/dev/null | tr -dc '0-9')
    else
        count=$(db_root_sql "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='${db}';" 2>/dev/null | tr -dc '0-9')
    fi
    [ -n "${count}" ] && [ "${count}" -gt 0 ] 2>/dev/null
}

unserve_worktree() {
    local name="$1" keep_db="${2:-true}" db
    validate_worktree_name "${name}" || return 1
    site_is_served "${name}" || { error "Site '${name}' is not served"; return 1; }

    # The database outlives the site unless --drop-db was asked for, so keep the
    # settings.php that goes with it — otherwise `serve` runs a fresh TYPO3 setup
    # against a populated database and TYPO3 refuses ("contains already N tables").
    # It holds no credentials: those are in additional.php, which serve symlinks.
    local saved settings
    saved="$(site_saved_settings "${name}")"
    settings="$(site_dir "${name}")/config/system/settings.php"
    if [ "${keep_db}" = "true" ] && [ -f "${settings}" ]; then
        mkdir -p "$(dirname "${saved}")"
        cp "${settings}" "${saved}" 2>/dev/null || true
    else
        # Dropping the database makes the old settings meaningless.
        rm -f "${saved}" 2>/dev/null || true
    fi

    rm -f "$(site_vhost_file "${name}")"
    rm -rf "$(site_dir "${name}")"
    write_worktree_config

    if [ "${keep_db}" != "true" ]; then
        db=$(site_database "${name}")
        info "Dropping database ${db}..."
        if db_is_postgres; then
            db_root_sql "DROP DATABASE IF EXISTS \"${db}\"" >/dev/null || true
        else
            db_root_sql "DROP DATABASE IF EXISTS \`${db}\`;" || true
        fi
    fi

    success "Site '${name}' removed (worktree kept)"
    # Removing a site always shrinks the hostname set, so the restart is
    # unavoidable — DDEV owns the routing rule and the certificate. Still drop the
    # vhost from the running webserver, or the site keeps answering on a hostname
    # that no longer has anything behind it.
    if in_container && sync_and_reload_webserver; then
        info "Stopped serving it now; the host releases the hostname."
    else
        info "Its hostname is released when DDEV restarts."
    fi
}

# --- Gerrit patch functions ---

# Resolve a Gerrit change number to its latest patchset ref.
# Sets: PATCH_SUBJECT, PATCH_REF, PATCH_NUMBER, PATCH_STATUS
resolve_patch_ref() {
    local change_id="$1"
    local api_url="${GERRIT_API}/changes/${change_id}?o=CURRENT_REVISION"

    local result
    local exit_code=0
    result=$(bash "$(tryout_script resolve-patch-ref.sh)" "${api_url}" 2>/dev/null) || exit_code=$?

    if [ "${exit_code}" -eq 2 ]; then
        error "Failed to fetch change ${change_id} from Gerrit (HTTP error)"
        error "  → Verify: ${GERRIT_URL}${change_id}"
        return 1
    elif [ "${exit_code}" -ne 0 ]; then
        error "Failed to parse Gerrit response for change ${change_id}"
        error "  → Verify: ${GERRIT_URL}${change_id}"
        return 1
    fi

    PATCH_SUBJECT=$(echo "${result}" | sed -n '1p')
    PATCH_REF=$(echo "${result}" | sed -n '2p')
    PATCH_NUMBER=$(echo "${result}" | sed -n '3p')
    PATCH_STATUS=$(echo "${result}" | sed -n '4p')
}

# Apply a single patch by change ID.
# Sets PATCH_RESULT to: "applied", "already_applied", "merged", "abandoned", "conflict", or "error"
apply_patch() {
    local change_id="$1"
    PATCH_RESULT="error"

    ensure_gerrit_remote

    info "Resolving change ${change_id}..."
    if ! resolve_patch_ref "${change_id}"; then
        PATCH_RESULT="error"
        return 1
    fi

    echo -e "  ${BOLD}Subject:${NC}  ${PATCH_SUBJECT}"
    echo -e "  ${BOLD}Patchset:${NC} ${PATCH_NUMBER}"
    echo -e "  ${BOLD}Status:${NC}   ${PATCH_STATUS}"

    if [ "${PATCH_STATUS}" = "MERGED" ]; then
        info "Change ${change_id} is already merged — skipping"
        PATCH_RESULT="merged"
        return 0
    fi
    if [ "${PATCH_STATUS}" = "ABANDONED" ]; then
        warn "Change ${change_id} is abandoned — skipping"
        PATCH_RESULT="abandoned"
        return 0
    fi

    info "Fetching from Gerrit..."
    if ! git -C "${CORE_DIR}" fetch gerrit "${PATCH_REF}"; then
        error "Failed to fetch ref ${PATCH_REF} from Gerrit"
        error "  → Verify: ${GERRIT_URL}${change_id}"
        PATCH_RESULT="error"
        return 1
    fi

    # Check if already applied via Change-Id
    local gerrit_change_id
    gerrit_change_id=$(git -C "${CORE_DIR}" log -1 --format=%b FETCH_HEAD | grep '^Change-Id:' | head -1 | awk '{print $2}')
    if [ -n "${gerrit_change_id}" ]; then
        if git -C "${CORE_DIR}" log --format=%b "origin/${BRANCH}..HEAD" | grep -q "^Change-Id: ${gerrit_change_id}$"; then
            info "Change ${change_id} is already applied — skipping"
            PATCH_RESULT="already_applied"
            return 0
        fi
    fi

    info "Cherry-picking change ${change_id}..."
    if git -C "${CORE_DIR}" cherry-pick FETCH_HEAD 2>/dev/null; then
        success "Applied change ${change_id}: ${PATCH_SUBJECT}"
        PATCH_RESULT="applied"
    else
        git -C "${CORE_DIR}" cherry-pick --abort 2>/dev/null || true
        error "Cherry-pick failed for change ${change_id} (merge conflict)"
        error "  Cherry-pick has been aborted automatically."
        error "  → Verify: ${GERRIT_URL}${change_id}"
        PATCH_RESULT="conflict"
        return 1
    fi
}

# Print a summary table of patch results.
# Uses parallel arrays: SUMMARY_IDS, SUMMARY_SUBJECTS, SUMMARY_RESULTS
print_patch_summary() {
    local count=${#SUMMARY_IDS[@]}
    [ "${count}" -eq 0 ] && return

    echo ""
    echo -e "${BOLD}Patch Summary${NC}"
    printf "%-10s %-37s %s\n" "Change" "Subject" "Result"
    printf "%-10s %-37s %s\n" "──────────" "─────────────────────────────────────" "──────────"

    for i in $(seq 0 $((count - 1))); do
        local id="${SUMMARY_IDS[$i]}"
        local subj="${SUMMARY_SUBJECTS[$i]}"
        local result="${SUMMARY_RESULTS[$i]}"

        if [ ${#subj} -gt 35 ]; then
            subj="${subj:0:32}..."
        fi

        local colored_result
        case "${result}" in
            applied)         colored_result="${GREEN}${result}${NC}" ;;
            merged)          colored_result="${CYAN}${result}${NC}" ;;
            already_applied) colored_result="${CYAN}${result}${NC}" ;;
            abandoned)       colored_result="${YELLOW}${result}${NC}" ;;
            conflict)        colored_result="${RED}${result}${NC}" ;;
            *)               colored_result="${RED}${result}${NC}" ;;
        esac

        printf "%-10s %-37s " "${id}" "${subj}"
        echo -e "${colored_result}"
    done
    echo ""
}

# Apply all patches from TRYOUT_PATCHES environment variable.
# Sets PATCHES_APPLIED to the number of patches actually cherry-picked.
apply_all_patches() {
    PATCHES_APPLIED=0
    local patches="${TRYOUT_PATCHES:-}"
    patches=$(echo "${patches}" | tr -d '[:space:]')

    if [ -z "${patches}" ]; then
        info "No patches configured."
        return 0
    fi

    info "Applying patches: ${patches}"
    echo ""

    IFS=',' read -ra PATCH_LIST <<< "${patches}"
    local applied=0
    local skipped=0
    local failed=0

    SUMMARY_IDS=()
    SUMMARY_SUBJECTS=()
    SUMMARY_RESULTS=()

    for patch_id in "${PATCH_LIST[@]}"; do
        patch_id=$(echo "${patch_id}" | tr -d '[:space:]')
        [ -z "${patch_id}" ] && continue

        if apply_patch "${patch_id}"; then
            SUMMARY_IDS+=("${patch_id}")
            SUMMARY_SUBJECTS+=("${PATCH_SUBJECT:-unknown}")
            SUMMARY_RESULTS+=("${PATCH_RESULT}")
            case "${PATCH_RESULT}" in
                applied) applied=$((applied + 1)) ;;
                *)       skipped=$((skipped + 1)) ;;
            esac
        else
            SUMMARY_IDS+=("${patch_id}")
            SUMMARY_SUBJECTS+=("${PATCH_SUBJECT:-unknown}")
            SUMMARY_RESULTS+=("${PATCH_RESULT}")
            failed=$((failed + 1))
            warn "Stopping — remaining patches skipped due to failure."
            break
        fi
        echo ""
    done

    print_patch_summary
    # shellcheck disable=SC2034 # read by cmd_patch in commands/host/tryout
    PATCHES_APPLIED=${applied}

    if [ "${failed}" -gt 0 ]; then
        error "${failed} patch(es) failed. ${applied} applied, ${skipped} skipped."
        error "  → Reset and retry: ddev tryout reset"
        return 1
    elif [ "${applied}" -eq 0 ]; then
        info "All ${skipped} patch(es) already applied, merged or abandoned."
    else
        success "${applied} patch(es) applied, ${skipped} skipped."
    fi
}

# ─────────────────────────────────────────────────────────────────────
# Contribution setup helpers (TYPO3 Core / Gerrit workflow)
# ─────────────────────────────────────────────────────────────────────

# Resolve the Gerrit username.
# Priority: $1 arg > TRYOUT_GERRIT_USER env > git config tryout.gerritUser > prompt.
# Stores result via `git -C CORE_DIR config tryout.gerritUser` and echoes it.
resolve_gerrit_user() {
    local user="${1:-}"
    if [ -z "${user}" ]; then
        user="${TRYOUT_GERRIT_USER:-}"
    fi
    if [ -z "${user}" ]; then
        user=$(git -C "${CORE_DIR}" config --get tryout.gerritUser 2>/dev/null || true)
    fi
    if [ -z "${user}" ]; then
        if [ -t 0 ]; then
            read -r -p "Gerrit username (review.typo3.org): " user
        fi
    fi
    if [ -z "${user}" ]; then
        error "No Gerrit username provided."
        error "  → ddev tryout cs setup <username>   or   export TRYOUT_GERRIT_USER=<username>"
        return 1
    fi
    git -C "${CORE_DIR}" config tryout.gerritUser "${user}"
    GERRIT_USER="${user}"
}

# Look up a Gerrit account anonymously. Accounts are world-readable on review.typo3.org,
# so no credentials are needed.
# $1: query, e.g. "username:jdoe" or "email:jdoe@example.com"
# Sets GERRIT_ACCOUNT_ID / GERRIT_ACCOUNT_EMAIL / GERRIT_ACCOUNT_NAME on a single match.
# Returns 1 when the lookup failed (offline), 2 when nothing matched.
query_gerrit_account() {
    local query="$1"
    GERRIT_ACCOUNT_ID=""
    GERRIT_ACCOUNT_EMAIL=""
    GERRIT_ACCOUNT_NAME=""

    local result
    local exit_code=0
    result=$(bash "$(tryout_script resolve-gerrit-account.sh)" \
        "${GERRIT_API}" "${query}" 2>/dev/null) || exit_code=$?

    [ "${exit_code}" -eq 3 ] && return 2
    [ "${exit_code}" -ne 0 ] && return 1

    GERRIT_ACCOUNT_ID=$(echo "${result}" | sed -n '1p')
    GERRIT_ACCOUNT_EMAIL=$(echo "${result}" | sed -n '2p')
    GERRIT_ACCOUNT_NAME=$(echo "${result}" | sed -n '3p')
}

# Make sure commits are authored with an address the Gerrit account owns.
#
# The project allows "forgeCommitter" but not "forgeAuthor", so the author address of every
# commit must be a registered email of the account that pushes. Without a repository-local
# user.email, commits silently inherit the global git identity, which is a different account
# for anyone separating business and open source work. Gerrit then rejects the push.
configure_author_identity() {
    local user="$1"

    if ! query_gerrit_account "username:${user}"; then
        warn "Could not look up Gerrit account '${user}' — skipping author identity check"
        return 0
    fi
    if [ -z "${GERRIT_ACCOUNT_EMAIL}" ]; then
        warn "Gerrit account '${user}' exposes no preferred email — set one manually:"
        warn "  git config user.email <your-gerrit-email>"
        return 0
    fi

    git -C "${CORE_DIR}" config tryout.gerritEmail "${GERRIT_ACCOUNT_EMAIL}"

    local current
    current=$(git -C "${CORE_DIR}" config --local --get user.email 2>/dev/null || true)
    if [ "${current}" = "${GERRIT_ACCOUNT_EMAIL}" ]; then
        success "Author identity already set to ${GERRIT_ACCOUNT_EMAIL}"
        return 0
    fi

    git -C "${CORE_DIR}" config user.email "${GERRIT_ACCOUNT_EMAIL}"
    [ -n "${GERRIT_ACCOUNT_NAME}" ] && git -C "${CORE_DIR}" config user.name "${GERRIT_ACCOUNT_NAME}"
    success "Author identity set to ${GERRIT_ACCOUNT_EMAIL} (repository-local)"

    if [ -n "${current}" ]; then
        warn "Previous value was ${current} — amend commits made before this with:"
        warn "  git commit --amend --reset-author --no-edit"
    fi
}

# Fall back to the address cs setup cached when Gerrit cannot be reached.
# Only an exact match is conclusive: the cache holds the account's preferred address,
# while an account may legitimately author with any of its registered addresses. A
# differing address is therefore left unknown rather than reported as a mismatch.
author_status_from_cache() {
    local cached
    cached=$(git -C "${CORE_DIR}" config --get tryout.gerritEmail 2>/dev/null || true)
    [ -n "${cached}" ] || return 1
    [ "${cached}" = "${CS_AUTHOR_EMAIL}" ] || return 1

    CS_AUTHOR_STATUS="ok"
    CS_AUTHOR_SOURCE="cache"
}

# Check the configured author email against Gerrit.
# Sets CS_AUTHOR_EMAIL, CS_AUTHOR_SCOPE (local|inherited|none),
# CS_AUTHOR_STATUS (ok|mismatch|unregistered|unknown|no-email) and
# CS_AUTHOR_SOURCE (live|cache), which tells where an "ok" verdict came from.
inspect_author_identity() {
    CS_AUTHOR_EMAIL=$(git -C "${CORE_DIR}" config --get user.email 2>/dev/null || true)
    CS_AUTHOR_SCOPE="none"
    CS_AUTHOR_STATUS="no-email"
    CS_AUTHOR_SOURCE="live"
    [ -z "${CS_AUTHOR_EMAIL}" ] && return 0

    if [ -n "$(git -C "${CORE_DIR}" config --local --get user.email 2>/dev/null || true)" ]; then
        CS_AUTHOR_SCOPE="local"
    else
        CS_AUTHOR_SCOPE="inherited"
    fi

    local user="${CS_USER:-}"
    if [ -z "${user}" ]; then
        CS_AUTHOR_STATUS="unknown"
        return 0
    fi

    # Resolve both sides to account ids: an account may have several registered addresses and
    # only the preferred one is visible anonymously, so comparing email strings is not enough.
    local expected_id
    if ! query_gerrit_account "username:${user}"; then
        author_status_from_cache || CS_AUTHOR_STATUS="unknown"
        return 0
    fi
    expected_id="${GERRIT_ACCOUNT_ID}"

    local rc=0
    query_gerrit_account "email:${CS_AUTHOR_EMAIL}" || rc=$?
    case "${rc}" in
        0) [ "${GERRIT_ACCOUNT_ID}" = "${expected_id}" ] \
               && CS_AUTHOR_STATUS="ok" || CS_AUTHOR_STATUS="mismatch" ;;
        2) CS_AUTHOR_STATUS="unregistered" ;;
        *) author_status_from_cache || CS_AUTHOR_STATUS="unknown" ;;
    esac
}

# Install the Gerrit commit-msg hook (Change-Id) from TYPO3 Core's copy.
install_commit_msg_hook() {
    local src="${CORE_DIR}/Build/git-hooks/commit-msg"
    local dst="${CORE_GIT_DIR}/hooks/commit-msg"

    if [ ! -f "${src}" ]; then
        warn "commit-msg hook not found at ${src} — Core may be too old."
        return 1
    fi
    cp "${src}" "${dst}"
    chmod +x "${dst}"
    success "Installed commit-msg hook (Change-Id generator)"
}

# Install the TYPO3 Core pre-commit hook (CGL / PHP-CS-Fixer checks).
install_pre_commit_hook() {
    local src="${CORE_DIR}/Build/git-hooks/unix+mac/pre-commit"
    local dst="${CORE_GIT_DIR}/hooks/pre-commit"

    if [ ! -f "${src}" ]; then
        warn "pre-commit hook not found at ${src}"
        return 1
    fi
    cp "${src}" "${dst}"
    chmod +x "${dst}"
    success "Installed pre-commit hook (CGL checks)"
}

# Remove installed hooks.
remove_hooks() {
    rm -f "${CORE_GIT_DIR}/hooks/commit-msg" "${CORE_GIT_DIR}/hooks/pre-commit"
    success "Removed commit-msg and pre-commit hooks"
}

# Install the commit-message template and wire it into git config.
install_commit_template() {
    if [ ! -f "${COMMIT_TEMPLATE_SRC}" ]; then
        warn "Commit template not found at ${COMMIT_TEMPLATE_SRC}"
        return 1
    fi
    # Point git directly at the canonical template under .ddev/tryout/.
    # The path is given relative to the typo3-core working tree so it resolves
    # correctly both on the host (when running `git commit` from typo3-core/)
    # and inside the DDEV container.
    # The Core working tree IS the project root now, so .ddev/ is a direct child.
    local tmpl_path=".ddev/tryout/gitmessage.txt"
    git -C "${CORE_DIR}" config commit.template "${tmpl_path}"
    # Drop any leftover copy from an older setup; the canonical file lives
    # in .ddev/tryout/ now.
    rm -f "${CORE_GIT_DIR}message.txt"
    success "Commit template wired to ${DIM}${tmpl_path}${NC}"
}

# Configure push URL to Gerrit SSH so `git push` submits to review.
configure_gerrit_push_url() {
    local user="${1:-${GERRIT_USER:-}}"
    if [ -z "${user}" ]; then
        error "configure_gerrit_push_url: no username"
        return 1
    fi
    local push_url="ssh://${user}@${GERRIT_SSH_HOST}:${GERRIT_SSH_PORT}/${GERRIT_PROJECT}"
    git -C "${CORE_DIR}" remote set-url --push origin "${push_url}"
    # Refs go to refs/for/<branch> — user still needs `git push origin HEAD:refs/for/main`.
    success "Push URL set: ${DIM}${push_url}${NC}"
}

# Diagnose Gerrit SSH reachability + authentication.
# Returns 0 on full success. On failure sets CS_SSH_REASON to one of:
#   no-user       no Gerrit username configured
#   unreachable   TCP port 29418 is not reachable
#   no-agent-key  reachable, but ddev-ssh-agent holds no identities
#   denied        keys were presented but Gerrit refused them
diagnose_gerrit_ssh() {
    local user="${1:-${GERRIT_USER:-}}"
    CS_SSH_REASON=""
    if [ -z "${user}" ]; then
        CS_SSH_REASON="no-user"
        return 1
    fi

    # 1. Raw TCP reachability to the Gerrit SSH port.
    #
    # NOT bash's /dev/tcp: on macOS the kernel SIGKILLs a shell that opens one to
    # an external host, so the probe died with "Killed: 9" printed straight to the
    # terminal and the doctor then reported the port unreachable — on a machine
    # where it was reachable, and one line after a successful authenticated call
    # to the very same host. (Verified: /dev/tcp to 127.0.0.1 works, to any
    # external host it is killed, rc=137.)
    #
    # `nc -z -w` is quiet, takes the same flags on the BSD nc macOS ships and on
    # GNU/OpenBSD nc, and reports the truth. Where there is no nc at all, skip
    # this step rather than guess: the auth probe below is a real connection to the
    # same host and port, so an unreachable server cannot slip through — it just
    # says "denied" instead of "unreachable".
    if command -v nc >/dev/null 2>&1; then
        if ! nc -z -w 5 "${GERRIT_SSH_HOST}" "${GERRIT_SSH_PORT}" >/dev/null 2>&1; then
            CS_SSH_REASON="unreachable"
            return 1
        fi
    fi

    # 2. Ask SSH, rather than guess from what an agent happens to hold. This is the
    #    thing being reported on, so it decides — an ssh-add check ahead of it
    #    answers a DIFFERENT question and used to return early on its answer:
    #    a host whose agent is empty but which authenticates from a key on disk
    #    (~/.ssh/id_rsa, offered and accepted by Gerrit — the ordinary case) was
    #    told "no-agent-key" and pointed at an ssh-add it did not need.
    #
    #    Costs one round trip where the early return was free. Worth it: the
    #    short-circuit was cheap because it was answering the wrong question.
    if ssh -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new \
           -p "${GERRIT_SSH_PORT}" "${user}@${GERRIT_SSH_HOST}" gerrit version >/dev/null 2>&1; then
        return 0
    fi

    # 3. It failed — now work out why, to say something better than "denied".
    #    No identity anywhere is its own diagnosis: in the CONTAINER that is the
    #    normal shape of the problem, since ~/.ssh there holds only `config` and
    #    the forwarded ddev-ssh-agent is the only route, so an empty one fails
    #    with a misleading "permission denied". A key present but refused is a
    #    different fix (upload it to Gerrit), so keep the two apart.
    #
    #    The on-disk test mirrors what ssh itself would try by default. Plain
    #    globbing, no bash-4 features.
    local have_key="false" k
    ssh-add -l >/dev/null 2>&1 && have_key="true"
    if [ "${have_key}" = "false" ]; then
        for k in "${HOME}/.ssh"/id_*; do
            case "${k}" in *.pub) continue ;; esac
            [ -r "${k}" ] && { have_key="true"; break; }
        done
    fi
    if [ "${have_key}" = "false" ]; then
        CS_SSH_REASON="no-agent-key"
        return 1
    fi

    CS_SSH_REASON="denied"
    return 1
}

# Back-compat wrapper: older callers just want a 0/non-zero answer.
verify_gerrit_ssh() {
    diagnose_gerrit_ssh "$@"
}

# Produce a short, dim-coloured next-step hint for the current CS_SSH_REASON.
# Used by both the setup flow and the doctor report.
gerrit_ssh_hint() {
    case "${CS_SSH_REASON:-}" in
        no-user)
            echo "→ set a username: ddev tryout cs setup <gerrit-user>" ;;
        unreachable)
            echo "→ check firewall/VPN for ${GERRIT_SSH_HOST}:${GERRIT_SSH_PORT}" ;;
        no-agent-key)
            if in_container; then
                echo "→ ddev auth ssh   (hands your host keys to ddev-ssh-agent)"
            else
                echo "→ load your key into your host SSH agent, e.g.: ssh-add ~/.ssh/id_ed25519"
            fi ;;
        denied)
            echo "→ upload your public key at https://review.typo3.org/settings/#SSHKeys" ;;
        *)
            echo "" ;;
    esac
}

# One line about the HOST's SSH agent, printed after cs setup/doctor ran in the
# container. The probe in there covers ddev-ssh-agent; a push from a host shell
# uses the host's keys instead, and both answers are worth having.
host_gerrit_ssh_report() {
    local user="${1:-}"
    [ -n "${user}" ] || return 0
    if diagnose_gerrit_ssh "${user}"; then
        echo -e "  Host SSH:        ${GREEN}✓${NC} reachable (authenticated) — pushing from a host shell works"
    else
        echo -e "  Host SSH:        ${YELLOW}!${NC} ${CS_SSH_REASON} ${DIM}$(gerrit_ssh_hint)${NC}"
    fi
    echo ""
}

# Report the current state of contribution setup.
# Sets: CS_HOOK_COMMIT_MSG, CS_HOOK_PRE_COMMIT, CS_TEMPLATE, CS_PUSH_URL, CS_USER (0/1 flags or value)
inspect_contribution_setup() {
    CS_HOOK_COMMIT_MSG=0
    CS_HOOK_PRE_COMMIT=0
    CS_TEMPLATE=0
    CS_PUSH_URL=""
    CS_USER=""

    [ -x "${CORE_GIT_DIR}/hooks/commit-msg" ] && CS_HOOK_COMMIT_MSG=1
    [ -x "${CORE_GIT_DIR}/hooks/pre-commit" ]  && CS_HOOK_PRE_COMMIT=1

    local tmpl
    tmpl=$(git -C "${CORE_DIR}" config --get commit.template 2>/dev/null || true)
    if [ -n "${tmpl}" ] && [ -f "${CORE_DIR}/${tmpl}" ]; then
        CS_TEMPLATE=1
    fi

    CS_PUSH_URL=$(git -C "${CORE_DIR}" remote get-url --push origin 2>/dev/null || true)
    CS_USER=$(git -C "${CORE_DIR}" config --get tryout.gerritUser 2>/dev/null || true)
}

# ─────────────────────────────────────────────────────────────────────
# Contribution setup (ddev tryout cs)
# ─────────────────────────────────────────────────────────────────────

# ─────────────────────────────────────────────────────────────────────
# setup — Wire up hooks, template, push URL
# ─────────────────────────────────────────────────────────────────────
cmd_cs_setup() {
    require_core

    echo ""
    echo -e "${BOLD}TYPO3 Core — Contribution Setup${NC}"
    echo "─────────────────────────────────────"
    echo ""

    info "[1/6] Resolving Gerrit username..."
    if ! resolve_gerrit_user "${1:-}"; then
        exit 1
    fi
    echo -e "       ${DIM}user: ${GERRIT_USER}${NC}"
    echo ""

    info "[2/6] Installing commit-msg hook (Change-Id)..."
    install_commit_msg_hook || warn "commit-msg hook install skipped"
    echo ""

    info "[3/6] Installing pre-commit hook (CGL checks)..."
    install_pre_commit_hook || warn "pre-commit hook install skipped"
    echo ""

    info "[4/6] Installing commit-message template..."
    install_commit_template || warn "template install skipped"
    echo ""

    info "[5/6] Configuring Gerrit push URL..."
    configure_gerrit_push_url "${GERRIT_USER}"
    echo ""

    info "[6/6] Configuring commit author identity..."
    configure_author_identity "${GERRIT_USER}"
    echo ""

    # Optional SSH probe — informational only
    info "Probing Gerrit SSH (${GERRIT_USER}@${GERRIT_SSH_HOST}:${GERRIT_SSH_PORT})..."
    if diagnose_gerrit_ssh "${GERRIT_USER}"; then
        success "SSH reachable — you can push to Gerrit"
    else
        warn "SSH probe failed (${CS_SSH_REASON})"
        echo -e "       ${DIM}$(gerrit_ssh_hint)${NC}"
    fi

    echo ""
    echo "─────────────────────────────────────"
    success "Contribution setup complete!"
    echo ""
    echo -e "  ${BOLD}Push a change for review:${NC}"
    echo -e "    ${DIM}git push origin HEAD:refs/for/${BRANCH}${NC}"
    echo ""
    echo -e "  ${BOLD}Diagnose state:${NC} ddev tryout cs doctor"
    echo ""
}

# ─────────────────────────────────────────────────────────────────────
# doctor — Diagnose current contribution setup
# ─────────────────────────────────────────────────────────────────────
cmd_cs_doctor() {
    require_core
    inspect_contribution_setup

    local OK="${GREEN}✓${NC}"
    local FAIL="${RED}✗${NC}"
    local WARN="${YELLOW}!${NC}"

    echo ""
    echo -e "${BOLD}Contribution Setup — Doctor${NC}"
    echo "─────────────────────────────────────"

    if [ -n "${CS_USER}" ]; then
        echo -e "  Gerrit user:     ${OK} ${CS_USER}"
    else
        echo -e "  Gerrit user:     ${FAIL} not set"
        echo -e "                   ${DIM}→ ddev tryout cs setup <username>${NC}"
    fi

    if [ "${CS_HOOK_COMMIT_MSG}" = "1" ]; then
        echo -e "  commit-msg hook: ${OK} installed"
    else
        echo -e "  commit-msg hook: ${FAIL} missing"
    fi

    if [ "${CS_HOOK_PRE_COMMIT}" = "1" ]; then
        echo -e "  pre-commit hook: ${OK} installed"
    else
        echo -e "  pre-commit hook: ${FAIL} missing"
    fi

    if [ "${CS_TEMPLATE}" = "1" ]; then
        echo -e "  Commit template: ${OK} configured"
    else
        echo -e "  Commit template: ${FAIL} not set"
    fi

    if echo "${CS_PUSH_URL}" | grep -q "^ssh://.*@${GERRIT_SSH_HOST}"; then
        echo -e "  Push URL:        ${OK} ${CS_PUSH_URL}"
    elif [ -n "${CS_PUSH_URL}" ]; then
        echo -e "  Push URL:        ${WARN} ${CS_PUSH_URL}"
        echo -e "                   ${DIM}(not pointing at Gerrit SSH)${NC}"
    else
        echo -e "  Push URL:        ${FAIL} origin missing"
    fi

    inspect_author_identity
    case "${CS_AUTHOR_STATUS}" in
        ok)
            echo -e "  Author identity: ${OK} ${CS_AUTHOR_EMAIL}"
            if [ "${CS_AUTHOR_SOURCE}" = "cache" ]; then
                echo -e "                   ${DIM}(from cache — Gerrit unreachable)${NC}"
            fi
            if [ "${CS_AUTHOR_SCOPE}" != "local" ]; then
                echo -e "                   ${WARN} inherited from your global git config"
                echo -e "                   ${DIM}→ ddev tryout cs setup ${CS_USER}${NC}"
            fi
            ;;
        mismatch)
            echo -e "  Author identity: ${FAIL} ${CS_AUTHOR_EMAIL} belongs to another Gerrit account"
            echo -e "                   ${DIM}→ ddev tryout cs setup ${CS_USER}${NC}"
            ;;
        unregistered)
            echo -e "  Author identity: ${FAIL} ${CS_AUTHOR_EMAIL} is not registered on Gerrit"
            echo -e "                   ${DIM}pushes are rejected as \"invalid author\"${NC}"
            echo -e "                   ${DIM}→ ddev tryout cs setup ${CS_USER}${NC}"
            ;;
        no-email)
            echo -e "  Author identity: ${FAIL} no user.email configured"
            echo -e "                   ${DIM}→ ddev tryout cs setup ${CS_USER}${NC}"
            ;;
        *)
            echo -e "  Author identity: ${WARN} ${CS_AUTHOR_EMAIL} (could not verify)"
            ;;
    esac

    # Live SSH check (only if we have a user)
    if [ -n "${CS_USER}" ]; then
        if diagnose_gerrit_ssh "${CS_USER}"; then
            echo -e "  Gerrit SSH:      ${OK} reachable (authenticated)"
        else
            case "${CS_SSH_REASON}" in
                unreachable)
                    echo -e "  Gerrit SSH:      ${FAIL} network unreachable" ;;
                no-agent-key)
                    echo -e "  Gerrit SSH:      ${WARN} no key in ddev-ssh-agent" ;;
                denied)
                    echo -e "  Gerrit SSH:      ${FAIL} auth denied by Gerrit" ;;
                *)
                    echo -e "  Gerrit SSH:      ${FAIL} probe failed (${CS_SSH_REASON})" ;;
            esac
            echo -e "                   ${DIM}$(gerrit_ssh_hint)${NC}"
        fi
    fi

    echo "─────────────────────────────────────"
    echo ""
}

# ─────────────────────────────────────────────────────────────────────
# uninstall — Revert contribution setup
# ─────────────────────────────────────────────────────────────────────
cmd_cs_uninstall() {
    require_core

    info "Removing git hooks..."
    remove_hooks

    info "Unsetting commit template..."
    git -C "${CORE_DIR}" config --unset commit.template 2>/dev/null || true
    rm -f "${CORE_DIR}/.gitmessage.txt"

    info "Resetting origin push URL to fetch URL..."
    local fetch_url
    fetch_url=$(git -C "${CORE_DIR}" remote get-url origin 2>/dev/null || echo "")
    if [ -n "${fetch_url}" ]; then
        git -C "${CORE_DIR}" remote set-url --push origin "${fetch_url}"
    fi

    git -C "${CORE_DIR}" config --unset tryout.gerritUser 2>/dev/null || true
    git -C "${CORE_DIR}" config --unset tryout.gerritEmail 2>/dev/null || true
    success "Contribution setup removed"
}

cmd_cs_help() {
    echo ""
    echo -e "${BOLD}ddev tryout cs${NC} — TYPO3 Core contribution setup"
    echo ""
    echo "Commands:"
    echo -e "  ${BOLD}setup [user]${NC}   Install hooks, template, and Gerrit push URL (default)"
    echo -e "  ${BOLD}doctor${NC}         Diagnose the current contribution setup"
    echo -e "  ${BOLD}uninstall${NC}      Remove hooks, template, and reset push URL"
    echo -e "  ${BOLD}help${NC}           Show this help"
    echo ""
    echo "Username resolution order:"
    echo "  1. argument:   ddev tryout cs setup jdoe"
    echo "  2. env:        TRYOUT_GERRIT_USER=jdoe"
    echo "  3. git config: tryout.gerritUser (cached from previous setup)"
    echo "  4. prompt      (interactive)"
    echo ""
}
