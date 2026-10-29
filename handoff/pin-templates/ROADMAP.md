# Pin templates per device class: roadmap

Scope: standard pin names per device class (coil `A1 A2`, motor `U1 V1 W1`, brake `BD1 BD2`, breaker poles), generic IEC conventions only, stored as strings with several valid sets per class.

Done (first PR)
- `novo-symbols::pin_template`: templates, `template_for(function)`, `check_pins(function, pins)`, `Deviation::message`.
- Test: every built-in symbol of both packs follows its template.
- `novo-checks` kind `PinNames` (info, key `check.pin_names`): placed device pins and the terminal names of a single assigned part, in Check > Issues, filter chip "Pin names".

Done (second PR)
- `suggest_renames` (novo-symbols) and `pin_fix` (novo-checks): Issues right-click on a Pin names problem offers the rename of symbol pins, or of the part's terminals, to the class names. Only clear matches (case/spacing, prefix, single left over); never position guesses.

Done (third PR)
- Company-defined templates: TOML text in the project property `pin.templates` (`novo_symbols::parse_company_templates`, a class replaces the built-in names for the functions it lists), edited in Project > Rules > Usual pin names; the Pin names check and the Issues rename use it. QA scenarios 2410 to 2413 and the pin-names fixture are in the repo.

Next
- A company templates file shared across projects (today the text lives in each project).
- Offer the part-terminal mapping at the moment of part assignment (today it is on the Issues row).
- Templates for PLC cards, network devices and terminals if the shop wants them.
