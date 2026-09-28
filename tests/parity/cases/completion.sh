# Every position of every verb, empty and partly typed, on a project with a
# served and an unserved worktree and a configured patch list.
SIDE=complete
setup() {
    add_worktree v13 13.4; add_worktree old 12.4; serve_site v13 8.2
    printf 'web_environment:\n  - TRYOUT_PATCHES=56947, 12345\n' > .ddev/config.tryout-patches.yaml
}
CORPUS=(
    "''" "st" "ui ''" "ui stop ''" "status ''" "composer ''" "help ''"
    "download ''" "download -" "download --reset ''"
    "checkout ''" "checkout 13.4 ''" "checkout --site ''"
    "patch ''" "patch 56947 ''" "patch --all-branches ''"
    "reset ''" "reset v13 ''" "launch ''" "launch -" "launch --backend ''"
    "exec ''" "exec v13 ''" "exec v13 typo3 ''"
    "delete ''" "delete --all ''" "delete -"
    "cs ''" "cs setup ''" "cs doctor ''"
    "worktree ''" "worktree add ''" "worktree add x ''" "worktree add x 13.4 ''"
    "worktree add x -" "worktree add x --php ''" "worktree add x --serve ''"
    "worktree branches ''" "worktree list ''" "worktree list --plain ''"
    "worktree use ''" "worktree use -" "worktree remove ''" "worktree rm --force ''"
    "worktree serve ''" "worktree serve v13 --php ''" "worktree serve old --php ''" "worktree serve -"
    "worktree unserve ''" "worktree rename ''" "worktree rename v13 ''" "worktree rename v13 new ''"
    "nope ''"
)
