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
| Sealed enclosure 50 W, mean air rise (fixed in #675 with ThermalBc::Overall; re-measured 2026-10-17 on main) | IEC 60890 hand calc 6.29 K, 2D engine 6.10 K | 3D volume-mean air rise | 5.48 K on 1000 cells, 5.38 on 1800: -12.8% and -14.4% vs IEC, -10.1% and -11.8% vs 2D (steady path 5.478 and 5.380 K); heat imbalance at most 0.1%. The 3D mean is lower because the air is stratified; test band 20% |
| Vented box 100 W, 1000 then 1800 cells (CFD-C2, `validation_vented.rs`) | 2D engine, mixed air at the vents | air rise seen by the walls (wall-conductance weighted) | +4.5% then +6.7% (model vs model, not a measurement) |
| Same vented box | 2D engine | volume-mean air rise | -14.6% then -14.8%; does not flatten with the mesh (stratification, see below) |
| Same vented box | 2D engine | exhaust air rise (plume over the device) | +22.2% then +21.2% (11.2 and 11.1 K vs 9.15 K) |
| Same vented box | 2D engine | vent flow | -11.2% then -13.5% (3D 0.00320 and 0.00312 vs 0.00361 m3/s) |
| Same vented box | 2D engine | heat leaving through vents | +8.5% then +4.8% (42.5 and 41.0 W vs 39.1 W); heat imbalance +0.2% |
| Single device 50 W in a sealed cabinet, 700 / 1000 / 1400 cells (CFD-C2, `validation_device.rs`) | 2D engine | housing rise | +4.5% / +5.1% / +5.1% (60.2 K vs 57.3 K); plate share 0.264 to 0.266 vs 0.268 |
| Full cabinet 50 W, 1500 / 6000 / 20000 cells | 2D engine | housing rise | +5.1% / +10.7% / +14.3%: creeps up with the mesh, not mesh independent; test limit 20% |
| Cabinet solver as the app runs it (`tests/validation_cabinet.rs`, CFD-INT), sealed 50 W and vented 100 W | 2D engine (and IEC 60890 for the sealed case) | mean air rise; housing rise; plate share | 1500 to 6000 cells: air -11 to -19% (IEC sealed -13 to -14%), housing +5 to +11%, plate share within 0.011. 20000 cells: air -16% sealed and -31% vented, housing +13 to +14%, plate share within 0.022. Not flat with refinement, so `ENCLOSURE_VALIDATED` stays false. Bands in the test: 20% (35% at 20000) air, 15% (20%) housing, 0.03 plate share |
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
| Cabinet with several devices and vents against the 2D engine | own test in a later PR (single device, full cabinet and one vented box are done) |
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
