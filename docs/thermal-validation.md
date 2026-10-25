# Thermal solver validation

Owner: lane CFD-G. Design: `docs/design/thermal-cfd.md` section 5. Numbers below are
from the `novo-cfd` flow core at main `a70747b` (2026-10-16). Refresh with

```
cargo test -p novo-cfd --test 'validation_*' --test flow_cavity --test flow_heated_cavity -- --nocapture
```

The `validation_*` tests print `VALIDATION | case | quantity | value | reference | error` rows.
Errors are signed, relative to the reference unless marked abs.

## Results

| Case | Reference | Quantity | Error |
| --- | --- | --- | --- |
| Lid cavity Re 100, 48x48 | Ghia, Ghia, Shin 1982 | centreline u / v, rms (lid speed 1) | 0.0024 / 0.0050 (max 0.0050 / 0.0092) |
| Lid cavity Re 1000, 48x48 stretched | same | centreline u / v, rms | 0.0030 / 0.0041 (max 0.0102 / 0.0067) |
| Heated cavity Ra 1e3, 48x48 | de Vahl Davis 1983 | Nu / u_max / v_max | +0.55% / +0.16% / +2.97% |
| Heated cavity Ra 1e4, 32x32 | same | Nu / u_max / v_max | +0.62% / +0.68% / +0.99% |
| Heated cavity Ra 1e5, 40x40 | same | Nu / u_max / v_max | +0.15% / +3.06% / +0.94% |
| Heated cavity Ra 1e6, 56x56 | same | Nu / u_max / v_max | +0.29% / +2.19% / +1.50% |
| Cavity Ra 1e4 mesh study 12, 24, 48 | de Vahl Davis Nu 2.243 | Nu | +2.28%, +0.82%, +0.44% |
| Same, Richardson | | observed order / extrapolated Nu / GCI(fine) | p 1.95 / 2.2499 (+0.31%) / 0.17% |
| Cavity turned 90 degrees | itself | Nu difference | below 1e-6 |
| Cavity 4 cells deep vs 1 | itself | Nu difference | below 1e-6 |
| Cavity centro-symmetry | exact | max abs T(x,y)+T(1-x,1-y), dT = 1 | 5.9e-9 |
| Steady slab, fixed walls | linear profile | max error, dT 60 K | below 1e-6 K |
| Slab with heat source, 10 / 20 / 40 cells | parabola | mean temperature rise | +2.0% / +0.5% / +0.125% (second order) |
| Heat balance of a source | generated = wall flux | ratio error | below 1e-6 |
| Heated vertical plate in open air, Ra_H 5.2e7, Pr 0.705, 24x40 then 36x60 | Churchill-Chu 1975 (laminar) | mean Nu | +7.8% then +3.9% (correlation itself good to about 5%) |
| Same plate, other Ra (lane CFD-A, #665, `flow_plate.rs`) | Churchill-Chu 1975 | mean Nu | Ra 1e5, 40x40: -1.8%; Ra 1e6, 32x32: +1.2% (tolerance 4%); not a 1% accuracy claim, the correlation is good to about 5% |
| Layered conduction, 3 layers with different k (lane CFD-C2, `solids.rs`) | series resistance | max temperature error | below 1e-5 K |
| Sealed enclosure 50 W, heat balance (CFD-C2) | generated = wall heat | imbalance | +0.10% |
| Fan into a vent (CFD-C2, `openings.rs`) | analytic operating point | flow | within 3% (test limit) |
| Porous slab as fan load (CFD-C2, `ducts.rs`) | exact operating point | flow | -0.04% and -0.03% at two time steps |
| IEC 60890 wall films (CFD-C2) | k A_eff | conductance sum | below 1e-9 relative |
| Chimney, 10 / 40 / 160 W, 8x16x8 cells (`validation_chimney.rs`; default test runs 40 and 160 W, 10 W measured once by hand) | ideal fully mixed stack law | flow | -14.5% / -14.4% / -14.5%; heat carried out within 0.1% of the source |
| Chimney 40 W, 12x24x12 cells (ignored test, run with `--ignored`) | ideal stack law | flow | -10.8% (the finer mesh moves the flow +4.3% toward the law); scaling Q ~ P^(1/3) holds to 0.1% |
| Vented cabinet, 3 devices (40, 60, 20 W), louvre low and high, 1800 / 3500 cells (`validation_cabinet_vents.rs`; 3500 is an ignored test) | 2D engine | housing rise per device | device 1 +4.7% / +5.5%, device 2 (hottest in both engines) +4.1% / +5.4%, device 3 +4.8% / +4.3%; plate shares 0.25 vs 0.26; heat imbalance +0.2% (model vs model, not a measurement) |
| Sealed enclosure 50 W, mean air rise (fixed in #675 with ThermalBc::Overall; re-measured 2026-10-17 on main) | IEC 60890 hand calc 6.29 K, 2D engine 6.10 K | 3D volume-mean air rise | 5.48 K on 1000 cells, 5.38 on 1800: -12.8% and -14.4% vs IEC, -10.1% and -11.8% vs 2D (steady path 5.478 and 5.380 K); heat imbalance at most 0.1%. The 3D mean is lower because the air is stratified; test band 20% |
| Vented box 100 W, 1000 then 1800 cells (CFD-C2, `validation_vented.rs`) | 2D engine, mixed air at the vents | air rise seen by the walls (wall-conductance weighted) | +4.5% then +6.7% (model vs model, not a measurement) |
| Same vented box | 2D engine | volume-mean air rise | -14.6% then -14.8%; does not flatten with the mesh (stratification, see below) |
| Same vented box | 2D engine | exhaust air rise (plume over the device) | +22.2% then +21.2% (11.2 and 11.1 K vs 9.15 K) |
| Same vented box | 2D engine | vent flow | -11.2% then -13.5% (3D 0.00320 and 0.00312 vs 0.00361 m3/s) |
| Same vented box | 2D engine | heat leaving through vents | +8.5% then +4.8% (42.5 and 41.0 W vs 39.1 W); heat imbalance +0.2% |
| Single device 50 W in a sealed cabinet, 700 / 1000 / 1400 cells (CFD-C2, `validation_device.rs`) | 2D engine | housing rise | +4.5% / +5.1% / +5.1% (60.2 K vs 57.3 K); plate share 0.264 to 0.266 vs 0.268 |
| Full cabinet 50 W, 1500 / 6000 / 20000 cells | 2D engine | housing rise | +5.1% / +10.7% / +14.3%: creeps up with the mesh, not mesh independent; test limit 20% |
| Cabinet solver as the app runs it (`tests/validation_cabinet.rs`, CFD-INT), sealed 50 W and vented 100 W | 2D engine (and IEC 60890 for the sealed case) | mean air rise; housing rise; plate share | 1500 to 6000 cells: air -11 to -19% (IEC sealed -13 to -14%), housing +5 to +11%, plate share within 0.011. 20000 cells: air -16% sealed and -31% vented, housing +13 to +14%, plate share within 0.022. Not flat with refinement, so `ENCLOSURE_VALIDATED` stays false. Bands in the test: 20% (35% at 20000) air, 15% (20%) housing, 0.03 plate share |
| Cooling what-if fixes, 3D against 2D (`validation_whatif.rs`, 3000 cells): sealed 600 x 800 x 300 mm box, one 120 W device at 35 C outside, the what-if's vent and fan fixes | 2D engine | housing temperature; housing K gained; order of the fixes | housing +4.9% as it is, -1.0% with filter fan in and vent out, +4.9% with two vents; K gained: vents +5.3% (6.5 K vs 6.2 K), fan pair +59% (24.2 K vs 15.2 K: the 2D model read the fan benefit low, before the jet speed term; see "Fan gain: the jet speed term"); 3 of 3 pairs more than 3 K apart in 2D are ordered the same in 3D (model vs model, not a measurement) |
| Fan gain sweep, 3D against 2D (`validation_fan_gain.rs`, ignored test, 3000 cells, 36 cases, about 18 minutes in debug): fan 60 / 120 / 250 m3/h, 60 / 120 W, boxes 600x800x300, 800x1200x300 and 600x600x400 mm, fan low in with vent on top or fan on top with vent low | 2D engine | housing K gained by the fan fix, 3D / 2D (3D at 3000 cells; its gain rises 9 to 11% from 3000 to 12000 cells, see below) | mean 1.90, median 1.89, range 0.94 to 3.09; the 2D model reads the gain low against the 3D solver in 33 to 34 of 36 cases (the three 600x600x400 cases at 60 m3/h are within about 0.1 of 1). By flow: 60 m3/h 0.94 to 2.07 (mean 1.46), 120: 1.36 to 2.56 (1.90), 250: 1.68 to 3.09 (2.35). By box: 600x800x300 mean 1.94, 800x1200x300 mean 2.34, 600x600x400 mean 1.42. The two fan positions agree within 0.25. No single factor fits (model vs model, not a measurement) |
| Transient slab, dt = tau/200 and tau/400 | Fourier series | centre temperature at 0.1 tau | +2.34% and +1.19% (first order in dt) |

## What this says

- Flow and natural convection: Nusselt within 0.7% of the benchmark at every Ra tested; peak velocities within about 3% (worst +3.06%).
  Mesh convergence is clean second order (p = 1.95), so the error is discretisation, not a bug.
- The solver is axis independent (turned problem, extra depth cells) and conserves heat.
- Conduction in solids is exact to rounding for linear profiles and second order for sources.
- Transient accuracy is first order in time (backward Euler). Heat-up simulations must pick the
  step by the time scale, not only by CFL. The transient slab shows 1.2% at tau/400.
  Reported to CFD-A as a design note, not a defect: steady results are unaffected.

## Closed finding: sealed-enclosure air rise (history)

The first run of the sealed-enclosure check (`tests/validation_enclosure.rs`) failed: the 3D mean air rise was
14.5 K on 6000 cells and 20.0 K on 1500 against 6.29 K by IEC 60890 and 6.10 K by the 2D engine, with a correct heat
balance. Cause, confirmed by CFD-C2: the IEC k is an overall air-to-air coefficient but was applied as a wall film on top
of the resolved inner convection. Fixed in #675 (`ThermalBc::Overall`), which also removed the mesh dependence: the
result now lies in a 5.4 to 5.8 K band on 1000 to 6000 cells, not exactly mesh independent. The check is in CI.

## Vented box: known difference to the 2D estimate

The 2D estimate assumes mixed air at the vents. Where a heat source sits below a vent the 3D plume leaves about 20% hotter here (up to 50% in CFD-B's turbulence study, 13.5 K against 8.99 K), the volume-mean air rise is about 15% lower, and the vent flow is 11 to 13%
low, while the walls (+4% to +7% in the wall-seen rise) and the total heat balance agree. This is a model difference, not
a proven error of either side: nobody has measured this box. The wall-seen metric is the honest comparison for
housing temperature; the volume mean is not expected to match. The wall conductance counts the vent openings, so the
wall-seen rise reads a few percent low by construction. The full cabinet row creeps up with the mesh (+5% to +14%),
so the 20% test band is a guard, not an accuracy claim.

## Chimney: model, not exact

The stack law is an idealisation (fully mixed exhaust, no plume), not a reference solution. The solver sits 11 to 15% below it and the finer mesh moves toward it (+4.3%), so the grid dependence is not yet bounded; the exponent and the energy balance are what the law fixes exactly, and both match. Not a measurement of a real cabinet.

## Caveats on single rows

- Turbulence (lane CFD-B, `turb_channel.rs`, `turb_plate.rs`, slow tests run with `--ignored`): channel Re_tau 590, SST resolved
  4.7% log-layer error (centreline U+ 20.69 vs DNS 21.3), SST with wall function 2.7%, k-epsilon 7.1%. Heated wall against
  Churchill-Chu: SST resolved +4.3% (Ra 1.0e9, 40x80) and +9.8% (Ra 3.4e9, 28x56); k-epsilon on the resolved grid +34.5% (not
  made for resolved buoyant layers); coarse wall cells (16x48): SST and k-epsilon agree with each other but are 52% low.
  Hot/cold wall flux mismatch in these runs is 5%. CFD-B's solid mask (#679) is merged: turbulence now treats solid cells of the medium as walls. No validation row covers turbulent flow with solids yet, so that combination is untested here.
- Plate: a 2D slice with the far opening one plate height away, so the result depends somewhat on that domain
  width (not quantified). Two meshes show a trend, not a measured order.

## Not yet validated (blocked, tracked here)

| Case | Blocked on |
| --- | --- |
| Richardson study on a real cabinet scenario | C1 meshing; the cavity study stands in |

Each becomes a `validation_*` test when its lane lands; the sign-off PR for a lane adds its rows here.

## Accuracy statement (text the app may quote today)

> Laminar natural convection and conduction in closed boxes reproduce published benchmarks
> (Ghia 1982, de Vahl Davis 1983) to within 1% in heat transfer and about 3% in peak velocity on
> meshes of 50 to 60 cells per side. Mesh convergence is second order. A heated plate in open air
> is within about 4% of the Churchill-Chu correlation (itself good to about 5%); a sealed enclosure
> lies up to 15% below the IEC 60890 estimate (stratified air). Vents and fans match analytic operating points;
> in a vented box the air leaving over a heat source is about 20% hotter and the flow 11 to 13% lower than the 2D estimate,
> while housing temperature agrees within about 7%.
> Turbulence is validated only on a channel and a heated wall, and wall functions are not valid for
> thin natural-convection layers. Radiation is not yet validated. Results for a cabinet are
> estimates and carry the uncertainty of the input loss data and boundary conditions.

Model limits are those of design section 7.

## Reduced model for large cabinets (lane CFD-LARGE)

A cabinet whose full grid is over 1.5 times the level budget runs as a reduced model (`novo-cfd/src/large`, see its module doc). Model against model, not against a measurement; `ENCLOSURE_VALIDATED` stays false.

| Case | Quantity | Result |
| --- | --- | --- |
| Drive 60 W and 3 cold devices side by side, reduced (5.8 k cells) against full (9.4 k cells), `tests/large.rs` | housing rise of the drive / air rise round it | -0.2 % / -1.0 % (test limits 5 % and 15 %) |
| Same, reduced at 3.7 k cells | air rise round the drive | -30 % (the air next to a hot device depends on the grid; at 10.5 k cells it is +10 %) |
| Same, cold devices | air round them | 0.7 to 1.8 K above the full run (test limit 3 K) |
| CV-200 (115 devices, 460 W, planning losses), Coarse / Medium / Fine | cells, time (4 cores), converged | 6.5 k, 36 s / 19.6 k, 44 s / 47.6 k, 6 min; heat imbalance 0.0 / 0.3 / 0.1 % |
| Same | drive housing temperature / margin to its limit | 89 / 95 / 86 C and -7.2 / -7.9 / -8.0 K (the 2D model gives about -15 K) |
| Full scenario of the same cabinet at Coarse | cells, result | 1.48 M, not converged in 180 s |
| Example cabinet (7 devices) at Coarse | cells | 199 k full, 12.7 k reduced |

Read with care: the reduced grid is a lumped, shifted copy of the cabinet. Devices inside a lumped block show the block's air temperature and a housing temperature estimated from their own loss and the block's film coefficient, with wide bands. Reproduce: `cargo run --release -p novo-cfd --example large_cabinet -- FILE.novo coarse|medium|fine solve`.

## Fan gain: the 2D model reads it low

This section is the state before the jet speed term of the next section; the sweep row above and the numbers here are the 2D model without it.

The 3D solver is a reduced model, not the truth: its housing temperature is 5 to 14% above the 2D reference, it creeps with the mesh, and the sweep ran at 3000 cells. Against it, the 2D estimate of what a fan fix wins at the housing is lower in 33 to 34 of 36 cases (sweep row in the table above), and in the typical case (median 1.89) the 2D gain is about half the 3D gain. Which of the two is nearer a real cabinet is not known; neither is a measurement. The 3D gain grows with the fan flow, more slowly than the flow and faster than the 2D gain (60 W, 600x800x300 mm: 9.5, 15.4 and 21.4 K at 60, 120 and 250 m3/h, flow 4.2 times, gain 2.25 times); the 2D gain levels off (6.0, 7.4 and 8.7 K), so the gap widens with flow, and it is larger in a taller box and at low loss.

**Mesh.** The sweep ran on one grid. The cabinet solver is not flat with refinement (see the cabinet row above), and so is the 3D gain: 60 W, 600x800x300 mm, 3000 / 6000 / 12000 cells: 15.4 / 16.6 / 17.1 K at 120 m3/h and 21.4 / 22.1 / 23.4 K at 250 m3/h, so the 3D gain rises by 11% (120 m3/h) and 9% (250 m3/h) over the grid steps and the 2D model would read lower still against a finer grid. These mesh figures, and the surface coefficients and air drops in the next paragraph, come from one-off diagnostic runs of single cases that are not kept in the test suite. The conclusion "the 2D gain is low" does not depend on the mesh; its size does.

**Where the gap comes from.** The air temperature next to the device drops about as much in both models (2D 9.6 K, 3D 11.9 K at 250 m3/h, 60 W, first box). The device surface coefficient does not: in 3D it rises from 9.8 to 13.3 W/m2K as the fan jet blows over the device, in 2D it stays at 9.5 to 9.7, because the 2D flow is depth averaged and the surface coefficient sees only that speed. Across the sweep the 2D coefficient moves with the loss only (9.5 to 11.0 W/m2K) while the 3D one grows with the fan flow.

The error is not a constant, so the 2D result is not scaled: a factor fitted to this sweep would be right for these boxes and wrong elsewhere. For the what-if: the ranking has held in every comparison so far (#1483), but the size of a fan fix may be under- or over-stated: check the fix you pick with Simulate in 3D. The fix belongs in the 2D engine (the speed the device surface sees from the fan jet), with its own validation against this sweep. `ENCLOSURE_VALIDATED` stays false.

## Fan gain: the jet speed term

The diagnosis above (the 2D device surface coefficient stays at 9.5 to 9.7 W/m2K while the 3D one rises to 13.3) led to a term in the 2D engine: the fans' stirring is a speed at the device surfaces, `JET_SPEED_COEFFICIENT * fan flow / (inner depth * smaller inner side)` with the coefficient 3.0 (`novo-thermal/src/setup.rs`), combined with the local speed of the depth-averaged flow in the surface coefficient. Without a fan it is zero and nothing changes.

It was fitted by hand on one box (600x800x300 mm, six cases: 60 and 120 W, 60 / 120 / 250 m3/h) and checked on the other two boxes without refitting (`crates/novo-thermal/tests/fan_gain.rs`, the 3D gains frozen from the sweep, the fan low in with the vent on top; the other fan position agrees within 0.25 in 3D):

| 3D gain / 2D gain | before | after |
| --- | --- | --- |
| all 18 cases | 0.94 to 3.09, median 1.89 | 0.69 to 1.28, median 1.00, mean 0.96 |
| 600x800x300 (fitted) | mean 1.94 | 0.91 to 1.02, mean 0.98 |
| 800x1200x300 (held out) | mean 2.34 | 1.00 to 1.28, mean 1.11 |
| 600x600x400 (held out) | mean 1.42 | 0.69 to 0.92, mean 0.80 |
| by flow 60 / 120 / 250 m3/h | means 1.46 / 1.90 / 2.35 | means 0.99 / 1.00 / 0.90 |

So on the held-out boxes the 2D gain is between 22% below and 45% above the 3D gain, the flat-with-flow error is gone (the 2D surface coefficient now rises with the flow, 10.3, 11.5 and 14.0 W/m2K at 60, 120 and 250 m3/h for 60 W in the first box), and the two boxes miss in opposite directions, so the cross-section is not what separates them. One coefficient cannot make all three boxes agree; the test holds each case to the band 0.65 to 1.35 and the fitted box to 10%. This is a fit to a 3D solver that is itself a reduced model, at 3000 cells where its gain rises 9 to 11% with the mesh: the 2D fan gain is now about what that solver says, which is not a measurement. `ENCLOSURE_VALIDATED` stays false. The what-if says so next to a fix with fans.

Limits of the term: it was checked only for one device in a box with a 60 to 250 m3/h fan. The jet speed is linear in the flow and not capped (about 1.2 m/s at 250 m3/h and 3.7 m/s at 800 m3/h in the 600x800x300 box), so a bigger fan, which the what-if ladder offers, is an extrapolation; and in the deeper 600x600x400 box the gain is overstated by up to 45 %, consistently on the optimistic side. Also: the jet speed is the same for every device, whatever its distance from the fan, and with several fans it uses the larger of the intake total and the exhaust total; the fitting and checking cases are one device in a box with one fan. A cabinet with two fans, or a device far from the jet, may be off by more than the band above. Distance from the fan is the next step (`handoff/cooling-whatif/ROADMAP.md`).

## Fan gain: where the device sits

The jet speed term gives every device the same speed. The 3D solver does not: the jet runs from the fan to the vent, and a device beside that line sees much less of it. `crates/novo-cfd/tests/validation_fan_position.rs` (ignored sweep, 3000 cells, 120 W, 35 C outside, fan low in the middle with the vent on top in the middle) moves the device along that line and sideways in four boxes, the fourth (1000x600x250 mm) one that no fit looked at. 3D housing gain over 2D housing gain (1 is agreement, below 1 means the 2D model gains too much), fan 250 m3/h:

| device position | 600x800x300 | 800x1200x300 | 600x600x400 | 1000x600x250 |
| --- | --- | --- | --- | --- |
| on the line, near the vent | 0.99 | 1.13 | 0.64 | 1.32 |
| on the line, middle | 0.88 | 0.29 | 0.51 | 0.82 |
| on the line, near the fan | 1.15 | 1.16 | 0.89 | 0.83 |
| beside the line (0.3 of the width off), left / right | 0.65 / 0.57 | 0.33 / 0.33 | 0.58 / 0.52 | -0.11 / -0.10 |

What this says:
- The fit of the previous section put its device within 25 mm of the line, and there the ratio is the 0.69 to 1.28 of the previous section. Along the line it is not: the 3D gain is 0.2 to 1.3 times the 2D one, so the 2D gain is between 0.8 and 3.4 times the 3D one at 250 m3/h, and up to 5 times at 120 m3/h (the middle of the line in the tall box reads 0.20 there), median 3D/2D about 0.9. The notes in the program say "0.8 to 5 times". Only the fan end (the "near the fan" row of the table) is within the band of the previous section; the middle of the line reads 0.29 to 0.88 and near the vent 0.64 to 1.32.
- Beside the line the 2D gain is one and a half to three times the 3D one in three boxes, and in the wide 1000 mm box the 3D solver shows the device getting slightly hotter with the fan on (about 4 K). The 2D model cannot show that.
- In the tall box the middle of the line is also low (0.2 to 0.3): the jet does not reach that far in the 3D solver at 3000 cells.
- At 120 m3/h (3D over 2D; not pinned by a test, seen once): near the vent 1.11 / 1.12 / 0.86 / (not run for the fourth box), middle 0.99 / 0.20 / 0.71, near the fan 1.26 / 1.27 / 0.86, beside the line 0.76 / 0.67 (first box), 0.63 / 0.71 (tall box), 0.68 / 0.59 (deep box); boxes in the order of the table. So beside the line the 3D solver gave a third to three quarters of the 2D gain (0.33 to 0.76).

A position weight was tried (the code was not kept: it was a throwaway test and the numbers below are not reproducible from the repository): the jet speed times a lateral Gaussian around the fan-to-vent line and an axial fall-off from either opening, with a depth exponent (five parameters, fitted on 48 cases of this sweep and the previous one, `rms ln` of the 3D/2D ratio 0.47 before, 0.29 after). It was not kept: the fit ran the depth exponent to its upper bound, made the cases of the previous section worse (the tall box rose to 1.4 to 1.7), and on the held-out box it helped near the vent (1.01 against 1.32) but made the middle and the fan end worse (0.70 and 0.62 where the merged term gives 0.82 and 0.83) while the beside-the-line cases still read far too high. A fit like that would be tuned to this sweep. What is kept is the finding and the wording: the what-if says a device beside the stream between fan and vent may gain much less than shown. `tests/fan_position.rs` pins this (on the line the median 3D/2D within 0.8 to 1.2; beside it below 0.85) so a change that moves it is noticed. The 3D solver is itself a reduced model, so which side is closer to a real cabinet is not known.
