@echo off
rem Starts KoperCAD from this folder. No installation and no admin rights needed.
rem The window stays open when KoperCAD does not start, so the message can be read.
if not exist "%~dp0kopercad.exe" (
  echo kopercad.exe is missing from %~dp0
  echo Antivirus or SmartScreen may have removed it.
  pause
  exit /b 1
)
start "" /wait "%~dp0kopercad.exe"
set "KC_EXIT=%errorlevel%"
if not "%KC_EXIT%"=="0" (
  echo.
  echo KoperCAD stopped with exit code %KC_EXIT%.
  echo Log files: %LOCALAPPDATA%\KoperCAD\logs
  echo Please send the newest file in that folder, and this exit code.
  pause
  exit /b %KC_EXIT%
)
