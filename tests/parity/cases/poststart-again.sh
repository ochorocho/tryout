SIDE=poststart
setup() { mkdir -p TYPO3-Instances/primary/config/system; echo "<?php return [];" > TYPO3-Instances/primary/config/system/settings.php; }
