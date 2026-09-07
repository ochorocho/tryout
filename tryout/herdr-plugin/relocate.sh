#!/usr/bin/env bash
#ddev-generated

# Moves a Core worktree herdr created outside its DDEV project back to
# <project>/typo3-core-<name>, where every ddev tryout command can see it.
#
# THIS HOOK FIRES FOR EVERY WORKTREE HERDR CREATES, ON ANY REPOSITORY. It must do
# nothing at all unless the worktree belongs to a repo owned by a tryout project —
# see the bail-out below. Getting that wrong would move other people's checkouts.
#
# Never exits non-zero: a failed hook helps nobody and the checkout still exists
# where herdr put it. Problems are reported through herdr's notifications and the
# plugin log (herdr plugin log list).

set -uo pipefail

BIN="${HERDR_BIN_PATH:-herdr}"

note() { "${BIN}" notification show --message "tryout: $*" >/dev/null 2>&1 || true; }

# --- what happened ---------------------------------------------------------
EVENT="${HERDR_PLUGIN_EVENT_JSON:-}"
[ -n "${EVENT}" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0

WT_PATH=$(printf '%s' "${EVENT}" | jq -r '.worktree.path // empty' 2>/dev/null)
WT_BRANCH=$(printf '%s' "${EVENT}" | jq -r '.worktree.branch // empty' 2>/dev/null)
WS_ID=$(printf '%s' "${EVENT}" | jq -r '.workspace.workspace_id // empty' 2>/dev/null)

[ -n "${WT_PATH}" ] && [ -d "${WT_PATH}" ] || exit 0

# --- bail out unless this is a tryout Core worktree -------------------------
# The owning repo is what identifies it: a linked worktree's git dir points back at
# the main checkout, and a tryout project has .ddev/tryout/functions.sh above it.
COMMON_DIR=$(git -C "${WT_PATH}" rev-parse --git-common-dir 2>/dev/null) || exit 0
case "${COMMON_DIR}" in
    /*) ;;
    *)  COMMON_DIR="$(cd "${WT_PATH}" && cd "$(dirname "${COMMON_DIR}")" 2>/dev/null && pwd)/$(basename "${COMMON_DIR}")" ;;
esac
REPO_ROOT="$(cd "$(dirname "${COMMON_DIR}")" 2>/dev/null && pwd)" || exit 0

APPROOT=""
dir="${REPO_ROOT}"
while [ -n "${dir}" ] && [ "${dir}" != "/" ]; do
    if [ -f "${dir}/.ddev/tryout/functions.sh" ]; then
        APPROOT="${dir}"
        break
    fi
    dir="$(dirname "${dir}")"
done
[ -n "${APPROOT}" ] || exit 0    # not ours — leave it exactly where it is

# Already where it belongs? The tryout routes land there directly. Compare resolved
# paths: on macOS /var and /private/var name the same place.
WT_REAL="$(cd "${WT_PATH}" 2>/dev/null && pwd -P)" || WT_REAL="${WT_PATH}"
APPROOT_REAL="$(cd "${APPROOT}" 2>/dev/null && pwd -P)" || APPROOT_REAL="${APPROOT}"
case "${WT_REAL}/" in
    "${APPROOT_REAL}/typo3-core-"*) exit 0 ;;
esac

# --- relocate ---------------------------------------------------------------
DDEV_APPROOT="${APPROOT}"; export DDEV_APPROOT
# shellcheck disable=SC1090
. "${APPROOT}/.ddev/tryout/functions.sh" >/dev/null 2>&1 || exit 0

NAME="$(worktree_name_from_ref "${WT_BRANCH:-$(basename "${WT_PATH}")}")"
[ -n "${NAME}" ] || NAME="$(worktree_name_from_ref "$(basename "${WT_PATH}")")"
validate_worktree_name "${NAME}" >/dev/null 2>&1 || {
    note "could not derive a name for ${WT_PATH}"
    exit 0
}

# On a collision, suffix rather than give up. Leaving the checkout outside the
# project hides it from every ddev tryout command — `worktree list` does not show
# it, `herdr` does not open it — and two branches slugging to the same name is not
# rare (bugfix/12345 and bugfix-12345). Inside the project under a second name is
# always the better answer; `worktree rename` fixes it up afterwards.
BASE="${NAME}"
TARGET="$(core_worktree_dir "${NAME}")"
n=2
while [ -e "${TARGET}" ]; do
    if [ "${n}" -gt 99 ]; then
        note "'${BASE}' and 98 variants of it already exist — left ${WT_PATH} where it is"
        exit 0
    fi
    NAME="${BASE}-${n}"
    TARGET="$(core_worktree_dir "${NAME}")"
    n=$((n + 1))
done

# git worktree move, never mv: it rewrites the worktree metadata on both sides.
if ! git -C "${WT_PATH}" worktree move "${WT_PATH}" "${TARGET}" >/dev/null 2>&1; then
    note "could not move ${WT_PATH} (dirty or locked?)"
    exit 0
fi

# The old workspace now points at a directory that no longer exists, so replace it.
[ -n "${WS_ID}" ] && "${BIN}" workspace close "${WS_ID}" >/dev/null 2>&1
open_worktree_in_herdr "${NAME}" "true" "true" >/dev/null 2>&1

# Name the branch too: with a suffixed name, or two branches that slug alike, the
# folder alone does not say which checkout this is.
if [ "${NAME}" != "${BASE}" ]; then
    note "moved worktree to typo3-core-${NAME}${WT_BRANCH:+ (${WT_BRANCH})} — typo3-core-${BASE} was taken"
else
    note "moved worktree to typo3-core-${NAME}${WT_BRANCH:+ (${WT_BRANCH})}"
fi
exit 0
