# Cabinet toolbar cleanup: roadmap

Scope: visual audit items C2, C3, C5, C6, C7 and the folding rule of `docs/design/cabinet-toolbar.md`.

Done (PR on branch `claude/project-thread-zxmg7r`):
- Measured folding into More, one item at a time (Fold in `cabinet_bar.rs`).
- Heat strip is its own wrapped row; counts are one pill style.
- Heat legend never shows "-0 W"; badges carry the device tag.
- One banner for guessed sizes; empty state has its button under the sentence.

Next: R3/B review fixes; Windows DPI 125/150 percent check (not done here).
