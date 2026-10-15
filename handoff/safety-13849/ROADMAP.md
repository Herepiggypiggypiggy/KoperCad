# Functional safety (ISO 13849), lane B of "PLC link and functional safety"

Home: `crates/novo-plc/src/safety/` (no new crate, no novo-core change). Lane A owns `novo-plc/src/export/`; the PLC interpreter thread may also use novo-plc, so keep files separate. Not started: paused at 80% usage limit.

## Plan
1. Engine (`safety/pl.rs`): inputs category (B,1,2,3,4), MTTFd per channel, DC, CCF score, mission time. Compute MTTFd of a channel (parallel symmetrisation formula), DCavg, PFHd and the resulting PL (a-e) via the standard's PFHd bands. Formulas cited by name only, no standard text. A simplified Markov-free lookup of the category/MTTFd/DC to PL chart is a table of numbers kept in code.
2. Subsystems and blocks (`safety/chain.rs`): a safety function is a chain (input, logic, output); PFHd adds, PL is the minimum rule plus the PFHd check.
3. Required PL (`safety/riskgraph.rs`): the risk graph (S, F, P) giving PLr.
4. Storage (`safety/store.rs`): safety functions as project properties `safety.*`, linking each block to devices by tag; component data (B10d, MTTFd) from part properties.
5. Checks: PL below PLr, category not met, missing data, as `novo-checks` problems.
6. Report: `safety` report in novo-reports (SISTEMA-style table: function, PLr, PL, PFHd, blocks).
7. Safety function editor: a drawer in novo-app listing functions, blocks and live PL.

## Validation
Worked examples from published ISO TR 23849 style cases (two-channel category 3 e-stop, single-channel category 1). Unit tests assert PL and PFHd within the band; state clearly that results are planning values, not certification.

## Where the user finds it
Devices drawer > Safety tab (editor), Reports > Safety functions, Issues pill for failed PL.
