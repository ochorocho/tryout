SIDE=host
ARGS=(worktree unserve v13)
FILES=("tree:TYPO3-Instances" .ddev/config.worktrees.yaml)
setup() { add_worktree v13 13.4; serve_site v13 8.2; mkdir -p TYPO3-Instances/v13/config/system; echo "<?php return [];" > TYPO3-Instances/v13/config/system/settings.php; }
