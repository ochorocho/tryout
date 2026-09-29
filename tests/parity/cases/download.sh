SIDE=host
ARGS=(download)
FILES=("git:log --oneline -3")
setup() { git -C ../origin-src commit -q --allow-empty -m "[TASK] Upstream moved"; git -C ../origin-src push -q ../origin.git main; }
