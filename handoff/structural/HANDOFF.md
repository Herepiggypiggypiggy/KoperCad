# Structural estimates: handoff

Branch `claude/project-thread-tk5tlr`, PR in kopercad (see PROGRESS.md).
Engine is pure (no Project). Tests in the module compare with hand sums,
textbook beam maxima and Timoshenko plate coefficients (0.00406, 0.01160).
UI inputs live in the Calculations window state only (not saved in the
project): there is no busbar or device weight in the model yet.
Resume: `cargo test -p novo-checks --lib mechanical`, `cargo test -p novo-app --lib -- calculations`.
Traps: filament force is conservative for wide bars; clamped ends assume
rigid clamps; plate assumes all four edges supported.
