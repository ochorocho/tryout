SIDE=host
ARGS=(worktree rename old fresh)
FILES=("tree:worktrees" "git:worktree list --porcelain")
setup() { add_worktree old 12.4; }
