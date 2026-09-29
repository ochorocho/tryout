SIDE=host
ARGS=(cs doctor)
setup() { git config tryout.gerritUser ada; git config user.email ada@example.com; git config commit.template .ddev/tryout/gitmessage.txt; git remote set-url --push origin ssh://ada@127.0.0.1:29418/Packages/TYPO3.CMS; mkdir -p .git/hooks; for h in commit-msg pre-commit; do printf "#!/bin/sh\n" > .git/hooks/$h; chmod +x .git/hooks/$h; done; }
