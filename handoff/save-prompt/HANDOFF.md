# Save prompt and recovery (KC-02)

Hardware finding KC-02 (qa/hardware-results-review.md): edits were written to the project file about 500 ms after each change, Quit never asked, and docs promised otherwise.

Now: the project file changes only at Save. `StorageBackend::tick` writes the recovery copy only (per-user recovery folder). `NovoApp::leave_then` asks "Save changes?" (Save / Don't save / Cancel) before New, Open, Open read-only, Recover, Example, Practice, Close and Quit, and the window's close button is intercepted the same way. File pickers run before the question so cancelling a dialog costs nothing. Backend `close` without an answer (shutdown, lesson replacing the project) leaves the work in the recovery file, offered back at the next open. `discard_unsaved` is "Don't save". Top bar says "Unsaved changes". Docs: DESIGN.md Saving, FIRST-DAY.md. QA scenario 640.

Open: untitled drafts still keep a `kept-*` file when closed without an answer.
