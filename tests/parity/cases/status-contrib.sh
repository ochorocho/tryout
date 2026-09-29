SIDE=host
ARGS=(status)
setup() { git config tryout.gerritUser ada; git remote set-url --push origin ssh://ada@127.0.0.1:29418/P; mkdir -p .git/hooks; printf "#!/bin/sh\n" > .git/hooks/commit-msg; chmod +x .git/hooks/commit-msg; }
