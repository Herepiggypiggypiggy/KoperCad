# Hands-on check: place wheel (points 14 and 16)

Code and headless tests say these work; they need a person with a mouse.

1. Hold Q for over 250 ms with the pointer on empty canvas: the wheel opens
   at the pointer and the part search opens with it.
2. Sweep slowly across the five inner sectors: the first sector's items show
   at once, and the outer ring changes only after the pointer rests on a
   new sector for about 180 ms. A quick sweep toward an item must not
   flicker or swap the items under the pointer (point 14).
3. Click a sector: its items stay up. Move away and back: they stay until an
   item is clicked or Esc closes the wheel (point 16).
4. Click an outer-ring item: the symbol attaches to the pointer and the
   click that drops it places it. R and X rotate and mirror it.
5. Centre reads **Other…** and opens the place surface; sector 5 reads
   **Points** (feed, interruption, PE, earth, chassis).
6. Release Q on an item without clicking: it picks that item (a flick).

Anything that does not match: note which step and the pointer speed, and
tell the App shell thread.
