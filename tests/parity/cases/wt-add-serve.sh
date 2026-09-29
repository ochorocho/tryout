SIDE=host
ARGS=(worktree add x 13.4 --serve --php 8.2)
FILES=(TYPO3-Instances/x/.tryout-site TYPO3-Instances/x/composer.tryout.json .ddev/nginx_full/tryout-site-x.conf .ddev/config.worktrees.yaml .ddev/nginx_full/tryout-server-names-hash.conf "tree:TYPO3-Instances")
setup() { fpm_running 8.2; }
