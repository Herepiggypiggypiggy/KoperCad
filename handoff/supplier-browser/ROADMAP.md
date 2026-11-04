# Supplier browser: roadmap

Scope: let the user get part files from supplier sites that cannot be imported directly (Rittal PARTcommunity, TraceParts, WAGO, Siemens, Phoenix, Schneider, ABB, EPLAN Data Portal) inside KoperCAD: the site opens in its own window, the user signs in and downloads, the file lands in Parts without a trip through Explorer.

## Done (PR from branch claude/project-thread-k9oxaa)
- `novo-parts::supplier_site`: supplier list as data (+ user `suppliers.json`), private profile and landing folders, file-kind and file-name rules, `LandingWatcher`, forget sign-in, purge.
- `novo-app::supplier_browser`: separate window process (`kopercad --supplier-browser <id> [--order N]`), wry + tao (WebView2), Windows only; other systems report "no runtime".
- Parts window: "Get parts from supplier" menu, strip while open, existing page buttons (Import, Get CAD from TraceParts, Open page again) route known suppliers into the window, fallback to the normal browser plus a Downloads watch.

## Next
1. Real-hardware pass on Windows: WebView2 present/missing, Rittal/WAGO/TraceParts download end to end, check download_started path handling, blob downloads, pop-up login flows (SSO), per-supplier cookies surviving a restart.
2. Confirm supplier addresses (only WAGO is verified). Fill `verified` and search URLs (Siemens, Rittal, Phoenix) from his captures.
3. Address bar / back / home buttons in the window (today: title bar shows page, host; WebView2 keys Alt+Left, F5 work). Needs a native toolbar or a second webview; no page script allowed.
4. Missing runtime: link to Microsoft's per-user Evergreen installer from the note (not done: unverified that it installs without admin).
5. Disable browser autofill / password manager in WebView2 (wry additional args) once verified on Windows.
6. Landed-file housekeeping beyond the 7-day purge (delete after Parts has taken it).

## Open questions
- Is the WebView2 Evergreen runtime installed on his work PC (Windows 10/11 normally yes)?
- Which suppliers does he actually use most; order of the menu.
