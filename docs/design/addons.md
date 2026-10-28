# Add-ons: the add-on manager, the package format and how to write one

Lane C0b. Plan: `plans/controller-live-view.md` section 5b (project files). Where add-ons appear in the program and how they look: [visual-redesign-addons.md](visual-redesign-addons.md). Crate: `novo-addons` (pure; no UI, no network, no admin rights).

## What it is, in one paragraph

Some features are not part of the base program. They are **add-ons**: a file ending in `.kcaddon` that the person installs, turns on and off, updates, rolls back and removes in **File > Add-ons...** (also Ctrl+K, "Add-ons"). Everything is per user, under `%LOCALAPPDATA%\KoperCAD\addons`, works offline and needs no administrator rights. The base program knows nothing about any particular add-on: an add-on adds rows to tables the program already owns (commands, panels, icons, data files for kinds the program reads) and never adds a surface of its own.

## Where the user finds it

| Where | What |
| --- | --- |
| File > Add-ons... | The Manage add-ons window (a resizable centre window, 960 by 640). Lane V2b moves the entry to the app-menu footer; the command id `addons.manage` stays |
| Ctrl+K "Add-ons" (also "plugin", "extension", "install") | The same window |
| Install from file... in the window, or drop a `.kcaddon` on the window | The install prompt: publisher, permissions asked for, fingerprint, and the "I trust this file" tick for a file no trusted publisher signed |
| The foot of a menu area, under the label "Add-ons" | Rows an enabled add-on put there (at most 4 per area) |
| Ctrl+K | Commands of enabled add-ons, with "Add-on: <name>" after the label |
| A floating panel | A panel an enabled add-on declares, drawn by the program with `panel.rs` (move, resize, remembered) |
| `kopercad-cli addons list / inspect / install / enable / disable / remove / rollback / pack` | The same from a script or CI |
| `kopercad.addons_list()`, `addons_install()`, `addons_set_enabled()`, `addons_remove()` (Python) | The same from Python |

Not yet in the app, and listed so nobody looks for them: the rail slot and flyout (lane V3a draws the rail; the rows are ready in `Extensions::rail_rows`), the app-menu footer (V2b), the add-on chip in Ctrl K (V5), the drawn-icon painter for add-on icons (V0; the add-on glyph stands in), and the controller's own engine (the host is in; the controller add-on is lane C3).

## The package

A `.kcaddon` is a zip:

```
manifest.toml       required; see below
signature           optional; Ed25519 over the manifest
engine.wasm         optional; the sandboxed engine (see "The engine")
data/...            panel layouts, profiles, symbols, word lists, icons, help
LICENSE             optional
```

`manifest.toml` lists the SHA-256 of **every other file** under `[files]`. A file that is not listed, a listed file that is missing and a file whose hash differs each refuse the package. Because the manifest names every hash, one signature covers the whole package.

```toml
[addon]
id = "controllers"                  # 2-48 chars: a-z, 0-9, hyphen; starts with a letter
name = "Controllers"
version = "0.1.0"
publisher = "KoperVonk"
description = "Virtual operator controllers for the live view."
requires_app = ">=0.1.0, <0.3"      # the program versions it works with
api_version = 1                     # add-on API generation (this program speaks 1)
permissions = ["menu", "floating-panel"]

[[command]]                         # one command per row it adds
id = "panel"                        # registry id becomes addon.controllers.panel
label = "Controller panel"          # at most 28 characters
description = "Open the virtual controller panel."
icon = "controllers.radio"          # own icon, a built-in name, else the add-on glyph
keywords = ["radio", "pendant"]
key = "Shift+L"                     # a wish; a key the program uses wins
home = "menu"                       # menu | rail | palette-only
menu = "Simulate"                   # for home = menu: File Edit Draw View Check Reports Simulate Help
opens = "radio"                     # a declared panel; without it the engine handles the command

[[panel]]
id = "radio"
title = "Radio"
slot = "floating"                   # floating | live-view-card
file = "data/panels/radio.panel.toml"

[[contribute]]                      # a data file for something the program already reads
kind = "controller-profile"
file = "data/generic-radio.controller.toml"

[engine]                            # optional; see "The engine"
file = "engine.wasm"
host_api = 1
```

### Permissions

