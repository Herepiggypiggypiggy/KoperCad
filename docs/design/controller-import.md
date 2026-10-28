# Controller import wizard (lane C4)

Where the user finds it: **File > Import > Controller profile from I/O table...** and the command field
(`import controller`, Dutch `importeer bediening`). Code: `novo_controller::import` (pure engine, std only) and
`novo-app` `app/controller_import.rs` (the window).

The wizard turns a standard I/O page (the terminal table of a radio receiver or pendant) into a controller profile
(`docs/design/controller-profile.md`) and a binding table. It only guesses; the preview shows what it would create and
a warning for every row it is unsure about. Nothing is written until **Save profile**. Saved files go to
`%APPDATA%\KoperCAD\controllers\<id>.controller.toml` plus `<id>.bindings.csv`; a file the wizard did not write is never
replaced (the next free name is used).

## Ways in (all end in the same editable table)

| Way | What it reads |
| --- | --- |
| Paste | Excel columns (tabs), CSV (`;` or `,`), pipe tables, columns split by two or more spaces. Header words in Dutch and English; without a header the columns are guessed (short labels with a digit = terminal, step-like cells = step, the widest text = function) |
| From the selected device | The pins of the device selected in the drawing, the label of what each pin is wired to (`-K61:A1`) as binding target, and the far device's description as the function text when the pin has none |
| Open table or EPLAN file | A `.csv`/`.tsv`/`.txt` table (UTF-8 or Windows-1252), or an EPLAN export (`.epj`/`.xml`, read with `novo-interop`): choose a page, terminal texts are paired with the nearest text on the same line. The pairing is a guess and shown in the table for correction |
| Generic library | Five neutral profiles built from their own tables by the same wizard: radio 3 and 5 motions 2-step, wired pendant 3 motions, pendant with E-stop and horn, joystick with two axes |

Real `.xlsx` files are not read directly: copy the cells from Excel and paste.

## Words (data, not code)

`crates/novo-controller/data/words-en.txt` and `words-nl.txt`, one line per category (`motion.hoist`, `dir.up`,
`combo.hoist.up`, `speed.slow`, `role.estop`, `kind.radio`...), phrases and `stem*` prefixes allowed, longest phrase
first. A user list in the same format at `%APPDATA%\KoperCAD\controllers\words.txt` is added on top; lines it cannot
read are shown in the wizard. The lists contain no vendor or product names (a test checks that).

## What it makes

A control per motion and direction pair (`rocker2` up to two steps, `rocker3` for three), signals with `group` and
`step` (`HOIST_UP_SLOW`, `HOIST_UP_FAST` for two steps, numbers for three), `pin` = the terminal label, `common`,
`supply` when a voltage with AC/DC is written (`48 VAC`), E-stop (NC chain `ESTOP_OK`, ungated), main/start latch
(`requires estop=released`, `drops_when stop=on`, `needs_link` for a radio), stop, horn, light. When a main row exists
the profile gates the motions on `start=on` (a checkbox turns that off). Step 2 keeping step 1 closed is the default
(checkbox).

## Warnings the preview gives

A row that names both directions (`Hoist up/down`), several motions, a motion without direction, a direction without a
motion when the table names several, an unreadable step cell, step column and text that disagree, a bare number that
could be a step or a unit number, more than three steps, a step with no lower pin, a direction with no opposite, the
same terminal used for two signals, a second common, a missing common, an AC/DC unknown supply, selector switches
(never guessed), and every row that was not recognised at all.

## Honest limits

It reads words, not meaning: a profile is only as good as the page it came from, and the table shows every guess.
It does not model radio protocols or receiver firmware. Vendor profiles are never shipped; the user's own pages and
profiles stay in their own folders. Selector switches, analog signals and interlocks between separate controls are
added in the profile editor (lane C3).
