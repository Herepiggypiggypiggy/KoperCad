# Notes for the tester README (REL-DOCS)

## KoperCAD does not start, or is very slow (graphics)

KoperCAD picks a graphics adapter itself: a real GPU first, then a virtual one
(remote desktop, virtual machine), then software rendering. Software rendering
works but is slow. The adapters found are written to the log.

Testers can steer the choice with environment variables (set per user, no admin
rights needed: `setx NAME value`, then start KoperCAD again):

- `WGPU_ADAPTER_NAME`: part of an adapter's name, any case, for example
  `intel` or `basic render`. Matching adapters are tried first.
- `WGPU_POWER_PREF=low`: prefer the integrated GPU over the discrete one.
- `WGPU_BACKEND`: `dx12`, `vulkan` or `gl` restricts the graphics API.

The log line `Using graphics adapter ...` now ends with the reason: named by
`WGPU_ADAPTER_NAME`, `WGPU_POWER_PREF` asks for low power, or best ranked.

## Window title bar colour (Windows)

KoperCAD asks Windows for a dark or light title bar to match the theme (egui's
`sync_window_theme` sends it on the first frame and when the theme changes; winit
applies it through the dark-mode window attribute). Windows 10 before version 1809
ignores the request and keeps a white bar. On Windows 10 1809 or newer and Windows 11
the bar should follow the theme (Preferences, Theme). If it stays white on such a PC,
report the Windows version and the Theme setting: it is a bug, not a limit. Not
verified on a Windows PC from the cloud.

## Running with a clean profile (Windows)

Start KoperCAD with `--profile-dir`, or set `KOPERCAD_PROFILE_DIR` for the run:

    kopercad.exe --profile-dir C:\Users\you\kopercad-test

Every per-user folder the app uses (settings, parts catalogue, recovery files, unsaved new
projects, lesson progress, logs, and the folder where new projects are saved) then sits under
that folder, and nothing is written to the real profile. A missing or empty folder is a first
run; delete it to start over. No admin rights. Details: `handoff/rel-polish/PROFILE-DIR.md`.
Setting `APPDATA` alone does not work on Windows, so use the flag. The Downloads folder
(parts import watch) is not moved.

## Start screen

The UNSAVED list shows the four newest unsaved projects; the others sit under a
folded "N older" line. The card is a fixed 320 pt column, centred, so on a wide
screen it should sit in the middle; a card that fills most of a 2560 px window was not
reproduced on main (the old build tested was not main).
