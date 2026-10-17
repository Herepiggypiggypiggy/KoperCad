@echo off
rem Updates this folder to the newest build and starts it. No admin rights.
rem The branch is replaced by a single new commit on every build, so fetch
rem and reset (not pull) is the update that always works.
rem The window stays open when KoperCAD does not start, so the message can be read.
cd /d "%~dp0"
where git >nul 2>nul
if errorlevel 1 (
  echo git was not found; starting the build you already have.
) else (
  git fetch --depth 1 origin latest-build
  if errorlevel 1 (
    echo Could not reach GitHub; starting the build you already have.
  ) else (
    git reset --hard FETCH_HEAD
  )
)
if not exist "%~dp0kopercad.exe" (
  echo.
  echo kopercad.exe is missing from %~dp0
  echo Antivirus or SmartScreen may have removed it. Restore it, or run again after git reset.
  pause
  exit /b 1
)
echo Starting KoperCAD...
start "" /wait "%~dp0kopercad.exe"
set "KC_EXIT=%errorlevel%"
if not "%KC_EXIT%"=="0" (
  echo.
  echo KoperCAD stopped with exit code %KC_EXIT%.
  echo Log files: %LOCALAPPDATA%\KoperCAD\logs
  echo Please send the newest file in that folder, and this exit code.
  if exist "%LOCALAPPDATA%\KoperCAD\logs" explorer "%LOCALAPPDATA%\KoperCAD\logs"
  pause
  exit /b %KC_EXIT%
)
