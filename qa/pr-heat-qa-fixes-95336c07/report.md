# QA evidence, source at git 95336c07abb827a6337f005974664b5d38ac597b

Scenarios are the three from #1887 (taken from its branch, unchanged), run at 1366x768 on the head above (software Vulkan, Xvfb). Pictures: /mnt/project-files/qa/pr-heat-qa-fixes-95336c07/.

| Scenario | Before (main b3f9747) | After (this head) |
| --- | --- | --- |
| 16176 heat refuses 20 m enclosure | notice cleared every frame | pass |
| 230 workpack out of date after cooler layout | Apply the moves button cut off by the docked window, stale line never shown | pass |
| 380 cooling advice 2D vs 3D | Add button and footer cut off in the docked window, Added never happens | 1366: failed at `click_text Quick` in the stock run (the automatic run restarted before the click); 1920: `Model comparison` not yet on screen because the Quick 3D run needs more than the stock `wait_s 120` on this machine. With `wait_s 400` it passes at both sizes (run not committed). |

380 needs the longer wait or a faster machine; the window itself now reaches the Add button and shows the comparison block.
