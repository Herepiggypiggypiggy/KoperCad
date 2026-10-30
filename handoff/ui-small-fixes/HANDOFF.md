
## 3D toolbar and view cube (follow-up PR)
At 1366x768 with the Navigator open the 3D toolbar ran under the view cube: its width ignored the room the Navigator takes. `view3d_controls::toolbar` now subtracts `panel::flyout_width`. Scenario 7382 (both sizes).
Drag-dock scenario for the refit: not added. The Navigator is a flyout, dragging its title only moves it and does not change the canvas size, so it cannot exercise the refit; a dockable panel in the 3D workspace is needed for that.
