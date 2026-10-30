# TraceParts L1 handoff
Code: `crates/novo-parts/src/traceparts/` (error, transport, keystore, client, cache, service). Tests: unit tests there + `crates/novo-parts/tests/traceparts_mock.rs` (mock gateway on localhost through the real ureq transport).

API for the UI (L3): `KeyStore::default_store()`, `DownloadCache::default_cache()`, `Client::new(UreqTransport::new(), creds)`, `Service::new(cache, Some(client)).fetch_step(mpn)` -> `Fetched{path, from_cache}` or a `TpError` whose Display is user-ready text. `Service.mode = Mode::CacheOnly` for offline. `Client::check_key()` for a "Test key" button. Call everything on a worker thread (blocking).

Unverified (no key yet): endpoint shapes come from developers.traceparts.com reference pages (2026-10-30). The weakest link is how `cadPartUniqueId`/`logicName` are obtained: `ids_from_view_url` assumes the CADViews view URL carries them as query parameters. If the real answer differs, adjust `Service::fetch_step` only. The CADFileUrl body is sent as `{}` (configuration parameters unknown). Key is plain JSON (no DPAPI without unsafe); say so in the UI.
Downloaded models are personal: never bundle into .kcpl or the repo.

## L3: Parts window button


L3 detail: the button uses `tp_handoff` (`search_url`, `TracePartsWatcher`) through `Import::start_traceparts`; files named by TraceParts ids are judged by content with cadmatch. The `traceparts` module (keyed client) is not behind the button yet.