Each is declared in `permissions`, listed in the install prompt and in Details, and checked: a command that sits in a menu needs `menu`, a rail row `rail-item`, a panel `live-view-card` or `floating-panel`. Also defined for what C2/C3 will ask: `read-project`, `write-bindings`, `add-symbols`, `add-checks`, `input-devices`. A permission an add-on does not declare is refused when its manifest is read.

### Limits (all checked at install and at every start)

| What | Limit |
| --- | --- |
| Package file | 64 MiB |
| Entries / one file after expansion / all files after expansion | 1,000 / 32 MiB / 128 MiB |
| Expansion of a file over 1 MiB | at most 200 times its compressed size |
| `manifest.toml` | 256 KiB |
| Rows per add-on | 4 per menu area, 6 per workspace rail flyout |
| Icons | 64 per add-on, 48 shapes each, every coordinate within 0 to 24, no fill except a dot of radius 1.5 or less |
| Panel | 200 widgets, nesting 2 deep, text 200 characters, no colours (meanings only: neutral, accent, ok, warn, error, info) |
| Names | relative `/` paths; no `..`, drive letters, `:`, `\`, control characters, Windows device names, trailing dot or space, symbolic links, or two names differing only in case |

### Panels

`*.panel.toml` describes a panel as data: `heading`, `label`, `separator`, `button`, `lamp`, `row`. The program draws it with its own widgets and palette, so dark, light and High contrast work without the add-on knowing. A lamp is a filled or hollow mark **and** its text, never colour alone. Buttons send their id to the add-on's engine (op `button`); with no engine they are greyed and the tooltip says so. An engine's answer can set lamp states (`lamps`) and show a message (`notice`). Lane C3 adds the rocker with steps, joystick with detents, tabs and tables to this set; an unknown kind is refused at install, so an older program never shows half a panel.

### Icons

`data/icons.toml` (optional): icons as shape data on the 24 grid (`line`, `poly`, `rect`, `circle`, `arc`), named `<add-on id>.<name>`. Never pixels or SVG. Until lane V0's painter lands the program shows the add-on glyph (a puzzle piece) in their place; nothing else changes.

## Trust

| State | Meaning | Install |
| --- | --- | --- |
| Signed | `signature` verifies with a key this PC trusts | no tick |
| Unsigned, from file | no signature, or signed with a key this PC does not trust | needs "I trust this file" |
| Signature broken | `signature` names a trusted key but does not match the manifest | refused |

Keys: built-in keys (`novo_addons::trust::BUILTIN_KEYS`, **empty until the release thread generates the KoperVonk key**; the private key never enters the repository) and keys the person adds in `%LOCALAPPDATA%\KoperCAD\addons\trusted-keys.toml` (`[[key]]` with `id`, `owner`, `public` as 64 hex digits). The signed message is `kopercad-addon-manifest-v1\n` followed by the manifest bytes.

Honest limits: a signature says who made the file, not that it is good. An add-on's engine runs in a sandbox: no file, network, clock or thread access, a step budget and a memory limit. What it can do is limited to computing, so the worst case is wrong results inside what its permissions allow, which is why the permissions are listed in plain words. The `.kcaddon` format is new; nothing outside this repository reads it.

## The engine

An add-on may carry `engine.wasm`: a WebAssembly module the program runs in the `wasmi` interpreter (pure Rust, no `unsafe` in our crates, binary modules only). Source: `crates/novo-addons/src/host.rs`, tests `crates/novo-addons/tests/host.rs`.

| Rule | How |
| --- | --- |
| No access to the PC | The module gets one import, `kopercad.log(ptr, len)`. Any other import (files, WASI, clock, threads) refuses the add-on when it loads, naming what it asked for |
| Endless loops end | Every call (and the start-up) has a step budget of 50 million interpreter steps; a `kopercad.log` call costs 2 000 steps on top (a host call is not free), so a loop of log calls ends as fast as any loop; running out stops the add-on |
| Memory and stack | 32 MiB of linear memory (a failed grow is seen by the engine, a bigger start is refused), recursion depth 512, messages at most 1 MiB, log text at most 8 KiB kept |
| A broken engine stops alone | A trap, a bad reply, a budget overrun or a start that fails *stops* that add-on for the session: its menu rows, commands and panels go, Details says why, a message names it, and the program goes on. Turning it off and on in the manager starts it again |
| Deterministic | No clock or random source exists for it; the same calls cost the same steps |
| Honest limit | The engine can still compute a wrong answer inside what its permissions allow; and WebAssembly floating point may differ in NaN bit patterns between machines |

Host API 1, as the engine sees it (`[engine] host_api = 1`):

| Export | Signature | What |
| --- | --- | --- |
| `memory` | memory | the messages go through it |
| `kc_alloc` | `(len: i32) -> i32` | a buffer the host writes the request into |
| `kc_call` | `(ptr: i32, len: i32) -> i64` | handle it; return `(reply_ptr << 32) \| reply_len` |
| `kc_free` | `(ptr: i32, len: i32)` | optional; the host frees the request and the reply with it |

Request `{"api":1,"op":"<name>","data":{...}}`, reply `{"ok":true,"data":{...}}` or `{"ok":false,"error":"words"}` (a refusal: the add-on stays on). Ops sent today: `hello` (once at start, answer `{"api":1}`), `command` (`{"id":"<command>"}`, for a command without a panel) and `button` (`{"panel","id"}`). Replies may carry `lamps` (`{"<id>":{"state":"ok","lit":true}}`) and `notice`. Lane C3 adds ops for the controller; the transport does not change. Build an engine from Rust with `cargo build --target wasm32-unknown-unknown` (not needed to use the host: the fixture engine `qa/fixtures/addons/engine-src/demo-engine.wat` is WebAssembly text).

## Install, update, roll back, remove

- **Install** reads the file in memory first (every check above), then writes `<root>/<id>/<version>/` through a staging folder and updates `<root>/registry.toml` atomically. A refused file writes nothing.
- **Update** from a newer file keeps the old version beside the new one; the on/off state is kept. **Roll back** swaps the two (and can be undone the same way). One previous version is kept.
- **Same version, same content**: "already installed". **Same version, other content**: refused (raise the version). **Older than installed**: refused (roll back or remove first). **Installed copy broken**: the same file repairs it.
- **Remove** deletes every version. Projects keep any data they stored for the add-on, untouched.
- **At every start** each installed add-on is checked again: manifest fingerprint, the hash of every file, the signature against the current keys, the icon and panel data, compatibility. One that fails shows as *Failed* with the reason and does not load; the others are unaffected. A damaged `registry.toml` is set aside as `registry.toml.damaged` and the program starts with none.
- **Compatibility**: `requires_app` and `api_version`. An add-on the program has outgrown stays installed and shows "Needs another version", off.

## For code that adds features

- Ask `app.addon_enabled("id")` before offering anything that belongs to an add-on (installed, intact, compatible, on). Never test for the add-on by its files.
- Menus, the command search and panels read `Extensions` (built from the running add-ons whenever one is installed, turned on or off, removed). Nothing else should list add-on commands.
- Data files for a `kind` your feature understands: `AddonSet::contributions(kind)` then `AddonSet::read_file(id, path)` (hash checked again on every read).
- Tests: `crates/novo-addons/tests/` (hostile packages, install flow), `crates/novo-tests/tests/addons_fixtures.rs` (the fixtures and the CLI), `crates/novo-app/src/app/addons_tests.rs` (the window), QA scenarios `qa/scenarios/560-addons-manager.qa` and `561-addons-engine.qa`.
- Fixtures: sources in `qa/fixtures/addons/src/<name>/`, packed files beside them. Rebuild with `KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures`.

## Follow-ups (not in this lane)

| Follow-up | Where |
| --- | --- |
| Gate the controller import wizard (File > Import > Controller profile) behind the controller add-on, once that package exists | after C3 packages the add-on |
| Build the controller as `engine.wasm` (`wasm32-unknown-unknown`) and pack it in CI | C3 / release |
| Project data blocks tagged with the add-on id, kept untouched when the add-on is missing | needs a generic block in `novo-core`/`novo-storage` (WS-02/WS-04) |
| Rail slot and flyout, app-menu footer with count badge, add-on chip and "Turn on" row in Ctrl K, drawn icons | V3a, V2b, V5, V0 |
| Release signing key and `scripts/pack-addon` in CI | release thread |
