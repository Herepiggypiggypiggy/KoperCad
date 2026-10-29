//! Install, update, roll back, remove, and what happens when the files on disk go wrong.

mod common;

use common::*;
use novo_addons::{
    AddonError, AddonSet, AppInfo, Change, Extensions, Home, InstallOptions, Status, Store, Version,
};

fn tick() -> InstallOptions {
    InstallOptions {
        accept_unsigned: true,
        enable: true,
    }
}

#[test]
fn unsigned_needs_the_tick_signed_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let store = trusting_store(dir.path());
    let unsigned = package("demo", "1.0.0", "x", false);
    assert!(matches!(
        store.install_bytes(&unsigned, InstallOptions::default()),
        Err(AddonError::NotAccepted)
    ));
    assert!(store.scan().entries.is_empty());

    let signed = package("demo", "1.0.0", "x", true);
    let i = store
        .install_bytes(&signed, InstallOptions::default())
        .unwrap();
    assert!(i.package.trust.is_signed());
    let set = store.scan();
    assert!(set.is_enabled("demo"));
    assert_eq!(set.get("demo").unwrap().active, Version::new(1, 0, 0));
}

#[test]
fn a_signed_package_is_only_signed_where_its_key_is_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path()); // no trusted keys here
    let signed = package("demo", "1.0.0", "x", true);
    let inspection = store.inspect(&signed).unwrap();
    assert!(inspection.needs_trust_tick());
    assert!(!inspection.package.trust.is_signed());
}

#[test]
fn install_enable_read_data_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "payload-one", false), tick())
        .unwrap();
    let set = store.scan();
    let e = set.get("demo").unwrap();
    assert_eq!(e.status, Status::Enabled);
    assert!(e.verified.values().all(|ok| *ok));
    assert_eq!(
        set.read_file("demo", "data/profile.toml").unwrap(),
        b"payload-one"
    );
    assert_eq!(
        set.contributions("demo-profile"),
        vec![("demo".to_owned(), "data/profile.toml".to_owned())]
    );
    assert!(set.contributions("other").is_empty());
    // only listed files can be read
    assert!(set.read_file("demo", "../registry.toml").is_err());
    assert!(set.read_file("demo", "manifest.toml").is_err());
    assert!(set.read_file("nope", "data/profile.toml").is_err());

    store.set_enabled("demo", false).unwrap();
    let set = store.scan();
    assert_eq!(set.get("demo").unwrap().status, Status::Disabled);
    assert!(!set.is_enabled("demo"));
    assert!(
        set.read_file("demo", "data/profile.toml").is_err(),
        "a disabled add-on serves nothing"
    );
    assert!(set.contributions("demo-profile").is_empty());

    store.set_enabled("demo", true).unwrap();
    assert!(store.scan().is_enabled("demo"));
    store.remove("demo").unwrap();
    assert!(store.scan().entries.is_empty());
    assert!(!store.root().join("demo").exists());
    assert!(matches!(store.remove("demo"), Err(AddonError::Unknown(_))));
    assert!(matches!(store.remove("../x"), Err(AddonError::Unknown(_))));
}

#[test]
fn update_keeps_the_previous_version_and_rolls_back_and_forward() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "one", false), tick())
        .unwrap();
    store.set_enabled("demo", false).unwrap();
    let i = store
        .install_bytes(&package("demo", "1.1.0", "two", false), tick())
        .unwrap();
    assert_eq!(
        i.change,
        Change::Update {
            from: Version::new(1, 0, 0)
        }
    );
    let set = store.scan();
    let e = set.get("demo").unwrap();
    assert_eq!(e.active, Version::new(1, 1, 0));
    assert_eq!(e.previous, Some(Version::new(1, 0, 0)));
    assert_eq!(
        e.status,
        Status::Disabled,
        "an update keeps the on/off state"
    );

    store.set_enabled("demo", true).unwrap();
    assert_eq!(
        store.scan().read_file("demo", "data/profile.toml").unwrap(),
        b"two"
    );
    assert_eq!(store.roll_back("demo").unwrap(), Version::new(1, 0, 0));
    let set = store.scan();
    assert_eq!(set.read_file("demo", "data/profile.toml").unwrap(), b"one");
    assert_eq!(
        set.get("demo").unwrap().previous,
        Some(Version::new(1, 1, 0))
    );
    // and forward again
    assert_eq!(store.roll_back("demo").unwrap(), Version::new(1, 1, 0));
    assert_eq!(
        store.scan().read_file("demo", "data/profile.toml").unwrap(),
        b"two"
    );

    // a third version drops the oldest folder
    store
        .install_bytes(&package("demo", "1.2.0", "three", false), tick())
        .unwrap();
    assert!(store.root().join("demo/1.1.0").exists());
    assert!(!store.root().join("demo/1.0.0").exists());

    store.remove("demo").unwrap();
    store
        .install_bytes(&package("demo", "1.0.0", "one", false), tick())
        .unwrap();
    assert!(matches!(
        store.roll_back("demo"),
        Err(AddonError::NoPrevious(_))
    ));
}

