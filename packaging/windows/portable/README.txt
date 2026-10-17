KoperCAD portable (beta)

Unzip this folder anywhere you can write to (for example your Documents folder)
and double-click run.cmd. No installation and no admin rights are needed.

Files
  kopercad.exe      the application
  kopercad-cli.exe  the command-line tool (run it from a terminal)
  run.cmd           starts kopercad.exe
  BUILD_INFO.txt    the version, the commit and the build time

Your projects, settings and parts live under your user profile
(%APPDATA%\KoperCAD and %LOCALAPPDATA%\KoperCAD), not in this folder, so you can
replace this folder with a newer one at any time.

Windows may show a SmartScreen warning the first time, because the program is
not code-signed yet: choose "More info", then "Run anyway".

Check the download against SHA256SUMS from the release page:
  Get-FileHash .\KoperCAD-<version>-portable.zip -Algorithm SHA256
