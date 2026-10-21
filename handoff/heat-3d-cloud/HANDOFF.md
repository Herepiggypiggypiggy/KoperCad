# Handoff: 3D heat cloud and heat flow

Where: Cabinet > 3D view > Heat strip, checkboxes `Cloud` (with opacity slider) and `Heat flow`.
Code: `crates/novo-app/src/app/heat_cloud.rs` (stack of translucent slices, cut across the axis the camera looks along most, far to near; `CloudView::from_eye` keeps the old axis until another leads by 15 %), `heat_carry.rs` (lines, dashes, conduction arrows), engine `crates/novo-heatview/src/flux.rs` (flux field, seeds, lines). `cfd::trace_field` is the shared tracer.
Physics: air `q = rho cp (T - ambient) u`; solids `q = -k grad T` from same-material neighbours; no radiation, no conduction through air. Derived from the solved fields, so labelled "no radiation" in the legend. Air colder than ambient gives flux against the wind (cold inflow).
Cost: about 16 layers x 48 cells, only cells with >= 12 % of the scale make quads; rebuilt only on result, setting or main-direction change. Not yet timed on the Intel HD 5500; only headless tests ran, the pictures were not seen on a screen.

For a later Windows tester (Intel HD 5500, OpenGL only): switch on Cloud and Heat flow together, orbit, and note the frame rate at 16 layers; lower the layers setting (Heat strip) if it stutters, and report the number that is smooth. Probe readout `heat N W/m2` = rho cp |T - ambient| |u| at the point, a model value.
