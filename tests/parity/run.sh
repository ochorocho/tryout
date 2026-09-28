#!/usr/bin/env bash
# Runs every parity case twice — through the bash implementation and through the
# Rust binary — in identical fresh projects, and diffs what each produced:
# stdout, stderr, exit code, every call to an external tool, and the files the
# case names. Differences listed in whitelist.sed are normalised away first.
#
#   tests/parity/run.sh [<case>…]        default: every case in cases/
#   TRYOUT_BIN=<binary>                  the Rust side (default: tui/target/debug/tryout)
#   PARITY_SELFTEST=1                    run bash on both sides: proves the harness
#                                        itself is deterministic
#
# A case is a file in cases/ defining SIDE (host|ctr|complete), ARGS (an array),
# and optionally FILES (paths to compare) and setup() (run in the project first,
# with the helpers below). SIDE=complete takes CORPUS instead of ARGS: one
# command line per entry, words split on spaces, `''` for an empty word.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "${here}/../.." && pwd)"
bin="${TRYOUT_BIN:-${repo}/tui/target/debug/tryout}"
work="$(mktemp -d "${TMPDIR:-/tmp}/tryout-parity.XXXXXX")"
[ -n "${PARITY_KEEP:-}" ] && echo "work dir: ${work}"

"${here}/mkproject.sh" "${repo}" "${work}/template" >/dev/null

# The real tools a case may use, and nothing else from the host: gum above all
# stays out, as it is out of the web container, where these verbs run.
mkdir -p "${work}/tools"
for t in php git jq curl; do ln -s "$(command -v "${t}")" "${work}/tools/${t}"; done

# Gerrit, answering every listing with gerrit/changes/index.html.
port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')"
python3 -m http.server "${port}" --bind 127.0.0.1 --directory "${here}/gerrit" >/dev/null 2>&1 &
gerrit_pid=$!
trap 'kill "${gerrit_pid}" 2>/dev/null; wait "${gerrit_pid}" 2>/dev/null; [ -n "${PARITY_KEEP:-}" ] || rm -rf "${work}"' EXIT
for _ in 1 2 3 4 5 6 7 8 9 10; do curl -sf "http://127.0.0.1:${port}/changes/" >/dev/null && break; sleep 0.2; done

# --- helpers for a case's setup() --------------------------------------------
g() { git -c advice.detachedHead=false "$@" >/dev/null 2>&1; }
# A worktree at worktrees/<name>, detached on origin/<base>.
add_worktree() { mkdir -p worktrees && g worktree add --detach "worktrees/$1" "origin/$2"; }
# A served site for <name>: its marker, with an optional PHP version.
serve_site() { mkdir -p "TYPO3-Instances/$1" && printf 'php=%s\n' "${2:-8.4}" > "TYPO3-Instances/$1/.tryout-site"; }

