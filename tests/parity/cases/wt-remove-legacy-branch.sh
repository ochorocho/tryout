SIDE=ctr
ARGS=(worktree remove old)
FILES=("git:branch --list old")
setup() { git branch old origin/12.4 >/dev/null; mkdir -p worktrees; git worktree add worktrees/old old >/dev/null 2>&1; }
