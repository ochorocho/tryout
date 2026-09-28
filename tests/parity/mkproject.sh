#!/usr/bin/env bash
# Builds a tryout project the parity cases run in: a tiny "TYPO3 Core" git repo
# at the root (three sysexts, a PHP constraint, a local bare origin with the
# branch shapes the add-on sorts and filters), with the payload laid out under
# .ddev/ the way `ddev add-on get` does. Everything is dated and named so two
# builds are byte-identical.
#   mkproject.sh <repo-checkout> <dir>
set -euo pipefail

repo="$1"
dir="$2"
export GIT_AUTHOR_NAME=Parity GIT_AUTHOR_EMAIL=parity@example.com
export GIT_COMMITTER_NAME=Parity GIT_COMMITTER_EMAIL=parity@example.com
export GIT_AUTHOR_DATE="2026-01-01T00:00:00Z" GIT_COMMITTER_DATE="2026-01-01T00:00:00Z"
g() { git -c init.defaultBranch=main -c advice.detachedHead=false "$@"; }

rm -rf "${dir}"
mkdir -p "${dir}/origin-src"
cd "${dir}/origin-src"
g init -q
printf '{\n    "name": "typo3/cms",\n    "require": {\n        "php": "^8.2"\n    }\n}\n' > composer.json
for ext in core backend frontend; do
    mkdir -p "typo3/sysext/${ext}"
    printf '{\n    "name": "typo3/cms-%s"\n}\n' "${ext}" > "typo3/sysext/${ext}/composer.json"
done
g add -A && g commit -qm "[TASK] Initial"
g branch TYPO3_4-5
# Release branches each carry a commit of their own, so the base a detached
# checkout reports is the branch it really came from.
for b in 12.4 13.4; do
    g checkout -q -b "${b}"
    [ "${b}" = 12.4 ] && printf '{\n    "name": "typo3/cms",\n    "require": {\n        "php": ">=8.1 <8.4"\n    }\n}\n' > composer.json
    echo "${b}" > VERSION && g add -A && g commit -qm "[RELEASE] ${b}"
    g checkout -q main
done
g clone -q --bare . "${dir}/origin.git"

mkdir -p "${dir}/project"
cd "${dir}/project"
g init -q
g remote add origin "${dir}/origin.git"
g fetch -q origin
g checkout -q -B main origin/main

mkdir -p .ddev/commands/host/autocomplete
cp -R "${repo}/tryout" .ddev/tryout
cp "${repo}/commands/host/tryout" .ddev/commands/host/tryout
cp "${repo}/commands/host/autocomplete/tryout" .ddev/commands/host/autocomplete/tryout
cp "${repo}/config.tryout.yaml" .ddev/
# What install.yaml's post_install_actions create.
mkdir -p TYPO3-Instances/primary/config/system packages
cp "${repo}/tryout/composer.tryout.json" TYPO3-Instances/primary/
