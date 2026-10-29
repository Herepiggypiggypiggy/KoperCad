# Evidence for the circuit library PRs

QA runs live in `/mnt/project-files/qa/pr-circuits<batch>-<sha8>/` (one folder per head, with `HEAD.txt`).
Pictures per circuit (picked in the Place window with the mounting view, then placed on the page) come from scenarios:

| Scenario | Circuit |
| --- | --- |
| 17100 | Mounting view in the Place window |
| 17102 | Chapter lines in the place palette |
| 17103 | Pendant station |
| 17104 | Bridge travel two motors on one drive |
| 17105 | Hoist load limit with latching relay |
| 17106 | Hoist drive with encoder speed feedback |
| 17107 | Pump feeder frequency controlled |
| 17108 | Pump level control with dry-run protection |

Run: `scripts/qa.sh --only 1710 --out /mnt/project-files/qa/pr-circuits<batch>-<sha8>`, then write the full head sha to `HEAD.txt`.
