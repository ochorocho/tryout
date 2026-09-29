SIDE=ctr
ARGS=(cs setup ada)
FILES=("git:config --local --get user.email")
setup() { git config user.email old@example.com; }
