# KoperCAD latest build

This branch holds only the newest experimental Windows build of KoperCAD
(unverified: main plus the open pull requests). It is replaced by a single new
commit on every build, so it never grows. Deleting the branch removes it.

Get it once (no admin rights; Git for Windows installs per user):

    git clone --branch latest-build --single-branch --depth 1 https://github.com/Herepiggypiggypiggy/KoperCad.git KoperCAD-latest

Run and update: double-click `run.cmd` in that folder. It fetches the newest
build, resets to it and starts `kopercad.exe`. `BUILD_INFO.txt` lists exactly
what is inside. Your projects live under your user profile, not here.
