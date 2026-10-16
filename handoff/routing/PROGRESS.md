2026-10-15 17:30 UTC Design doc written (docs/design/wire-routing.md), handoff folder started.
2026-10-15 17:55 UTC Engine written: network, stubs, shortest path, slack, free air; 12 tests, clippy clean. Added to the design PR to avoid losing work while it waits for merge.
2026-10-15 18:10 UTC #334 merged. Follow-up PR: NaN guard, collinear merge, reach rule, doc planned markers; 15 tests.
2026-10-15 18:25 UTC #353 merged; reviewer B found merge_collinear dropping a turn-back point. Fix PR: only drop a middle point lying between its neighbours.
2026-10-15 18:50 UTC #355 merged. Fill PR: plan(), DuctFill, Problem, penalty, 6 tests in tests/fill.rs.
2026-10-15 19:10 UTC #358 merged. Follow-up PR for reviewer notes: unsized ducts, duplicate ids.
2026-10-15 19:40 UTC #364 merged, WS-02 #362 merged. Bridge PR: model::ducts/network with tests/model.rs.
2026-10-15 20:15 UTC #371 merged. Connection PR: route_connections, anchors at footprint edge midpoints.
2026-10-15 20:40 UTC #381 merged. Follow-up: library pin direction for anchors, duplicate connection ids unrouted. Wire list thread owns the WireList column; I take the novo-checks routing problems next.
2026-10-15 21:15 UTC #460 merged: Cabinet routes overlay, route_project, OverlayCache. Waiting on #413 for the two infos; hook is the UI integration thread's.
