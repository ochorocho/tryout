SIDE=host
ARGS=(worktree rename v13 fresh)
FILES=("tree:TYPO3-Instances" .ddev/config.worktrees.yaml)
setup() { add_worktree v13 13.4; serve_site v13 8.2; fpm_running 8.2; }
