# Handoff: Place button QA scenarios

Branch `claude/project-thread-q6odn3`, one PR from main (touches only `qa/scenarios/*.qa` and this folder).

Cause: before V3a (#1683) the tool dock had its own Place button, so the macro panel's button was match #2. The dock is gone, so only one Place Button exists and `#2` finds nothing. Fix: plain `click_text Place Button` in 16 scenarios.
Follow-on stale steps, once Place worked: 790/790b used `click_text Corners` (now inside the Connect flyout): use `key w`, and click a real pin (F1 top, 718,258 at 1366x768; 1007,441 at 1920x1080). 794/794b double-clicked a spot that no longer holds a tag (the circuit now lands at the pointer): now the -F1 tag (703,274; 989,465).
Evidence: /mnt/project-files/qa/place-button-fix/ (HEAD.txt, final/). Run: `scripts/qa.sh run <files> --out <dir>`.
Trap: `#2` on "Next" and "Simulate" buttons in scenarios 204, 212, 215 is a different case, not touched.
