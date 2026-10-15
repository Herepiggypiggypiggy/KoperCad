"""Writes the IEC symbols added by WS-25 (library growth) into symbols/iec.

usage: python3 gen_growth.py <crates/novo-symbols/symbols>

Each new symbol is derived from an existing hand-maintained IEC symbol by
changing its artwork, function or behavior. Run it before gen_nfpa.py, which
draws the NFPA counterparts. The files it writes are the source of truth once
committed and may be edited by hand.
"""
import copy
import os
import sys
import tomllib

from symlib import A, C, L, PL, T, clear_tag, write_symbol

ROOT = sys.argv[1]


def load(path):
    with open(os.path.join(ROOT, "iec", path + ".toml"), "rb") as f:
        return tomllib.load(f)


def make(path, base, name, description, function, keywords, aliases, graphics=None, behavior=None):
    src = load(base)
    sym = copy.deepcopy(src)
    sym.pop("favorite", None)
    sym.update(id="iec/" + path, name=name, description=description, function=function,
               keywords=keywords, aliases=aliases)
    if graphics is not None:
        sym["graphics"] = graphics
    if behavior:
        sym["behavior"] = {**sym["behavior"], **behavior}
    sym["anchors"] = clear_tag(sym["anchors"], sym["graphics"])
    write_symbol(os.path.join(ROOT, "iec", path + ".toml"), sym)


def keep(base, drop_polyline=False):
    g = load(base)["graphics"]
    return [x for x in g if not (drop_polyline and x["kind"] == "polyline" and len(x["points"]) == 4)]


push_glyph = [{"kind": "polyline", "points": [[-6.5, 6], [-8, 6], [-8, 10], [-6.5, 10]]}]

# Operators: actuator variants of the pushbutton.
for kind, base in (("no", "operators/pushbutton_no"), ("nc", "operators/pushbutton_nc")):
    n = {"no": "make", "nc": "break"}[kind]
    code = {"no": "13/14", "nc": "11/12"}[kind]
    make(f"operators/pushbutton_mushroom_{kind}", base, f"Mushroom push button, {n} contact",
         "Push button with a mushroom head (palm button), not latching, not an emergency stop.",
         "contact." + kind + ".push", ["mushroom", "palm", "push button", n, "pb"],
         ["pbm"] if kind == "no" else ["pbmnc"],
         keep(base) + [A([-8, 8], 2, 90, 270)])
    make(f"operators/footswitch_{kind}", base, f"Foot switch, {n} contact",
         "Foot-operated switch (pedal).", "contact." + kind + ".foot",
         ["foot", "pedal", "switch", n], ["fs"] if kind == "no" else ["fsnc"],
         keep(base, True) + [PL([[-8, 6], [-8, 10], [-11, 10], [-11, 12]])])
    make(f"operators/pullcord_{kind}", base, f"Pull cord switch, {n} contact",
         "Switch operated by pulling a cord, such as a conveyor safety rope.",
         "contact." + kind + ".pull", ["pull cord", "rope", "switch", n],
         ["pull"] if kind == "no" else ["pullnc"],
         keep(base, True) + [C([-9.5, 8], 1.5)])

# Sensors: flow switch.
for kind, base in (("no", "sensors/float_no"), ("nc", "sensors/float_nc")):
    n = {"no": "make", "nc": "break"}[kind]
    g = [x for x in load(base)["graphics"] if x["kind"] != "circle"
         and not (x["kind"] == "line" and x["from"][0] <= -8 and x.get("stroke") != "dashed")]
    make(f"sensors/flow_{kind}", base, f"Flow switch, {n} contact",
         "Contact operated by a flow of liquid or air.", "contact." + kind + ".flow",
         ["flow", "flow switch", "sensor", n], ["flow"] if kind == "no" else ["flownc"],
         [x for x in g if x["kind"] != "polyline" or len(x["points"]) != 4] + [T([-10.5, 8], "Q", 3)])

# Coils.
rel = load("coils/relay")["graphics"]
for path, label, name, function, aliases, kw in (
        ("coils/brake", "B", "Brake coil", "coil.brake", ["brake"], ["brake", "holding brake", "coil"]),
        ("coils/solenoid", "Y", "Solenoid coil", "coil.solenoid", ["sol"], ["solenoid", "magnet", "actuator"])):
    make(path, "coils/relay", name, name + ".", function, kw + ["a1", "a2"], aliases,
         rel + [T([0, 8], label, 3)])

# Meters.
vm = load("measuring/voltmeter")["graphics"]
for path, letter, name, quantity, aliases in (
        ("measuring/pressure_gauge", "p", "Pressure gauge", "pressure", ["pg"]),
        ("measuring/tachometer", "n", "Tachometer", "speed", ["tacho"]),
        ("measuring/flow_meter", "Q", "Flow meter", "flow", ["fm"])):
    g = [dict(x, text=letter) if x["kind"] == "text" else x for x in vm]
    make(path, "measuring/voltmeter", name, f"{name} (measures {quantity}).", "meter." + quantity,
         [quantity, "meter", "measurement"], aliases, g, {"quantity": quantity})

# Coloured indicator lamps: the IEC 60757 colour code sits beside the lamp.
for colour, code, alias in (("red", "RD", "lr"), ("green", "GN", "lg"), ("amber", "YE", "la"),
                            ("white", "WH", "lw"), ("blue", "BU", "lb")):
    g = load("signalling/lamp")["graphics"] + [T([5, 8], code, 2.2, "left")]
    make(f"signalling/lamp_{colour}", "signalling/lamp", f"Indicator lamp, {colour}",
         f"Indicator lamp with its colour ({code}) written beside it.", "signal.lamp." + colour,
         ["lamp", "indicator", colour, code.lower()], [alias], g)
