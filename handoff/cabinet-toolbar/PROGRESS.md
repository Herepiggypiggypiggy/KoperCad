# Cabinet toolbar progress
- 2026-10-16: design note and mock merged (#621).
- 2026-10-16: build PR 1: grouped bar (Panel, Place, Arrange, Route, View, Output), Place all as the copper primary with Ctrl+Shift+P (`cabinet.place-all`, anchor `cabinet.place-all`), status strip with count pills and the placed reason, Heat controls in the strip, narrow fold into More. Not verified on screen (no screenshot harness in the cloud); needs a look on Windows.
- Next: lesson labels and anchors with the lesson-plan thread; Rails and ducts (#612) joins the Route group; PANEL-B3 surface picker joins Panel.
- Review fixes on #626: empty Route group skipped, joined Plate|Table switch, toast when Place all finds nothing to place, "Why here?" dropped from the note (the why stays per device in the inspector). Focus return after a pointer click on the bar is not implemented; check on Windows that R and Delete work right after Place all and Accept.
- 2026-10-16: merged main (#612 Rails and ducts, #630 homeless) into #626: Rails and ducts is in the Route group (More when narrow); Place all and the strip also count devices without a cabinet.
