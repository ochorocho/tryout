#!/usr/bin/env bash
# Captures what the bash/PHP generators write, as golden files the Rust tests
# compare against byte for byte (tui/tests/fixtures/generators/). Run it while
# the bash implementation still exists; the goldens outlive it.
#   tests/parity/goldens.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "${here}/../.." && pwd)"
out="${repo}/tui/tests/fixtures/generators"
work="$(mktemp -d "${TMPDIR:-/tmp}/tryout-goldens.XXXXXX")"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${out}"

"${here}/mkproject.sh" "${repo}" "${work}" >/dev/null
root="${work}/project"

# Run bash with functions.sh sourced, in the project, with a fixed DDEV env.
fn() {
    (cd "${root}" && env -i HOME="${work}" PATH="/usr/bin:/bin:$(dirname "$(command -v php)"):$(dirname "$(command -v git)")" \
        DDEV_APPROOT="${root}" DDEV_SITENAME=parity DDEV_PHP_VERSION=8.4 DDEV_WEBSERVER_TYPE="${WS:-nginx-fpm}" \
        bash -c "source .ddev/tryout/functions.sh >/dev/null 2>&1; $1")
}

WS=nginx-fpm fn 'generate_site_vhost v13 8.2; cat "$(site_vhost_file v13)"' > "${out}/vhost-nginx.conf"
WS=nginx-fpm fn 'generate_site_vhost v13 8.4; cat "$(site_vhost_file v13)"' > "${out}/vhost-nginx-default-php.conf"
WS=apache-fpm fn 'generate_site_vhost v13 8.2; cat "$(site_vhost_file v13)"' > "${out}/vhost-apache.conf"

fn 'write_server_names_hash v13.parity; cat "$(site_hash_config_file)"' > "${out}/hash-short.conf"
fn 'write_server_names_hash a.parity a-worktree-name-of-quite-ordinary-length.some-project-name; cat "$(site_hash_config_file)"' > "${out}/hash-long.conf"

# Four served sites: two share 8.2, one runs the project's own 8.4.
for s in a:8.2 b:8.3 c:8.2 d:8.4; do
    mkdir -p "${root}/TYPO3-Instances/${s%%:*}"
    printf 'php=%s\n' "${s##*:}" > "${root}/TYPO3-Instances/${s%%:*}/.tryout-site"
done
# One deliberate change: the FPM daemon is the binary's `__fpm`, not the script.
fn 'write_worktree_config; cat "${WORKTREE_CONFIG}"' \
    | sed 's#"bash /var/www/html/.ddev/tryout/tryout-php-fpm.sh #"/var/www/html/.ddev/tryout/tryout __fpm #' \
    > "${out}/config.worktrees.yaml"
fn 'served_hostname_set' > "${out}/hostname-set.txt"

# The overlays: the shipped template with a custom require and stale managed ones.
overlay="${root}/TYPO3-Instances/primary/composer.tryout.json"
php -r '
    $f = $argv[1]; $d = json_decode(file_get_contents($f), true);
    $d["require"]["vendor/custom"] = "^1.0";
    $d["require"]["typo3/cms-removed"] = "@dev";
    $d["require"]["typo3/theme-camino"] = "@dev";
    file_put_contents($f, json_encode($d, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES) . "\n");
' "${overlay}"
cp "${overlay}" "${out}/overlay-before.json"
touch "${root}/TYPO3-Instances/primary/composer.tryout.lock"
(cd "${root}" && PROJECT_ROOT="${root}/TYPO3-Instances/primary" TRYOUT_CORE_DIR="${root}" php .ddev/tryout/sync-composer.php) > "${out}/sync.out"
cp "${overlay}" "${out}/overlay-synced.json"

(cd "${root}" && DDEV_APPROOT="${root}" php .ddev/tryout/site-composer.php v13 8.2 >/dev/null)
cp "${root}/TYPO3-Instances/v13/composer.tryout.json" "${out}/overlay-site-v13.json"

(cd "${root}" && DDEV_APPROOT="${root}" php .ddev/tryout/use-core.php v13) > "${out}/use-core-v13.out"
cp "${overlay}" "${out}/overlay-use-v13.json"
(cd "${root}" && DDEV_APPROOT="${root}" php .ddev/tryout/use-core.php) > "${out}/use-core-root.out"
cp "${overlay}" "${out}/overlay-use-root.json"

ls "${out}"
