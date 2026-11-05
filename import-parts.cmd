@echo off
rem Imports real part data into KoperCAD in one go. No admin rights.
rem 1. Updates this folder to the newest build (same as run.cmd).
rem 2. Creates the inbox folder with one subfolder per maker.
rem 3. Reads every catalog file you dropped there, checks it, and adds the
rem    parts to your parts catalog. Run it again any time: it only adds or updates.
rem Add --dry-run to read and report without changing anything.
setlocal
cd /d "%~dp0"
if exist ".git" (
  git fetch --depth 1 origin latest-build
  if not errorlevel 1 git reset --hard FETCH_HEAD
)
set "CLI=%~dp0kopercad-cli.exe"
if not exist "%CLI%" set "CLI=%LOCALAPPDATA%\NovoElectric\experimental\current\kopercad-cli.exe"
if not exist "%CLI%" (
  echo kopercad-cli.exe was not found. It ships with builds made after the parts import was added;
  echo run run.cmd once to update, then run this again.
  pause
  exit /b 1
)
set "INBOX="
for /f "tokens=1,* delims= " %%a in ('"%CLI%" parts prepare') do if "%%a"=="Inbox:" set "INBOX=%%b"
echo.
"%CLI%" parts import %*
set "RC=%ERRORLEVEL%"
echo.
if "%RC%"=="1" (
  echo Opening the inbox. Put the downloaded files in the maker's folder, then run import-parts.cmd again.
  echo "%CLI%" parts guide   lists what to download from whom.
  if defined INBOX start "" explorer "%INBOX%"
)
pause
exit /b %RC%
