# Progress
- 2026-10-19: cloud, heat flow, flux engine, legend notes, tests (heatview flux, heat_cloud, heat_carry, legend). Headless only.
- 2026-10-21: verdict sentence at the top of the 3D Heat panel (`heat_verdict.rs`, device name selects the device); progress pill shows an animated "working" bar instead of 0 % until the residual falls, label shortened ("it 45") to stop the wrap. Not reproduced headless: panel over the view cube at 1920x1080 (panel is anchored under the cube; check on a real screen). Open from the creative audit: pinned hot-spot labels in 3D and the what-if drag. Open nits from Reviewer A on #1073: label the probe "heat" as carried by the air; test cloud_layers.
- 2026-10-21: #1032 merged. Cloud layer count (4 to 40, default 16) in the Heat strip; probe readout adds the heat the air carries (W/m2, up/down).
- 2026-10-21: probe line now reads 'air carries N W/m2' (Reviewer A nit on #1073); test that the cloud layer count changes what is drawn.
