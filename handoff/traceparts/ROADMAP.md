# TraceParts L1 roadmap
- [x] API client over a Transport trait (token, CADViews/ByItemNumber, Item/ByAttribute, IsCADAvailable, AvailableFormats, CADFileUrl, download)
- [x] Per-user key store (%APPDATA%\KoperCAD\traceparts.json, env override)
- [x] Download cache (%LOCALAPPDATA%\KoperCAD\cache\traceparts, cap 2 GB LRU, miss memory 7 days)
- [x] Service::fetch_step, cache-only mode, mock-server tests
- [ ] Try with a real key (blocked on Tadjiro's key request, see PLAN.md)
- [ ] L3: Parts window panel (novo-app) calls Service on a worker thread