# Run one case with one implementation; leave the results in ${work}/out/<impl>.
run_case() {
    local case_file="$1" impl="$2"
    local out="${work}/out/${impl}" root="${work}/run/project"
    rm -rf "${work}/run" "${out}"
    mkdir -p "${out}"
    cp -R "${work}/template" "${work}/run"
    # Case variables, fresh for each run.
    local SIDE="" FILES=()
    local ARGS=() CORPUS=()
    unset -f setup 2>/dev/null || true
    # shellcheck disable=SC1090
    source "${case_file}"

    local env=(
        env -i
        HOME="${work}/home" PATH="${here}/fakes:${work}/tools:/usr/bin:/bin:/usr/sbin:/sbin"
        LANG=C.UTF-8 TERM=dumb
        DDEV_APPROOT="${root}" DDEV_SITENAME=parity DDEV_PHP_VERSION=8.4
        DDEV_WEBSERVER_TYPE=nginx-fpm DDEV_DATABASE=mariadb:10.11
        DDEV_PRIMARY_URL=https://parity.ddev.site
        FAKE_LOG="${out}/calls" FAKE_ANSWERS="${work}/run/answers"
        TRYOUT_GERRIT_API="http://127.0.0.1:${port}"
    )
    # The Rust side's container half: the fake ddev runs the launcher, which
    # takes the binary from here.
    [ "${impl}" = rust ] && [ "${PARITY_SELFTEST:-}" != 1 ] && env+=(TRYOUT_BIN="${bin}")
    mkdir -p "${work}/home"
    : > "${out}/calls"
    if declare -F setup >/dev/null; then (cd "${root}" && setup); fi

    local rc=0
    if [ "${SIDE}" = complete ]; then
        local line words=()
        for line in "${CORPUS[@]}"; do
            read -r -a words <<< "${line}"
            echo "## ${line}" >> "${out}/stdout"
            if [ "${impl}" = bash ] || [ "${PARITY_SELFTEST:-}" = 1 ]; then
                (cd / && "${env[@]/DDEV_APPROOT=*/DDEV_NO_APPROOT=1}" "${root}/.ddev/commands/host/autocomplete/tryout" tryout "${words[@]}" \
                    </dev/null >>"${out}/stdout" 2>>"${out}/stderr") || rc=$?
            else
                (cd / && "${env[@]}" "${bin}" __complete tryout "${words[@]}" \
                    </dev/null >>"${out}/stdout" 2>>"${out}/stderr") || rc=$?
            fi
        done
        echo "${rc}" > "${out}/exit"
        finish_case "${out}"
        return 0
    fi

    local cmd=()
    case "${impl}:${SIDE}" in
        bash:ctr)  cmd=(bash "${root}/.ddev/tryout/tryout-container.sh") ;;
        bash:host) cmd=(bash "${root}/.ddev/commands/host/tryout") ;;
        rust:ctr)  cmd=("${bin}" ctr) ;;
        rust:host) cmd=("${bin}") ;;
        *) echo "bad SIDE '${SIDE}' in ${case_file}" >&2; return 2 ;;
    esac
    [ "${PARITY_SELFTEST:-}" = 1 ] && [ "${impl}" = rust ] && case "${SIDE}" in
        ctr)  cmd=(bash "${root}/.ddev/tryout/tryout-container.sh") ;;
        host) cmd=(bash "${root}/.ddev/commands/host/tryout") ;;
    esac

    (cd "${root}" && "${env[@]}" "${cmd[@]}" ${ARGS[@]+"${ARGS[@]}"} \
        </dev/null >"${out}/stdout" 2>"${out}/stderr") || rc=$?
    echo "${rc}" > "${out}/exit"
    local f
    for f in ${FILES[@]+"${FILES[@]}"}; do
        mkdir -p "${out}/files/$(dirname "${f}")"
        if [ -e "${root}/${f}" ]; then cp "${root}/${f}" "${out}/files/${f}"; else echo "(absent)" > "${out}/files/${f}"; fi
    done
    finish_case "${out}"
}

# Paths differ only by the temp dir; the whitelist then normalises the rest.
finish_case() {
    find "$1" -type f -exec sed -i.bak -e "s#${work}#<WORK>#g" -f "${here}/whitelist.sed" {} \;
    find "$1" -name '*.bak' -delete
}

cases=("$@")
[ ${#cases[@]} -gt 0 ] || cases=($(cd "${here}/cases" && ls))
failed=0
for c in "${cases[@]}"; do
    c="${c%.sh}"
    run_case "${here}/cases/${c}.sh" bash
    run_case "${here}/cases/${c}.sh" rust
    if diff -ru "${work}/out/bash" "${work}/out/rust" > "${work}/diff"; then
        echo "ok   ${c}"
    else
        echo "FAIL ${c}"
        sed 's/^/     /' "${work}/diff" | head -${PARITY_DIFF_LINES:-60}
        failed=$((failed + 1))
    fi
done
[ "${failed}" -eq 0 ]
