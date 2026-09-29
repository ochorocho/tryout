SIDE=host
ARGS=(delete --yes)
FILES=("tree:TYPO3-Instances/primary")
setup() { mkdir -p TYPO3-Instances/primary/public/fileadmin/x TYPO3-Instances/primary/config/system; touch TYPO3-Instances/primary/public/fileadmin/x/a.jpg TYPO3-Instances/primary/config/system/settings.php; }