#[test]
fn same_older_and_changed_same_version_are_refused_with_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.1.0", "a", false), tick())
        .unwrap();
    let again = store
        .install_bytes(&package("demo", "1.1.0", "a", false), tick())
        .unwrap_err();
    assert!(again.to_string().contains("already installed"), "{again}");
    let older = store
        .install_bytes(&package("demo", "1.0.0", "a", false), tick())
        .unwrap_err();
    assert!(
        older.to_string().contains("newer than this file"),
        "{older}"
    );
    let changed = store
        .install_bytes(&package("demo", "1.1.0", "different", false), tick())
        .unwrap_err();
    assert!(
        changed.to_string().contains("different content"),
        "{changed}"
    );
    assert_eq!(
        store.scan().read_file("demo", "data/profile.toml").unwrap(),
        b"a"
    );
}

#[test]
fn an_incompatible_package_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let newer = pack_with(|m| m.replace(">=0.1.0, <0.3", ">=0.5.0"));
    let err = store.install_bytes(&newer, tick()).unwrap_err();
    assert!(
        err.to_string()
            .contains("needs KoperCAD 0.5.0 or newer (this is 0.1.0)"),
        "{err}"
    );
    let too_new_app = pack_with(|m| m.replace(">=0.1.0, <0.3", "<0.1.0"));
    let err = store.install_bytes(&too_new_app, tick()).unwrap_err();
    assert!(err.to_string().contains("works with KoperCAD"), "{err}");
    assert!(!store.root().exists() || listing(store.root()).is_empty());
}

fn pack_with(edit: impl Fn(String) -> String) -> Vec<u8> {
    novo_addons::pack::pack_parts(
        edit(manifest("demo", "1.0.0", "")).as_bytes(),
        &files("demo", "x"),
        None,
    )
    .unwrap()
}

#[test]
fn an_add_on_that_fits_a_later_program_shows_as_incompatible_not_gone() {
    let dir = tempfile::tempdir().unwrap();
    let store1 = store(dir.path());
    store1
        .install_bytes(&package("demo", "1.0.0", "a", false), tick())
        .unwrap();
    // the program is updated past the add-on's range
    let store2 = Store::new(
        store1.root().to_path_buf(),
        AppInfo {
            version: Version::new(0, 3, 0),
            api_version: 1,
        },
    );
    let set = store2.scan();
    assert!(
        matches!(&set.get("demo").unwrap().status, Status::Incompatible(m) if m.contains("works with KoperCAD"))
    );
    assert!(!set.is_enabled("demo"));
    assert_eq!(set.attention(), 1);
}

#[test]
fn damage_is_contained_to_the_one_add_on() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("good", "1.0.0", "a", false), tick())
        .unwrap();
    store
        .install_bytes(&package("bad", "1.0.0", "a", false), tick())
        .unwrap();

    // a data file edited after install
    std::fs::write(store.root().join("bad/1.0.0/data/profile.toml"), "tampered").unwrap();
    let set = store.scan();
    assert!(set.is_enabled("good"));
    assert!(
        matches!(&set.get("bad").unwrap().status, Status::Failed(m) if m.contains("data/profile.toml"))
    );
    assert!(set.read_file("bad", "data/profile.toml").is_err());
    assert_eq!(set.attention(), 1);

    // manifest edited
    let manifest_path = store.root().join("bad/1.0.0/manifest.toml");
    let text = std::fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("Demo bad", "Demo worse");
    std::fs::write(&manifest_path, text).unwrap();
    assert!(
        matches!(&store.scan().get("bad").unwrap().status, Status::Failed(m) if m.contains("manifest changed"))
    );

    // files deleted
    std::fs::remove_dir_all(store.root().join("bad/1.0.0")).unwrap();
    assert!(
        matches!(&store.scan().get("bad").unwrap().status, Status::Failed(m) if m.contains("missing"))
    );

    // repairing it from the same file works, and it remains one entry
    let i = store
        .install_bytes(&package("bad", "1.0.0", "a", false), tick())
        .unwrap();
    assert_eq!(i.change, Change::Repair);
    assert!(store.scan().is_enabled("bad"));
    assert_eq!(store.scan().entries.len(), 2);
}

