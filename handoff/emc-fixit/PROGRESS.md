2026-10-25 UTC thread started from audit/creative/round34.md idea 3.
2026-10-25 UTC engine preview + three more applicable fixes, window preview card, two nits fixed, tests and QA 220-224 green at 1366x768, 1920x1080, light.
2026-10-25 UTC follow-up 1 (PR A): previewed fix drawn before/after in the 3D and plate EMC views (WhatIfView in emc_view.rs, emc_whatif_sync in emc.rs, banner with Before/After/Apply/Cancel), QA 440-443.
2026-10-25 UTC follow-up 2 (PR B): schematic cable card offers Preview (same card as Check > EMC check) instead of applying at once; QA 156/157 updated, 420/421 new (Light theme, Cancel).
2026-10-25 UTC follow-up 3 (PR C): novo-routing `plan_avoiding` + wire property `route.avoid_ducts` (soft, 12x cost, never forbidden); EMC `Fix::AvoidDuct` on the victim of EMC-01; fixture emc-cabinet-two-ducts.novo (qa/fixtures/make_emc_two_ducts.py); QA 430/431. SeparateRuns/MoveDevice stay by hand: they need a geometry move (placement or a duct clearance model) that routing cannot offer.

2026-10-25 UTC follow-up (after #1479): the avoid-duct Apply notice says the wire route and cut length can change; scenario 430 pictures the plate after Apply; docs/EMC.md says route.avoid_ducts holds duct ids and goes stale harmlessly.
