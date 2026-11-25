@echo off
rem Starts KoperCAD with the software renderer. Slower, but it works when the
rem graphics driver fails or the PC has no usable GPU (remote desktop, virtual machine).
cd /d "%~dp0"
if not exist "kopercad.exe" goto missing
start "" "kopercad.exe" --safe-graphics
exit /b 0
:missing
echo kopercad.exe is missing from this folder.
pause
exit /b 1
