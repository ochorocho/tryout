SIDE=host
ARGS=(worktree use v13)
FILES=(TYPO3-Instances/primary/composer.tryout.json)
setup() { add_worktree v13 13.4; mkdir -p TYPO3-Instances/primary/vendor; }
