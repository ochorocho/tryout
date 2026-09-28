# The primary serves worktrees/v13 (use moved it): its sysexts decide.
SIDE=ctr
ARGS=(composer)
FILES=(TYPO3-Instances/primary/composer.tryout.json)
setup() { add_worktree v13 13.4; rm -rf worktrees/v13/typo3/sysext/frontend; sed -i.bak "s#\.\./\.\./typo3/sysext#../../worktrees/v13/typo3/sysext#" TYPO3-Instances/primary/composer.tryout.json; }
