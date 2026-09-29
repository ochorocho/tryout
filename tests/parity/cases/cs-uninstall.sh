SIDE=host
ARGS=(cs uninstall)
FILES=("git:config --local --get-regexp ^(tryout|commit)\." "git:remote get-url --push origin" "tree:.git/hooks")
setup() { git config tryout.gerritUser ada; git config commit.template x; git remote set-url --push origin ssh://ada@x/y; mkdir -p .git/hooks; touch .git/hooks/commit-msg; }
