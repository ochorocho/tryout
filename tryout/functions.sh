#!/usr/bin/env bash
#ddev-generated

# Shared functions for TYPO3 tryout DDEV commands.
# Source this file: source "${DDEV_APPROOT}/.ddev/tryout/functions.sh"

PROJECT_ROOT="${DDEV_APPROOT}"
CORE_DIR="${PROJECT_ROOT}/typo3-core"
CORE_GIT_DIR="${CORE_DIR}/.git"
CORE_REPO="https://github.com/typo3/typo3.git"
GERRIT_REMOTE="https://review.typo3.org/Packages/TYPO3.CMS"
GERRIT_API="https://review.typo3.org"
GERRIT_URL="https://review.typo3.org/c/Packages/TYPO3.CMS/+/"
GERRIT_SSH_HOST="review.typo3.org"
GERRIT_SSH_PORT="29418"
GERRIT_PROJECT="Packages/TYPO3.CMS"
COMMIT_TEMPLATE_SRC="${PROJECT_ROOT}/.ddev/tryout/gitmessage.txt"

# Core worktrees live next to the main clone as typo3-core-<name>; CORE_DIR is a
# symlink to whichever one is active. See `ddev tryout worktree`.
CORE_WORKTREE_PREFIX="${PROJECT_ROOT}/typo3-core-"
DEFAULT_CORE_WORKTREE="main"

