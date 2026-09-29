SIDE=host
ARGS=(checkout 12.4 --site v13)
FILES=(TYPO3-Instances/v13/composer.tryout.json)
setup() { add_worktree v13 13.4; serve_site v13 8.2; cp TYPO3-Instances/primary/composer.tryout.json TYPO3-Instances/v13/; }
