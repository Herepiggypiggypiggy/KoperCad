# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 48 run(s): 48 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target2/debug/kopercad`, built 1793780314 (unix time), source at git 54da4ccc75.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Controller panel: a virtual radio drives the receiver of a two-speed hoist (1581-controller-live-radio-hoist) | Live | 1366x768 | pass | - | 5 |
| Controller panel: a virtual radio drives the receiver of a two-speed hoist (1920) (1581b-controller-live-radio-hoist-1920) | Live | 1920x1080 | pass | - | 5 |
| Controller panel and Hoist run: the radio lifts the hook through the drawn circuit (1583-controller-hoist-run) | Live | 1366x768 | pass | - | 4 |
| Controller panel and Hoist run: the radio lifts the hook through the drawn circuit (1920) (1583b-controller-hoist-run-1920) | Live | 1920x1080 | pass | - | 4 |
| Drive hoist run: the drive opens the brake through the drawn relay, the hook rises and the run reports the sensed load (1709-drive-hoist-run) | Simulate | 1366x768 | pass | - | 3 |
| Drive hoist run at 1920: the drive opens the brake through the drawn relay, the hook rises and the run reports the sensed load (1709b-drive-hoist-run-1920) | Simulate | 1920x1080 | pass | - | 3 |
| Circuit library: the pendant station circuit is found and placed (17103-pendant-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the pendant station circuit is found and placed (17103b-pendant-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the bridge travel two-motor circuit is found and placed (17104-bridge-two-motors-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the bridge travel two-motor circuit is found and placed (17104b-bridge-two-motors-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the hoist load limit circuit is found and placed (17105-hoist-load-limit-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the hoist load limit circuit is found and placed (17105b-hoist-load-limit-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the hoist encoder circuit is found and placed (17106-hoist-encoder-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the hoist encoder circuit is found and placed (17106b-hoist-encoder-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the pump VFD feeder circuit is found and placed (17107-pump-feeder-vfd-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the pump VFD feeder circuit is found and placed (17107b-pump-feeder-vfd-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the pump level control circuit is found and placed (17108-pump-level-control-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the pump level control circuit is found and placed (17108b-pump-level-control-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the crane supply surge circuit is found and placed (17109-crane-supply-surge-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the crane supply surge circuit is found and placed (17109b-crane-supply-surge-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Circuit library: the pump alternation circuit is found and placed (17110-pump-alternation-circuit-places) | Schematic | 1366x768 | pass | - | 2 |
| Circuit library: the pump alternation circuit is found and placed (17110b-pump-alternation-circuit-places-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Controller machine ports: the radio drives the hoist through bound ports (17300-controller-machine-ports) | Live | 1366x768 | pass | - | 3 |
| Controller machine ports: the radio drives the hoist through bound ports (1920) (17300b-controller-machine-ports-1920) | Live | 1920x1080 | pass | - | 3 |
| Closed-loop winch run: torque is proven, the drawn brake contactor opens the brake and the hook rises (1735-closed-winch-run) | Simulate | 1366x768 | pass | - | 4 |
| Closed-loop winch run: torque is proven, the drawn brake contactor opens the brake and the hook rises (1735b-closed-winch-run-1920) | Simulate | 1920x1080 | pass | - | 4 |
| Two closed-loop winches on one beam: the first winch's radio lifts both through their drawn brake contactors (1738-closed-winch-pair-run-1920) | Simulate | 1920x1080 | pass | - | 4 |
| Two closed-loop winches on one beam: the first winch's radio lifts both through their drawn brake contactors (1738a-closed-winch-pair-run) | Simulate | 1366x768 | pass | - | 4 |
| Drive hoist slow-down switch: the hook slows before the final upper limit and the run reports the speed at the limit (1366x768) (1760-drive-hoist-slow-down) | Simulate | 1366x768 | pass | - | 3 |
| Drive hoist slow-down switch: the hook slows before the final upper limit and the run reports the speed at the limit (1920x1080) (1760b-drive-hoist-slow-down-1920) | Simulate | 1920x1080 | pass | - | 3 |
| Closed-loop winch on the ground: lowering past the ground leaves the rope slack and the run says so (1766-closed-winch-ground-slack) | Simulate | 1366x768 | pass | - | 6 |
| Closed-loop winch on the ground: lowering past the ground leaves the rope slack and the run says so (1766b-closed-winch-ground-slack-1920) | Simulate | 1920x1080 | pass | - | 6 |
| Hoist in 3D follows a closed-loop winch of Drive run: the hook rises with the winch and the line under the controls says so (1767-winch-in-3d) | Simulate | 1366x768 | pass | - | 3 |
| Hoist in 3D follows a closed-loop winch of Drive run: the hook rises with the winch and the line under the controls says so (1767b-winch-in-3d-1920) | Simulate | 1920x1080 | pass | - | 3 |
| Drive run shows the travel brake, the wind and the stop distance of an axis (1790-travel-brake-wind) | Simulate | 1366x768 | pass | - | 3 |
| Drive run at 1920 shows the travel brake, the wind and the stop distance of an axis (1790b-travel-brake-wind-1920) | Simulate | 1920x1080 | pass | - | 3 |
| The Travel drive rows of the motor sheet and the Hoist in 3D status line show the brake, the wind and the grip (1791-travel-sheet-and-3d) | Simulate | 1366x768 | pass | - | 4 |
| At 1920 the Travel drive rows of the motor sheet and the Hoist in 3D status line show the brake, the wind and the grip (1791b-travel-sheet-and-3d-1920) | Simulate | 1920x1080 | pass | - | 4 |
| Crane simulation example: opens from Templates and examples, the radio moves hoist, bridge and crab through the drawn circuit (1809-crane-sim-example) | Simulate | 1366x768 | pass | - | 14 |
| Crane simulation example at 1920x1080: opens from Templates and examples, the radio moves hoist, bridge and crab through the drawn circuit (1809b-crane-sim-example-1920) | Simulate | 1920x1080 | pass | F-001 | 15 |
| Drive hoist on the floor: the first lift takes up the rope and the run reports the snatch of the load (1366x768) (2430-drive-hoist-floor) | Simulate | 1366x768 | pass | - | 3 |
| Drive hoist on the floor: the first lift takes up the rope and the run reports the snatch of the load (1920x1080) (2430b-drive-hoist-floor-1920) | Simulate | 1920x1080 | pass | - | 3 |
| Live current paths: thin dashes on the wires that carry a load current (338-live-current-paths) | Schematic | 1366x768 | pass | - | 5 |
| Live current paths in the Light look (339-live-current-paths-light) | Schematic | 1366x768 | pass | - | 1 |
| Live view: hold a pushbutton (395-live-hold-pushbutton) | Schematic | 1366x768 | pass | - | 3 |
| Live view: hold a pushbutton (wide window) (396-live-hold-pushbutton-wide) | Schematic | 1920x1080 | pass | - | 3 |
| Motor at variable frequency at 1920: curves, readouts, model label, and the project without a motor (610-motor-frequency-1920) | Simulate | 1920x1080 | pass | - | 5 |
| Motor at variable frequency: curves, readouts, model label, and the project without a motor (610-motor-frequency) | Simulate | 1366x768 | pass | - | 5 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Simulate | 1920x1080 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1809b-crane-sim-example-1920-1920x1080-15-finding-1.png |

### F-001 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1809b-crane-sim-example-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text File #1`
4. `wait 6`
5. `click_text "Templates and examples..."`
6. `wait 10`
7. `text crane`
8. `wait 8`
9. `shot centre-search-crane`
10. `expect "Open crane sim example"`
11. `click_text "Open crane sim example"`
12. `wait 6`
13. `shot tile-picked`
14. `expect "A crane you can run"`
15. `click_text "Open"`
16. `wait 60`
17. `shot example-open`
18. `expect "Overhead cran"`
19. `key shift+l`
20. `wait 20`
21. `click_text "Controller…"`
22. `wait 20`
23. `key ctrl+k`
24. `wait 3`
25. `text Simulate hoist run`
26. `wait 4`
27. `key Enter`
28. `wait 20`
29. `drag 1554 100 480 100`
30. `wait 10`
31. `click_text "Start"`
32. `wait 20`
33. `expect "Hoists in this project: =CRN+FLD-M11"`
34. `shot controller-bound`
35. `click 1272 387`
36. `wait 10`
37. `expect "Signals (2 of 15 high)"`
38. `mouse_down 1312 524`
39. `wait 150`
40. `expect "fine"`
41. `expect "brake open"`
42. `shot hoist-fine-rising`
43. `mouse_up 1312 524`
44. `wait 10`
45. `mouse_down 1312 554`
46. `wait 150`
47. `expect "coarse"`
48. `shot hoist-coarse-rising`
49. `mouse_up 1312 554`
50. `wait 150`
51. `expect "0 rpm"`
52. `shot hoist-stopped`
53. `click 1410 100`
54. `wait 6`
55. `key ctrl+k`
56. `wait 3`
57. `text Simulate drive run`
58. `wait 4`
59. `key Enter`
60. `wait 20`
61. `drag 1624 100 480 100`
62. `wait 10`
63. `click_text "Start"`
64. `wait 20`
65. `expect "=CRN+FLD-M21, =CRN+FLD-M22"`
66. `shot drive-run-started`
67. `mouse_down 1312 636`
68. `wait 200`
69. `shot bridge-forward-slow`
70. `mouse_up 1312 636`
71. `wait 10`
72. `mouse_down 1312 666`
73. `wait 200`
74. `expect "Hz"`
75. `shot bridge-forward-fast`
76. `mouse_up 1312 666`
77. `wait 200`
78. `scroll 1704 500 -400`
79. `wait 10`
80. `shot controller-scrolled`
81. `mouse_down 1739 378`
82. `wait 200`
83. `expect "Hz"`
84. `shot crab-right-fast`
85. `mouse_up 1739 378`
86. `wait 100`
87. `click 767 100`
88. `wait 6`
89. `key ctrl+k`
90. `wait 3`
91. `text Hoist in 3D`
92. `wait 4`
93. `key Enter`
94. `wait 40`
95. `expect "Hoist in 3D"`
96. `drag 1354 119 300 119`
97. `wait 10`
98. `shot hoist-in-3d-before`
99. `mouse_down 1619 236`
100. `wait 200`
101. `shot hoist-in-3d-bridge-moving`
102. `mouse_up 1619 236`
103. `wait 10`
104. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Select' over 'Hide'; 'Place symbol' over 'Hide'

![F-001](shots/1809b-crane-sim-example-1920-1920x1080-15-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
