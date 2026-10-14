# WS-14 Design checks: roadmap

## Scope
Live design checks (roadmap WS-14): problems derived from the model, never
stored, marked in place (DESIGN.md §2, §5), listed in Issues and walked with
F8 / Shift+F8 (§3). Never blocking: export, save and editing go on.
Engine only: crate `crates/novo-checks`. The UI that consumes it (issue pill
in the top bar, Issues outline mode, marks on the drawing) belongs to WS-01
and WS-03. Done-criterion: every deliberate error in a test project is
flagged.

## Done
- `check(&Project)` / `check_with(&Project, &Netlist)` -> `Vec<Problem>` in
  reading order (page order, then x, then y, like core's cross references);
  `Summary { errors, warnings, infos }` with `pill_text()`.
- Checks: unconnected points (plus touching-but-unwired points), duplicate
  tags, duplicate terminal names under `SameName::Off`, no main function,
  several main functions, too many contacts (`contacts.main/no/nc/co`
  device props, the keys WS-10 reads), interruption point without partner,
  potential conflicts (short), zero-length wires (touching pins exempt),
  missing parts (devices, cables; info).
- Tests: per-check tests; broken Gate 1 starter flags each of the 10 kinds
  exactly once; clean starter yields nothing. Release timing example.

## Next, in order
1. PR from `claude/ws14-design-checks` once the lead asks; reviewer: main
   reviewer.
2. Wire cross-section vs protective device, once rated current exists
   (WS-02 / WS-07 request below).
3. Optional connection points, once pins carry an `optional` flag.
4. Share contact classification with novo-xref (WS-10, PR #31) when it
   merges; ideally from the symbol library's `function` field.
5. Contact limits from parts (WS-07) instead of `contacts.*` device props.
6. Help WS-01/WS-03 consume the API: pill from `Summary`, marks from
   `Problem.at` + `severity`, F8 over the Vec.
7. Incremental checking from ChangeSet only if the full pass becomes a
   frame problem (it runs per revision, not per frame).
8. Further checks as rules appear: terminal proposals (WS-11), cable cores
   vs cross-section (WS-12), PLC addresses (WS-17).

## Requests to WS-02 (novo-core), not done here
- `Pin::optional: bool` (copied from the symbol library) so unused spare
  pins are not flagged.
- A rated current for protective devices (device or part property schema,
  with WS-07) to check wire cross-section; `Wire::cross_section_mm2` exists.
- `Net` could expose which object defines each potential
  (`Vec<(PotentialId, AnyId)>`); novo-checks re-derives it from feeds, wire
  potentials and terminal names, duplicating the netlist's rules.
- A documented iteration-order guarantee (or none) for `Project::symbols()`.

## Open questions
- Severity choices (errors: duplicate tag, too many contacts, short;
  warnings: incomplete drawing; info: missing part) are WS-14's defaults;
  the Workflow thread may want them as project rules.
- Should infos show on the pill? Currently not (pill counts things to fix).
