SIDE=host
ARGS=(composer)
FILES=(TYPO3-Instances/primary/composer.tryout.json TYPO3-Instances/primary/composer.tryout.lock)
setup() { touch TYPO3-Instances/primary/composer.tryout.lock; }
