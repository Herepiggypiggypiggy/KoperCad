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

fn repo_file(relative: &str) -> String {
    let path = format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Non-comment lines of a workflow or a script (`#` comments removed).
fn code_of(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_experimental_zip_has_launchers_readmes_and_one_copy_of_the_program() {
    let workflow = code_of(&repo_file(".github/workflows/experimental.yml"));
    let package = workflow
        .split("- name: Package")
        .nth(1)
        .and_then(|rest| rest.split("- uses: actions/upload-artifact").next())
        .expect("the Package step");
    for shipped in [
        "target/release/kopercad.exe",
        "target/release/kopercad-cli.exe",
        "packaging/windows/portable/run.cmd",
        "packaging/windows/portable/run-safe-graphics.cmd",
        "packaging/windows/experimental/README-EXPERIMENTAL.txt",
        "docs/TESTER-README.md",
    ] {
        assert!(package.contains(shipped), "the zip does not get {shipped}");
    }
    // 74 MB each: a second copy of the program and the stress demo stay out.
    assert!(!package.contains("dist/novoelectric.exe"));
    assert!(!package.contains("dist/canvas-demo.exe"));
    assert!(package.contains("out/canvas-demo.exe"));
    // A portable marker would send data to the folder the launcher replaces.
    assert!(!package.contains("kopercad.portable"));
    for file in [
        "packaging/windows/portable/run.cmd",
        "packaging/windows/portable/run-safe-graphics.cmd",
        "packaging/windows/experimental/README-EXPERIMENTAL.txt",
    ] {
        assert!(!repo_file(file).trim().is_empty(), "{file} is empty");
    }
    assert!(
        repo_file("packaging/windows/portable/run-safe-graphics.cmd").contains("--safe-graphics")
    );
    // The branch for `git clone` users gets the safe graphics launcher too.
    assert!(workflow.contains("packaging/windows/portable/run-safe-graphics.cmd stage/"));
}

#[test]
fn the_launcher_scripts_need_no_github_sign_in() {
    // The repository is public: no `gh`, no token, no `gh auth`.
    for script in [
        "scripts/run-latest.ps1",
        "scripts/setup-new-pc.ps1",
        "scripts/build-local.ps1",
    ] {
        let code = code_of(&repo_file(script));
        for needle in ["gh.exe", "gh auth", "Bearer", "Get-Command gh"] {
            assert!(
                !code.contains(needle),
                "{script} still uses {needle:?}, which needs a GitHub sign-in"
            );
        }
        // The command `gh` as a word ("through " must not count).
        let word_gh = code
            .match_indices("gh ")
            .any(|(i, _)| !code[..i].ends_with(|c: char| c.is_alphanumeric()));
        assert!(!word_gh, "{script} still calls the GitHub CLI");
    }
    let run = repo_file("scripts/run-latest.ps1");
    assert!(run.contains("releases/download/experimental"));
}
