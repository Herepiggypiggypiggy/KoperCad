# Unreleased

- **Cable length now means the whole cable as bought.** A core on a cable with a stated Length (m) is cut at exactly that length; no service loop is added. Projects that typed the length of only the outside run into a cable will see those core lengths change: check the cable lengths before cutting.
- **Cabinet entry point and wire outside the entry** (Cabinet > Routes). Supply leads are routed to the entry and use the length you state outside it; until stated they stay estimates.
- A cable whose stated length is shorter than the wire's route inside the cabinet cannot reach the entry: its cores show the inside route as an estimate until the length is corrected.
- **I/O list import: PLC cards are now tagged `-A1, -A2, ...`** (they used to be `-K1`), and a card never takes a tag that the list itself uses. Existing projects keep their tags; the letter can still be set to `K`. A new card is also free of tags in other plants and locations.
- **I/O list import draws the whole card.** Spare channels are drawn as "Spare" (no terminal, no wire), each channel shows its address under it, and the address follows later edits of cards and of the address style. A fresh import adds no warnings: the field side of its terminals is left open on purpose.