# Served sites. The PRIMARY site is the project root (public/, vendor/, config/)
# serving whichever worktree typo3-core points at; every other served worktree gets
# its own tree under sites/<name>/. See `ddev tryout worktree serve`.
SITES_DIR="${PROJECT_ROOT}/sites"
PRIMARY_SITE="@primary"
WORKTREE_CONFIG="${PROJECT_ROOT}/.ddev/config.worktrees.yaml"
FPM_WRAPPER="${PROJECT_ROOT}/.ddev/tryout/tryout-php-fpm.sh"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# --- Output helpers ---
info()    { echo -e "${CYAN}==>${NC} $*"; }
success() { echo -e "${GREEN}==>${NC} $*"; }
warn()    { echo -e "${YELLOW}==>${NC} $*"; }
error()   { echo -e "${RED}✗${NC} $*" >&2; }

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
# nothing but main contains it do we say main.
detect_detached_base_branch() {
    local refs found
    # Drop origin/HEAD, which for-each-ref reports as a bare "origin".
    refs=$(git -C "${CORE_DIR}" for-each-ref --format='%(refname:short)' \
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
        error "TYPO3 Core not found at typo3-core/"
        error "  → Run: ddev tryout download"
        exit 1
    fi
}

# Rebuild a site. Called with no argument it targets the primary, exactly as before,
# so the existing call sites keep their behaviour; pass a served site name to rebuild
# that one under its own PHP, composer root and database.
rebuild_typo3() {
    local name="${1:-${PRIMARY_SITE}}"

    if site_is_primary "${name}"; then
        info "Running composer install..."
        ddev composer install || { error "Composer install failed"; return 1; }
        info "Running extension:setup..."
        ddev typo3 extension:setup 2>/dev/null || true
        info "Flushing caches..."
        rm -rf "${PROJECT_ROOT}/var/cache"/* 2>/dev/null || true
        ddev typo3 cache:flush 2>/dev/null || true
        success "Rebuild complete"
        return
    fi

    local php
    php=$(site_php_version "${name}")
    info "Running composer install for '${name}' on PHP ${php}..."
    ddev exec "php${php}" /usr/local/bin/composer install \
        --working-dir="/var/www/html/sites/${name}" --no-interaction \
        || { error "Composer install failed for ${name}"; return 1; }
    info "Running extension:setup for '${name}'..."
    site_exec "${name}" "vendor/bin/typo3 extension:setup" >/dev/null 2>&1 || true
    info "Flushing caches for '${name}'..."
    rm -rf "$(site_dir "${name}")/var/cache"/* 2>/dev/null || true
    site_exec "${name}" "vendor/bin/typo3 cache:flush" >/dev/null 2>&1 || true
    success "Rebuild complete for '${name}'"
}

reset_core_to_main() {
    git -C "${CORE_DIR}" fetch origin
    # A detached worktree has no local branch to check out; reset in place.
    if git -C "${CORE_DIR}" symbolic-ref -q HEAD >/dev/null 2>&1 || \
       git -C "${CORE_DIR}" show-ref -q --verify "refs/heads/${BRANCH}" 2>/dev/null; then
        git -C "${CORE_DIR}" checkout "${BRANCH}" 2>/dev/null \
            || git -C "${CORE_DIR}" checkout -b "${BRANCH}" "origin/${BRANCH}"
    fi
    git -C "${CORE_DIR}" reset --hard "origin/${BRANCH}"
    git -C "${CORE_DIR}" clean -fd
    # ${1} names the site whose cache to drop; defaults to the primary's.
    rm -rf "$(site_dir "${1:-${PRIMARY_SITE}}")/var/cache"/* 2>/dev/null || true
}

# ─────────────────────────────────────────────────────────────────────
# Core worktrees
#
# typo3-core is a symlink to the active typo3-core-<name>. Keeping the path
# stable means composer.json's path repository, sync-composer.php and CORE_DIR
# all keep working untouched. Composer resolves the symlink when it writes
# vendor/ links, so a switch MUST be followed by a rebuild — see use_core_worktree.
# ─────────────────────────────────────────────────────────────────────

core_worktree_dir() { echo "${CORE_WORKTREE_PREFIX}$1"; }

# With Mutagen (the default performance mode on macOS) a directory just created on
# the host is not yet visible in the container, so anything that runs in there —
# sync-composer.php, composer install — fails with a confusing "not found". Flush
# the sync before handing a new tree over to container-side work. A no-op when
# Mutagen is off.
sync_to_container() {
    [ "${DDEV_MUTAGEN_ENABLED:-false}" = "true" ] || return 0
    info "Syncing to container..."
    ddev mutagen sync >/dev/null 2>&1 || true
}

# A name becomes a directory, so keep it strictly harmless (no slashes, no ..).
validate_worktree_name() {
    local name="${1:-}"
    if [ -z "${name}" ]; then
        error "Missing worktree name"
        error "  → ddev tryout worktree add <name> [<branch>]"
        return 1
    fi
    if ! echo "${name}" | grep -Eq '^[A-Za-z0-9._-]+$'; then
        error "Invalid worktree name '${name}' (allowed: letters, digits, . _ -)"
        return 1
    fi
    if [ "${name}" = "." ] || [ "${name}" = ".." ]; then
        error "Invalid worktree name '${name}'"
        return 1
    fi
}

core_is_symlinked() { [ -L "${CORE_DIR}" ]; }

# Name of the active worktree, empty on the legacy plain-clone layout.
active_worktree_name() {
    core_is_symlinked || return 0
    basename "$(readlink "${CORE_DIR}")" | sed "s|^$(basename "${CORE_WORKTREE_PREFIX}")||"
}

# The worktree that owns the object store; git lists it first. worktree add must
# run against a real worktree, and it can never be removed.
main_core_worktree_dir() {
    git -C "${CORE_DIR}" worktree list --porcelain 2>/dev/null \
        | awk '/^worktree /{print substr($0,10); exit}'
}

core_worktree_is_dirty() {
    local dir="$1"
    ! git -C "${dir}" diff --quiet 2>/dev/null \
        || ! git -C "${dir}" diff --cached --quiet 2>/dev/null
}

# One-time move to the symlink layout: the real clone becomes typo3-core-<branch>
# and typo3-core becomes a symlink to it. Idempotent; never runs unless a
# worktree command asks for it, so existing installs stay untouched.
migrate_core_to_worktree_layout() {
    core_is_symlinked && return 0

    local name target
    name=$(git -C "${CORE_DIR}" branch --show-current 2>/dev/null || true)
    [ -z "${name}" ] && name="${DEFAULT_CORE_WORKTREE}"
    validate_worktree_name "${name}" || name="${DEFAULT_CORE_WORKTREE}"
    target=$(core_worktree_dir "${name}")

    if [ -e "${target}" ]; then
        error "Cannot migrate: ${target} already exists"
        error "  → Move it aside, then retry"
        return 1
    fi
    if core_worktree_is_dirty "${CORE_DIR}"; then
        error "TYPO3 Core has uncommitted changes — refusing to migrate"
        error "  → Commit or stash them in typo3-core/, then retry"
        return 1
    fi

    info "Migrating typo3-core/ to the worktree layout..."
    mv "${CORE_DIR}" "${target}"
    ln -sfn "$(basename "${target}")" "${CORE_DIR}"
    success "typo3-core -> $(basename "${target}")"
}

# Create a sibling worktree. Detached by default: git refuses one branch in two
# worktrees, and the common case is several worktrees on the same tip carrying
# different Gerrit patches. Pushes go to refs/for/<branch>, never from a local
# branch, so a detached HEAD is the normal working state here.
add_core_worktree() {
    local name="$1" branch="${2:-${BRANCH}}" attach="${3:-false}" dir
    validate_worktree_name "${name}" || return 1
    dir=$(core_worktree_dir "${name}")

    if [ -e "${dir}" ]; then
        error "Worktree '${name}' already exists at $(basename "${dir}")"
        error "  → ddev tryout worktree use ${name}"
        return 1
    fi

    local main_dir
    main_dir=$(main_core_worktree_dir)
    [ -z "${main_dir}" ] && main_dir="${CORE_DIR}"

    info "Fetching origin..."
    git -C "${main_dir}" fetch origin || { error "Fetch failed"; return 1; }

    if ! git -C "${main_dir}" rev-parse --verify --quiet "origin/${branch}" >/dev/null; then
        error "Branch '${branch}' does not exist on origin"
        error "  → ddev tryout checkout   (lists available branches)"
        return 1
    fi

    info "Creating worktree '${name}' at origin/${branch}..."
    if [ "${attach}" = "true" ]; then
        git -C "${main_dir}" worktree add -B "${branch}" "${dir}" "origin/${branch}" || return 1
    else
        git -C "${main_dir}" worktree add --detach "${dir}" "origin/${branch}" || return 1
    fi
    sync_to_container
    success "Worktree '${name}' created"
}

set_active_core() {
    ln -sfn "$(basename "$(core_worktree_dir "$1")")" "${CORE_DIR}"
}

# Switch the active Core. The rebuild is mandatory, never optional: Composer
# binds vendor/ to the resolved real path, so without it the site silently keeps
# serving the previous Core.
use_core_worktree() {
    local name="$1" force="${2:-false}" dir
    validate_worktree_name "${name}" || return 1
    dir=$(core_worktree_dir "${name}")

    if [ ! -d "${dir}" ]; then
        error "No worktree '${name}'"
        error "  → ddev tryout worktree list"
        return 1
    fi

    local active
    active=$(active_worktree_name)
    if [ "${active}" = "${name}" ]; then
        info "'${name}' is already active — rebuilding anyway"
    elif [ -n "${active}" ] && [ "${force}" != "true" ] \
         && core_worktree_is_dirty "$(core_worktree_dir "${active}")"; then
        error "Active worktree '${active}' has uncommitted changes"
        error "  Switching would hide them from typo3-core/."
        error "  → Commit or stash them, or: ddev tryout worktree use ${name} --force"
        return 1
    fi

    set_active_core "${name}"
    success "Active Core: ${name}"

    # Sysext sets differ between versions, so regenerate before installing.
    info "Syncing composer.tryout.json..."
    ddev php /var/www/html/.ddev/tryout/sync-composer.php || warn "composer sync had warnings"
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

    local args=("worktree" "remove")
    [ "${force}" = "true" ] && args+=("--force")
    if ! git -C "${CORE_DIR}" "${args[@]}" "${dir}"; then
        error "Failed to remove worktree '${name}'"
        error "  → Uncommitted changes? Retry with --force"
        return 1
    fi
    git -C "${CORE_DIR}" worktree prune 2>/dev/null || true
    success "Removed worktree '${name}'"
}

# Emit "name<TAB>head<TAB>branch<TAB>dirty<TAB>active" per worktree.
list_core_worktrees() {
    local active dir name head branch dirty
    active=$(active_worktree_name)
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

# --- herdr integration -----------------------------------------------------
# herdr (https://herdr.dev) is a terminal multiplexer built around coding agents.
# It is an OPTIONAL host tool: the add-on never installs it, and nothing else here
# depends on it. `ddev tryout herdr` opens one workspace per Core worktree.

# This project's own herdr session, so Core worktrees never land among the user's
# everyday workspaces. Session names accept dots and uppercase, so the DDEV project
# name goes in verbatim.
herdr_session_name() {
    # DDEV only exports DDEV_SITENAME to commands it runs itself. The herdr popups
    # are launched by herdr, not ddev, so fall back to the project config — without
    # this they resolve to the session "tryout" and talk to a server that is not
    # there.
    local name="${DDEV_SITENAME:-}"
    if [ -z "${name}" ] && [ -f "${PROJECT_ROOT}/.ddev/config.yaml" ]; then
        name="$(sed -n 's/^name: *//p' "${PROJECT_ROOT}/.ddev/config.yaml" 2>/dev/null \
                | head -1 | sed -e 's/^["'"'"']//' -e 's/["'"'"']$//')"
    fi
    [ -n "${name}" ] && { echo "tryout-${name}"; return; }
    echo "tryout"
}

# Every herdr call goes through here. `--session` MUST precede the subcommand: put it
# after and herdr SILENTLY IGNORES it and talks to the default session instead.
# (HERDR_SESSION is no good either — it reports the right name but resolves the
# default socket.)
herdr_cli() { herdr --session "$(herdr_session_name)" "$@"; }

# The binary and the JSON parser we need. Not a check for a running server: we start
# one ourselves. Deliberately NOT gated on HERDR_ENV either — the herdr CLI talks to
# a socket, not to the calling pane, and a DDEV host command never runs inside one.
# Install hints worth reading: name the tool, why it is needed, and the command for
# the platform actually in use rather than a list to pick from.
missing_tool() {
    local tool="$1" why="$2" brew="$3" apt="$4" docs="$5"
    error "${tool} is not installed on the host"
    error "  ${why}"
    # uname may be missing from a stripped PATH; fall back to bash's own OSTYPE.
    local os="${OSTYPE:-}"
    command -v uname >/dev/null 2>&1 && os="$(uname -s)"
    case "${os}" in
        Darwin|darwin*)
            error "  → brew install ${brew}"
            ;;
        Linux|linux*)
            if command -v apt-get >/dev/null 2>&1; then
                error "  → sudo apt-get install ${apt}"
            elif command -v dnf >/dev/null 2>&1; then
                error "  → sudo dnf install ${apt}"
            elif command -v pacman >/dev/null 2>&1; then
                error "  → sudo pacman -S ${apt}"
            else
                error "  → install '${apt}' with your package manager"
            fi
            ;;
    esac
    [ -n "${docs}" ] && error "  ${DIM}${docs}${NC}"
    return 1
}

