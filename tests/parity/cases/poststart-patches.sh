SIDE=poststart
CASE_ENV=(TRYOUT_PATCHES='90000,91234')
FILES=("git:log --oneline -2")
setup() { mkdir -p TYPO3-Instances/primary/config/system; touch TYPO3-Instances/primary/config/system/settings.php; }
