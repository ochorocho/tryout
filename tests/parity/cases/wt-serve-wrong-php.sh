SIDE=host
ARGS=(worktree serve old --php 8.5)
FILES=(TYPO3-Instances/old/.tryout-site)
setup() { add_worktree old 12.4; }