herdr_available() {
    if ! command -v herdr >/dev/null 2>&1; then
        error "herdr is not installed on the host"
        error "  'ddev tryout herdr' opens your Core worktrees in it."
        # Homebrew carries it; no Linux distro packages it, so the installer is the
        # honest answer there rather than an apt-get line that would fail.
        case "${OSTYPE:-$(uname -s 2>/dev/null)}" in
            darwin*|Darwin) error "  → brew install herdr" ;;
        esac
        error "  → curl -fsSL https://herdr.dev/install.sh | sh"
        error "  ${DIM}https://herdr.dev/docs/install/${NC}"
        return 1
    fi
    # Control commands answer in JSON; we parse IDs out rather than predict them.
    if ! command -v jq >/dev/null 2>&1; then
        missing_tool "jq" \
            "herdr answers in JSON and its replies have to be parsed." \
            "jq" "jq" "https://jqlang.github.io/jq/download/"
        return 1
    fi
}

# Drop the user into their session. A DDEV host command has no stdin/stdout tty of
# its own, but /dev/tty still reaches the real terminal, so herdr can take it over —
# this is what makes `ddev tryout herdr` land you in the session rather than printing
# a command to copy. Two cases cannot attach:
#   - no controlling terminal (a script, CI, an editor task runner)
#   - already inside herdr, which refuses to nest
# Both fall back to printing the command, so nothing is lost.
attach_herdr_session() {
    local session
    session="$(herdr_session_name)"

    if [ "${HERDR_ENV:-}" = "1" ]; then
        info "  ${DIM}→ already in herdr; switch to '${session}' or: herdr session attach ${session}${NC}"
        return 0
    fi

    if [ ! -e /dev/tty ] || ! { : < /dev/tty; } 2>/dev/null; then
        info "  ${DIM}→ herdr session attach ${session}${NC}"
        return 0
    fi

    info "Attaching to '${session}'..."
    # Deliberately NOT herdr_cli: `session attach` takes the session as its argument,
    # and this call must own the real terminal. Every other call goes through the
    # wrapper; a unit test allows this one line by name.
    herdr session attach "${session}" < /dev/tty > /dev/tty 2>&1
}

# Start this project's session unless it is already up. A server spawned here
# outlives the command, which is what makes `ddev tryout herdr` usable at all: a DDEV
# host command has no TTY, so it can never host the session itself.
ensure_herdr_session() {
    herdr_cli status server --json 2>/dev/null | grep -q '"running":true' && return 0

    info "Starting herdr session '$(herdr_session_name)'..."
    nohup herdr --session "$(herdr_session_name)" server >/dev/null 2>&1 &
    disown 2>/dev/null || true

    # Poll rather than sleep blindly: the server answers in ~30ms, and a successful
    # status check is a reliable gate for real work.
    local i
    for i in $(seq 1 50); do
        herdr_cli status server --json 2>/dev/null | grep -q '"running":true' && return 0
        sleep 0.1
    done

    error "herdr session '$(herdr_session_name)' did not start"
    return 1
}

# Worktree names allow uppercase and dots (see validate_worktree_name); herdr
# agent names must match [a-z][a-z0-9_-]{0,31}. Map one onto the other.
herdr_agent_name() {
    printf '%s' "${1:-}" \
        | tr '[:upper:]' '[:lower:]' \
        | sed -e 's/[^a-z0-9_-]/-/g' -e 's/^[^a-z]/x&/' \
        | cut -c1-32
}

