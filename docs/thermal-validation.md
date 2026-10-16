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
| Sealed enclosure 50 W, mean air rise | IEC 60890 hand calc 6.29 K, 2D engine 6.10 K | 3D mean air rise | **fails**: 14.5 K on 6000 cells, 20.0 K on 1500 (+130% to +218%), see below |
| Transient slab, dt = tau/200 and tau/400 | Fourier series | centre temperature at 0.1 tau | +2.34% and +1.19% (first order in dt) |

## What this says

- Flow and natural convection: Nusselt within 0.7% of the benchmark at every Ra tested; peak velocities within about 3% (worst +3.06%).
  Mesh convergence is clean second order (p = 1.95), so the error is discretisation, not a bug.
- The solver is axis independent (turned problem, extra depth cells) and conserves heat.
- Conduction in solids is exact to rounding for linear profiles and second order for sources.
- Transient accuracy is first order in time (backward Euler). Heat-up simulations must pick the
  step by the time scale, not only by CFL. The transient slab shows 1.2% at tau/400.
  Reported to CFD-A as a design note, not a defect: steady results are unaffected.

## Open finding: sealed-enclosure air rise is too high

The 3D engine and the IEC 60890 / 2D estimates disagree on a sealed 600x800x300 steel wall enclosure with 50 W
(`tests/validation_enclosure.rs`, ignored until fixed). The heat balance is fine (0.10%), the temperature is not:
the 3D mean air rise is 2.3 to 3.2 times the IEC value. Probable cause, reported to CFD-C2: the IEC coefficient k is
an overall air-to-air value (both films included), but the solver applies it as the wall film and also resolves the
air-to-wall convection inside, so the inner film is counted twice. The result also depends on the mesh (14.5 K at
6000 cells, 20.0 K at 1500), which is expected when the inner boundary layer is not resolved. Until this is fixed, 3D
enclosure temperatures must not be quoted as validated; the accuracy statement in code says so under its limits.
The test runs about 55 s at 1500 cells and 10 min at 6000 cells in release.

## Caveats on single rows

- Plate: a 2D slice with the far opening one plate height away, so the result depends somewhat on that domain
  width (not quantified). Two meshes show a trend, not a measured order.

## Not yet validated (blocked, tracked here)

| Case | Blocked on |
| --- | --- |
| Cabinet temperature rise against IEC 60890 hand calculation and the 2D engine | the open finding above |
| Chimney against the 2D formulas (CFD-C2 reports a 40% band, laminar) | own test in a later PR |
| Turbulent channel and cavity | turbulence (CFD-B) |
| Richardson study on a real cabinet scenario | C1 meshing; the cavity study stands in |

Each becomes a `validation_*` test when its lane lands; the sign-off PR for a lane adds its rows here.

## Accuracy statement (text the app may quote today)

> Laminar natural convection and conduction in closed boxes reproduce published benchmarks
> (Ghia 1982, de Vahl Davis 1983) to within 1% in heat transfer and about 3% in peak velocity on
> meshes of 50 to 60 cells per side. Mesh convergence is second order. This build has not yet
> been validated for open vents, fans, turbulence or radiation; results for a cabinet are
> estimates and carry the uncertainty of the input loss data and boundary conditions.

Model limits are those of design section 7.
