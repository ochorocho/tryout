SIDE=host
ARGS=(worktree remove old)
FILES=("tree:worktrees")
setup() { add_worktree old 12.4; }
