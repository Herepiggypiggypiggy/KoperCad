# Protection and cable engineering

The calculation core behind the Issues list rows *Cable rating*, *Voltage drop*, *Breaking capacity*,
*Disconnection time*, *Thermal withstand*, *Motor starter* and *Protective conductor*, plus the engines
a designer reaches from the command line today and from the Checks workspace next. All of it is pure
Rust in `crates/novo-checks/src/calc/` (no model, no I/O), so the app, `kopercad-cli` and Python share it.

## What is model, what is exact

Every result is a *planning value* until the tables behind it are checked against the standard in force.
The engine never hides that: each report prints the table version and says how many columns are unverified.

| Part | Status | What the user's licensed copy changes |
| --- | --- | --- |
| Ampacity tables (`calc::tables`) | Built-in values for copper PVC/XLPE, 1.5 to 120 mm², methods B1, B2, C. **Unverified**: entered without the standard; public secondary sources agree on most cells but disagree on column labels in others, so none is claimed as checked. XLPE method B2 with three loaded conductors is **absent** (returns "no value", never a guess) | Load IEC 60364-5-52 Tables B.52.2 to B.52.5 (ampacity), B.52.14 (ambient) and B.52.17 (grouping) as CSV; those columns become *Licensed* |
| Ik network (`calc::network`) | **Formulas follow IEC 60909-0** (c factors, KT, kappa ratio method, symmetrical components, motor contribution rule). Checked against a phase-domain solve and hand calculations in the tests; no published IEC 60909-4 worked example is reproduced yet | Nothing to load; check c_max (1.05 vs 1.10) for your supply tolerance |
| Time-current bands (`calc::curves`) | **IEC 60898-1** test points (1.13, 1.45, 2.55 In; 3-5, 5-10, 10-20 In), **IEC 60947-4-1** relay classes, **IEC 60269-1** conventional fuse currents. Between the points: straight lines on log-log axes. gG melting times and electronic trip tolerances are *not* in the standards: they are assumptions and are listed in the report | Maker curves (`Characteristic::Points`) replace the generic bands |
| Selectivity and back-up (`calc::selectivity`) | Generic bands are a conservative envelope (can only say "selective" when the standards guarantee it). **Back-up can only come from a maker table** | Load the maker's selectivity/back-up and coordination tables as CSV |
| SCCR (`calc::sccr`) | UL 508A SB procedure as understood; **default component ratings (SB4.1) are unverified planning values**. Limiter credit needs the maker's let-through data and gets none without it. IEC 61439 route uses IEC 60947-1 Table 16 peak factors | Load SB4.1 defaults; load limiter let-through |
| Starter sizing (`calc::starter`) | Rules of thumb are marked as such (fuse factors 1.6/2.0/2.5, magnetic trip ≥ 12 Ie); motor current estimated from typical IE3 values only when the nameplate is missing, and the report says so | Maker catalogs and coordination tables |

Nothing in this crate certifies a design. It assists; the engineer certifies.

## Where the user finds it

| Part | Entry point |
| --- | --- |
| Circuit report (Ib ≤ In ≤ Iz, drop, Icu, k²S², minimum short circuit, disconnection, PE, selectivity) for a supply described in JSON | `kopercad-cli calc plant.json [--tables licensed.csv] [--pairs maker.csv] [--json]`; example in `docs/examples/protection-sample.json`; exit code 1 when a circuit fails |
| Cable rating, breaking capacity (now from a transformer), disconnection, thermal withstand on the drawn project | Issues list (design checks panel): kind *Cable rating* etc., waivable; inputs are properties, see the table in `crates/novo-checks/src/calc_checks.rs` |
| Starter sizing, SCCR | Engine and tests today. The Checks workspace "Calculations" tab and the Ctrl+K command "Size this circuit" are the UI follow-up (needs the UI integration thread; the engine call is `calc::starter::size_starter`) |

## Property keys added in this lane

`install_method` (B1, B2, C), `ambient_c`, `grouped_circuits` on a wire or cable;
`supply.transformer_kva`, `supply.uk_percent`, `supply.transformer_pk_w`, `supply.min_short_circuit_ka`,
`supply.network_sk_mva_max`, `supply.network_sk_mva_min` on the project. Without `install_method` the cable
rating check says nothing: the rating depends on it and a default would be a guess.

## Data the parts library must provide

Sent to the Parts library thread: rated current, Icu/Ics, trip curve and magnetic band, adjustable Ir/Isd/Ii,
let-through I²t over prospective current, SCCR, gG class, contactor AC-3/AC-1 and largest SCPD (type 1 and 2),
relay range and class, motor data, cable r20/x/max temperature, terminal and busbar ratings, transformer data.
Each with a source tag; unknown stays unknown.

## CSV formats

- Ampacity tables: `ampacity,<PVC|XLPE>,<B1|B2|C>,<2|3>,<mm²>,<A>`; `ambient,<PVC|XLPE>,<°C>,<k>`; `grouping,<n>,<k>`.
- Pair tables: `selective,<up>,<down>,<kA|total>`; `backup,<up>,<down>,<kA>`; `coordination,<scpd>,<contactor>,<1|2>`.
- SCCR defaults: `<kind>,<kA>`.
