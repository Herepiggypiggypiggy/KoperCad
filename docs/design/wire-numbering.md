# Wire numbering

Crate `novo-numbering` (pure, depends on `novo-core` only). Where the user finds it: palette (Ctrl+K)
`wire numbering`, `number wires`, `renumber wires`, `check wire numbers`, `number wire <text>`,
`unlock wire number`. Macro placement numbers its wires with the same engine.

## Model

A number belongs to an electrical node. The engine groups wires by net (junctions and interruption
points join wires across pages, a contact or fuse ends the net) and every wire of a group carries the
group's number. Numbers are stored in `Wire.number`, so the wire list, labels, cut list, cable
schedule and terminal screens read them from one place and cannot disagree. A wire outside any net
is a group of its own. Groups sort in reading order: page order, grid column, top to bottom, left to
right.

## Schemes

Stored in the project properties (`wirenum.*`), so scripts, the palette and macros number the same way.

| Scheme | Looks like | Falls back to |
| --- | --- | --- |
| Sequential | `101` | |
| By page | `3.1` | |
| By page and column | `3.4.1` | |
| By potential | `L1.1`, `PE` | by page (net carries no single potential) |
| By cable and core | `W1.BK` | by page (wire has no cable core) |
| By function | `A2.1` | by page (page has no function aspect) |

Options: prefix, separator, start, step (10 leaves room to insert), digits (zero padding), reuse freed
numbers (off: a retired number stays retired), and PE/N/PEN keep their bare name on every net.

## Stable across edits

`Mode::Fill` is what "number wires" and macro placement run. It keeps every existing number that is
unique, whatever the scheme says, and only numbers what has none: a new net takes the next free count
after the series maximum. A net whose wires disagree (two numbered nets joined, or a wire added to a
numbered net) takes the number most of its wires carry, ties to the lower, and the others follow.
`Mode::FillStrict` also replaces numbers the scheme would not give. `Mode::Renumber` starts over in
reading order and keeps only typed (locked) numbers. All three can be limited to the project, pages
or the nets of some wires.

## Manual numbers

`set_number` types a number on a whole net and sets the `wirenum.locked` wire property: no run
changes it. It is refused when another net already carries the number (a bare PE/N name excepted).
`unlock` releases it.

## Conflicts

`check` and every `Plan` report: duplicate (a number on two nets, error), split (one net, two
numbers, error), partial and unnumbered (warning), off scheme (info). A plan lists the conflicts that
remain after it is applied.

## Preview, undo

`plan` never changes the project. `Plan::apply` runs inside the caller's transaction (one undo step)
and is refused when the project changed since the preview. The window saves the scheme in the same
step.

## Later

Cut list and label printer columns already read `Wire.number`. Design checks should wrap
`novo_numbering::check` as `wire.number.*` problems (owner: checks thread). A "keep numbers on page
move" option and per-page schemes (e.g. power pages by potential, control pages by page) are open
questions.
