# Release and builds: handoff

Thread: "Release and builds" (session cse_01UuGC6jT84yVrbcwGaSJ5Df). Owns .github/workflows/*,
packaging/windows/novoelectric.iss, scripts/run-latest.ps1, scripts/install-desktop-shortcut.ps1.

## Current state (2026-10-14 17:05 UTC)
- Branch claude/project-thread-4loohk, PR #41, head 618b36f. Sent to main reviewer
  (cse_016C9nTT8cN7voW8t1NQUG4W). Integration lead merges on READY without waiting for CI
  (workflow-only exception). #40 was closed in favour of #41.
- #41 does: experimental build drops only PRs that break `cargo check -p novo-app` (fallback
  path), main-only as last resort; no LTO, 16 codegen units for the experimental build;
  CI/installer cancel superseded PR runs; paths-ignore **/*.md and docs/** on CI and
  experimental; installer workflow only on packaging/icon/workflow changes, tags, dispatch;
  scripts/setup-new-pc.ps1 (one-paste setup on a new PC; copy also at launcher/setup-novoelectric-shortcut.ps1).

## Tadjiro's PC
- Shortcut %USERPROFILE%\Desktop\NovoElectric (latest).lnk -> run-latest.ps1 copy in
  %LOCALAPPDATA%\NovoElectric\experimental. gh installed per user and signed in.
- The owner's Remote Control session (id on the project mount) takes only Tadjiro's own typed messages.
- 60 fps GPU check pending: Tadjiro types "run the GPU speed check on my PC" in the
  coordination thread; RC runs `canvas-demo.exe --stress 10000 --seconds 10`; tester fills Gate 0.
- Never touch the owner's existing EPLAN project folder (path on the project mount only).

## Rules and traps
- No admin rights ever: per-user, HKCU, %LOCALAPPDATA%, PrivilegesRequired=lowest.
- Private repo, free plan: no branch protection, limited runners, Windows minutes count 2x.
  Keep CI load low. Experimental concurrency group keeps only the newest pending run.
- Publish job runs on ubuntu with the write token and runs no PR code; keep it that way.
- gh GraphQL is blocked (403) here; use REST (`gh api`) or the GitHub MCP tools.

- GPU check PASSED (P1000: 10k 1135 fps avg; 100k 113 avg, 38.5 1% low). Tester and canvas thread told.

## Next
1. Drive #41 to merge; then confirm the next experimental run publishes and BUILD_INFO lists PRs.
2. Drop canvas-demo from the experimental build (GPU check done) unless canvas thread wants it for the 100k stutter.
3. Launcher: show which build is running (BUILD_INFO) without a console window.
