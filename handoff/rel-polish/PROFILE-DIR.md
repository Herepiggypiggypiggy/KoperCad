# Clean profile: `--profile-dir` (for REL-DOCS and testers)

```
kopercad.exe --profile-dir C:\Users\you\kopercad-test
```

or set `KOPERCAD_PROFILE_DIR` for the run. Every per-user folder the app and its libraries
use then sits under that folder, and nothing is written to the real profile:

| Subfolder | What it holds (normally) |
| --- | --- |
| `roaming` | settings (`KoperCAD\app.ron`), parts catalog, templates, user keymap, cabinet rules |
| `config` | lesson progress |
| `local` | recovery files (`KoperCAD\recovery`), unsaved new projects, mesh cache |
| `cache` | interop caches |
| `documents` | where new projects are saved |

An empty or missing folder is a first run. Delete the folder to start over. It is created on
first use, per user, no admin rights. The flag may come before or after a project file
(`kopercad.exe --profile-dir X plant.novo`). `kopercad-cli` honours `KOPERCAD_PROFILE_DIR`
where it reads these folders. The Downloads folder (parts import watch) is not moved.

Replaces the rename-the-folders workaround in TESTER-NOTES.md once that note is updated.

Also under the profile: the user's macros folder (`roaming\KoperCAD\Macros`, `novo_parts::user_macros_dir`)
and the log and crash files (`local\KoperCAD\logs`, `diag.rs`; the flag is applied before logging starts).
