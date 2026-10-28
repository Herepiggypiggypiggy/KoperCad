//! The Windows installer script is checked on every platform: the no-admin
//! rule is a hard requirement and the installer itself can only be run on
//! Windows, so its text is held to the rule here.

fn script() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packaging/windows/novoelectric.iss"
    );
    std::fs::read_to_string(path).expect("the installer script is in the repository")
}

/// Lines with comments removed, in lower case.
fn code_lines(s: &str) -> Vec<String> {
    s.lines()
        .filter(|l| !l.trim_start().starts_with(';'))
        .map(|l| l.to_lowercase())
        .collect()
}

#[test]
fn the_installer_asks_for_no_admin_rights() {
    let lines = code_lines(&script());
    assert!(
        lines
            .iter()
            .any(|l| l.trim() == "privilegesrequired=lowest")
    );
    for bad in [
        "privilegesrequired=admin",
        "poweruserdialog",
        "{pf}",
        "{pf32}",
        "{pf64}",
        "{commonappdata}",
        "{commondesktop}",
        "{commonprograms}",
        "{win}",
        "{sys}",
        "hklm",
        "hkcr",
        "runasoriginaluser",
    ] {
        assert!(
            !lines.iter().any(|l| l.contains(bad)),
            "the installer script uses {bad}"
        );
    }
}

#[test]
fn files_go_under_the_users_profile() {
    let lines = code_lines(&script());
    let dir = lines
        .iter()
        .find(|l| l.starts_with("defaultdirname="))
        .expect("a default folder");
    assert!(dir.contains("{localappdata}"), "{dir}");
    // The folder question is skipped, so no one can pick Program Files.
    assert!(lines.iter().any(|l| l.trim() == "disabledirpage=yes"));
}

#[test]
fn both_project_extensions_open_and_older_installs_are_cleaned() {
    let s = script();
    assert!(s.contains(r#"Subkey: "Software\Classes\.kopercad""#));
    assert!(s.contains(r#"Subkey: "Software\Classes\.novo""#));
    assert!(s.contains("ChangesAssociations=yes"));
    // Every registry line is per-user and removed again on uninstall.
    for l in s.lines().filter(|l| l.starts_with("Root:")) {
        assert!(l.starts_with("Root: HKCU;"), "{l}");
    }
    assert!(s.contains(r#"Name: "{userprograms}\{#OldAppName}.lnk""#));
}

#[test]
fn an_update_over_a_running_copy_closes_it_and_the_user_data_survives_uninstall() {
    let s = script();
    assert!(s.contains("CloseApplications=yes"));
    // The uninstall question defaults to No and never touches projects.
    assert!(s.contains("MB_DEFBUTTON2"));
    assert!(!s.contains("Documents"));
}

#[test]
fn uninstall_leaves_no_empty_file_type_keys_and_clears_the_old_name_folders() {
    let s = script();
    for ext in [".kopercad", ".novo"] {
        let line = s
            .lines()
            .find(|l| l.contains(&format!(r#"Classes\{ext}""#)))
            .expect(ext);
        assert!(line.contains("uninsdeletekeyifempty"), "{line}");
    }
    assert!(s.contains("DelTree(OldLocal"));
    assert!(s.contains("DelTree(OldRoaming"));
}

#[test]
fn a_running_program_is_closed_before_install_and_uninstall() {
    // The Restart Manager cannot close the window (hardware test KC-14). Only
    // the copy in the install folder is closed, never a portable copy.
    let s = script();
    assert!(s.contains("function PrepareToInstall"));
    assert!(s.contains("function InitializeUninstall"));
    assert!(s.contains("CloseMainWindow"));
    // WizardSilent is Setup-only: the uninstaller must read UninstallSilent.
    assert!(s.contains("CloseRunningApp(WizardSilent)"));
    assert!(s.contains("CloseRunningApp(UninstallSilent)"));
    assert!(s.contains("{app}"));
    assert!(
        !s.contains("taskkill"),
        "match on the install path, not the image name"
    );
}
