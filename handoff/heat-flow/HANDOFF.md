# Heat flow readability: handoff

Code: `crates/novo-app/src/app/heat_flow.rs` (model, thinning, speed map, through-flow verdict, control and key), tests in `heat_flow_tests.rs`. Hooks: `Heat3d::flow` and `set_flow` (heat_3d.rs, keeps `lines` and `arrows` in step), `heat_arrows::arrows`, `heat_legend`, `view3d.rs` (`heat_controls`, `flow_scale`), `heat.rs::paint_airflow`.

Scale: 98th percentile of air speed rounded up to 0.1/0.2/0.5/1/2/3/5/10 m/s, so one fan jet cannot flatten the rest. Verdict: air warming `3.1 * loss_W / flow_m3h`: up to 5 K adequate, up to 10 K low, more very low, no flow none. Planning limits, not a standard; warming ignores wall losses (high side).

Conflicts to expect: #1599 (view3d.rs), heat-hang branch (heat_3d.rs, heat_legend.rs, view3d.rs). Hunks are small.
QA: scenario 1610. Pictures: /mnt/project-files/qa/flow-calm/.
