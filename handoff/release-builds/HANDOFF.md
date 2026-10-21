# Release and builds: handoff

Thread: "Release and builds" (session cse_01UuGC6jT84yVrbcwGaSJ5Df). Owns .github/workflows/*,
packaging/windows/novoelectric.iss, scripts/run-latest.ps1, scripts/install-desktop-shortcut.ps1,
scripts/setup-new-pc.ps1, scripts/build-local.ps1, scripts/cross-build-windows.sh, docs/LOCAL-BUILDS.md.

## v0.1.0-rc1 (2026-10-19 12:10 UTC)
- Tag v0.1.0-rc1 is on main 0ada7e7e (full CI Linux + Windows green). Draft pre-release "KoperCAD 0.1.0-rc1"
  holds installer, portable zip, SHA256SUMS, BUILD_INFO. Stays a DRAFT until Tadjiro says publish in his own words.
- Cloud sessions get HTTP 403 on tag pushes. Use the manually started workflow "Tag a release"
  (`tag-release.yml`, inputs tag + sha, #1033): it tags a sha that is on main, then starts
  `package-windows.yml` on the tag (tags made with the workflow token do not trigger workflows).
  The sha must be #1033's merge commit or later (the release job condition is read at the tagged commit).
- Windows CI: novo-cfd is skipped there (Linux shards run it), data files and .rs are LF (.gitattributes),
  2 wire-colour GPU tests skip on the WARP software adapter. Not verified on real Windows hardware:
  OpenGL laptop start-up (#1025), run.cmd stale-build guard (#1027), crane import, 3D heat.
- latest-build carries 0ada7e7e (experimental run 631).
- Follow-ups: guard step in tag-release.yml that the tagged sha has the dispatch condition; cheap novo-cfd
  smoke on Windows; per-test timeout on Windows.

## Earlier state (2026-10-15 19:50 UTC)
- Repo is public and renamed `Herepiggypiggypiggy/kopercad` (old slug redirects). Product KoperCAD,
  maker KoperVonk. Free Actions minutes.
- Standing order from the lead: do NOT dispatch an experimental build unless Tadjiro asks. Last
  build: 5 (run 600, main df249b4, 2026-10-15 19:27Z), asked for by Tadjiro, delivered on the
  `latest-build` branch (below). Release hold from the lead: no build with the STEP import button
  (#508) until #506 is merged; #511 makes the experimental build skip drafts and PRs labelled
  `no-experimental` (check whether it merged).
- Rename leftovers done: user-visible names (#201, #236), README line and WinLibs checksum check
  (#285), settings and parts catalog moved to %APPDATA%\KoperCAD with a migration, Documents folder
  rule (#302), program renamed to kopercad.exe (#304).
- Deliberately NOT renamed: `%LOCALAPPDATA%\NovoElectric` (launcher builds live there; the app's
  autosaves too), the release asset `novoelectric-experimental-windows.zip`, the `novo-*` crates, the
  `novoelectric` Python module (WS-28's call).
- Compatibility copy: the experimental zip and build-local also carry `novoelectric.exe`, because
  Tadjiro's installed run-latest.ps1 starts that name. Remove it once he has re-run
  install-desktop-shortcut.ps1 (new run-latest accepts either name).
- Stray remote branch `claude/local-builds` stays (delete was denied; content is on main).

## Tadjiro's PC
- Shortcuts "KoperCAD (latest)" and "KoperCAD (build here)" on the Desktop -> copies of
  run-latest.ps1 / build-local.ps1 in %LOCALAPPDATA%\NovoElectric\experimental. Those copies are only
  refreshed by running scripts\install-desktop-shortcut.ps1.
- Never touch the owner's existing EPLAN project folder (path on the project mount only).
- Not verified on his PC: build-local.ps1 end to end, the installer upgrade over an old install, the
  new run-latest.

## Rules and traps
- No admin rights ever: per-user, HKCU, %LOCALAPPDATA%, PrivilegesRequired=lowest.
- Main CI runs are not cancelled (only PR runs are); experimental concurrency group keeps the newest
  pending run. The publish job runs on ubuntu with the write token and runs no PR code.
- gh GraphQL is blocked (403) here; use REST or the GitHub MCP tools. Force-push is blocked by a repo
  rule; add a commit instead. The sandbox gets 403 from github.com release downloads.
- One designated branch per thread: wait for a PR to merge before reusing the branch.

## Next
1. Wait for Tadjiro to ask for a build (via coordinator or lead); then dispatch experimental.yml once.
   A run dispatched seconds before a workflow PR merges uses the old workflow (happened with run 599).
2. Hands-on checks on his PC (shortcut, build-local, installer upgrade).
3. Drop the novoelectric.exe copy later; consider pinning a WinLibs release and hash.

## latest-build branch (2026-10-15, Tadjiro's request)

Tadjiro runs builds from a laptop and wants a file that updates or vanishes with git. The
`publish` job in `experimental.yml` now also force-pushes an orphan branch `latest-build` (one
commit: `kopercad.exe`, `BUILD_INFO.txt`, `run.cmd`, `README.md`; sources in `scripts/latest-build/`).
Skipped with a log line if the exe is over 95 MB. Clone once, then run `run.cmd` (fetch, reset, start):

    git clone --branch latest-build --single-branch --depth 1 https://github.com/Herepiggypiggypiggy/KoperCad.git KoperCAD-latest

Delete the branch to remove the build. The release zip is still published. Verified: run 600 created the branch with the four files. Not yet started on his laptop.
