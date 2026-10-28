"""Source of the KoperCAD electrical icon set (docs/design/visual-redesign.md section 4).

Each icon is a list of SVG elements on a 24 x 24 grid, 1.5 stroke, round caps and
joins, live area 2..22. Only these elements are used so the Rust side can draw them
with an egui Painter: line / polyline (M L H V), rect (optional rx), circle, and
arcs (A) in paths. `dot` marks a filled circle (a junction or a node).

    python3 koper_icons.py <out-dir>      writes <name>.svg for every icon + sprite.svg
"""
import os
import sys

# name -> (group, tooltip name, body)
ICONS = {
    # ---- workspaces
    "schematic": ("workspace", "Schematic",
        '<path d="M5 3v18M19 3v18M5 8h4M15 8h4M5 17h5M14 17h5M10 17l4-2.8"/><rect x="9" y="5.5" width="6" height="5" rx=".5"/>'),
    "cabinet": ("workspace", "Cabinet",
        '<rect x="4" y="2.5" width="16" height="19" rx="1.5"/><path d="M7.5 9.5h9M7.5 17h9"/><rect x="8" y="5.5" width="3" height="4"/><rect x="12.5" y="5.5" width="3.5" height="4"/><rect x="8" y="13" width="8" height="4"/>'),
    "report": ("workspace", "Reports",
        '<rect x="4" y="3.5" width="16" height="17" rx="1.5"/><path d="M4 8.5h16M4 13.5h16M10 8.5v12"/>'),
    # ---- components (palette categories and tools)
    "coil": ("component", "Coil",
        '<path d="M12 2.5v5M12 16.5v5"/><rect x="8" y="7.5" width="8" height="9" rx=".5"/>'),
    "contact-no": ("component", "Make contact",
        '<path d="M12 2.5v6M12 21.5v-6M12 15.5L7.5 7.5"/>'),
    "contact-nc": ("component", "Break contact",
        '<path d="M12 2.5v6M7 8.5h5M12 21.5v-6M12 15.5L7.5 7.5"/>'),
    "contactor": ("component", "Contactor",
        '<path d="M6 3v5.5M12 3v5.5M18 3v5.5M6 21v-5.5M12 21v-5.5M18 21v-5.5M6 15.5L3.8 8.5M12 15.5L9.8 8.5M18 15.5L15.8 8.5"/><path d="M4.5 12h15" stroke-dasharray="1.5 2"/>'),
    "motor": ("component", "Motor",
        '<circle cx="12" cy="12" r="8.5"/><path d="M8.5 15.5v-7l3.5 4 3.5-4v7"/>'),
    "fuse": ("component", "Fuse",
        '<path d="M12 2.5v19"/><rect x="9" y="6.5" width="6" height="11" rx=".5"/>'),
    "breaker": ("component", "Breaker",
        '<path d="M12 2.5v5M12 21.5v-8.5l-4.5-5.5M10.2 5.7l3.6 3.6M13.8 5.7l-3.6 3.6"/>'),
    "overload": ("component", "Overload relay",
        '<path d="M12 2.5v3.5M12 18v3.5"/><rect x="5" y="6" width="14" height="12" rx=".8"/><path d="M8 13.5l2-4 2 4 2-4"/>'),
    "pushbutton": ("component", "Push button",
        '<path d="M12 2.5v6M12 21.5v-6M12 15.5L7.8 8M9.8 11.8H5.5M5.5 9v5.5"/>'),
    "estop": ("component", "Emergency stop",
        '<path d="M4.5 11.5a7.5 6 0 0 1 15 0z"/><path d="M12 11.5V16"/><rect x="8.5" y="16" width="7" height="3.5" rx=".5"/>'),
    "lamp": ("component", "Lamp",
        '<circle cx="12" cy="12" r="7"/><path d="M7.05 7.05l9.9 9.9M16.95 7.05l-9.9 9.9"/>'),
    "terminal": ("component", "Terminal",
        '<path d="M12 2.5v6M12 15.5v6"/><circle cx="12" cy="12" r="3.5"/>'),
    "terminal-strip": ("component", "Terminal strip",
        '<rect x="3" y="6" width="5" height="10" rx="1"/><rect x="9.5" y="6" width="5" height="10" rx="1"/><rect x="16" y="6" width="5" height="10" rx="1"/><path d="M2.5 19.5h19"/><circle cx="5.5" cy="11" r=".9" class="dot"/><circle cx="12" cy="11" r=".9" class="dot"/><circle cx="18.5" cy="11" r=".9" class="dot"/>'),
    "transformer": ("component", "Transformer",
        '<circle cx="9" cy="12" r="5.5"/><circle cx="15" cy="12" r="5.5"/>'),
    "psu": ("component", "Power supply",
        '<rect x="3" y="6" width="18" height="12" rx="1.5"/><path d="M12 6v12M5 12c.8-2.5 1.7-2.5 2.5 0s1.7 2.5 2.5 0M14.5 10.5h4.5M14.5 13.5h1.5M17.5 13.5h1.5"/>'),
    "drive": ("component", "Frequency drive",
        '<rect x="4" y="4" width="16" height="16" rx="1.5"/><path d="M4 20L20 4M6.5 8.5c.7-1.6 1.4-1.6 2.1 0s1.4 1.6 2.1 0M13.5 15.5h4M13.5 18h4"/>'),
    "sensor": ("component", "Sensor",
        '<rect x="2.5" y="8" width="10" height="8" rx="1"/><rect x="12.5" y="9.5" width="3" height="5"/><path d="M18 9a4 4 0 0 1 0 6M20.5 6.5a7.5 7.5 0 0 1 0 11"/>'),
    "plc": ("component", "PLC",
        '<rect x="4" y="3.5" width="16" height="13" rx="1.5"/><circle cx="7.8" cy="8" r=".9" class="dot"/><circle cx="7.8" cy="12" r=".9" class="dot"/><path d="M11 8h6M11 12h6M8 16.5v4M12 16.5v4M16 16.5v4"/>'),
    "ground": ("component", "Earth",
        '<path d="M12 3v8M5.5 11h13M8 14.5h8M10.5 18h3"/>'),
    "potential": ("component", "Potential feed",
        '<path d="M5 4h14l-7 8zM12 12v8.5"/>'),
    # ---- wiring and data
    "cable": ("wiring", "Cable",
        '<path d="M2.5 8.5H9M2.5 12H9M2.5 15.5H9M9 6.5h7.5a4.5 4.5 0 0 1 0 11H9zM12.5 6.5v11"/>'),
    "cable-section": ("wiring", "Cable cores",
        '<circle cx="12" cy="12" r="8.5"/><circle cx="9" cy="9" r="1.5"/><circle cx="15" cy="9" r="1.5"/><circle cx="9" cy="15" r="1.5"/><circle cx="15" cy="15" r="1.5"/>'),
    "xref": ("wiring", "Cross reference",
        '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/><path d="M10 6.5h4.5a2 2 0 0 1 2 2V14M14.5 12l2 2 2-2"/>'),
    "bridge": ("wiring", "Bridge",
        '<path d="M7 4v16M17 4v16M7 9h10"/><circle cx="7" cy="9" r="1.2" class="dot"/><circle cx="17" cy="9" r="1.2" class="dot"/>'),
    "wire-number": ("wiring", "Wire number",
        '<path d="M2.5 12h5M16.5 12h5"/><rect x="7.5" y="8" width="9" height="8" rx="1.5"/><path d="M11 11.2l1.5-1v4.6"/>'),
    "net": ("wiring", "Net",
        '<circle cx="5" cy="12" r="1.8"/><circle cx="19" cy="6" r="1.8"/><circle cx="19" cy="18" r="1.8"/><path d="M6.8 12h5.5l4.9-5.2M12.3 12l4.9 5.2"/>'),
    # ---- cabinet and production
    "din-rail": ("cabinet", "DIN rail",
        '<path d="M2.5 17h5V8.5h9V17h5M10 12.5h4"/>'),
    "duct": ("cabinet", "Wire duct",
        '<rect x="3" y="6" width="18" height="12" rx="1"/><path d="M7.5 6v4M7.5 14v4M12 6v4M12 14v4M16.5 6v4M16.5 14v4"/>'),
    "plate": ("cabinet", "Mounting plate",
        '<rect x="3" y="4" width="18" height="16" rx="1.5"/><circle cx="6.5" cy="7.5" r="1"/><circle cx="17.5" cy="7.5" r="1"/><circle cx="6.5" cy="16.5" r="1"/><circle cx="17.5" cy="16.5" r="1"/>'),
    "enclosure": ("cabinet", "Enclosure size",
        '<rect x="3" y="3" width="14" height="14" rx="1"/><path d="M3 20.5h14M3 19v3M17 19v3M20.5 3v14M19 3h3M19 17h3"/>'),
    "ferrule": ("production", "Ferrule",
        '<path d="M3 10.5h5M3 12h5M3 13.5h5M18 12h3.5"/><rect x="8" y="9" width="10" height="6" rx="1"/><path d="M12 9v6"/>'),
    "bench": ("production", "Bench output",
        '<path d="M2.5 10h19M5 10v11M19 10v11M8 10V6h5.5v4"/>'),
    # ---- workflow
    "macro": ("workflow", "Macro",
        '<rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="2.5 2.5"/><path d="M12 6.5v3.5M12 17.5v-3.5M12 14l-2.5-4"/>'),
    "place": ("workflow", "Place symbol",
        '<path d="M8.5 3v5.5M8.5 21v-5.5M8.5 15.5l-3.5-7M17.5 4v6M14.5 7h6"/>'),
    # ---- analysis
    "live": ("analysis", "Live potentials",
        '<path d="M2.5 12h14M18.5 8.5l3.5 3.5-3.5 3.5"/><circle cx="5.5" cy="12" r="1.3" class="dot"/><circle cx="10.5" cy="12" r="1.3" class="dot"/>'),
    "fault": ("analysis", "Fault",
        '<path d="M2.5 12h6M15.5 12h6M13 4.5l-4 8h5.5l-3.5 7.5"/>'),
    "emc": ("analysis", "EMC",
        '<circle cx="12" cy="12" r="9" stroke-dasharray="2.5 2.2"/><path d="M6 12c1.2-3.5 2.4-3.5 3.6 0s2.4 3.5 3.6 0 2.4-3.5 3.6 0"/>'),
}

HEAD = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">'


def body(b):
    # dots are filled, never stroked heavier
    return b.replace('class="dot"', 'fill="currentColor" stroke="none"')


def main(out):
    os.makedirs(out, exist_ok=True)
    sprite = ['<svg xmlns="http://www.w3.org/2000/svg" style="display:none">']
    for name, (_, _, b) in ICONS.items():
        with open(os.path.join(out, name + ".svg"), "w") as f:
            f.write(HEAD + body(b) + "</svg>\n")
        sprite.append(
            f'<symbol id="ki-{name}" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
            f'stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{body(b)}</symbol>')
    sprite.append("</svg>")
    with open(os.path.join(out, "sprite.svg"), "w") as f:
        f.write("\n".join(sprite) + "\n")


if __name__ == "__main__":
    main(sys.argv[1])
