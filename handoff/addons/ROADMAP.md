# Add-ons (lane C0b) roadmap

Scope: the add-on manager and package format (`novo-addons`), the manager window, the command-line and Python entry points. Guide: `docs/design/addons.md`. Plan: `plans/controller-live-view.md` section 5b.

## Done (C0b-1)
- Package format `.kcaddon`: zip, manifest with SHA-256 of every file, optional Ed25519 signature, limits and hostile-package tests.
- Per-user store, atomic registry, update, roll back, remove, re-verification at every start, failed add-ons isolated.
- Extension tables (menu rows, rail rows, panels, icons), panels as data, palette integration.
- File > Add-ons... window, Ctrl+K entry, `kopercad-cli addons`, Python `addons_*`.
- Fixtures `qa/fixtures/addons`, QA scenario 560.

## Next, in order
1. C0b-2: sandboxed engine (`wasmi`, JSON host API v1, step budget, traps disable the add-on for the session). Must land before C3 runs the controller engine in the app.
2. Package the controller as an add-on (C3), then gate File > Import > Controller profile behind `NovoApp::addon_enabled("controllers")`.
3. Rail slot and flyout (V3a), app-menu footer (V2b), Ctrl K add-on chip (V5), drawn add-on icons (V0).
4. Project data blocks tagged with the add-on id that survive a missing add-on (needs a generic block in `novo-core`/`novo-storage`, WS-02/WS-04).
5. Release signing key (`BUILTIN_KEYS` is empty) and `scripts/pack-addon` in CI (release thread).

## Open questions
- Whether the controller import wizard stays in the base permanently (coordinator: gate it later, small follow-up).
