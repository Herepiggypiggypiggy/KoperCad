# Thermal CFD: the full solve behind the instant estimate

Scope change (Tadjiro, 2026-10-15): not a reduced-order model alone. Real 3D
CFD with conjugate heat transfer, local, Rust, no admin rights, on a normal
laptop. The 2D engine of `thermal-simulation.md` (PR #505) stays as the
**instant preview and the starting guess**; the CFD refines it in the
background (progressive fidelity) and never blocks the UI.

## 1. Method: finite volume, not lattice Boltzmann

| | Finite volume (FV) | Lattice Boltzmann (LBM) |
| --- | --- | --- |
| Buoyancy and heat | Boussinesq, energy equation, standard | needs a second distribution, weakly compressible, small Mach limits make steady natural convection slow |
| Turbulence | k-epsilon / k-omega SST with wall functions are textbook | LES-lite only; wall functions are awkward |
| Conjugate solids | conductivity field, harmonic face means, exact | possible, less standard |
| Steady solve | SOR/multigrid, big steps | time-marching only; thousands of steps for natural convection |
| GPU | needs care (multigrid) | very natural |
| Validation | Ghia, de Vahl Davis tables apply directly | same, but accuracy of walls needs care |

**Decision: FV on a Cartesian staggered (MAC) grid**, fractional-step
projection, implicit diffusion, TVD/QUICK convection, geometric multigrid
preconditioned CG for pressure. Reasons: steady natural convection and
conjugate heat transfer are its home ground, wall-function turbulence is
standard, and the benchmarks are directly comparable. A wgpu compute path
(LBM or multigrid smoothers) stays an optional later lane.

## 2. Model

- Incompressible Navier-Stokes, Boussinesq buoyancy `rho g beta (T - Tref)`,
  energy equation with `k_eff = k + cp mu_t / Pr_t`.
- Turbulence: k-omega SST (default), k-epsilon as cross check, wall functions
  on solid cells. Laminar mode for the benchmarks.
- Meshing: Cartesian grid from the cabinet model, immersed boundaries by cell
  classification with volume fraction (cut-cell lite); non-uniform grading
  towards walls and devices; target 0.5 to 3 million cells on a laptop.
- Solids in the same grid with their own conductivity, density, cp:
  device bodies (volumetric heat source), plate, walls, rails, duct walls,
  conjugate heat transfer through harmonic-mean face conductivity. Walls lose
  heat to ambient with the IEC film coefficient on the outside.
- Openings: fans as momentum source / pressure jump with the maker curve;
  vents and louvres as open boundaries with loss coefficient or porous zones;
  slotted ducts as anisotropic porous media.
- Radiation: surface-to-surface between solid faces, grey, view factors by ray
  sampling on a coarse patch set, solved by radiosity (coupled to the
  conduction at solid faces).
- Steady (pseudo-time to convergence) and transient (heat-up, load steps).

## 3. Progressive fidelity

1. Instant (ms to 1 s): the 2D estimate of `novo-thermal` (existing).
2. Coarse 3D (seconds): about 50 000 cells, initialised from the 2D fields.
3. Medium (tens of seconds): about 500 000 cells, grid sequencing (prolongate
   the coarse solution).
4. Fine (minutes): 1 to 3 million cells, turbulence and radiation on.
Each level publishes a `Snapshot` (below) when it improves; the UI shows the
latest with a fidelity badge and convergence indicator. Cancel is instant (an
atomic flag checked every iteration); worker threads never touch the UI.

## 4. Result contract (agreed with the visuals thread)

All in the interior frame of `novo-thermal` (mm, `y` down) plus `z` from the
plate towards the door. Fields are on one `Grid3`; cell data row-major
`(k * ny + j) * nx + i`. Types land in `novo-thermal::cfd::result` (additive
to `ThermalResult`).

```rust
pub struct Grid3 { pub nx: usize, pub ny: usize, pub nz: usize,
    pub x_edges_mm: Vec<f64>, pub y_edges_mm: Vec<f64>, pub z_edges_mm: Vec<f64> }
pub enum Fidelity { Preview2d, Coarse, Medium, Fine }
pub struct Progress { pub fidelity: Fidelity, pub iteration: usize,
    pub residuals: Residuals, pub converged: bool, pub elapsed_s: f64,
    pub energy_balance_rel: f64, pub monitor_drift_k: f64 }
pub struct CfdSnapshot { pub grid: Grid3,
    pub temperature_c: Vec<f32>, pub velocity: Vec<[f32; 3]>, // m/s, y down
    pub pressure_pa: Vec<f32>, pub turb_visc_ratio: Vec<f32>,
    pub material: Vec<u16>,  // 0 = air, else solid id from `materials`
    pub materials: Vec<MaterialInfo>, // device id, plate, wall, duct...
    pub wall_flux_w_m2: Vec<f32>, // on solid faces, for heat flux maps
    pub devices: Vec<DeviceResult>, // same type as the 2D result
    pub hot_spots: Vec<HotSpot>, pub progress: Progress,
    pub notes: Vec<String>, pub accuracy: AccuracyStatement }
```
Helpers: `slice(plane, offset)` (scalar and vector slices for the 2D canvas),
`streamlines(seeds)` in 3D, `iso_surface(level)` for the 3D view. The 2D
`ThermalResult` stays for the preview; `CfdSnapshot::as_2d()` produces one
(plate-adjacent or depth-averaged slice) so existing drawing works at once.

## 5. Validation (hard, reported honestly)

Tests and a report table with the error numbers, not only pass/fail:
1. Lid-driven cavity, Re 100 and 1000: centreline u and v against Ghia, Ghia
   and Shin (1982).
2. Differentially heated square cavity, Ra 1e3 to 1e6: Nusselt and extreme
   velocities against de Vahl Davis (1983).
3. Natural convection from a heated vertical plate: local Nusselt against
   Churchill-Chu / laminar similarity.
4. Enclosure temperature rise against the IEC 60890 hand calculation and the
   2D engine; energy balance residual below 0.1 percent.
5. Conjugate: slab with heat source, analytic 1D conduction through layers.
6. Fan and chimney cases against the same formulas as the 2D tests.
7. Mesh convergence: Richardson estimate on a cabinet case.
The accuracy statement shown to the user quotes the benchmark errors of the
current build and states what the cabinet model leaves out.

## 6. Sub-lanes (proposed threads)

| Lane | Scope | Depends on |
| --- | --- | --- |
| A. Flow core (this thread) | 3D MAC grid, projection, Boussinesq, multigrid pressure, benchmarks 1 to 3 | none |
| B. Turbulence | k-omega SST, k-epsilon, wall functions, turbulent benchmarks (channel, turbulent cavity) | A |
| C. Meshing and conjugate solids | scenario to Cartesian mesh, cut-cell fractions, grading, solids, CHT, openings, fans, porous | A |
| D. Radiation | surface-to-surface view factors, radiosity coupling | C |
| E. Performance and scheduling | rayon threading, progressive scheduler on a background thread, cancel, optional wgpu compute with CPU fallback | A |
| F. Results and visuals | `CfdSnapshot` slices, streamlines, iso-surfaces, 3D view, convergence badge | agreed types above |
| G. Validation and report | benchmark suite, Richardson study, accuracy statement | A, C |

Needs from the integration lead: crate `novo-cfd` (pure engine, new crate,
depends on novo-thermal for shared types) and `rayon` in
`[workspace.dependencies]`.

## 7. Honest limits

Cartesian immersed boundaries are first-order at curved or tilted faces;
wall functions assume attached flow; radiation is grey and surface-only;
meshing and turbulence inputs for a real cabinet carry uncertainty that no
solver removes. The result says so.
