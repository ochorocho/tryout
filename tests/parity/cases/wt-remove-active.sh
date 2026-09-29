SIDE=ctr
ARGS=(worktree remove v13)
setup() { add_worktree v13 13.4; sed -i.bak "s#\.\./\.\./typo3/sysext#../../worktrees/v13/typo3/sysext#" TYPO3-Instances/primary/composer.tryout.json; }
