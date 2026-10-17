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

Setting `APPDATA` does not move the profile on Windows: KoperCAD (like most apps)
asks Windows for the Roaming, Local and Documents folders, so a test launch with a
changed `APPDATA` still rewrites the real `app.ron`. There is no `--profile-dir` flag
yet (it would touch about fourteen folder lookups in four crates; a follow-up, not a
small change). Until then:

- Use a second Windows user for tests; or
- Back up and restore these before and after a test: `%APPDATA%\KoperCAD` (settings,
  `app.ron`, parts catalog, templates), `%LOCALAPPDATA%\KoperCAD` (recovery files,
  mesh cache) and `%LOCALAPPDATA%\NovoElectric` (unsaved new projects). Rename the
  folders to start clean, rename them back afterwards.

`LOCALAPPDATA` is the one variable the app does read for unsaved projects and recovery
files, so changing only that isolates those two.

## Start screen

The UNSAVED list shows the four newest unsaved projects; the others sit under a
folded "N older" line. The card is a fixed 320 pt column, centred, so on a wide
screen it should sit in the middle; a card that fills most of a 2560 px window was not
reproduced on main (the old build tested was not main).
