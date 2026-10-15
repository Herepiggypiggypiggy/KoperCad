# Building without GitHub Actions minutes

GitHub Actions bills Windows minutes at double rate, and the free allowance
(2000 a month) ran out. None of the work Actions does needs a GitHub machine.
Three ways to do it locally, all without admin rights. **Use option 2 for the
desktop shortcut and option 1 for checks.** Option 3 is not recommended.

## 1. Cross-build from Linux (Claude sessions, any Linux box)

`scripts/cross-build-windows.sh check` runs clippy with warnings as errors for
the Windows target. `scripts/cross-build-windows.sh zip DIR` makes a release
zip. It needs `gcc-mingw-w64-x86-64` and the `x86_64-pc-windows-gnu` target.

- Works today: a full release build takes about 4 minutes on a cold cache.
- Main caveat: it is compiled and linked, never run, on Windows. The result
  imports only system DLLs and asks for no elevation, but "starts without
  crashing" needs a human launch. It is also the GNU build, not the MSVC one
  Actions makes.
- It cannot update the shortcut: the zip lands in the project files, and the
  launcher downloads from the GitHub release, which a Linux session cannot
  write to without a token.

## 2. Build on Tadjiro's PC (recommended)

`scripts/build-local.ps1` downloads the source of a branch with `gh`, builds it
and puts it where the "NovoElectric (latest)" shortcut starts from. The
installer script adds a second desktop shortcut, "NovoElectric (build here)",
that runs it and starts the result.

- **Admin rights: not needed with the GNU toolchain.** `rustup-init` installs
  under the user profile, and WinLibs (portable MinGW-w64) is a zip. The
  usual route, MSVC, needs the Visual Studio Build Tools, whose installer asks
  for elevation. Getting the MSVC libraries without admin is possible (xwin)
  but needs more parts (clang-cl for the SQLite C code), so this script does
  not.
- First run downloads about 550 MB and compiles every dependency (roughly
  15 to 30 minutes on a workstation); later runs rebuild only what changed.
- `run-latest.ps1` now replaces the local copy only with a release built
  *after* it, so a stale release does not undo a local build.
- Main caveats:
  - It builds one ref (main by default, or `-Ref pull/175/head`). Main plus
    all open PRs is what the Actions build adds; that merge is not done here.
  - It uses the GNU target, and this script and that build have not been run
    on Windows yet. Treat the first run as a test.
  - It runs only when he clicks it.

## 3. Self-hosted GitHub runner on his PC (not recommended)

Registering the PC as a runner (`config.cmd`, then `run.cmd`) needs no admin if
it runs in a console instead of as a service, and self-hosted jobs have not
used the Actions minutes. I could not confirm that the current billing block
spares them, nor whether GitHub still plans a per-minute fee for self-hosted
runners; check the billing page.

- Main caveat: every pull request's build scripts and tests would run on his
  PC as his user, with read access to everything in his profile, including
  the EPLAN folder that must never be touched. A separate Windows user would
  isolate that but needs admin to create. It also only works while he is
  logged in with the PC on.
