# Crane import: boxes, busbars and special devices (lane 2 of 4)
Scope: faithful shapes for black boxes, PLC boxes, busbars and other special devices in the crane configurator import.
Done: box rectangle read from the placement (`S89x5`), drawn at true size (dashed for black boxes); busbar rails inside a frame; capacitor and resistor mapped to library symbols.
Next: PLC box and PLC connection point artwork (needs the symbol-artwork lane), "Semiconductors" and "Device connection point" mapping, PE rail with its terminals, line styles per side (`A412..A415` numeric values are not decoded), potential colours/thickness of rails.
Open: where EPLAN's artwork of busbar connection points puts the pin (we keep the library pin; rails run at the insertion points).