#[test]
fn a_damaged_registry_is_set_aside_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "a", false), tick())
        .unwrap();
    std::fs::write(store.root().join("registry.toml"), "this is [[ not toml").unwrap();
    let set = store.scan();
    assert!(set.entries.is_empty());
    assert!(
        set.warnings
            .iter()
            .any(|w| w.contains("registry.toml.damaged"))
    );
    assert!(store.root().join("registry.toml.damaged").exists());
    // and the program can install again
    store
        .install_bytes(&package("demo", "1.0.0", "a", false), tick())
        .unwrap();
    assert!(store.scan().is_enabled("demo"));
}

#[test]
fn a_hand_edited_registry_cannot_point_outside_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    std::fs::create_dir_all(store.root()).unwrap();
    std::fs::write(
        store.root().join("registry.toml"),
        "[[addon]]\nid = \"../../etc\"\nenabled = true\nactive = \"1.0.0\"\nfingerprint = \"x\"\n\n[[addon]]\nid = \"ok-one\"\nenabled = true\nactive = \"../../x\"\nfingerprint = \"x\"\n",
    )
    .unwrap();
    let set = store.scan();
    assert!(set.warnings.iter().any(|w| w.contains("unusable")));
    // a version that is not a version is dropped on load, never joined to a path
    assert!(set.entries.is_empty());
}

#[test]
fn a_signed_add_on_whose_key_was_removed_stops_loading_until_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let trusting = trusting_store(dir.path());
    trusting
        .install_bytes(
            &package("demo", "1.0.0", "a", true),
            InstallOptions::default(),
        )
        .unwrap();
    assert!(trusting.scan().is_enabled("demo"));
    std::fs::remove_file(trusting.root().join("trusted-keys.toml")).unwrap();
    let fresh = Store::new(trusting.root().to_path_buf(), app());
    let set = fresh.scan();
    assert!(
        matches!(&set.get("demo").unwrap().status, Status::Failed(m) if m.contains("not signed by a trusted publisher"))
    );
}

#[test]
fn extensions_are_built_from_running_add_ons_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("one", "1.0.0", "a", false), tick())
        .unwrap();
    store
        .install_bytes(&package("two", "1.0.0", "a", false), tick())
        .unwrap();
    store.set_enabled("two", false).unwrap();
    let set: AddonSet = store.scan();
    let ext = Extensions::build(&set, &|key| key == "Shift+D");
    // both add-ons ask for Shift+D; the program already uses it, so neither gets it
    let open = ext.command("addon.one.open").unwrap();
    assert_eq!(open.label, "Open demo");
    assert!(open.key_conflict);
    assert_eq!(open.opens.as_deref(), Some("addon.one.main"));
    assert_eq!(open.home, Home::Menu("Simulate".into()));
    assert!(
        ext.command("addon.two.open").is_none(),
        "a disabled add-on adds nothing"
    );
    assert_eq!(ext.panel("addon.one.main").unwrap().title, "Main");
    assert!(ext.icons.contains_key("one.thing"));
    assert_eq!(ext.menu_rows("Simulate").len(), 1);
    assert!(ext.menu_rows("File").is_empty());
    let rail = ext.rail_rows("3d");
    assert_eq!(rail.len(), 1);
    assert_eq!(rail[0].0, "one");
    assert!(ext.rail_rows("cabinet").is_empty());

    // a free key goes to the first add-on only
    store.set_enabled("two", true).unwrap();
    let ext = Extensions::build(&store.scan(), &|_| false);
    assert!(!ext.command("addon.one.open").unwrap().key_conflict);
    assert!(
        ext.command("addon.two.open").unwrap().key_conflict,
        "two add-ons cannot share a key"
    );
}

#[test]
fn packing_is_deterministic_and_a_folder_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(src.join("data")).unwrap();
    std::fs::write(src.join("manifest.toml"), manifest("demo", "1.0.0", "")).unwrap();
    for (name, bytes) in files("demo", "x") {
        std::fs::write(src.join(name), bytes).unwrap();
    }
    let a = novo_addons::pack::pack_dir(&src, Some(&signer())).unwrap();
    let b = novo_addons::pack::pack_dir(&src, Some(&signer())).unwrap();
    assert_eq!(a, b);
    let store = trusting_store(dir.path());
    let i = store.install_bytes(&a, InstallOptions::default()).unwrap();
    assert!(i.package.trust.is_signed());
    assert_eq!(i.package.files.len(), 4);

    // the packer refuses a manifest that already has [files], or a stray signature file
    std::fs::write(
        src.join("manifest.toml"),
        format!("{}\n[files]\n", manifest("demo", "1.0.0", "")),
    )
    .unwrap();
    assert!(
        novo_addons::pack::pack_dir(&src, None)
            .unwrap_err()
            .to_string()
            .contains("[files]")
    );
}

