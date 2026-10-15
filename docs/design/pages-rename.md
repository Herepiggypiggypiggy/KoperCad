# Rename and reorder of groups, panels and pages

Status: proposal for approval (Pages thread). No code changes with this
document. It extends DESIGN.md "Pages navigator" and §3.

## What is stored today

Groups (`=10D41`) and panels (`+K1B1G1`) are not entities. They are tag
levels copied as strings into five places:

| Where | Field |
| --- | --- |
| `Page` | `structure` (function and location levels) |
| `PageFreeze` | `structure`, the value printed when the page was issued |
| `Device` | `tag` (`=10D41+K1B1G1-K1`) |
| `Cable` | `tag` |
| `Structure` | `tag`, the optional node that carries `description` and `enclosure` |

Nothing refers to a group or panel by id. The enclosure lookup
(`Project::enclosure_of`, used by terminal suggestions) compares a device's
location levels with `Structure.tag.location`. So renaming a panel by
editing one place leaves the rest pointing at a name that no longer exists:
devices fall out of their enclosure, terminal strips lose their inner side,
and the panel splits in two in the outline.

## One command renames every copy

New additive command `RenameLevel { aspect, from, to }` (WS-02 owns the
API; same pattern as `RenamePage` in #314).

- `aspect` is Function (group) or Location (panel). `from` and `to` are
  level paths, so `from = [K1B1G1]` and `to = [K1B2G1]`.
- It rewrites every copy listed above whose levels for that aspect start
  with `from`, replacing that prefix with `to`. Prefix matching keeps
  children attached: renaming `+CAB1` also renames `+CAB1+DOOR`, so the
  parent chain `enclosure_of` walks is unchanged.
- It is one transaction and one undo step. Because the `Structure` node is
  rewritten in the same transaction as the devices, `enclosure_of` gives
  the same answer for every device before and after. A test pins this: the
  terminal suggestions of a project are identical before the rename and
  after it, and again after undo.
- Frozen pages keep what was printed. `PageFreeze.structure` and the frozen
  name are not rewritten (pinned flow, DESIGN.md §8); the live page and its
  devices are.
- Renaming a group changes only the function aspect and a panel only the
  location aspect, so a panel shared by two groups stays one panel when
  either group is renamed.

Refusals, each with a reason shown in the hint and nothing changed:

- Empty target, or a target that does not parse as a level.
- The target already exists and the rewrite would make two devices (or two
  cables, or two structure nodes) carry the same full tag. The reason names
  the first clash. Merging two panels that do not clash is allowed and is
  the same command (rename to an existing name); it is shown in the
  confirmation line as "merges with the existing panel".
- Renaming a level into its own child path (`+CAB1` to `+CAB1+DOOR`).

## Where it appears

- Outline (Function and Location views): double-click a group or panel row,
  or right-click, Rename. Same inline editor as page rename (Enter or click
  away keeps, Esc cancels). The hint after a rename says what moved, for
  example "Renamed +K1B1G1 to +K1B2G1: 6 pages, 41 devices, 3 cables".
- Palette: "Rename group" and "Rename panel", acting on the current page's
  group or panel when nothing is selected in the outline.
- Scripts and AI edits use the same command.

## Reordering

Order today is derived: pages sort by group, panel, then page name in
natural order; groups and panels sort by their level names. Dragging cannot
express a different order without changing names.

Recommended: an explicit sort key kept in `Properties` (key `order`, an
integer), no new field and no migration.

- Pages: dropping a page between two siblings (same group and panel) writes
  `order` on that sibling list as 10, 20, 30, and so on, in one command
  `ReorderPages`. A scope with no `order` set keeps today's natural
  order by name, so existing projects do not move.
- Groups and panels: the same key on their `Structure` node, which is
  created on first reorder if missing (it already exists to carry a
  description and the enclosure flag).
- `page_cmp` sorts by `(order, name)` inside a scope where any order is set.
  This is the one change to WS-02 code beyond the commands.
- Page names are not renumbered by a drag. Cross references keep their
  numbers; if the user wants names that follow the order, "Renumber pages"
  (DESIGN.md §3) already does that, and it is a separate, visible step.

Alternative considered: renumber page names on every drag. Rejected because
printed references change under the reader without a decision, and names
like `3.1` (inserted pages) have meaning.

## Contents page per panel

The contents list today is one generated page (`toc`) for the project. For
a panel the user also wants its own list (the pages of one cabinet drawing
set).

- A generated page whose structure holds only a location lists only the
  pages of that panel. The generator id is `toc@+K1B1G1`; `LiveReports`
  (WS-13) resolves the part after `@` as the scope. The project contents
  list is unchanged and still lists every page, with these pages in it.
- Outline right-click on a panel: "Add contents page for this panel". It is
  created once per panel and follows renames: `RenameLevel` rewrites the
  page's structure and generator id together.
- Page order inside the panel list is the same order as the outline,
  including `order` when set.

## Delivery in small PRs

1. This document (docs only).
2. `RenameLevel` in `novo-core` with tests in a new file
   (`tests/rename_level.rs`: devices, cables, pages, structure,
   enclosure lookups unchanged, frozen untouched, clash refusal, merge,
   undo).
3. Outline rename on group and panel rows in `novo-app`.
4. `ReorderPages` and `order` in `page_cmp`, then drag in the outline.
5. Contents page per panel (WS-13 owns the generator, so it is a request to
   them or a small PR they accept).
6. Handoff update in one last commit.

## Decisions wanted

- Is an explicit `order` property acceptable (recommended), or should
  order stay derived from names only?
- Is merging into an existing panel on rename acceptable (recommended, with
  the confirmation line), or should a clash always refuse?
