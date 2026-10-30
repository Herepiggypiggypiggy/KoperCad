# TraceParts integration plan (2026-10-30)

## Legitimate route (researched, not yet tested with a key)
- TraceParts has a Developer Hub (developers.traceparts.com) with a REST "Product Explorer API Gateway":
  token POST, CADServices (is CAD available, formats, URL for viewer or file), CADViews, Item (find by attribute or UPC).
- Every call needs an ApiKey. The key is requested by form (info.traceparts.com ... request-api-key), validated by
  TraceParts "over several days", tied to a contract. Eligibility/cost/rate limits/redistribution terms are NOT stated on public pages.
- No public docs found for a free general search endpoint for end users. Do NOT scrape traceparts.com or bypass login.
- Tadjiro must: request an API key (as KoperVonk, ask whether a CAD-software integrator/end-user key is allowed, ask about
  caching and redistribution terms), and keep downloaded models personal (not bundled in the MIT repo).

## Design
1. Fallback that works with no key (build first): hand-off flow. From a part, "Find CAD on TraceParts" opens the browser
   search for the manufacturer part number (URL only); the user downloads STEP while signed in; KoperCAD watches the
   Downloads folder / accepts drop, matches with existing cadmatch.rs, files into the per-user cache.
2. Keyed flow: API client (blocking on worker thread), key stored per user (%APPDATA%, never in project files or repo),
   search by MPN, check CAD availability, download STEP to %LOCALAPPDATA%\KoperCAD\cache\traceparts, offline = cache only,
   clear errors (no key, 401, 404, rate limit, offline, timeout).
3. STEP -> 3D view and footprint: already in crates/novo-interop/src/step.rs and part3d, cadmatch.rs. Audit and extend
   (bounding box exactness, mounting footprint, drill plan) rather than rebuild.
4. UI: Parts window gets a TraceParts panel on a part; shows link, cache state, key settings.

## Lanes
- L1 API client + cache + key store (novo-parts, new module traceparts/, pure + mock-server tests)
- L2 Hand-off flow + STEP footprint/drilling audit (novo-interop, cadmatch)
- L3 Parts window UI + part linking (novo-app), after L1/L2 API settles
