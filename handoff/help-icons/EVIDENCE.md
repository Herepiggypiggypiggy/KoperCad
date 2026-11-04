# Evidence for the first help-icon PR (#1846)

Pictures: `/mnt/project-files/qa/pr-help-icons-<sha8>/` (report.md and HEAD.txt there name the exact head). Sizes 1366x768 and 1920x1080.

## What moved behind a "?", per window
| Window | Where the user finds it | Moved behind the "?" | Still visible |
|---|---|---|---|
| Heat strip (3D view, top) | "?" right of the word "Heat" | one paragraph per active layer (slice, shell, flow, cloud, heat flow, device bodies); placeholder sentence | colour keys, controls, "Probe" |
| Heat panel (3D, right) | "?" in the header, right of the fold caret | phase sentence, "What is this?", accuracy statement and limits, verdict caveat | status chip ("Heat: Coarse 3D · not validated"), verdict, "Model estimate" |
| Vents and fans | "?" in the header | the two legend lines, balance explanation, opening-flow note, fan-size note, how to use | opening list, numbers with "(model)", "Run in progress", "Result out of date" |
| Cooling what-if, Cooling advice | "?" in the header | the ranking note | MODEL, NOT MEASURED pill |
| Hoist run (Simulate > Simulate hoist run) | "?" in the header | what to press, model limits, assumptions | "Model, not measurement. Not a safety function.", hoist list, rows |
| Hoist in 3D | "?" in the header | travel-axis, end-limit and no-run notes, picture limits | "Picture, not measurement", swing numbers |
| Drive run (Simulate > Simulate drive run) | "?" in the header | what to press, motor fit assumptions, duty cycle intro, plot legend, model limits | "Model, not measurement. Not a safety function.", axes, rows |
| Status bar | none (text removed) | idle hints "Q or Space to place...", "Home zoom to fit" | coordinates, status chips |

## Checks run
fmt, clippy `-D warnings`, `cargo test -p novo-app -p novo-cli --lib` (2,200 passed before the last merge; 437 focused tests after it). Scenarios 7430, 7431, 7432 (new), 1599, 1730, 1814, 209, 110, 112, 160 from the PR head; 1599 also from clean main (report names the sha). QA `check_text` passes at a 40-word limit.

## Not tested / not done
- Floating modal forms and cards without a header have no "?" yet (text added there is dropped): the other areas.
- The "?" popup was only clicked in the scenarios above, not with keyboard navigation.
- Cooling selection result sentences and page names still run to 20 to 36 words (data, not explanation).
- Hover tooltips were left alone.
