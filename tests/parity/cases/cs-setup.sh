SIDE=host
ARGS=(cs setup ada)
FILES=("git:config --local --get-regexp ^(user|tryout|commit)\." "git:remote get-url --push origin" "tree:.git/hooks")
