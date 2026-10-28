# Contact mirror (cross-reference contact table) roadmap

Scope: make the contact table under a coil readable. Terminal numbers on either side of a small contact symbol, one line per pole, location once per contact, spare contacts dim, double click jumps.

Done
- Table layout `13 [symbol] 14  3.4` (`novo-canvas::note_table`), three-pole and four-pole main contacts stacked with a dashed coupling, changeover with stacked throws, spare rows (dim, "spare").
- `novo-xref`: `Label.spare` (`SpareRow`), numbers continue the EN 50005 sequence of the drawn contacts.
- App: Preferences > Cross references > Spare contacts in the table (Show/Hide); double click on a table row jumps to that contact, on a contact's reference jumps to its coil.
- PDF/print: same scene, spare rows in grey (`print_style().note_dim_color`).

Next / open
- Spare numbers are derived, not read from the part. When parts carry contact numbering, read it from there.
- Hover highlight of a row under the pointer, and a tooltip naming the double click.
- Contact reference at a contact still prints only the coil location (`2.5`); a coil pole mirror ("which pole of the coil") has no source in the model.
