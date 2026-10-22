# SCALE-2 handoff

Branch `claude/project-thread-f9dpzb`, PR from main (see PROGRESS for the number). Resume: `cargo test --release -p novo-app --lib scale_tests -- --nocapture`; engine: `scripts/scale-bench.sh --size large --budget --only generate,save,open,search,page,edit,undo,number,design,report,autopilot,pdf,route,3d,xref,json,cut`.

Profiling trick that worked: run the release test binary directly and sample with `gdb -p PID -batch -ex "thread apply all bt 60"` in a loop; count the top novo_ frames. No perf in the container.

Traps: write the big file with `novo_storage::ProjectFile::create` from a separate document; the app's own connection keeps the file locked and the open waits 5 s. Disk: `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`.
Numbers and ranked leftovers: RESULTS.md.
