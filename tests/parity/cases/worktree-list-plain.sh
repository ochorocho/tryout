SIDE=host
ARGS=(worktree list --plain)
setup() { add_worktree v13 13.4; add_worktree old 12.4; serve_site v13 8.2; echo x >> worktrees/old/composer.json; }
