# Device descriptions by recipe: roadmap

Scope: compose device and part descriptions from part data with an editable recipe per device kind (poles, kind, rating, contacts `nM/nm/nv`, voltage, power, coil voltage and frequency).

Done:
- Engine `novo-parts/src/describe.rs`: templates with `{field}` and `[optional group]`, English and Dutch presets, user recipes in the profile, honest "missing fields" list.
- Commands `describe_devices` and `describe_parts` (one undo step, never overwrite written text unless asked).
- UI: Edit > Parts > Describe devices from parts (`parts.describe`), recipe editor with live preview.

Next:
- Fill `main_contacts_no`, `aux_contacts_*`, `coil_frequency_hz`, `coil_current_type`, `rated_voltage_v` from BMEcat/ETIM/table import (parts importer threads).
- Optional: show the generated text live in the property sheet instead of writing it.
- Descriptions in device tags on the drawing: tags themselves stay IEC 81346; the description text is what labels and the BOM show.

Open: which data keys real maker files give for contact counts (ETIM features), unverified.
