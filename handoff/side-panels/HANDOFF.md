# Side panels: handoff

- New theme widgets: `collapsing_section` (caret, caption caps title, summary, remembered open state) and `paint_text_ellipsis` in `novo-theme/src/widgets.rs`.
- Inspector (`app/inspector.rs`): header (tag, pin icon), description, behaviour fields (time, other half), Part, Relations, Calculation inputs, All properties. Sections fold; empty ones are hidden. Search box only appears with more than 6 properties.
- Outline (`app.rs::outline`, `app/outline.rs`): underlined tabs, New + Groups/Cabinets segmented, "..." beside the filter, titles end in an ellipsis.
- Issues and Commissioning panels restyled; anchors and note ids unchanged.
- Verify with `scripts/screenshot.sh` or an Xvfb run at 1366x768 and 1920x1080.
