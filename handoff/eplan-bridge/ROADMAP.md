# EPLAN-BRIDGE roadmap

Scope: the user-run "Export from EPLAN" helper (lane 4 of `/mnt/project-files/interop/crane-configurator-findings.md`). No admin rights, no EPLAN API licence, no EPLAN content in the repository.

Done
- `novo_interop::eplan::export_script`: script, EN/NL instructions, synthetic self-test PXF, manifest and version check, PXF head reader.
- `crates/novo-interop/assets/eplan/KoperCAD_ExportFromEPLAN.cs`.
- `kopercad-cli eplan script|check`.
- App: File > Import > Get the EPLAN export script (guide with English / Nederlands toggle).

Next
1. Test the script in a real EPLAN 2024 (Tadjiro or findings thread); fix API details.
2. The EPLAN-PXF-READ lane calls `check_exchange` before importing and lights up File > Import > EPLAN project.
3. Reverse direction (`import /TYPE:PXFPROJECT`) with EPLAN-PXF-WRITE.

Open questions
- Is `export` without `PROJECTNAME` the open project in the user's EPLAN? Assumed yes; the env var `KOPERCAD_EPLAN_PROJECT` overrides.
- Exact Utilities > Scripts menu names per EPLAN version and language.