#[test]
fn installing_the_version_that_a_roll_back_left_behind_keeps_it() {
    // 1.0.0, 2.0.0, Roll back (active 1.0.0, previous 2.0.0), then the 2.0.0 file again.
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let v1 = package("demo", "1.0.0", "one", false);
    let v2 = package("demo", "2.0.0", "two", false);
    store.install_bytes(&v1, tick()).unwrap();
    store.install_bytes(&v2, tick()).unwrap();
    assert_eq!(store.roll_back("demo").unwrap(), Version::new(1, 0, 0));
    store.install_bytes(&v2, tick()).unwrap();
    let set = store.scan();
    let e = set.get("demo").unwrap();
    assert_eq!(e.active, Version::new(2, 0, 0));
    assert_eq!(e.status, Status::Enabled, "{:?}", e.status);
    assert_eq!(e.previous, Some(Version::new(1, 0, 0)));
    assert!(
        dir.path()
            .join("addons")
            .join("demo")
            .join("2.0.0")
            .join("manifest.toml")
            .exists()
    );
    assert!(
        dir.path()
            .join("addons")
            .join("demo")
            .join("1.0.0")
            .join("manifest.toml")
            .exists()
    );
}

#[test]
fn roll_back_keeps_the_answer_for_each_version() {
    // 1.0.0 signed (no tick), 2.0.0 unsigned (ticked): rolling back to the signed one needs no
    // tick and rolling forward again still has the earlier answer for 2.0.0.
    let dir = tempfile::tempdir().unwrap();
    let store = trusting_store(dir.path());
    store
        .install_bytes(
            &package("demo", "1.0.0", "one", true),
            InstallOptions::default(),
        )
        .unwrap();
    store
        .install_bytes(&package("demo", "2.0.0", "two", false), tick())
        .unwrap();
    assert_eq!(store.roll_back("demo").unwrap(), Version::new(1, 0, 0));
    assert_eq!(store.roll_back("demo").unwrap(), Version::new(2, 0, 0));
    assert!(store.scan().is_enabled("demo"));

    // An unsigned 1.0.0 that was never ticked cannot come back by Roll back: the answer
    // belonged to 2.0.0 only.
    let dir = tempfile::tempdir().unwrap();
    let store = trusting_store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "one", false), tick())
        .unwrap();
    store
        .install_bytes(
            &package("demo", "2.0.0", "two", true),
            InstallOptions::default(),
        )
        .unwrap();
    assert_eq!(store.roll_back("demo").unwrap(), Version::new(1, 0, 0));
}

#[test]
fn a_file_the_manifest_does_not_list_fails_the_add_on() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "x", false), tick())
        .unwrap();
    std::fs::write(
        dir.path()
            .join("addons")
            .join("demo")
            .join("1.0.0")
            .join("data")
            .join("extra.toml"),
        "x",
    )
    .unwrap();
    let set = store.scan();
    match &set.get("demo").unwrap().status {
        Status::Failed(why) => assert!(why.contains("extra.toml"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ids_windows_keeps_for_devices_are_refused() {
    for id in ["con", "nul", "aux", "prn", "com1", "lpt9"] {
        let bytes = package(id, "1.0.0", "x", false);
        assert!(
            store(tempfile::tempdir().unwrap().path())
                .inspect(&bytes)
                .is_err(),
            "{id}"
        );
    }
}

#[test]
fn a_stale_lock_from_a_crashed_program_does_not_block() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "x", false), tick())
        .unwrap();
    let lock = dir.path().join("addons").join("registry.lock");
    std::fs::write(&lock, "").unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    std::fs::File::options()
        .write(true)
        .open(&lock)
        .unwrap()
        .set_modified(old)
        .unwrap();
    store.set_enabled("demo", false).unwrap();
    assert!(!lock.exists());
}

#[test]
fn files_a_file_manager_drops_in_do_not_fail_an_add_on() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "x", false), tick())
        .unwrap();
    let folder = dir.path().join("addons").join("demo").join("1.0.0");
    std::fs::write(folder.join("Thumbs.db"), "x").unwrap();
    std::fs::write(folder.join("data").join("desktop.ini"), "x").unwrap();
    assert!(store.scan().is_enabled("demo"));
}

#[test]
fn a_version_in_the_registry_that_is_not_a_version_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .install_bytes(&package("demo", "1.0.0", "x", false), tick())
        .unwrap();
    let registry = dir.path().join("addons").join("registry.toml");
    let text = std::fs::read_to_string(&registry).unwrap();
    std::fs::write(
        &registry,
        text.replace("active = \"1.0.0\"", "active = \"../../x\""),
    )
    .unwrap();
    let set = store.scan();
    assert!(set.entries.is_empty());
    assert!(
        set.warnings.iter().any(|w| w.contains("versions")),
        "{:?}",
        set.warnings
    );
}
