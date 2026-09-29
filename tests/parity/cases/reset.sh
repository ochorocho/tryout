SIDE=host
ARGS=(reset)
FILES=("git:log --oneline -2")
setup() { git commit -q --allow-empty -m local; }
