# ui-pdf-options roadmap

Scope: the PDF options dialog (gap "PDF options dialog"). Engine: novo-pdf `PdfOptions`; no engine change.

Done
- File > Export drawings > Export PDF... (`export.pdf_options`): pages (all, page on screen, ranges), bookmarks, links, cross references, title block revision row, changed-page marks, design checks pages, look (black or colour), title. Remembered per user in the settings file.

Next
- Language and paper size are not in `PdfOptions` (pages keep their own size, drawing text has no language): add them in the dialog when the engine grows them.
- A "Save as..." file picker and a page list with checkboxes.
