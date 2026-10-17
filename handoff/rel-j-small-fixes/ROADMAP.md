# rel-j-small-fixes: roadmap

Scope: the three S3 findings of Windows pass J (/mnt/project-files/qa/v0.1/J-windows-install-rerun.md).

- Done: example opens centred at 1920 px (a drawing that nearly fills the sheet fits the whole sheet).
- Done: File > Export drawings entries are distinct: "Export PDF" (one click, defaults) and "Export PDF with options...".
- Done: every PDF export goes to `<Documents>\KoperCAD` (profile-aware, follows `--profile-dir`), named after the saved project.
- Next: re-run J on Windows once a build holds this PR. Freeze: PR stays draft until rc1 is tagged.
- Not done: other exports (DXF, xlsx, bench, plate PDF) still sit beside the project.
