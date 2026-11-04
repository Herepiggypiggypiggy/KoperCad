# PR 1864 evidence

Source at git 6d0d955bf755f5f4ca8df5c5472e225376953150 (HEAD.txt; this folder is the only change after it). Scenario 18640, 1366x768 and 1920x1080 with check_layout, Linux, binary built from that head (report.md: "source at git 6d0d955bf7"). Pictures: /mnt/project-files/qa/pr1864-6d0d955b/ (not committed).

Where the user finds it: Parts window, row under the toolbar: **Get parts from supplier** (suppliers, Downloads folder, Forget sign-in, Use my normal browser) with a "?" beside it. The Import, Get CAD from TraceParts and Open page again buttons route known suppliers into the same window.

Pictures: 01 supplier-menu, 02 submenu-downloads-folder, 03 submenu-forget-sign-in, 04 help-text, 05 fallback-no-webview (NoRuntime note and strip, Files taken: 0, Stop), 06 strip-closed (Stop pressed: strip and orange note gone; `expect_not "WebView2"` passes). The one S4 finding at 1920 (F-001) is the floating Parts window sitting over the top bar, as accepted on the earlier heads.

## NOT tested
- A real WebView2 window: nothing was opened, signed in to or downloaded on Windows. host.rs is type-checked and clippy-checked for x86_64-pc-windows-gnu only.
- Files taken above 0 is covered by unit tests (`parts_suppliers`), not by a picture.
- Supplier addresses other than WAGO are unconfirmed (`verified: false`). Pop-up SSO log-ins, blob downloads, cookies across a restart untested.

## Answers carried over
- URL rules: http/https only, no user:pass@, plus `about:blank` and `blob:http...`; pop-ups open in the same window.
- WebView2 user-data folder: `%LOCALAPPDATA%\KoperCAD\browser\profile\<id>\` (portable installs: beside the program). No registry, HKLM or admin; the runtime is Microsoft's Evergreen runtime, not installed by this PR; missing means exit code 3 and the normal-browser fallback (picture 05).
- Stored: only what the user downloads, only the supplier's kinds, `.part` renamed when complete, in `...\browser\landing\<id>\`, outside the library; 7-day purge; Forget sign-in deletes the profile; no password read, no script injected (test over host.rs).
- Personal rule: a file imported from a landing folder is recorded as `supplier-window:<id>`, matched by `is_personal_model` independent of the supplier list (custom, edited, removed suppliers). Test `a_file_landed_from_a_supplier_window_stays_out_of_a_share_bundle` uses an id in no list.
- Downloads fallback: watches the whole Downloads folder for 30 minutes, filtered to the supplier's kinds; said in the "?".
- Non-Windows unaffected: wry and tao are `cfg(windows)` dependencies; host.rs is `#[cfg(windows)]`.
