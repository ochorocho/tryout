SIDE=host
ARGS=(worktree unserve v13 --drop-db)
FILES=("tree:TYPO3-Instances")
setup() { add_worktree v13 13.4; serve_site v13 8.2; }
