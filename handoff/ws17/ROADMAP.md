# WS-17 PLC and I/O: roadmap

Scope: PLC cards and channels as devices, I/O address allocation and
formatting, channel cross references, the I/O list rows, PLC problems.
Crate: `novo-plc` (registered on main, not created yet). Design:
`docs/design/plc.md`.

## Done
- Design doc `docs/design/plc.md` (#143, merged).

## Next, in order
1. (done) Design PR #143 merged.
2. `novo-plc` skeleton: card layout parser (`DI16 DQ8`), address spaces,
   next free start for a new card, address formatting (IEC, Siemens,
   Rockwell, Plain). Pure functions, unit tests.
3. Channel index: symbol -> (card, group, channel), labels, free channels,
   next channel for placement, the problems of plc.md §7.
4. Commands: add card with allocated start, place next channel, `to plc`,
   `renumber io` with preview (novo-core commands, one undo step).
5. I/O list rows (for the Outputs thread's `io` report) and Devices drawer
   rows (for WS-01).

## Coordination owed
- Outputs thread: `io` report in novo-reports and PLC problems in novo-checks.
- WS-10: channel image on the card overview, address labels via the xref label path.
- WS-08 / WS-06: `di A4`, `to plc` verbs and aliases.
- WS-07: `plc.channels` on catalog parts.
- WS-02 (later): typed fields listed in plc.md §9.

## Open questions
See plc.md §11 (Rockwell allocation, safety channels, distributed I/O).

## io_rows() spec agreed with Outputs (2026-10-15 03:51 UTC)
Rows in address order grouped by station; each row: station key + title
(section key/title), address, type, card tag, channel, pin, signal (text or
potential), connected-to tags + their AnyIds, drawn-at page.column + symbol
AnyId (None for spare), spare flag. `novo_plc::READS` lists record kinds read
(plus project info). Checks: Issue::is_error(), Issue::message(project) with no
trailing period, Issue::at() AnyIds (done in PR 3). Ping Outputs
(session_01GNLWH7SKcFppZwCxZoC1rw) when io_rows lands.

## Follow-ups (after #165)
- renumber_plan: report cards skipped because their layout does not parse, so the preview can say "-K3 skipped: layout does not parse". PR opened from main after #165 merged (f74e14e).
- ModelError::Format is used for "no free channel" and layout errors; ask WS-02 whether an InvalidValue-style variant is wanted.
- Brief integration owners: WS-08/WS-06 (`di K4`, `to plc`, aliases), WS-03/WS-10 (address label at the `address` anchor, channel image, G between channel and overview), WS-01 (Devices drawer card rows from io_rows), WS-07 (`plc.channels` on catalog parts).
