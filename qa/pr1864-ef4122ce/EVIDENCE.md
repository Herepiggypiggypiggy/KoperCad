# PR 1864 evidence

Source at git ef4122ce09602881999e682a09d1358232e7fc3f (HEAD.txt; this folder is the only change after it). Scenario 18640, 1366x768 and 1920x1080, Linux, binary built from that head. Pictures: /mnt/project-files/qa/pr1864-ef4122ce/ (not committed).

Where the user finds it: Parts window, row under the toolbar, **Get parts from supplier** (list of suppliers, Downloads folder, Forget sign-in, Use my normal browser, "?"). The Import, Get CAD from TraceParts and Open page again buttons route known suppliers into the same window.

Shown by the scenario: the menu (supplier-menu) and the fallback with no web view available (Linux): the normal browser is used, the strip says so and which folder is watched (fallback-no-webview). The one S4 finding at 1920 is the spinner beside the strip text, no overlap of controls (see picture).

## NOT tested
- A real WebView2 window. Nothing was opened, signed in to or downloaded on Windows. host.rs only type-checks and passes clippy for x86_64-pc-windows-gnu; the Windows CI job runs clippy and tests of the changed crates.
- Supplier addresses other than WAGO are unconfirmed (`verified: false` in `supplier_site::builtin()`).
- Pop-up SSO log-ins, blob downloads, per-supplier cookies surviving a restart.

## Questions from Lead A
- **URL allow-list:** there is no host list (SSO log-ins hop between domains). Pages that load: http and https only, no user name or password in the address, plus `about:blank` and `blob:http...` for downloads. file:, data:, javascript:, ftp: and the like are refused (`url_allowed`, tested). Pop-ups open in the same window. Suppliers added in `suppliers.json` need an http/https start page and a folder-safe id, else the entry is skipped with a note.
- **WebView2 user-data folder:** `%LOCALAPPDATA%\KoperCAD\browser\profile\<supplier id>\` (through `novo_storage::profile::data_local_dir`, so a portable install keeps it beside the program). One folder per supplier. No registry write, no HKLM, no admin. The WebView2 runtime itself is Microsoft's Evergreen runtime, part of Windows 11 and current Windows 10; this PR does not install it. If it is missing the window exits with code 3 and the Parts window uses the normal browser (scenario shows this path).
- **What is downloaded and stored:** only what the user downloads in the window, and only STEP, DXF, ZIP, EDZ and catalog files that the supplier entry accepts (others are refused at the download handler, e.g. setup.exe). Written as `<stamp>-<name>.part` under `%LOCALAPPDATA%\KoperCAD\browser\landing\<id>\` and renamed when complete; the Parts window feeds it through the normal drop pipeline (the matcher reads it first). Landed files older than 7 days are deleted when a session starts. The folders are outside the parts library, so library bundles (.kcpl) and the repo never include them (test). Cookies stay in the profile folder; "Forget sign-in" deletes it. KoperCAD never reads, fills in or stores a password and injects no script (test over host.rs).
- **Licence terms:** KoperCAD fetches nothing and does not click through terms; the user accepts each supplier's own terms on its page (CADENAS portals forbid automated queries, so none are made). Models stay personal, the same rule as the Downloads hand-off.
- **Non-Windows builds unaffected:** wry and tao are `[target.'cfg(windows)'.dependencies]`; host.rs is `#[cfg(windows)]`. On Linux `supplier_browser::launch` answers "no runtime" and the Parts window falls back, which is what the scenario shows. Cargo.lock gains entries for them (they are not built on Linux).
