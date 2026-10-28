# Display geometry: roadmap

Goal: no crash and no dead panels on rotated, mirrored or mixed-scale monitors (hardware test rc10: KC-01, KC-04, KC-11, KC-12, KC-13).

- [x] Window size sanity every frame (`crates/novo-app/src/display.rs`): surface side capped to the device limit, absurd aspect replaced, window fitted after a monitor or scale change.
- [x] wgpu errors logged instead of panicking (`install_error_guard`).
- [x] File dialogs owned by the main window on Windows (KC-11).
- [x] Repaint burst after a native dialog (KC-13).
- [ ] Confirm KC-04 on the real mirrored portrait monitor (needs the "Display:" line from Help > Copy problem report).
