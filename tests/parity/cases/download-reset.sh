SIDE=host
ARGS=(download --reset)
FILES=("git:status --short")
setup() { echo x >> composer.json; }
