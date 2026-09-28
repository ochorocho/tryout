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
# A case is a file in cases/ defining SIDE (host|ctr), ARGS (an array), and
# optionally FILES (paths to compare) and setup() (run in the project first).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "${here}/../.." && pwd)"
bin="${TRYOUT_BIN:-${repo}/tui/target/debug/tryout}"
work="$(mktemp -d "${TMPDIR:-/tmp}/tryout-parity.XXXXXX")"
[ -n "${PARITY_KEEP:-}" ] && echo "work dir: ${work}" || trap 'rm -rf "${work}"' EXIT

"${here}/mkproject.sh" "${repo}" "${work}/template" >/dev/null

# Run one case with one implementation; leave the results in ${work}/out/<impl>.
run_case() {
    local case_file="$1" impl="$2"
    local out="${work}/out/${impl}" root="${work}/run/project"
    rm -rf "${work}/run" "${out}"
    mkdir -p "${out}"
    cp -R "${work}/template" "${work}/run"
    # Case variables, fresh for each run.
    local SIDE="" FILES=()
    local ARGS=()
    unset -f setup 2>/dev/null || true
    # shellcheck disable=SC1090
    source "${case_file}"

    local env=(
        env -i
        HOME="${work}/home" PATH="${here}/fakes:/usr/bin:/bin:/usr/sbin:/sbin:$(dirname "$(command -v php)"):$(dirname "$(command -v git)")"
        LANG=C.UTF-8 TERM=dumb
        DDEV_APPROOT="${root}" DDEV_SITENAME=parity DDEV_PHP_VERSION=8.4
        DDEV_WEBSERVER_TYPE=nginx-fpm DDEV_DATABASE=mariadb:10.11
        DDEV_PRIMARY_URL=https://parity.ddev.site
        FAKE_LOG="${out}/calls" FAKE_ANSWERS="${work}/run/answers"
    )
    mkdir -p "${work}/home"
    : > "${out}/calls"
    if declare -F setup >/dev/null; then (cd "${root}" && setup); fi

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

    local rc=0
    (cd "${root}" && "${env[@]}" "${cmd[@]}" ${ARGS[@]+"${ARGS[@]}"} \
        </dev/null >"${out}/stdout" 2>"${out}/stderr") || rc=$?
    echo "${rc}" > "${out}/exit"
    local f
    for f in ${FILES[@]+"${FILES[@]}"}; do
        mkdir -p "${out}/files/$(dirname "${f}")"
        if [ -e "${root}/${f}" ]; then cp "${root}/${f}" "${out}/files/${f}"; else echo "(absent)" > "${out}/files/${f}"; fi
    done
    # Paths differ only by the temp dir; the whitelist then normalises the rest.
    find "${out}" -type f -exec sed -i.bak -e "s#${work}#<WORK>#g" -f "${here}/whitelist.sed" {} \;
    find "${out}" -name '*.bak' -delete
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
