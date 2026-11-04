# T12: handoff

Mechanism: `NovoApp::job_fail(job_key, title, level, text)` / `job_fail_at(.., path)` raise the log message (already dismissed, so no toast), and set a `Source::Job` flag anchored `Anchor::Job(job_key)`, `Life::Until`. One flag per job key (a second failure replaces the first). `job_ok(job_key)` clears it on success. `flags_jobs::retry_command` maps a job key to the command Retry queues; jobs that need a chosen file have no Retry. The top bar triangle (`job_pill`, left of the Issues pill) opens the popover (`flag_popover`) of the newest failed job; Retry, Open folder (`OPEN_FOLDER`), Copy details, Dismiss (`DISMISS`).

Census: `problem_census_files` counts `.message(`/`.raise(` with Level::Error or Level::Warning in the area's non-test files against `app/problem_census_files.txt` (66 on main before this PR, 18 after). Lower it with every follow-up PR.

Left (class P/R, still toasts or coloured sentences): see `sites/files.md`; the 18 remaining toasts are caveats ("ferrule standard has a size twice", "lengths are estimates", plot set notices) that belong on the field or result, not on a job.

Traps: job keys must stay stable (they are the flag id); titles are at most 28 characters; `error_words::plain` runs in `raise`, so the popover shows the advice already.
