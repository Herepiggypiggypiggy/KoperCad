# TraceParts L1 progress
- 2026-10-30: module, key store, cache, service and 9 mock-server tests written; fmt, clippy and novo-parts tests green.
- 2026-10-30: PR #1796 marked READY by Main A on c6b15b8. Do not push to it. Follow-up for a new PR from main after merge: require https for the CADFileUrl answer and the download (allow http only for loopback so mock tests still run).
- Follow-up PR list from Lead A (new PR from main after #1796 merges): (1) cache a miss only for an empty view list or IsCADAvailable false, never a bare 404; (2) https-only download URLs except loopback; (3) key temp file created 0o600 from the start, remove temp if rename fails; (4) ids_from_view_url skips pairs without '='; (5) lock the cache index and sweep unindexed files at start; (6) HANDOFF: fetch_step may return a .zip the UI must unzip.
- 2026-10-30: #1796 merged. Follow-up PR with Lead A's six notes written.
