# Add-ons in the new design: proposal to the redesign threads (2026-10-28)

From lane C0b (add-on manager). Reads `docs/design/visual-redesign.md`. Nothing here is hard-coded into menus or
toolbars that V2/V3 replace: every surface goes through the command registry. Please confirm or adjust items 2, 3, 4.

1. **Manage add-ons is a centre window** (960x640, resizable, V1 centre class, built on `panel::dialog`). Left: add-ons
   with icon, name, version, publisher, status chip (Enabled, Disabled, Needs newer app, Unsigned). Right: details
   (permissions, files and hashes, what it adds). One primary action "Install from file...". Row actions: Enable/Disable,
   Update from file, Roll back, Remove. Dropping a `.kcaddon` on the window installs it too.
2. **Entry points.** Command `addons.manage`, label "Add-ons" (<=28), group Project, keywords "add-on plugin extension
   install controller". Home today: registry placement under File. **Proposal for V2b:** the app-menu footer beside
   Preferences and About, so File stays inside its budget of 20. Also Ctrl K ("Manage add-ons") and verb `addons`.
3. **Icon.** One extra drawn icon for the V0 set: `workflow-addon` (a block with a plug-in tab, 24 grid, stroke 1.5).
   Used for the entry, the manager header and the add-on chip. Until V0 lands: Phosphor puzzle-piece behind one constant
   (`ADDON_ICON`), so the swap is one line. Status marks reuse the existing status icons; never colour alone.
4. **Commands from add-ons** are not added to the static `COMMANDS` slice. A runtime `ExtensionRegistry` (crate
   `novo-addons`, read by `novo-app`) returns descriptors: id `addon.<addon-id>.<name>`, short label <=28 (checked at
   install; a manifest that breaks it is refused), tooltip/description, icon NAME from the built-in set (unknown falls back
   to `workflow-addon`; add-ons ship no pixels in v1), keywords, and a declared home: `menu` (an existing area, e.g.
   Simulate), `rail` (workspace + zone assists|panels) or `palette-only`. Ctrl K shows them with the type chip
   "Add-on: <name>" only while enabled. **Ask for V2b/V3a/V5:** app menu, rail tables and palette read ONE function
   `commands::all()` that merges static + extension commands; the presentation lookup is static table first, extension
   descriptor second; the tests "every command has an icon, a label <=28 and a home" also run over extension commands, at
   install time rather than at compile time.
5. **Rail.** No add-on zone. Add-on rail items append to the Assists or Panels zone of the declared workspace, max 2 per
   add-on; the 6-per-zone cap holds (overflow becomes palette-only and the manager says so). **Ask for V3a:** rail data
   tables accept dynamic rows, not only const arrays.
6. **Windows.** Panels an add-on declares are drawn by the host with `panel.rs` (title drag, resize, dock, remembered,
   `panel::id("addon.<id>.<panel>")`), so they inherit the V1 rules. Add-ons cannot open their own windows. The live
   view card slot and the menu slot are extension points on the same registry.
7. **Lessons.** New menu anchor `menu.addons` for lane V7.
8. **Discoverability.** Where an add-on's feature would be (for example the live-view side panel), the base shows one
   line "Controllers: add-on not installed. Manage add-ons" instead of the feature.
