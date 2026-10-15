2026-10-15 17:30 UTC Design doc written (docs/design/wire-routing.md), handoff folder started.
2026-10-15 17:55 UTC Engine written: network, stubs, shortest path, slack, free air; 12 tests, clippy clean. Added to the design PR to avoid losing work while it waits for merge.
2026-10-15 18:10 UTC #334 merged. Follow-up PR: NaN guard, collinear merge, reach rule, doc planned markers; 15 tests.
2026-10-15 18:25 UTC #353 merged; reviewer B found merge_collinear dropping a turn-back point. Fix PR: only drop a middle point lying between its neighbours.
