@echo off
rem Updates this folder to the newest build and starts it. No admin rights.
rem The branch is replaced by a single new commit on every build, so fetch
rem and reset (not pull) is the update that always works.
cd /d "%~dp0"
git fetch --depth 1 origin latest-build
if errorlevel 1 (
  echo Could not reach GitHub; starting the build you already have.
) else (
  git reset --hard FETCH_HEAD
)
start "" "%~dp0kopercad.exe"
