# Pages from an I/O list: roadmap

Scope: round 7 proposal 32 (audit/creative/round7.md). A machine builder starts from a signal list. Import it (CSV or Excel) and generate the PLC card pages, field terminals, wires and review notes, editable afterwards, one undo step.

## Done
- novo-plc `generate`: `read_list` (headings in English or Dutch, any column order, German E/A addresses, signal type from codes or words), `plan` (cards per station and type, addresses kept with spares in the gaps, signals already in the project left alone), `apply` (card, page, strip, channels, terminals, wires, live texts in one Tx).
- App: Draw > PLC > Import I/O list (Ctrl+K "import io list"), review dialog with Enter/Esc.

## Next
1. Terminal strip grouping by signal type and jumpers for supply (24 V, 0 V) per strip, a cable per field device, labels (proposal 32 asks for these; not built).
2. Card main symbol (overview) so channels cross-reference to it; today the I/O list and Devices drawer give "drawn at page.column".
3. Sync, not only add: update the description of a signal already imported, flag signals removed from the list.
4. Location column to split strips and pages per field box.
5. CLI: `kopercad-cli import io-list`.

## Open questions
- Which card sizes the workshop buys (default 16 digital, 4 analog, settable in the review).
