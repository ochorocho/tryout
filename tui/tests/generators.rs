//! The generated files, byte for byte against what the bash and PHP wrote
//! (tests/fixtures/generators/, captured by tests/parity/goldens.sh).

use std::path::{Path, PathBuf};

use tryout::core::composer;
use tryout::core::ctx::{Ctx, DdevEnv};
use tryout::core::webserver;

fn golden(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/generators")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The parity fixture's shape: three sysexts and the primary overlay.
fn project(webserver: &str) -> (tempfile::TempDir, Ctx) {
    let d = tempfile::tempdir().unwrap();
    for ext in ["core", "backend", "frontend"] {
        let dir = d.path().join("typo3/sysext").join(ext);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("composer.json"),
            format!("{{\n    \"name\": \"typo3/cms-{ext}\"\n}}\n"),
        )
        .unwrap();
    }
    let instance = d.path().join("TYPO3-Instances/primary");
    std::fs::create_dir_all(&instance).unwrap();
    std::fs::write(
        instance.join("composer.tryout.json"),
        golden("overlay-before.json"),
    )
    .unwrap();
    let env = DdevEnv {
        sitename: "parity".into(),
        php_version: "8.4".into(),
        webserver_type: webserver.into(),
        ..Default::default()
    };
    let ctx = Ctx::new(d.path(), env);
    (d, ctx)
}

/// The project after `sync` — where goldens.sh took the later snapshots.
fn synced() -> (tempfile::TempDir, Ctx) {
    let (d, ctx) = project("nginx-fpm");
    std::fs::write(
        ctx.instance_dir().join("composer.tryout.json"),
        golden("overlay-synced.json"),
    )
    .unwrap();
    (d, ctx)
}

fn read(p: PathBuf) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn vhosts_match_for_both_webservers() {
    let (_d, ctx) = project("nginx-fpm");
    assert_eq!(
        webserver::vhost(&ctx, "v13", "8.2"),
        golden("vhost-nginx.conf")
    );
    assert_eq!(
        webserver::vhost(&ctx, "v13", "8.4"),
        golden("vhost-nginx-default-php.conf")
    );
    let (_d, ctx) = project("apache-fpm");
    assert_eq!(
        webserver::vhost(&ctx, "v13", "8.2"),
        golden("vhost-apache.conf")
    );
}

#[test]
fn the_nginx_vhost_passes_https_literally() {
    let (_d, ctx) = project("nginx-fpm");
    assert!(webserver::vhost(&ctx, "v13", "8.2").contains("fastcgi_param HTTPS $fcgi_https;\n"));
}

#[test]
fn the_server_name_hash_is_sized_for_the_longest_name() {
    assert_eq!(
        webserver::hash_config(&["v13.parity".into()]).unwrap(),
        golden("hash-short.conf")
    );
    let long = [
        "a.parity".to_string(),
        "a-worktree-name-of-quite-ordinary-length.some-project-name".into(),
    ];
    assert_eq!(
        webserver::hash_config(&long).unwrap(),
        golden("hash-long.conf")
    );
    assert_eq!(webserver::hash_config(&[]), None);
}

#[test]
fn the_worktree_config_lists_hosts_and_one_daemon_per_extra_php() {
    let (d, ctx) = project("nginx-fpm");
    for (name, php) in [("a", "8.2"), ("b", "8.3"), ("c", "8.2"), ("d", "8.4")] {
        let dir = d.path().join("TYPO3-Instances").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".tryout-site"), format!("php={php}\n")).unwrap();
    }
    std::fs::create_dir_all(d.path().join(".ddev")).unwrap();
    webserver::write_worktree_config(&ctx).unwrap();
    assert_eq!(read(ctx.worktree_config()), golden("config.worktrees.yaml"));
    assert!(webserver::hash_config_file(&ctx).is_file());
    let set: String = webserver::served_hostname_set(&ctx)
        .iter()
        .map(|h| format!("{h}\n"))
        .collect();
    assert_eq!(set, golden("hostname-set.txt"));

    // Nothing served: both files go.
    for name in ["a", "b", "c", "d"] {
        std::fs::remove_file(
            d.path()
                .join("TYPO3-Instances")
                .join(name)
                .join(".tryout-site"),
        )
        .unwrap();
    }
    webserver::write_worktree_config(&ctx).unwrap();
    assert!(!ctx.worktree_config().exists());
    assert!(!webserver::hash_config_file(&ctx).exists());
}

#[test]
fn sync_requires_exactly_the_sysexts_on_disk() {
    let (d, ctx) = project("nginx-fpm");
    let lock = ctx.instance_dir().join("composer.tryout.lock");
    std::fs::write(&lock, "").unwrap();
    let msg = composer::sync(&ctx.instance_dir(), d.path()).unwrap();
    assert_eq!(format!("{msg}\n"), golden("sync.out"));
    assert_eq!(
        read(ctx.instance_dir().join("composer.tryout.json")),
        golden("overlay-synced.json")
    );
    assert!(
        !lock.exists(),
        "the lock goes, so a removed sysext really disappears"
    );
}

#[test]
fn sync_refuses_without_sysexts() {
    let (d, ctx) = project("nginx-fpm");
    std::fs::remove_dir_all(d.path().join("typo3")).unwrap();
    assert!(
        composer::sync(&ctx.instance_dir(), d.path())
            .unwrap_err()
            .contains("Clone TYPO3 Core first")
    );
}

#[test]
fn a_site_overlay_points_at_its_own_worktree() {
    let (_d, ctx) = synced();
    composer::site_overlay(&ctx, "v13", "8.2").unwrap();
    assert_eq!(
        read(ctx.instances_dir().join("v13/composer.tryout.json")),
        golden("overlay-site-v13.json")
    );
    assert!(composer::site_overlay(&ctx, "../x", "").is_err());
    assert!(composer::site_overlay(&ctx, "v13", "8").is_err());
}

#[test]
fn use_core_moves_only_the_sysext_repository() {
    let (_d, ctx) = synced();
    let overlay = ctx.instance_dir().join("composer.tryout.json");
    assert_eq!(
        format!("{}\n", composer::use_core(&ctx, "v13").unwrap()),
        golden("use-core-v13.out")
    );
    assert_eq!(read(overlay.clone()), golden("overlay-use-v13.json"));
    assert_eq!(ctx.active_worktree_name(), "v13");
    assert_eq!(
        format!("{}\n", composer::use_core(&ctx, "").unwrap()),
        golden("use-core-root.out")
    );
    assert_eq!(read(overlay), golden("overlay-use-root.json"));
    assert!(composer::use_core(&ctx, "-x").is_err());
}
