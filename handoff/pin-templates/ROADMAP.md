# Pin templates per device class: roadmap

Scope: standard pin names per device class (coil `A1 A2`, motor `U1 V1 W1`, brake `BD1 BD2`, breaker poles), generic IEC conventions only, stored as strings with several valid sets per class.

Done (first PR)
- `novo-symbols::pin_template`: templates, `template_for(function)`, `check_pins(function, pins)`, `Deviation::message`.
- Test: every built-in symbol of both packs follows its template.
- `novo-checks` kind `PinNames` (info, key `check.pin_names`): placed device pins and the terminal names of a single assigned part, in Check > Issues, filter chip "Pin names".

Next
- Apply a template when placing a user or imported symbol whose pins are unnamed (rename suggestion with undo).
- Let a company pack declare its own templates (TOML) next to its symbols.
- Compare part terminals with symbol pins and offer the mapping on part assignment.
- Templates for PLC cards, network devices and terminals if the shop wants them.
