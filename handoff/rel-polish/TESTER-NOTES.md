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
