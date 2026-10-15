# PLC link: roadmap

Scope: brief 9 lane A (docs/AMBITION-REVIEW.md, PR #504): tag-file export and import for the PLC programming tools. Design: docs/design/plc-link.md. Lane B (ISO 13849) is another thread.

## Done
- novo-plc `link`: tag list, CSV, PLCopen XML, TIA xlsx, Studio 5000 CSV, address plan, reader, import plan/apply.
- App: File > Export PLC (5 formats), File > Import PLC tags with preview; commands in Ctrl+K; `kopercad-cli export plc`.

## Next
1. First real import into CODESYS, TIA Portal and Studio 5000 (needs Tadjiro's machine or a free tool); fix any layout differences.
2. Hardware config: GSD/EDS device description import, fieldbus topology and diagnostic addresses.
3. Symbol tables for ladder tools beyond I/O (memory bits) if wanted.

## Open questions
- Which PLC family the workshop uses first decides which file gets validated first.
