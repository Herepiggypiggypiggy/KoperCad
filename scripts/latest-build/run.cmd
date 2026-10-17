@echo off
rem Updates this folder to the newest build and starts it. No admin rights.
rem The branch is replaced by a single new commit on every build, so fetch
rem and reset (not pull) is the update that always works.
rem The window stays open when KoperCAD does not start, so the message can be read.
rem No parenthesised blocks: a folder or user name containing ")" would break them.
cd /d "%~dp0"
tasklist /fi "imagename eq kopercad.exe" 2>nul | find /i "kopercad.exe" >nul
if not errorlevel 1 goto running
where git >nul 2>nul
if errorlevel 1 goto nogit
git fetch --depth 1 origin latest-build
if errorlevel 1 goto nofetch
git reset --hard FETCH_HEAD
if errorlevel 1 goto noreset
goto info
:running
echo.
echo KoperCAD is already running, possibly an OLD version that was started earlier.
echo Close all KoperCAD windows, then run run.cmd again.
pause
exit /b 1
:nogit
echo.
echo WARNING: git was not found, so this folder was NOT updated. Starting the build you already have.
goto info
:nofetch
echo.
echo WARNING: could not reach GitHub, so this folder was NOT updated. Starting the build you already have.
goto info
:noreset
echo.
echo WARNING: the files could not be replaced, so you may be starting an OLD version.
echo Close KoperCAD and any antivirus scan of this folder, then run run.cmd again.
pause
exit /b 1
:info
echo.
git log -1 --format="Build on disk: %%s" 2>nul
if exist "BUILD_INFO.txt" set /p KC_INFO=<"BUILD_INFO.txt"
if exist "BUILD_INFO.txt" echo %KC_INFO%
if exist "kopercad.exe" for %%F in ("kopercad.exe") do echo kopercad.exe file date: %%~tF
echo Builds are republished about every two hours, so the newest build can be older than main.
echo The same build time is in Help, About and in the first line of the log.
echo.
if not exist "kopercad.exe" goto missing
echo Starting KoperCAD from %CD% ...
start "" /wait ".\kopercad.exe"
set "KC_EXIT=%errorlevel%"
if not "%KC_EXIT%"=="0" goto failed
exit /b 0
:missing
echo.
echo kopercad.exe is missing from this folder.
echo Antivirus or SmartScreen may have removed it. Restore it, or run again after git reset.
pause
exit /b 1
:failed
echo.
echo KoperCAD stopped with exit code %KC_EXIT%.
echo Log files: %LOCALAPPDATA%\KoperCAD\logs
echo Please send the newest file in that folder, and this exit code.
if exist "%LOCALAPPDATA%\KoperCAD\logs" explorer "%LOCALAPPDATA%\KoperCAD\logs"
pause
exit /b %KC_EXIT%
