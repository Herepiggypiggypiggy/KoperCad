# The outline tree (Pages section)

Tadjiro, testing build 5: the project structure in the left nav was nearly unusable. No right click on empty space, an almost empty row menu, grouping that did not seem to work, and cabinets needed a naming convention typed in first. This is the redesign.

## What the person sees

A tree: page group (`=G1`), cabinet inside it (`+A1`), pages inside that. A toggle above it flips the nesting: **Groups** (groups, then cabinets) or **Cabinets** (cabinets, then groups). Groups and cabinets with no page yet are still rows, so a new one is never invisible. Contents and report pages sit in a "Contents and reports" row at the end. Pages that are in no group or cabinet stay at the top.

- **+ New** above the tree, and right click on empty space: New page, New page group, New cabinet. Several pages... keeps the old bulk form.
- **Right click on a page**: New page after this, Duplicate page (devices get new tags), Rename, Move up/down, Move to (every group and cabinet, or none), Properties, Delete.
- **Right click on a group or cabinet**: New page here, New group/cabinet, Open cabinet layout, Add contents page, Rename, Renumber its pages, Properties, Remove (keep the pages), Delete with its pages (asks first).
- **Keys** (when the tree has the last click): F2 rename, Delete, Ctrl+Up/Down move a page, Ctrl+D duplicate a page, Insert new page, arrows walk the tree and open pages, Left/Right fold, Enter opens or folds.
- **Drag and drop**: a page onto a group or cabinet files it there; onto another page it takes its place (or joins that page's group and cabinet); a cabinet onto a group moves its pages into that group. The list scrolls while a drag is held near its edge.
- Every delete is one undo step and says so.

## Automatic, never a prerequisite

- A new group or cabinet is named from the project: the prefix most names share, one past the highest number, as wide as the habit (`BAY1G2` then `BAY1G3`, `A01` then `A02`, `CAB` then `CAB2`). A project with none starts from the user's letters (default `G` and `A`, Project settings, with a live preview). `outline_tree::next_name`.
- New pages take the next number in their group and cabinet and land beside the page being worked on. A page moved into a group where its number is taken gets the next free one. Renumber puts a group in 1, 2, 3 reading order.
- New cabinet: name filled in, enclosure picked from the user's list (`enclosures.csv`) or the standard 600 x 800 plate, first page added in the chosen group. Enter creates it. The enclosure name is kept as property `enclosure` on the cabinet's structure node.
- Pages in no group or cabinet whose devices are tagged (`=A1+CAB-K1`) are offered a home: "File them" in the tree, or "File pages by their device tags" in the menu. The example project is filed this way.
- Devices placed on a filed page already take the page's group and cabinet in their tag (`novo-editor` naming), which is why filing matters.

## Where the code is

`crates/novo-app/src/app/outline_tree.rs` (tree, names, menus as data, every edit; tested on a plain Document), `outline.rs` (egui: rows, keys, drops, windows), `outline_tests.rs` (clicks, keys and drags in a headless window). The old `structure_headers` outline is gone; `pages.rs` keeps rename, reorder and the Several pages form.

## Not done

Dragging groups themselves, a free-form cabinet that is not an enclosure, and a hands-on look in a real window (the cloud has no GPU).
