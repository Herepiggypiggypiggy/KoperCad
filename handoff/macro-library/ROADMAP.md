# Macro library and sharing: roadmap

Scope: the library side of macros (the Macros thread owns capture, placement, renumbering).

Done (PR 1): `.kcmacro` format, `MacroLibrary` in `novo-parts::macros` (folders, search, rename, move, delete, import/export, packs, safe names, atomic writes).
Next, in order:
1. Starter macros (motor starter DOL, star-delta, E-stop chain, power supply) built through `DefineBlock`, shipped as files, copied into the user library on first use.
2. Thumbnails: render a macro's members to a small image/scene (app layer, after the Macros thread's placement lands).
3. Browser window in novo-app: tree, search field, preview, rename/delete, Import/Export buttons (thin UI, per DESIGN).
4. Company folder setting and update offers (compare `BlockSource.version` with file `revision`).
5. CLI: `kopercad-cli macros list|import|export`.
Open: confirm file shape with the Macros thread (sent 2026-10-15 18:25 UTC).
