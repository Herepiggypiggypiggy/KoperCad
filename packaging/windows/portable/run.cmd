@echo off
rem Starts KoperCAD from this folder. No installation and no admin rights needed.
rem The window stays open when KoperCAD does not start, so the message can be read.
rem No parenthesised blocks: a folder or user name containing ")" would break them.
cd /d "%~dp0"
if not exist "kopercad.exe" goto missing
start "" /wait "kopercad.exe"
set "KC_EXIT=%errorlevel%"
if not "%KC_EXIT%"=="0" goto failed
exit /b 0
:missing
echo kopercad.exe is missing from this folder.
echo Antivirus or SmartScreen may have removed it.
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
