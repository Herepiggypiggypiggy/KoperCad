2026-10-16 15:26 UTC thread started by the coordinator: design lead.
2026-10-16 15:50 UTC captured 22 screens at 1366x768 and 1920x1080 (software Vulkan, Xvfb).
2026-10-16 16:10 UTC wrote visual audit and design direction (project mount design/).
2026-10-16 16:40 UTC PR 1: Pages button into the top bar, outline tabs fit; tests green (novo-app lib 524).

- 2026-10-16: #728, #740, #746 merged; #787 About card; light sweep PR moves 161 colour-token uses in app panels to theme::ui_palette() and adds a guard test.
- 2026-10-17: #792 merged (light sweep). Checked Light in the app (Xvfb, 1366x768 and 1920x1080): start, schematic, Cabinet, 3D, Reports, Commissioning, File/View/Check menus, Parts, Preferences, Safety functions, Report designer, Languages. No colour gaps. Screens in the project mount design/light-2026-10-17/. Fixes: Commissioning tab row no longer cut off (header menu on its own row, Commissioning tab in the left panel); Preferences theme note no longer says panels are "not themed yet". Open: Heat/Dimensions buttons in 3D look dim in Light only because disabled (expected).
