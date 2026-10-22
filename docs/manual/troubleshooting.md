# Troubleshooting and recovery

## The program will not start

- Do not run it as administrator; it does not help.
- On a machine with no usable graphics, the program falls back instead of failing. If a start-up message appears, copy the problem report (below).
- To start with clean settings without touching your real ones, run `kopercad.exe --profile-dir C:\Users\you\kopercad-test` (a new empty folder).
- Windows may say "Windows protected your PC" the first time because the program is not code-signed yet. Choose More info, then Run anyway.

## Getting your work back

1. **After a crash or power cut**, start KoperCAD again. It offers the recovered work. Your `.kopercad` file is only changed by Ctrl+S, so it still holds your last save.
2. **A mistake you saved**: Ctrl+Z goes back through the whole session. A revision (File > Revisions) is a snapshot you can compare with and restore a page from.
3. **A damaged file**: open it as usual. The program salvages the readable objects into `<name>.recovered.kopercad`, opens that copy and asks you to check every page.
4. **The file is from a newer version**: install the newer build. The older program refuses it, untouched.

## Something looks wrong

| You see | Likely cause |
| --- | --- |
| A key does nothing | Letter keys work only on the drawing; click it first. A text box has the focus when you are typing. |
| A command is grey | It needs something (a project, a selection). Hover for the reason, or search it with **Ctrl+K**. |
| Wire lengths with ~ | The cabinet is not laid out and routed yet. See [Model or exact?](numbers.md#wire-lengths) |
| A part has no weight or price | The part data did not have it. |
| Heat shows `stalled` | The solver stopped early; the result is marked. Run again or refine. |
| Text is cut at a small window | Maximise the window; 1366 x 768 is the smallest tested size. |

## Report a problem

Help > **Copy problem report** copies the version, build, system, graphics card and the last lines of the log to the clipboard. Nothing is sent anywhere. Paste it into your message with what you did, what you expected and what happened. After a crash the program offers to open the crash report; Help > **Open log folder** has it.
