SIDE=host
ARGS=(status)
setup() { add_worktree v13 13.4; add_worktree old 12.4; serve_site v13 8.2; mkdir -p packages/my_ext; echo x > typo3/sysext/core/new.txt; git add typo3/sysext/core/new.txt; }
