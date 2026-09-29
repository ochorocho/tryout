SIDE=host
ARGS=(worktree serve old --no-restart)
FILES=(TYPO3-Instances/old/.tryout-site .ddev/config.worktrees.yaml)
setup() { add_worktree old 12.4; fpm_running 8.3; }
