# EMC magnetic field estimate

Where to find it: **Check > EMC check > Magnetic field estimate** (settings, the current in each wire, the field at each PLC or analog module, the voltage in each signal wire), the **Exposure map** layer of the EMC view on the Cabinet plate and in 3D (the field slice, with its microtesla scale in the Legend), and findings **EMC-19** and **EMC-20** in the Issues list once the EMC check is on. Engine: `crates/novo-emc/src/field.rs`. Plan: `docs/EMC.md` (levels, phase E4).

It is a model. Every number carries the level **Estimate** and says "not a measurement". It never says a design complies.

## What it computes

1. **Currents.** A wire has a current when the user states one (`emc.current_a` on the wire or its cable, or the number field in the EMC check window) or, for motor and power wires, when a device at an end has a rated current (`rated_current_a`, the smaller of the two ends is used and the source line says "planning value: the smaller rated current"). The phase comes from `emc.phase_deg`, else the pin name (U/V/W, L1/L2/L3, T1/T2/T3, contactor poles 1 to 6; pins "1" and "2" both read as phase 0, so a single-phase go and return on those adds instead of cancelling, which is conservative), else a DC potential (return = 180 degrees), else the core order of a cable (2 cores: 0 and 180; 3 cores: 0, -120, 120). A wire with no phase is taken as a lone conductor, which is the worst case, and the table says so.
2. **Field.** Each routed wire is a chain of straight segments at the wire height above the plate. The flux density is the exact Biot-Savart field of a finite straight segment, summed as complex phasors, so a balanced three-phase bundle or a go-and-return pair cancels as drawn and a lone conductor does not. RMS value in microtesla.
3. **Wires on one line.** The router gives every wire of a duct the same centre line. They are laid side by side, one wire diameter apart in id order, before the field is computed (stacked balanced phases would cancel to exactly zero).
4. **Plate.** Optionally a perfect conducting plane (image current of opposite sign below it). Default: on from 1 kHz up, off below (a thin steel or galvanised plate hardly screens 50 Hz). The step estimate of EMC-20 always uses the plane: a steep edge is megahertz content. Set `emc.plate_image` to override.
5. **Voltage in a signal wire (EMC-20).** The flux through the victim loop is the loop integral of the vector potential of the same segments (Neumann), so no area sampling is needed. The loop is go and return when both are drawn (two cores of one cable), else the wire above the plate with the plate as its return. Induced RMS voltage at the line frequency is 2 pi f Phi; the step value is the phasor-summed flux of all motor wires as drawn divided by the largest current (a bundle reads about a fifth of a lone wire; this assumes all motor phases step together at the stated rate), times the assumed current step (`emc.didt_a_per_us`, shared with EMC-02). Twisting multiplies the flux by `emc.twist_factor` (0.1).
6. **Map.** The flux density on a grid over the plate at the slice height (default 30 mm), drawn on a log colour ramp with the planning value at the middle: a tenth of it at the bottom, ten times at the top.

## Settings (project properties)

| Property | Default | Meaning |
| --- | --- | --- |
| `emc.field_hz` | 50 | frequency of the currents, Hz |
| `emc.field_limit_ut` | 12.6 | planning value at a sensitive device, microtesla (10 A/m; a house value, the power-frequency field immunity levels of IEC 61000-4-8 are the yardstick, levels not verified) |
| `emc.induced_limit_mv` | 10 | planning value in the most sensitive signal loop, millivolt (analog; divided by the class sensitivity for the others) |
| `emc.field_slice_mm` | 30 | height of the slice above the plate |
| `emc.twist_factor` | 0.1 | share of the flux a twisted pair keeps |
| `emc.plate_image` | auto | `true` or `false` |
| `emc.wire_height_mm` | 15 | shared with the other EMC estimates |

## Validation (unit tests in `field.rs` and `tests/field.rs`)

| Case | Closed form | Test |
| --- | --- | --- |
| Long straight wire, 10 A at 0.1 m | mu0 I / (2 pi d) = 20 uT | within 0.01 % |
| Finite wire at its middle | mu0 I / (4 pi d) 2 sin(theta) | within 1e-9 |
| Twin pair 10 mm apart, 10 A | mu0 I / (2 pi) (1/(D - s/2) - 1/(D + s/2)); falls as 1/D^2 | 0.1 %; doubling the distance gives 0.25 |
| Square loop, centre | 2 sqrt(2) mu0 I / (pi a) | within 1e-9 |
| Square loop, far on its axis | dipole mu0 I A / (2 pi z^3) | within 0.5 % |
| Three phases on a 10 mm circle | 1/D^2 fall, far below one wire | ratio 0.25 within 2 % |
| Wire over a conducting plane | surface field doubles, normal part zero | within 1e-6 |
| Flux in a rectangle beside a wire | mu0 I l ln(d2/d1) / (2 pi) | within 0.01 % |
| Wire over plate, victim returning through the plate | the level 2 formula `mutual_per_m` (EMC-02) | within 0.2 % |
| Faraday | V = 2 pi f B A | within 0.1 % |

Printed numbers for a lab fixture (25 A drive, three phases in a duct, PLC 30 mm from the duct line) are in `crates/novo-tests/tests/emc_field_fixture.rs` and `qa/fixtures/emc-field.novo`.

## What it does not do

Differential-mode current only. No common-mode current (the real motor cable problem), no capacitive coupling, no eddy currents in a steel plate at mains frequency, no screening by the door or cabinet, no riser wires between plate and device, no shield effect at low frequency (a shield bonded at both ends helps from some kHz up; it is not credited here). The router's lanes are a layout model, not measured positions. Frequencies above 30 MHz are out of band. The planning values are house values.
