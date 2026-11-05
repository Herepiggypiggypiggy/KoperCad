# PR 1938 evidence (scrolling "?" popups)

Code head: fec08f150f80c3b753473a4f323ad62a819ebb7f (HEAD.txt). Report says "source at git fec08f150f". Pictures and logs (19 MB) are on the project mount: /mnt/project-files/qa/pr1938-fec08f15/ (not in the public repo). This commit only adds the three text files in this folder.

Main merged in (clean, no conflicts, no force push). The triangle chip popovers (#1937/#1945) use their own popover, not help_text, so they did not scroll. The problem-triangle popover now wraps its detail lines in the same scroll area (shared helper help_scroll in novo-theme), so a long detail stays inside the window too.

## Run
17 runs (12 scenario files, both sizes where the file is dual-size), 17 passed, 0 failed. One S4 non-blocking finding (F-001, 2 pairs of controls may overlap in 7430 at 1366x768; docks stack on purpose, same finding on clean main). check_layout is clean in every scenario that calls it.

| Scenario | Sizes | Result |
|---|---|---|
| 7392-help-popups-emc-bench | 1366, 1920 | pass |
| 724-top-bar-help | 1366, 1920 | pass |
| 9510-routes-count-chips | 1366, 1920 | pass |
| 9524 / 9525 bench estimate chip | 1366 / 1920 | pass |
| 9522 / 9523 job flag failed import | 1366 / 1920 | pass |
| 1814-3d-vents-air-in-equals-air-out | 1366, 1920 | pass |
| 7430-help-icons-3d-heat | 1366, 1920 | pass |
| 7433 / 7434 long popup scroll (top, scrolled, end) | 1366 / 1920 | pass |
| 7435-help-popup-short (new) | 1366 | pass |

## Short popup picture
shots/7435-help-popup-short-1366x768-1366x768-01-short-popup.png: the Heat strip "?" popup. It is three short lines, not one (I found no one-line "?" popup reachable without a 200 s heat run). It fills its frame: no empty strip below the text and no scrollbar.

## Baseline on clean main
7430 at 1366x768 on main 7f6aba3e passes (S4 overlap finding only). The only red seen on the PR head was step click_text Simulate #2 at 1366x768 in 7430 (two of four runs): the heat-up run started late on this slow machine and Cancel was still showing, so Simulate was not there yet. It fails before any Help step, so it is unrelated to the popup change. Fix: one more wait 60 and try click_text Cancel before the Settings step. Green at both sizes after that.

## Why scenario 7430 was rewritten earlier, and what I did about it
Earlier head 2f908bea changed 7430: it dropped the Help #3 steps, changed Help #1 to Help #2, and replaced the second Help click by a fixed point (click 317 343) with expect "Where air gets in". The commit message gives no reason, and I could not reproduce a need for it: on the merged head the original main steps (Help #1, #2, #3, close by click 700 690) pass at both sizes. The fixed point was also fragile: the Vents and fans window is not always at the left (in one run it sat at x 510 to 820), so click 317 343 missed its "?" and 7430 failed at 1366 with 'Where air gets in not on screen'. So the rewrite is reverted; 7430 now equals main plus the one wait. The Heat popup is no longer asserted in 7430; 7433/7434 cover it with texts only inside the popup.
