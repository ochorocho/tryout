SIDE=host
ARGS=(worktree remove v13 --yes)
FILES=("tree:worktrees" "tree:TYPO3-Instances")
setup() { add_worktree v13 13.4; serve_site v13 8.2; }
