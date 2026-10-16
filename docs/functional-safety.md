# Functional safety (novo-safety)

**Calculation aid, not a certification.** `novo-safety` implements the simplified method of ISO 13849-1: B10d to MTTFd, channel MTTFd, two-channel symmetrisation, DCavg weighting, CCF score check (65), category rules, PFHd, PL bands, the required-PL risk graph and subsystem series/parallel combination.

What is exact: PL bands, risk graph, MTTFd formulas, symmetrisation, category B/1 PFHd (`1/(MTTFd*8760)`), series sum.
What is a model: PFHd of categories 2, 3 and 4 is a closed-form estimate (beta 0.02, 20 year mission time), not ISO 13849-1 table K.1. Parallel combination of whole subsystems is outside the standard. Anchors tested: category B (3 and 100 years), category 4 at 100 years and 99 percent DC (about 2.47e-8, tested within 15 percent). Replace the model with table K.1 when the table is available (see handoff/safe-1).

Where the user finds it: the engine has no UI yet. SAFE-2 adds the safety function editor and Checks > Safety.
