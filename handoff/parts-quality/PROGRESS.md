2026-10-16 07:50 UTC lane started from spawn table; read parts code, plan: three small PRs.
2026-10-16 08:10 UTC PR 1 engine + Quality tab written, clippy and 14 integration tests green locally.
2026-10-17 09:40 UTC PR 1 (#568) merged. PR 2 started from main: duplicate_groups, conflicts_between, incoming_conflicts and a Duplicates tab.
2026-10-17 10:45 UTC #979: lead B asked for order-number labels; R2 had said READY on f7e2154. Pushed the label fix plus a Refresh button on the Duplicates tab (stale cache) at the lead's request.
2026-10-17 11:30 UTC #979 merged. PR 3 written: ReviewLedger (sidecar parts-review.json), Trust, queue, Review tab. Note for later: incoming_conflicts has no caller yet (needs the import dialog thread).
2026-10-17 11:45 UTC #1014: R2 required wording fix (review is a record, not a gate yet). Pushed. Consumers of Trust (part picker warning, design check) are a follow-up PR.
2026-10-21 design check recorded as post-v0.1 (stale-stamp catch); tags shipped via #1084.
