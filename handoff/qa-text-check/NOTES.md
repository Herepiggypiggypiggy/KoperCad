# QA text check: table cells opt-in

- `check_text N` flags every label over N words. `check_text N cells` (opt-in per scenario) skips a long label only when short labels (4 words or fewer) sit on both sides of its line, two or more in all: a catalog data row. Sentences are always flagged.
- Used by scenario 7432 (Parts and Place macro part rows) only.
- Issues 1976 and 1958 were already closed by merged #1977 and #1961.
- Evidence: /mnt/project-files/qa/check-text-cells, source git aee8bfa6, both sizes pass.
