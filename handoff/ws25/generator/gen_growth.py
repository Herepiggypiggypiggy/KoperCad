"""Writes the IEC symbols added by WS-25 (library growth) into symbols/iec.

usage: python3 gen_growth.py <crates/novo-symbols/symbols>

Each new symbol is derived from an existing hand-maintained IEC symbol by
changing its artwork, function or behavior. Run it before gen_nfpa.py, which
draws the NFPA counterparts. The files it writes are the source of truth once
committed and may be edited by hand.
"""
import copy
import math
import os
import sys
import tomllib

from symlib import A, C, L, PL, T, clear_tag, write_symbol

ROOT = sys.argv[1]


def load(path):
    with open(os.path.join(ROOT, "iec", path + ".toml"), "rb") as f:
        return tomllib.load(f)


def make(path, base, name, description, function, keywords, aliases, graphics=None, behavior=None,
         connections=None):
    src = load(base)
    sym = copy.deepcopy(src)
    sym.pop("favorite", None)
    if connections is not None:
        sym["connections"] = connections
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


# ---- growth 2: three-wire sensors, beacon, encoder, fans

SENSOR_GLYPHS = {
    "inductive": lambda: [PL([[2, 5.5], [5, 8], [2, 10.5], [-1, 8]], True)],
    "capacitive": lambda: [L([0.5, 5.5], [0.5, 10.5]), L([3.5, 5.5], [3.5, 10.5])],
    "photoelectric": lambda: [L([-1.5, 10], [3, 5.5]), L([0.5, 10.5], [4.5, 6.5]),
                              PL([[3, 5.5], [1.3, 5.9], [2.6, 7.2]], True, True),
                              PL([[4.5, 6.5], [2.8, 6.9], [4.1, 8.2]], True, True)],
}
SENSOR_ALIASES = {("inductive", "pnp"): "ind", ("inductive", "npn"): "indn",
                  ("capacitive", "pnp"): "cap", ("capacitive", "npn"): "capn",
                  ("photoelectric", "pnp"): "photo", ("photoelectric", "npn"): "photon"}
wires = [{"id": "1", "at": [0, 0], "dir": "up"}, {"id": "3", "at": [16, 0], "dir": "up"},
         {"id": "4", "at": [8, 16], "dir": "down"}]
for kind, glyph in SENSOR_GLYPHS.items():
    for out in ("pnp", "npn"):
        make(f"sensors/{kind}_{out}", "sensors/transmitter",
             f"{kind.capitalize()} sensor, 3-wire {out.upper()}",
             f"Three-wire {kind} sensor with a {out.upper()} switching output: 1 brown (+), 3 blue (-), "
             "4 black (signal).", "sensor." + kind,
             [kind, "sensor", "3-wire", out, "proximity"], [SENSOR_ALIASES[(kind, out)]],
             [L([0, 0], [0, 4]), L([16, 0], [16, 4]), L([8, 12], [8, 16]),
              {"kind": "rect", "at": [-4, 4], "size": [24, 8]}] + glyph()
             + [T([13, 8], out.upper(), 2.4)],
             {"type": "sensor", "poles": [["1", "3"]]}, wires)

rays = [L([3.68, 4.32], [4.95, 3.05]), L([-3.68, 4.32], [-4.95, 3.05]),
        L([3.68, 11.68], [4.95, 12.95]), L([-3.68, 11.68], [-4.95, 12.95])]
make("signalling/beacon", "signalling/lamp", "Beacon",
     "Signal beacon or flashing light; set the colour as a device property.", "signal.beacon",
     ["beacon", "flashing light", "signal", "strobe"], ["beacon"],
     load("signalling/lamp")["graphics"] + rays)

make("sensors/encoder", "sensors/transmitter", "Encoder",
     "Incremental encoder with supply (+, 0V) and channels A, B and zero pulse Z.",
     "sensor.encoder", ["encoder", "incremental", "rotary", "sensor", "speed"], ["enc"],
     [L([x, 0], [x, 4]) for x in (0, 8, 16, 24, 32)]
     + [{"kind": "rect", "at": [-4, 4], "size": [40, 10]}, T([16, 9], "ENC", 3)],
     {"type": "encoder", "poles": [["+", "0V"]]},
     [{"id": i, "at": [x, 0], "dir": "up"} for i, x in
      (("+", 0), ("0V", 8), ("A", 16), ("B", 24), ("Z", 32))])


def fan(base, path, name, function, alias, text_drop):
    src = load(base)
    circle = next(g for g in src["graphics"] if g["kind"] == "circle")
    cx, cy = circle["center"]
    g = [x for x in src["graphics"] if x["kind"] != "text"]
    g += [L([cx, cy], [cx + 3.2 * math.cos(math.radians(a)), cy - 3.2 * math.sin(math.radians(a))])
          for a in (90, 210, 330)] + [C([cx, cy], 0.8)]
    make(path, base, name, name + ".", function, ["fan", "blower", "ventilator", "cooling", "motor"],
         [alias], g)


fan("motors/motor_1ph", "motors/fan_1ph", "Fan, single-phase", "motor.ac1.fan", "fan", True)
fan("motors/motor_dc", "motors/fan_dc", "Fan, DC", "motor.dc.fan", "fandc", True)
