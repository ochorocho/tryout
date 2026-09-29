SIDE=host
ARGS=(worktree remove old --yes)
FILES=("git:worktree list --porcelain" "tree:worktrees")
setup() { add_worktree old 12.4; touch worktrees/old/untracked.txt; }
