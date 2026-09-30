#!/usr/bin/env bash
# The suites as CI runs them: `bash tests/ci.sh <suite>`.
#
# ddev/github-action-add-on-test runs its test_command unquoted, as plain
# words — no shell, so no `case`, no `&&`, no quotes — which is why the choice
# of suite, and building the binary first, live here.
set -euo pipefail
cd "$(dirname "$0")/.."

# The binary is part of the payload: CI's build job made it once
# (TRYOUT_PREBUILT=1, already in tryout/bin/); anywhere else, build it here.
build() {
  if [ -n "${TRYOUT_PREBUILT:-}" ] && ls tryout/bin/tryout-linux-* >/dev/null 2>&1; then
    echo "Using the prebuilt $(ls tryout/bin/tryout-linux-*)"
    return
  fi
  tui/scripts/build-linux.sh
}

case "${1:-}" in
  unit)
    bats tests/unit.bats
    ;;
  install)
    build
    bats tests/test.bats --filter-tags '!release'
    ;;
  project)
    # Project mode: a plain PHP app of the test's own, no Core clone.
    build
    bats tests/project.bats --filter-tags 'project,!db'
    ;;
  project-*)
    # One framework's journey (project-laravel, project-drupal, …): its
    # installer, its database, its own settings — minutes each.
    build
    bats tests/project.bats --filter-tags "project,${1#project-}"
    ;;
  lifecycle)
    build
    bats tests/lifecycle.bats
    ;;
  release)
    bats tests/test.bats --filter-tags release
    ;;
  *)
    echo "Usage: tests/ci.sh unit|install|project|project-<framework>|lifecycle|release" >&2
    exit 2
    ;;
esac
