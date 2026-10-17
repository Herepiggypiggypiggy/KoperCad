2026-10-16 08:05 UTC Thread started from the spawn table; #561 skeleton on main, #575 flow core in review.
2026-10-16 09:00 UTC Turbulence engine and channel validation written; clippy clean. Plate benchmark waits for the flow core hooks.
2026-10-16 09:10 UTC #578 merged after R1 READY. CFD-A hooks #650 merged.
2026-10-16 11:30 UTC Coupling object, plate benchmark (SST +4.3 % / +9.8 %, k-eps +34.5 %, coarse walls -52 %), PR 2 opened.
2026-10-16 12:15 UTC #669 merged. PR 3: solid mask from the medium, zero-alpha guard.
2026-10-16 13:00 UTC PR 4: turbulence option in CabinetSolver (`with_turbulence`), snapshot `turb_visc_ratio`, accuracy text; vented box study (see HANDOFF).
2026-10-17 09:40 UTC #694 merged (13:30 on 10-16). Doc PR: R1 corrections, exhaust print in turb_vented. Next: second-order convection for k and omega.
2026-10-17 11:00 UTC PR 5: second-order k/omega option, film faces and cut cells kept out of the turbulence heat hooks (device 35 % too cold / 4x too hot found), vented table redone: turbulence changes the vented box by under 3 %.
2026-10-17 12:30 UTC #1015 merged. Looked at a buoyancy-aware wall function: low payoff for the cabinet (enclosure walls use the Overall BC and devices the film model, so the turbulence wall alpha barely reaches the result); kept as documented limit. Accuracy text now says so. Draft only until the release candidate is tagged (coordinator).
