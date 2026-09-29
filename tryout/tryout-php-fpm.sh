#!/usr/bin/env bash
#ddev-generated
# Kept for one release: .ddev/config.worktrees.yaml written by an earlier tryout
# starts the PHP-FPM daemons through this script. The install rewrites those
# lines to `tryout __fpm`; this covers a project that has not been restarted
# since. Remove it in the release after this one.
exec "$(cd "$(dirname "$0")" && pwd)/tryout" __fpm "$@"
