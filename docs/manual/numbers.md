# Model or exact? Reading the numbers

Some numbers in KoperCAD are exact (counts, tags, lengths you typed, a parts list). Others come from a model: they are the best estimate the program can make, labelled as such wherever they appear. This page says which is which. In the program the same list is under Help > **About the numbers**.

| Number | Label in the program | What to know |
| --- | --- | --- |
| Cable and wire ampacity, voltage drop tables | planning value | From published knowledge, not checked against the licensed standard text. Confirm the clause in your copy before relying on a number. |
| PFHd, performance level | model, not exact | PFHd for categories 2 to 4 is a closed-form estimate. The ISO 13849-1 Table K.1 values are not used. Check functions that matter with SISTEMA or the standard. |
| Heat, airflow, device temperatures | model, not validated | A physical model with a stated accuracy. |
| Motor start speed and current | estimated | Standard induction-motor model from the nameplate. |
| Wire lengths before routing | ~ estimate | Distance on the schematic plus service loops. |
| Lengths of wires to the door | ~ door, no gap | Leaves out plate-to-door distance and hinge loop. |
| Standards checks | subset, not a certification | Rule checks on the model covering a subset of the standard. |

## Heat and airflow

The cabinet heat view solves heat conduction and air flow on a grid (2D in the Cabinet workspace, 3D in the 3D view). The solvers are tested against published benchmarks: lid-driven cavity, the heated cavity, natural convection on a plate, and analytic cases. The details and the numbers are in `docs/thermal-validation.md`.

What that does and does not mean:

- The flow and heat core matches the benchmarks closely (a few percent at worst).
- The whole enclosure result is **not** yet validated against a measurement or IEC 60890. In a sealed enclosure the 3D mean air rise has been about 13% below the IEC 60890 hand calculation. With a fan, the heat balance is only good to about 3% on one window; that is the figure in the program's own label (Help > About the numbers), and the fan-into-a-vent flow in `docs/thermal-validation.md` is within 3% too.
- The 2D mean air temperature with a fan is a low estimate against the 3D result.
- A heat run that stops early says so (**stalled**).

Use heat to compare layouts and to find hot spots. Do not use it as the only evidence that a cabinet stays below a limit.

## Short circuit and protection

The short-circuit network follows the IEC 60909-0 method (c factors, impedance corrections, motor contribution) and is checked against hand calculations in the tests. A published IEC 60909-4 worked example is not reproduced yet. Time-current curves use the test points of IEC 60898-1, 60947-4-1 and 60269-1 with straight lines between them on log-log axes. Back-up protection can only come from a maker table you load. Nothing here certifies a design; the engineer certifies.

## Ampacity

The ampacity tables are built-in values for copper PVC and XLPE cables from 1.5 to 120 mm², installation methods B1, B2 and C. They were entered without access to the standard; public sources agree on most cells and disagree on others, so no cell is claimed as checked. Each report prints the table version and says how many columns are unverified. A licensed copy of the standard lets you load your own table.

## Functional safety

PFHd and performance level for categories 2 to 4 use a closed-form estimate, not the ISO 13849-1 Table K.1 values. For Category B and 1 the numbers follow from the inputs. Treat the result as a first look, and use SISTEMA or the standard's tables for the function you will certify.

## Motor start

The start curve is estimated from the nameplate with a standard induction-motor model. The first-cycle inrush peak and the spike when a star-delta starter re-closes are not shown.

## Wire lengths

Until devices are placed and wires routed, a length is the distance on the schematic plus the service loops, marked **~**. After **Route all** the length is measured along the duct path. A wire that ends on the door is marked **~ door**: add the gap to the door and the hinge loop. Measure on the plate when it matters.

## Standards checks

The standards check covers a subset of IEC 60204-1, NFPA 79 and UL 508A as rule checks on the model. A pass means those rules found nothing; it is not a certification. The clause numbers are given as understood; check them in your licensed copy.
