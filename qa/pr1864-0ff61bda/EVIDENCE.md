# PR 1864 evidence

Source at git 0ff61bdafcc0f7c5892f533b4750d16c8d6967c3 (HEAD.txt; this folder is the only change after it). Scenario 18640, 1366x768 and 1920x1080, Linux, binary built from that head (report.md: "source at git 0ff61bdafc"). Pictures: /mnt/project-files/qa/pr1864-0ff61bda/ (not committed).

Where the user finds it: Parts window, row under the toolbar: **Get parts from supplier** (suppliers, Downloads folder, Forget sign-in, Use my normal browser) with a "?" beside it. The Import, Get CAD from TraceParts and Open page again buttons route known suppliers into the same window.

Pictures: 01 supplier-menu, 02 submenu-downloads-folder, 03 submenu-forget-sign-in, 04 help-text (the "?" popup), 05 fallback-no-webview (NoRuntime note, "Rittal PARTcommunity is open. Files taken: 0", Stop). The one S4 finding at 1920 is the spinner beside the strip text, not an overlap of controls.

## NOT tested
- A real WebView2 window: nothing was opened, signed in to or downloaded on Windows. host.rs is only type-checked and clippy-checked for x86_64-pc-windows-gnu; the Windows CI job is the first compile on Windows.
- Files taken counting above 0 is covered by unit tests (`parts_suppliers`), not by a picture.
- Supplier addresses other than WAGO are unconfirmed (`verified: false`).
- Pop-up SSO log-ins, blob downloads, cookies surviving a restart.

## Questions
- **URL allow-list:** no host list (SSO hops between domains). Only http and https, no user name or password in the address, plus `about:blank` and `blob:http...`. file:, data:, javascript:, ftp: refused (`url_allowed`, tested). Pop-ups open in the same window. A `suppliers.json` entry needs an http/https start page and a folder-safe id or it is skipped with a note.
- **WebView2 user-data folder:** `%LOCALAPPDATA%\KoperCAD\browser\profile\<supplier id>\` via `novo_storage::profile::data_local_dir` (a portable install keeps it beside the program). No registry write, no HKLM, no admin. The runtime is Microsoft's Evergreen runtime (Windows 11, current Windows 10); this PR does not install it. Missing: exit code 3 and the Parts window uses the normal browser (picture 05).
- **Stored:** only what the user downloads, only the supplier's accepted kinds (STEP, DXF, ZIP, EDZ, catalog), as `<stamp>-<name>.part` renamed when complete, in `...\browser\landing\<id>\`, outside the library so .kcpl bundles and the repo never include it (test). Landed files older than 7 days are deleted at session start. Cookies stay in the profile folder; Forget sign-in deletes it. No password is read or stored; no script is injected (test over host.rs).
- **Downloads fallback scope** (review note): the 30-minute watch covers the whole Downloads folder, filtered to the supplier's kinds; the "?" text now says so, and Parts still reads each file before taking it.
- **Share bundles / personal-model rule (#1862):** NOT verified. #1862 is not in this branch's base, so I could not check that it covers imports made through `Waiting::Dropped`. Window downloads are not tagged as personal by this PR. Needs a look once #1862 is on main; the landing folder itself is never part of a bundle.
- **Licence terms:** KoperCAD fetches nothing and clicks through no terms; the user accepts each supplier's terms on its page (CADENAS portals forbid automated queries, so none are made).
- **Non-Windows unaffected:** wry and tao are `[target.'cfg(windows)'.dependencies]`; host.rs is `#[cfg(windows)]`. On Linux `launch` answers "no runtime" and the Parts window falls back (picture 05). Cargo.lock gains entries only.
