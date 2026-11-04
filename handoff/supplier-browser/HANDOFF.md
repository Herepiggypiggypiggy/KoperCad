# Supplier browser: handoff (2026-11-04)

Branch `claude/project-thread-k9oxaa`, one PR (see PROGRESS.md for number and head).

## How it works
- Menu "Get parts from supplier" (Parts window toolbar, left of Import table). A selected part's order number is used for suppliers with a search address.
- Starts the same exe as `--supplier-browser <id>`: `crates/novo-app/src/supplier_browser/{mod,host}.rs`. host.rs is Windows only (wry 0.57 + tao 0.37, WebView2). Exit codes: 3 no runtime, 4 other failure. The Parts window sees an early non-zero exit and opens the normal browser, watching Downloads (only the supplier's file kinds).
- Downloads are written by the window to `<profile>\KoperCAD\browser\landing\<id>\<stamp>-<name>.part` and renamed when complete. `LandingWatcher` (novo-parts) gives finished files to `parts_suppliers.rs`, which feeds them to `PartsWindow::wait_for(Waiting::Dropped)`, the same pipeline as a dropped file (matcher reads it first).
- Sign-in cookies: WebView2 user data folder `...\browser\profile\<id>\`. "Forget sign-in" deletes it. KoperCAD never reads or stores passwords.
- Rules, enforced by test `the_host_never_scripts_a_page`: no evaluate_script, initialization script, IPC handler, custom protocol, user agent change. The user drives the browser (CADENAS and others forbid automated queries).
- Only http/https pages open; pop-ups open in the same window; downloads of other kinds are refused (setup.exe etc.).
- Data: built-in list in `supplier_site::builtin()`; the user adds/replaces/hides entries in `%LOCALAPPDATA%\KoperCAD\browser\suppliers.json` (array of Supplier objects, `"hidden": true` removes).
- Not in the repo or any `.kcpl` bundle: folders are outside the library root (test).

## Page buttons
`PartsWindow::open_page(ctx, url, maker, order)` is the one place that opens a maker page: known supplier by host, else by maker name, else the normal browser. The Rittal build thread's button only needs to call it, or open a `rittal.partcommunity.com` address through the existing `OpenPage`/`Import` actions.

## Verification state
- Linux: all novo-parts and parts tests pass; clippy clean.
- Windows: `cargo check`/`clippy` for `x86_64-pc-windows-gnu` pass (host.rs type-checks). NOT run on Windows. Never opened a real WebView2 window. Supplier addresses other than WAGO are unconfirmed (marked `verified: false`).
- Cargo.lock grew (wry/tao pull gtk/webkit entries for Linux; Windows-only dependency, not built on Linux).

## Traps
- wry download handler: `path` starts as the default Downloads path + suggested name; we replace it. Unverified that every portal uses a real file name (blob/POST downloads).
- Closing the window ends nothing in Parts; the strip stays 30 min or until Stop.
