# ui-outline-readonly roadmap

Scope: two gaps from the drawing UI coverage audit.

Done
- Outline: Wires (by page) and Cables sections under the page tree; a click opens the page and selects the wire (a cable goes to its first wired core). The page filter also filters them.
- File > Open read-only: opens without lock or recovery file, never writes, autosave off, edits taken back, "(read-only)" in the title, "Read-only" in the status bar, Save disabled, Save as makes an editable copy.

Next
- Read-only still lets a change flash for one frame before it is taken back (edit paths do not check it up front). A per-command guard would be cleaner.
- Cable click selects the first core's wire only; selecting every core wire needs a multi-item pending selection.
- Wire rows are grouped by page only; grouping by net or cabinet is open.
