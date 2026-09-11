<?php

// #ddev-generated — provided by the tryout DDEV add-on.
//
// Points the PRIMARY instance's overlay at a Core checkout.
//
// Usage: use-core.php <name>     serve worktrees/<name>'s Core
//        use-core.php            serve the root checkout (the default)
//
// This is what `ddev tryout worktree use` moves. It used to be the typo3-core
// symlink; the project root IS the Core clone now, so there is no symlink left to
// move and the overlay's path repository is the pointer instead. Editing it here
// rather than in shell because the file is JSON and site-composer.php already
// owns the same rewrite for served sites.

declare(strict_types=1);

$name = $argv[1] ?? '';
if ($name !== '' && !preg_match('/^[A-Za-z0-9][A-Za-z0-9._-]*$/', $name)) {
    fwrite(STDERR, "use-core: invalid worktree name '$name'\n");
    exit(64);
}

$root = getenv('DDEV_APPROOT') ?: '/var/www/html';
$file = $root . '/TYPO3-Instances/primary/'
      . (getenv('TRYOUT_COMPOSER_FILE') ?: 'composer.tryout.json');

if (!file_exists($file)) {
    fwrite(STDERR, "use-core: $file not found\n");
    exit(1);
}

$data = json_decode(file_get_contents($file), true);
if ($data === null) {
    fwrite(STDERR, "use-core: cannot parse $file\n");
    exit(1);
}

// Same shape site-composer.php writes for a served site, one level shallower:
// the primary lives at TYPO3-Instances/primary, so ../.. is the project root.
$url = $name === ''
    ? '../../typo3/sysext/*'
    : '../../worktrees/' . $name . '/typo3/sysext/*';

$found = false;
foreach (($data['repositories'] ?? []) as $i => $repo) {
    if (($repo['type'] ?? '') !== 'path') {
        continue;
    }
    // The sysext repository, whichever checkout it currently names. packages/*
    // is the other path repo and must be left alone.
    if (str_contains($repo['url'] ?? '', 'typo3/sysext')) {
        $data['repositories'][$i]['url'] = $url;
        $found = true;
    }
}

if (!$found) {
    fwrite(STDERR, "use-core: no sysext path repository in $file\n");
    exit(1);
}

$json = json_encode($data, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES);
if ($json === false || file_put_contents($file, $json . "\n") === false) {
    fwrite(STDERR, "use-core: cannot write $file\n");
    exit(1);
}

echo $url, "\n";
