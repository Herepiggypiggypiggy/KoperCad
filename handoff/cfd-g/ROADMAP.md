# CFD-G validation: roadmap

Scope: independent correctness of novo-cfd (tests/validation*, docs/thermal-validation.md, accuracy statement). No solver edits; report to lane threads.

Done: conduction, source, balance, transient, Richardson, symmetry/rotation/depth tests; report table.
Next, as lanes land: Churchill-Chu plate (open boundaries), IEC 60890 vs 2D, layered conjugate slab, fan/chimney, turbulent cases, cabinet Richardson, per-lane sign-off PRs, AccuracyStatement filled from the table.
