# Normalisations applied to both sides before diffing: differences that are
# intended, not regressions. Each rule says why it exists.
# The FPM daemon is the binary's `__fpm` now, not the bash script.
s#"bash /var/www/html/\.ddev/tryout/tryout-php-fpm\.sh #"/var/www/html/.ddev/tryout/tryout __fpm #
# worktree list --json gained "changes" (the Gerrit changes on top, by
# number) — add-only, and bash never had it.
s#,"changes":\[[0-9,]*\]}#}#