# True when any pane anywhere is already sitting in that worktree, which is what
# makes `ddev tryout herdr` safe to re-run. Deliberately NOT scoped to the current
# workspace: each worktree gets its own, so a scoped query would never find them.
# Keyed on the pane cwd rather than the label, which a user can rename by hand.
herdr_worktree_is_open() {
    local dir="$1"
    herdr_cli pane list 2>/dev/null \
        | jq -e --arg d "${dir}" \
            '[.result.panes[]? | select(.cwd == $d)] | length > 0' >/dev/null 2>&1
}

# Workspace labels live in one global sidebar alongside every other project, so a
# bare worktree name would be ambiguous there.
herdr_workspace_label() { echo "core-${1}"; }

# Workspace id for a worktree's workspace, empty when it is not open.
herdr_workspace_id() {
    herdr_cli workspace list 2>/dev/null \
        | jq -r --arg l "$(herdr_workspace_label "${1}")" \
            '.result.workspaces[]? | select(.label == $l) | .workspace_id' 2>/dev/null \
        | head -1
}

# A worktree name derived from a branch (or any path segment). herdr names its own
# checkouts from a generated word list on a "worktree/<slug>" branch, so take the last
# segment, drop a typo3-core- prefix, and force it into validate_worktree_name's
# grammar. Empty in, empty out — the caller decides what to do about that.
worktree_name_from_ref() {
    printf '%s' "${1:-}" \
        | sed -e 's|^worktree/||' -e 's|^refs/heads/||' -e 's/^typo3-core-//' \
              -e 's/[^A-Za-z0-9._-]/-/g' \
        | cut -c1-64
}

# Checkouts of the Core repo that live OUTSIDE the project, one path per line. These
# are invisible to every tryout command: not matched by list_core_worktrees' glob, not
# servable, never reachable through the typo3-core symlink. herdr's own New-worktree
# action creates them under its worktrees.directory.
list_foreign_core_worktrees() {
    [ -d "${CORE_DIR}" ] || return 0

    # Compare RESOLVED paths: on macOS the project root is reached through /var
    # while git reports /private/var, and a plain prefix test would call every
    # worktree foreign.
    local root
    root="$(cd "${PROJECT_ROOT}" 2>/dev/null && pwd -P)" || return 0

    git -C "${CORE_DIR}" worktree list --porcelain 2>/dev/null \
        | awk '/^worktree /{print substr($0,10)}' \
        | while IFS= read -r p; do
            [ -n "${p}" ] || continue
            local real
            real="$(cd "${p}" 2>/dev/null && pwd -P)" || real="${p}"
            case "${real}/" in
                "${root}/"*) ;;
                *) echo "${p}" ;;
            esac
          done
}

# --- job tracking ----------------------------------------------------------
# A tryout command launched into a pane used to be fire-and-forget: the caller
# printed a tick and exited, so a failed serve looked exactly like a successful
# one. Each job now leaves three small files behind — <id>.cmd, <id>.pane and,
# once it finishes, <id>.rc — which is what makes the outcome observable at all.
# Deliberately not herdr's pane.exited event: a wrapper writing its own exit code
# needs no daemon and survives the popup that launched it closing.

TRYOUT_JOBS_DIR="${PROJECT_ROOT}/.ddev/.tryout-jobs"

