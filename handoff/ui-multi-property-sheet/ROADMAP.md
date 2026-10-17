# ui-multi-property-sheet roadmap

Scope: the UI for the multi-device property sheet and bulk edit (engine: `novo_parts::PropertySheet`).

Done
- Inspector shows the shared sheet when two or more devices are selected: common value, "multiple values" hint, one undo step per edit, part for the whole selection, add and remove a property on all.
- Command `edit.shared_properties` (Edit menu, Ctrl+K "properties of the selection"): the same sheet in a window for one device, terminals or cables.

Next
- Cable and terminal multi-selection in the Inspector (today only the window).
- Dropdown for fields with fixed choices (`Row::choices`).
