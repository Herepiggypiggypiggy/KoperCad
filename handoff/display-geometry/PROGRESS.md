# Display geometry: progress

- 2026-10-28: display.rs, error guard, dialog owner, repaint after dialog, QA scenario 640-window-absurd-sizes, unit tests. Awaiting hardware check on the mirrored portrait monitor.
- 2026-10-28 (follow-up to #1568, reviewer points): SAFETY comment states rfd copies the handle and a stale handle is an OS error; graphics errors are counted, de-duplicated in the log (repeat at most every 10 s) and shown in the problem report; the 6:1 aspect rule no longer resets a maximized or full screen window (surface cap still applies).
