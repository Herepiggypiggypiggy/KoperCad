# Onboarding: example project and first-run walk-through

Status: proposal for review. Nothing here is built yet. Thin UI over existing commands, as
`docs/DESIGN.md` asks.

## Why

A new user (or Tadjiro on a new PC) opens KoperCAD and sees an empty canvas with New and Open. The
fast keys are the product's point and nothing shows them. `docs/DESIGN.md` §3 already promises
"Open example" on the start panel and "first-run hints"; neither is in the code
(`start_panel` in `crates/novo-app/src/app.rs` offers New, Open, Unsaved and Recent only).

## 1. Open example

- A fourth start-panel button, **Open example**, and a command `file.example` (so Ctrl+K finds it).
- It creates an untitled project holding the Gate 1 motor starter (two pages: main circuit,
  control circuit) through the same path as New: the fixture builder is one `doc.transact`, so the
  whole example is one undo step and the existing untitled/autosave/Unsaved flow applies. Naming it
  (Ctrl+S) moves it to `Documents\KoperCAD` as for any project; nothing is written to a fixed
  example file, so a user can never damage "the" example.
- Where the builder lives: today `motor_starter` is in `novo-tests/src/motor_starter.rs`, a test
  crate owned by the Tester. Two options:
  1. **(recommended)** `novo-app` depends on `novo-tests` as a normal dependency and calls
     `motor_starter`. No second copy to drift; the example is exactly what Gate 1 tests. Cost: a
     "tests" crate in the app's dependency list (it only depends on `novo-core`, and the dev-dependency
     on `novo-app` is not a cycle).
  2. Move the builder into a new `novo-app/src/example.rs` and make `novo-tests` use it from there.
     Cleaner naming, but touches the Tester's crate.
- The project name becomes "Motor starter (example)" so it reads as a sample.

## 2. First-run walk-through ("Try the fast keys")

A small card at the bottom right of the canvas, never modal, never blocking input. It lists six
steps; each ticks itself when the user does it, so it measures real use, not clicks on the card:

| Step | Tick when |
| --- | --- |
| Q: place a symbol | a symbol is added by the place tool (change set `added` contains a symbol) |
| W: draw a wire | a wire is added |
| R: rotate | a symbol's rotation changes (`modified` after the rotate command) |
| Ctrl+D: duplicate | the smart-duplicate action runs |
| C: place a cable | a cable is added |
| Ctrl+K: open the palette | the palette opens |

- Detection reuses what the shell already sees: the `ChangeSet` returned by every command (the same
  hook `reports_heard` uses) plus the `Action` the window runs. No new events in `novo-editor`.
- It runs on a **practice project**: the start panel gets **Try the fast keys**, which does New and
  adds one empty page, then shows the card. It can also be opened on any open project from the Help menu
  (**Fast keys walk-through**) and from the palette (`help.walkthrough`).
- Each step shows the key as a keycap and one line: what it does and what to expect (for Q: "hold
  Q for the wheel, tap for the last symbol"). Text comes from the keymap, so a user-remapped key shows
  its real key (`userkeymap.rs`).
- Progress persists in `Settings` (`onboarding`: ticked steps and `dismissed`), so closing the app
  mid-way resumes. **Skip** (or all six ticked) sets `dismissed`; it never reopens by itself. Reset is
  the Help menu item.
- First run only: if there is no `last_open`, no recent projects and `onboarding` is unset, the start
  panel highlights **Try the fast keys**; nothing opens on its own.

## 3. Out of scope

Dutch text (the aliases come with the language work), video or animated overlays, and a tour of the
inspector, drawers and reports. A longer tour can reuse the same card and step list later.

## 4. Where the code goes

`crates/novo-app/src/app/onboarding.rs` (card, steps, detection); small hooks in `app.rs`
(`start_panel`, the command runner, Help menu), `actions.rs` (two actions) and `settings.rs` (one
field). No change to `novo-core` or `novo-editor`. Tests: the six steps tick from driven commands in
the `gate1_tests` style, the card persists and dismisses, and the example opens as one undo step.

## Decisions (design reviewer)

1. The example builder is `novo-tests::motor_starter`, called from `novo-app` (option 1). Later, when it
   is cheap, the fixture can move into a small `novo-example` crate that both depend on.
2. The practice project is a fresh untitled project, not a page in the example. Because a cable needs
   two devices with terminals, it starts with a motor and a terminal strip already placed, in the same
   single undo step as the project. Q and W stay free to draw.
3. The card sits at the bottom right and steps aside, collapsed to a pill ("3 of 6"), while the live
   view is on. The key on each step comes from the keymap. The Help menu item resets the progress.