# Launch `ddev tryout <cmd...>` in a pane of its own and record it. Echoes the
# job id; non-zero means nothing was started.
start_tryout_job() {
    local label="$1"; shift
    [ $# -gt 0 ] || return 1

    local id pane wrapped
    mkdir -p "${TRYOUT_JOBS_DIR}" 2>/dev/null || return 1
    id="$(date +%s)-$$"
    printf '%s\n' "$*" > "${TRYOUT_JOBS_DIR}/${id}.cmd"

    pane=$(herdr_cli pane split --direction down --cwd "${PROJECT_ROOT}" --no-focus 2>/dev/null \
           | jq -r '.result.pane.pane_id // empty' 2>/dev/null)
    if [ -z "${pane}" ]; then
        rm -f "${TRYOUT_JOBS_DIR}/${id}.cmd"
        return 1
    fi
    printf '%s\n' "${pane}" > "${TRYOUT_JOBS_DIR}/${id}.pane"

    herdr_cli pane rename "${pane}" "tryout: ${label}" >/dev/null 2>&1 || true

    # The wrapper is the whole point: it records the exit code and says so.
    wrapped="ddev tryout $*; __rc=\$?;"
    wrapped="${wrapped} printf '%s' \"\${__rc}\" > '${TRYOUT_JOBS_DIR}/${id}.rc';"
    wrapped="${wrapped} if [ \"\${__rc}\" -eq 0 ]; then"
    wrapped="${wrapped} herdr notification show 'tryout: ${label}' --body 'finished';"
    wrapped="${wrapped} else"
    wrapped="${wrapped} herdr notification show 'tryout: ${label} FAILED' --body \"exit \${__rc}\";"
    wrapped="${wrapped} fi"

    if ! herdr_cli pane run "${pane}" "${wrapped}" >/dev/null 2>&1; then
        rm -f "${TRYOUT_JOBS_DIR}/${id}.cmd" "${TRYOUT_JOBS_DIR}/${id}.pane"
        return 1
    fi

    printf '%s' "${id}"
}

# One line per job, newest first: "<state>\t<cmd>\t<detail>".
# state is running | ok | failed.
tryout_jobs_status() {
    [ -d "${TRYOUT_JOBS_DIR}" ] || return 0
    local f id cmd rc
    for f in $(ls -t "${TRYOUT_JOBS_DIR}"/*.cmd 2>/dev/null); do
        id="$(basename "${f}" .cmd)"
        cmd="$(cat "${f}" 2>/dev/null)"
        if [ -f "${TRYOUT_JOBS_DIR}/${id}.rc" ]; then
            rc="$(cat "${TRYOUT_JOBS_DIR}/${id}.rc" 2>/dev/null)"
            if [ "${rc}" = "0" ]; then
                printf 'ok\t%s\t\n' "${cmd}"
            else
                printf 'failed\t%s\texit %s\n' "${cmd}" "${rc}"
            fi
        else
            printf 'running\t%s\t\n' "${cmd}"
        fi
    done
}

# Drop finished jobs older than an hour. Running ones are never touched.
reap_tryout_jobs() {
    [ -d "${TRYOUT_JOBS_DIR}" ] || return 0
    local f id
    for f in $(find "${TRYOUT_JOBS_DIR}" -name '*.rc' -mmin +60 2>/dev/null); do
        id="$(basename "${f}" .rc)"
        rm -f "${TRYOUT_JOBS_DIR}/${id}".{cmd,pane,rc} 2>/dev/null
    done
}

# --- herdr keybinding ------------------------------------------------------
# herdr's built-in "New worktree" (prefix+shift+G) cannot be redirected: it prompts
# for a branch and always checks out under worktrees.directory. A TYPO3 Core worktree
# must land at typo3-core-<name>, so the only way to make that key do the right thing
# is to unbind the built-in and bind our own popup. There is no pre-create hook to use
# instead — every worktree.* event is past tense.

herdr_config_path() { echo "${HERDR_CONFIG:-${HOME}/.config/herdr/config.toml}"; }

# Delimiters so unsetup-keys can remove exactly our block and nothing else. The
# project name is in the marker: this file is global and may serve several projects.
herdr_key_marker_start() { echo "# >>> tryout ${DDEV_SITENAME:-tryout} >>>"; }
herdr_key_marker_end()   { echo "# <<< tryout ${DDEV_SITENAME:-tryout} <<<"; }

herdr_key_block() {
    printf '%s\n%s\n' "$(herdr_key_marker_start)" "$(herdr_key_block_body)"
}

herdr_key_block_body() {
    cat <<BLOCK
# Added by 'ddev tryout herdr setup-keys'. Remove with 'unsetup-keys' — editing by
# hand is fine too, just take the whole block including both markers.
[keys]
new_worktree = ""

[[keys.command]]
key = "prefix+shift+g"
type = "popup"
command = "${PROJECT_ROOT}/.ddev/tryout/herdr-new-worktree.sh"
description = "new tryout worktree"
width = "60%"
height = "30%"

[[keys.command]]
key = "prefix+shift+t"
type = "popup"
command = "${PROJECT_ROOT}/.ddev/tryout/herdr-menu.sh"
description = "ddev tryout menu"
width = "70%"
height = "60%"

[[keys.command]]
key = "prefix+shift+d"
type = "popup"
command = "${PROJECT_ROOT}/.ddev/tryout/herdr-dashboard.sh"
description = "tryout dashboard"
width = "80%"
height = "70%"
$(herdr_key_marker_end)
BLOCK
}

herdr_keys_installed() {
    local cfg
    cfg="$(herdr_config_path)"
    [ -f "${cfg}" ] && grep -qF "$(herdr_key_marker_start)" "${cfg}"
}

TRYOUT_HERDR_PLUGIN_ID="tryout.worktree-guard"

# Link the guard plugin, which relocates a worktree herdr's own action puts outside
# the project. Idempotent; a failure is reported but never fatal — the keybindings
# still work without it.
herdr_link_plugin() {
    local dir="${PROJECT_ROOT}/.ddev/tryout/herdr-plugin"
    [ -f "${dir}/herdr-plugin.toml" ] || return 0

    if herdr plugin list 2>/dev/null | grep -q "${TRYOUT_HERDR_PLUGIN_ID}"; then
        return 0
    fi
    if herdr plugin link "${dir}" >/dev/null 2>&1; then
        success "Linked the worktree guard plugin"
    else
        warn "Could not link the worktree guard plugin"
        warn "  herdr's own 'New worktree' will check out outside the project"
    fi
}

herdr_unlink_plugin() {
    herdr plugin list 2>/dev/null | grep -q "${TRYOUT_HERDR_PLUGIN_ID}" || return 0
    herdr plugin unlink "${TRYOUT_HERDR_PLUGIN_ID}" >/dev/null 2>&1 \
        && success "Unlinked the worktree guard plugin" || true
}

herdr_setup_keys() {
    local assume_yes="${1:-false}" cfg backup
    cfg="$(herdr_config_path)"

    herdr_available || return 1

    if herdr_keys_installed; then
        info "The tryout keybinding is already in ${cfg}"
        info "  ${DIM}→ ddev tryout herdr unsetup-keys   to remove it${NC}"
        return 0
    fi

    echo ""
    echo -e "${BOLD}This adds the following to ${cfg}:${NC}"
    echo ""
    herdr_key_block | sed 's/^/  /'
    echo ""
    echo -e "  ${DIM}prefix+shift+G then creates a tryout Core worktree instead of herdr's${NC}"
    echo -e "  ${DIM}own, and prefix+shift+T opens the tryout command menu. That config is${NC}"
    echo -e "  ${DIM}global, so both keys are live in every herdr session — the popups say${NC}"
    echo -e "  ${DIM}so when you are not in a tryout project.${NC}"
    echo ""

    if [ "${assume_yes}" != "true" ]; then
        local reply=""
        if [ -e /dev/tty ] && { : < /dev/tty; } 2>/dev/null; then
            printf "  Write it? [y/N] " > /dev/tty
            read -r reply < /dev/tty
        fi
        case "${reply}" in
            [yY]|[yY][eE][sS]) ;;
            *) info "Nothing written."; return 0 ;;
        esac
    fi

    mkdir -p "$(dirname "${cfg}")"
    if [ -f "${cfg}" ]; then
        backup="${cfg}.tryout-backup-$(date +%Y%m%d%H%M%S)"
        cp "${cfg}" "${backup}" || { error "Could not back up ${cfg}"; return 1; }
        success "Backed up to $(basename "${backup}")"
    else
        : > "${cfg}"
    fi

    # Never fuse onto the user's last line: if the file does not end in a newline,
    # terminate it first. That byte is theirs, so unsetup-keys must not give it back —
    # hence the marker records whether we added one.
    local added_newline="false"
    if [ -s "${cfg}" ] && [ -n "$(tail -c1 "${cfg}")" ]; then
        printf '\n' >> "${cfg}"
        added_newline="true"
    fi

    # Separate the block from the user's content with exactly one blank line — but
    # only if there is not already one, and record that so unsetup can undo it.
    local blank_added="false"
    if [ -s "${cfg}" ] && [ -n "$(tail -c2 "${cfg}" | head -c1)" ]; then
        printf '\n' >> "${cfg}"
        blank_added="true"
    fi

    printf '%s\n%s\n' \
        "$(herdr_key_marker_start) newline_added=${added_newline} blank_added=${blank_added}" \
        "$(herdr_key_block_body)" >> "${cfg}" || {
        error "Could not write ${cfg}"
        return 1
    }
    success "Bound prefix+shift+G (worktree), +T (menu), +D (dashboard)"

    # The plugin covers the route a keybinding cannot: herdr's own New-worktree entry
    # in the sidebar right-click menu.
    herdr_link_plugin

    herdr_cli server reload-config >/dev/null 2>&1 \
        && info "  ${DIM}herdr reloaded its config${NC}" \
        || info "  ${DIM}→ restart herdr, or: herdr server reload-config${NC}"
}

herdr_unsetup_keys() {
    local cfg tmp
    cfg="$(herdr_config_path)"

    if ! herdr_keys_installed; then
        info "No tryout keybinding in ${cfg}"
        return 0
    fi

    tmp="${cfg}.tryout-tmp.$$"
    # Delete the marked block, plus the single blank line setup-keys put before it,
    # so a setup/unsetup round trip leaves the file byte for byte as it was.
    # Hold back blank lines and only emit them once a real line follows. The blank
    # line setup-keys wrote before the block is then dropped with it, so a
    # setup/unsetup round trip leaves the file byte for byte as it was.
    local strip_newline="false" drop_blank="false"
    grep -q "newline_added=true" "${cfg}" && strip_newline="true"
    grep -q "blank_added=true" "${cfg}" && drop_blank="true"

    awk -v start="$(herdr_key_marker_start)" -v end="$(herdr_key_marker_end)" \
        -v dropblank="${drop_blank}" '
        index($0, start) == 1 {
            inblock = 1
            # We added one blank line before the block; give back any others.
            if (dropblank == "true") sub(/\n$/, "", pending)
            printf "%s", pending
            pending = ""
            next
        }
        $0 == end             { inblock = 0; next }
        inblock               { next }
        /^[[:space:]]*$/ { pending = pending $0 "\n"; next }
        { printf "%s%s\n", pending, $0; pending = "" }
        END { printf "%s", pending }
    ' "${cfg}" > "${tmp}" || { error "Could not rewrite ${cfg}"; rm -f "${tmp}"; return 1; }

    # Give back the terminating newline we added, if we added it.
    if [ "${strip_newline}" = "true" ] && [ -s "${tmp}" ]; then
        printf '%s' "$(cat "${tmp}")" > "${tmp}.n" && mv "${tmp}.n" "${tmp}"
    fi

    mv "${tmp}" "${cfg}" || { error "Could not replace ${cfg}"; rm -f "${tmp}"; return 1; }
    success "Removed the tryout keybinding from ${cfg}"
    herdr_unlink_plugin

    herdr_cli server reload-config >/dev/null 2>&1 || true
}

# One workspace per worktree: the root pane runs the agent, a right split gives a
# shell. Both are rooted at the worktree. `workspace create` makes its first tab and
# root pane too, so one call covers the whole topology. Focus stays where the caller
# was unless asked.
open_worktree_in_herdr() {
    local name="$1" use_agent="${2:-true}" focus="${3:-false}" dir ws_json root_pane agent
    dir="$(core_worktree_dir "${name}")"

    if [ ! -d "${dir}" ]; then
        error "No worktree '${name}'"
        error "  → ddev tryout worktree add ${name} <branch>"
        return 1
    fi

    if herdr_worktree_is_open "${dir}"; then
        info "'${name}' is already open — skipping"
        return 0
    fi

    info "Opening '${name}'..."

    local focus_flag="--no-focus"
    [ "${focus}" = "true" ] && focus_flag="--focus"

    # `worktree open` registers the checkout as a workspace WITH git provenance, so
    # herdr groups it under the Core repo exactly like a natively created worktree.
    # `workspace create` sets no provenance, so it is only the fallback for a herdr
    # that does not know the subcommand.
    local label main_dir
    label="$(herdr_workspace_label "${name}")"
    main_dir="$(main_core_worktree_dir)"
    [ -z "${main_dir}" ] && main_dir="${dir}"

    ws_json=$(herdr_cli worktree open \
        --cwd "${main_dir}" --path "${dir}" --label "${label}" "${focus_flag}" 2>&1) \
        || ws_json=""

    if [ -z "${ws_json}" ] || ! printf '%s' "${ws_json}" | jq -e '.result' >/dev/null 2>&1; then
        ws_json=$(herdr_cli workspace create \
            --cwd "${dir}" --label "${label}" "${focus_flag}" 2>&1) || {
            error "herdr could not open '${name}'"
            echo "${ws_json}" >&2
            return 1
        }
    fi

    root_pane=$(printf '%s' "${ws_json}" | jq -r '.result.root_pane.pane_id // empty')
    if [ -z "${root_pane}" ]; then
        error "herdr did not report a pane for '${name}'"
        return 1
    fi

    # Split right: these panes are wide, and there is only ever one split per tab.
    herdr_cli pane split "${root_pane}" --direction right --cwd "${dir}" --no-focus \
        >/dev/null 2>&1 || warn "Could not add a shell pane for '${name}'"

    if [ "${use_agent}" = "true" ]; then
        agent="$(herdr_agent_name "${name}")"
        local start_err rc=0 attempt=0

        # `workspace create` answers before the pane's shell reaches its prompt, and
        # `agent start` needs an idle shell to take over — so a first attempt can
        # lose that race. Retry a few times before believing a failure.
        while :; do
            rc=0
            start_err=$(herdr_cli agent start "${agent}" --kind claude --pane "${root_pane}" 2>&1 >/dev/null) || rc=$?
            # Success, or a definite answer (the agent is up but blocked on its own
            # UI) — either way, stop.
            [ "${rc}" -eq 0 ] && break
            printf '%s' "${start_err}" | grep -q 'agent_not_ready' && break
            attempt=$((attempt + 1))
            [ "${attempt}" -ge 5 ] && break
            sleep 1
        done

        if [ "${rc}" -eq 0 ]; then
            success "'${name}' — claude '${agent}' + shell"
        elif printf '%s' "${start_err}" | grep -q 'agent_not_ready'; then
            # Claude launched but is waiting on its own UI — on a worktree it has
            # not seen before that is the folder-trust prompt. It is running and
            # named, so this is a normal first run, not a failure.
            success "'${name}' — claude '${agent}' + shell"
            info "  ${DIM}'${agent}' is waiting for input (folder trust?) — open core-${name}${NC}"
        else
            warn "Could not start claude in '${name}' — left as a shell"
        fi
    else
        success "'${name}' — two shells"
    fi
}

# True when vendor/ was built from a different Core than the active one. This is
# the silent failure mode of a symlink swap without a reinstall.
vendor_core_mismatch() {
    local link resolved active
    link="${PROJECT_ROOT}/vendor/typo3/cms-core"
    [ -L "${link}" ] || return 1
    active=$(active_worktree_name)
    [ -n "${active}" ] || return 1
    resolved=$(cd "$(dirname "${link}")" && cd "$(readlink "${link}")" 2>/dev/null && pwd -P) || return 1
    case "${resolved}" in
        "$(cd "$(core_worktree_dir "${active}")" && pwd -P)"/*) return 1 ;;
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
site_dir() {
    if site_is_primary "${1:-}"; then echo "${PROJECT_ROOT}"; else echo "${SITES_DIR}/$1"; fi
}

site_docroot() { echo "$(site_dir "${1:-}")/public"; }
site_vendor()  { echo "$(site_dir "${1:-}")/vendor"; }

# The Core checkout a site serves: the symlink for the primary, the named worktree
# otherwise — so the primary keeps following `worktree use`.
site_core_dir() {
    if site_is_primary "${1:-}"; then echo "${CORE_DIR}"; else core_worktree_dir "$1"; fi
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
served_site_names() {
    [ -d "${SITES_DIR}" ] || return 0
    local d name
    for d in "${SITES_DIR}"/*; do
        [ -d "${d}" ] || continue
        name="$(basename "${d}")"
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

generate_site_vhost() {
    local name="$1" php="$2" file docroot host db sock
    file=$(site_vhost_file "${name}")
    docroot="/var/www/html/sites/${name}/public"
    host=$(site_hostname "${name}")
    db=$(site_database "${name}")
    sock="/run/php-fpm-${php}.sock"
    [ "${php}" = "${DDEV_PHP_VERSION:-}" ] && sock="/run/php-fpm.sock"

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

# Create the site's database and grant the DDEV db user access.
ensure_site_database() {
    local db
    db=$(site_database "$1")
    [ "${db}" = "db" ] && return 0
    info "Ensuring database ${db}..."
    if [[ "${DDEV_DATABASE:-mariadb}" == postgres* ]]; then
        ddev exec -s db sh -c "PGPASSWORD=db psql -U db -tc \"SELECT 1 FROM pg_database WHERE datname='${db}'\" | grep -q 1 || PGPASSWORD=db createdb -U db ${db}" \
            || { error "Failed to create database ${db}"; return 1; }
    else
        ddev mysql -uroot -proot -e \
            "CREATE DATABASE IF NOT EXISTS \`${db}\`; GRANT ALL ON \`${db}\`.* TO 'db'@'%';" \
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

    # Paths must be container-side.
    local cdir="/var/www/html${dir#${PROJECT_ROOT}}"
    # shellcheck disable=SC2086 # TRYOUT_EXTRA_ENV is deliberately word-split
    ddev exec env TYPO3_DB_DBNAME="${db}" TRYOUT_SITE="${name}" ${TRYOUT_EXTRA_ENV:-} \
        sh -c "cd '${cdir}' && ${bin} $*"
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

    driver="mysqli"
    [[ "${DDEV_DATABASE:-mariadb}" == postgres* ]] && driver="postgres"
    server_type="other"
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in apache*) server_type="apache" ;; esac

    info "Running TYPO3 setup for '${name}' (db ${db}, PHP ${php})..."
    TRYOUT_EXTRA_ENV="TYPO3_DB_DRIVER=${driver}" \
        site_exec "${name}" "vendor/bin/typo3 setup --no-interaction --force --server-type=${server_type}" \
        || { error "TYPO3 setup failed for ${name}"; return 1; }
    success "Site '${name}' set up"
}

# Wipe one site back to a fresh TYPO3 install: its own database, its own
# fileadmin, its own settings.php. Works for the primary and for served sites.
delete_site() {
    local name="$1" db docroot dir driver server_type
    db=$(site_database "${name}")
    docroot=$(site_docroot "${name}")
    dir=$(site_dir "${name}")

    info "[1/4] Recreating database ${db}..."
    if [[ "${DDEV_DATABASE:-mariadb}" == postgres* ]]; then
        ddev exec -s db sh -c "PGPASSWORD=db dropdb -U db --if-exists ${db}" 2>/dev/null || true
        ddev exec -s db sh -c "PGPASSWORD=db createdb -U db ${db}" \
            || { error "Failed to reset database ${db}"; return 1; }
    else
        # Re-grant: DROP removes the privileges along with the schema.
        ddev mysql -uroot -proot -e \
            "DROP DATABASE IF EXISTS \`${db}\`; CREATE DATABASE \`${db}\`; GRANT ALL ON \`${db}\`.* TO 'db'@'%';" \
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
    [[ "${DDEV_DATABASE:-mariadb}" == postgres* ]] && driver="postgres"
    server_type="other"
    case "${DDEV_WEBSERVER_TYPE:-apache-fpm}" in apache*) server_type="apache" ;; esac

    info "[4/4] Running TYPO3 setup + extension:setup..."
    TRYOUT_EXTRA_ENV="TYPO3_DB_DRIVER=${driver}" \
        site_exec "${name}" "vendor/bin/typo3 setup --no-interaction --force --server-type=${server_type}" \
        || { error "TYPO3 setup failed for ${name}"; return 1; }
    site_exec "${name}" "vendor/bin/typo3 extension:setup" >/dev/null 2>&1 || warn "extension:setup had warnings"
    site_exec "${name}" "vendor/bin/typo3 cache:flush" >/dev/null 2>&1 || warn "cache:flush had warnings"
    success "Setup complete"
}

# --- Serve / unserve ---

# Build a site's own composer.json from the root one, repointing the Core path repo
# at that worktree. jq is not on the host, so this runs in the container.
generate_site_composer() {
    local name="$1" php="${2:-}"
    info "Generating sites/${name}/composer.tryout.json..."
    ddev exec php /var/www/html/.ddev/tryout/site-composer.php "${name}" "${php}" >/dev/null \
        || { error "Failed to generate the Composer overlay for ${name}"; return 1; }
}

# Make a worktree into a live site: own tree, composer.json, DB, vhost and daemon.
serve_worktree() {
    local name="$1" php="${2:-}" dir
    validate_worktree_name "${name}" || return 1
    site_is_primary "${name}" && { error "'${name}' is reserved"; return 1; }

    if [ ! -d "$(core_worktree_dir "${name}")" ]; then
        error "No worktree '${name}'"
        error "  → ddev tryout worktree add ${name} <branch>"
        return 1
    fi

    php="${php:-${DDEV_PHP_VERSION:-8.5}}"
    dir=$(site_dir "${name}")
    mkdir -p "${dir}/config/system" "${dir}/var"

    printf 'php=%s\n' "${php}" > "${dir}/.tryout-site"

    # TYPO3 loads config/system/additional.php relative to its OWN root, so a
    # served site would otherwise miss the DDEV overrides — including
    # trustedHostsPattern, without which its hostname is rejected outright.
    # Symlink rather than copy so there stays one source of truth.
    # Four levels up: system -> config -> <name> -> sites -> project root.
    ln -sfn ../../../../config/system/additional.php "${dir}/config/system/additional.php"

    generate_site_composer "${name}" "${php}" || return 1

    # sites/<name>/ was just created on the host; the container must see it before
    # composer runs in there.
    sync_to_container

    # Sysext set is version-specific, so sync against this worktree.
    info "Syncing sites/${name}/composer.tryout.json with its Core sysexts..."
    ddev exec env PROJECT_ROOT="/var/www/html/sites/${name}" \
        TRYOUT_CORE_DIR="/var/www/html/typo3-core-${name}" \
        php /var/www/html/.ddev/tryout/sync-composer.php \
        || { error "composer sync failed for ${name}"; return 1; }

    ensure_site_database "${name}" || return 1

    # Run composer under the site's own PHP so the lock file and the generated
    # platform_check match what its vhost will actually serve.
    info "Installing dependencies for ${name} on PHP ${php} (this takes a moment)..."
    ddev exec "php${php}" /usr/local/bin/composer install \
        --working-dir="/var/www/html/sites/${name}" --no-interaction \
        || { error "composer install failed for ${name}"; return 1; }

    generate_site_vhost "${name}" "${php}"
    write_worktree_config

    setup_site_typo3 "${name}" || return 1

    success "Site '${name}' prepared — PHP ${php}, db $(site_database "${name}")"
    warn "Run 'ddev restart' to register $(site_hostname "${name}") and issue its certificate."
    echo -e "  ${DIM}then: https://$(site_hostname "${name}")/typo3/  (admin / Password.1)${NC}"
}

# Remove the site but keep the worktree and its git state.
unserve_worktree() {
    local name="$1" keep_db="${2:-true}" db
    validate_worktree_name "${name}" || return 1
    site_is_served "${name}" || { error "Site '${name}' is not served"; return 1; }

    rm -f "$(site_vhost_file "${name}")"
    rm -rf "$(site_dir "${name}")"
    write_worktree_config

    if [ "${keep_db}" != "true" ]; then
        db=$(site_database "${name}")
        info "Dropping database ${db}..."
        if [[ "${DDEV_DATABASE:-mariadb}" == postgres* ]]; then
            ddev exec -s db sh -c "PGPASSWORD=db dropdb -U db --if-exists ${db}" || true
        else
            ddev mysql -uroot -proot -e "DROP DATABASE IF EXISTS \`${db}\`;" || true
        fi
    fi

    success "Site '${name}' removed (worktree kept)"
    warn "Run 'ddev restart' to release its hostname."
}

# --- Gerrit patch functions ---

# Resolve a Gerrit change number to its latest patchset ref.
# Sets: PATCH_SUBJECT, PATCH_REF, PATCH_NUMBER, PATCH_STATUS
resolve_patch_ref() {
    local change_id="$1"
    local api_url="${GERRIT_API}/changes/${change_id}?o=CURRENT_REVISION"

    local result
    local exit_code=0
    result=$(ddev exec bash /var/www/html/.ddev/tryout/resolve-patch-ref.sh "${api_url}" 2>/dev/null) || exit_code=$?

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
    result=$(ddev exec bash /var/www/html/.ddev/tryout/resolve-gerrit-account.sh \
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
        warn "  git -C typo3-core config user.email <your-gerrit-email>"
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
        warn "  git -C typo3-core commit --amend --reset-author --no-edit"
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
    local tmpl_path="../.ddev/tryout/gitmessage.txt"
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
    if ! (exec 3<>"/dev/tcp/${GERRIT_SSH_HOST}/${GERRIT_SSH_PORT}") 2>/dev/null; then
        CS_SSH_REASON="unreachable"
        return 1
    fi
    exec 3<&- 3>&- 2>/dev/null || true

    # 2. ssh-agent must hold at least one identity, otherwise auth will fail
    #    with a misleading "permission denied" instead of a clear hint.
    if ! ssh-add -l >/dev/null 2>&1; then
        CS_SSH_REASON="no-agent-key"
        return 1
    fi

    # 3. Full auth probe.
    if ssh -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new \
           -p "${GERRIT_SSH_PORT}" "${user}@${GERRIT_SSH_HOST}" gerrit version >/dev/null 2>&1; then
        return 0
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
            echo "→ load your key into your host SSH agent, e.g.: ssh-add ~/.ssh/id_ed25519" ;;
        denied)
            echo "→ upload your public key at https://review.typo3.org/settings/#SSHKeys" ;;
        *)
            echo "" ;;
    esac
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
    echo -e "    ${DIM}cd typo3-core && git push origin HEAD:refs/for/${BRANCH}${NC}"
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
