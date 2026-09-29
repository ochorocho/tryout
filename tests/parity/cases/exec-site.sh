SIDE=host
ARGS=(exec v13 vendor/bin/typo3 "config:set" "My Site")
setup() { add_worktree v13 13.4; serve_site v13 8.2; }
