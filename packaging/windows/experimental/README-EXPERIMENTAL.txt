KoperCAD experimental build
===========================

This is the rolling "experimental" build: main plus the open pull requests,
rebuilt about every two hours and not verified. BUILD_INFO.txt lists what is in
this one. No installation and no admin rights are needed.

Start it
  run.cmd                  starts KoperCAD. If it stops, the window stays open
                           and shows the exit code and the log folder.
  run-safe-graphics.cmd    starts KoperCAD with the software renderer. Slower,
                           but it works when the graphics driver crashes the
                           program (no window appears, no message).
  kopercad.exe             the program itself.
  kopercad-cli.exe         the command-line tool.

If no window appears
  1. Start run-safe-graphics.cmd.
  2. KoperCAD notices a start that never drew its first frame. The next time you
     start it asks whether to use safe graphics, and it tries the graphics
     adapter that stopped the start last from then on.
  3. Help > Copy problem report (or the newest file in the log folder) says which
     graphics adapter was used. The log folder is
     %LOCALAPPDATA%\KoperCAD\logs
     Send that file with the exit code, if there was one.
  Forcing a graphics path by hand also works: set WGPU_BACKEND=dx12 (or vulkan)
  before starting.

Where the data is
  Settings, logs, recovery copies and projects are in your user profile
  (%LOCALAPPDATA%\KoperCAD, %APPDATA%\KoperCAD, Documents\KoperCAD), not in this
  folder, so replacing this folder with a newer build keeps everything.

More
  TESTER-README.md has the longer guide (first ten minutes, what to report).
  The 74 MB canvas stress demo, for the GPU check on real hardware, is a
  separate file on the same release page: canvas-demo.exe.
