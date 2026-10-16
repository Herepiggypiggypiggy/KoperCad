"""Writes the NFPA pack (symbols/nfpa) from the IEC pack.

usage: python3 gen_nfpa.py <crates/novo-symbols/symbols>

Everything the model and the engines read (function, behavior, connection
ids, order, roles, aliases, favourites) is taken from the IEC counterpart, so
the two packs cannot disagree; this script only supplies NFPA artwork. A
symbol with no entry below has no NFPA symbol yet. See
docs/design/symbol-packs.md.
"""
import fnmatch
import glob
import os
import sys
import tomllib

from symlib import A, C, L, PL, T, clear_tag, write_symbol

ROOT = sys.argv[1]
STANDARD = "NFPA 79 / ANSI Y32.2"

# ----------------------------------------------------------------- reading

IEC = {}
for f in sorted(glob.glob(os.path.join(ROOT, "iec", "*", "*.toml"))):
    d = tomllib.load(open(f, "rb"))
    IEC[d["id"][len("iec/"):]] = d


def xs_of(src):
    """x of every pole's top connection, left to right."""
    return sorted({c["at"][0] for c in src["connections"] if c["at"][1] == 0})


# ----------------------------------------------------------------- building blocks
# A contact runs top to bottom over 16 mm. NFPA draws it as two bars across
# the conductor, 4 mm apart; a normally closed contact adds a diagonal slash.


def bars(x, nc=False):
    g = [L([x, 0], [x, 6]), L([x, 16], [x, 10]), L([x - 3, 6], [x + 3, 6]), L([x - 3, 10], [x + 3, 10])]
    if nc:
        g.append(L([x - 3, 12], [x + 3, 4]))
    return g


def changeover(head=None):
    """Make side on the left (14), break side on the right (12), common below (11)."""
    g = [
        L([-4, 0], [-4, 6]), L([4, 0], [4, 6]),
        L([-5.5, 6], [-2.5, 6]), L([-5.5, 10], [-2.5, 10]),
        L([2.5, 6], [5.5, 6]), L([2.5, 10], [5.5, 10]),
        L([2.5, 12], [5.5, 4]),
        PL([[-4, 10], [-4, 13], [4, 13], [4, 10]]), L([0, 13], [0, 16]),
    ]
    if head:
        g += head()
    return g


def stem(end_x, x=0):
    return [L([x - 1, 8], [end_x, 8])]


def flat_head():
    return stem(-8) + [L([-8, 5], [-8, 11])]


def mushroom_head():
    return stem(-8) + [A([-8, 8], 3.5, 90, 270), L([-8, 4.5], [-8, 11.5])]


def knob_head():
    return stem(-7) + [C([-9, 8], 2)]


def key_head():
    return knob_head() + [PL([[-11, 8], [-13.5, 8], [-13.5, 9.5]])]


def roller_head():
    return stem(-7) + [C([-8.4, 8], 1.4)]


def thermal_head():
    return stem(-4) + [PL([[-4, 8], [-5.5, 5.5], [-7.5, 10.5], [-9.5, 5.5], [-11, 8]])]


def delay_head(on):
    if on:
        return stem(-8) + [A([-6, 8], 2, 90, 270)]
    return stem(-8) + [A([-10, 8], 2, -90, 90)]


def letter_head(letter):
    return stem(-7) + [T([-9, 8], letter, 3)]


def diamond_head():
    return stem(-5) + [PL([[-5, 8], [-7.5, 5.5], [-10, 8], [-7.5, 10.5]], closed=True)]


def float_head():
    return stem(-6) + [C([-8, 8], 2, fill=True)]


def contact(nc, head=None):
    return lambda src: bars(0, nc) + (head() if head else [])


def contact_co(head=None):
    return lambda src: changeover(head)


def multi(pole_fn, link=True, head=None):
    """The same pole at every x, joined by a dashed link."""
    def build(src):
        xs = xs_of(src)
        g = []
        for x in xs:
            g += pole_fn(x)
        if link and len(xs) > 1:
            g.append(L([xs[0], 8], [xs[-1], 8], dashed=True))
        if head:
            g += head()
        return g
    return build


def blade(x, mark=None):
    g = [L([x, 0], [x, 5]), L([x, 16], [x, 11]), L([x, 11], [x - 4, 4])]
    if mark == "arc":
        g.append(A([x, 3.5], 1.5, 180, 360))
    return g


def blade_arc(x):
    return blade(x, "arc")


def fuse_s(x):
    """An S-curve between two leads."""
    return [
        L([x, 0], [x, 4]),
        A([x, 6], 2, 270, 450),
        A([x, 10], 2, 90, 270),
        L([x, 12], [x, 16]),
    ]


def fuse_switch_pole(x):
    return [
        L([x, 0], [x, 1.5]), A([x, 3], 1.5, 270, 450), A([x, 6], 1.5, 90, 270),
        L([x, 7.5], [x, 10.5]), L([x - 3, 10.5], [x + 3, 10.5]),
        L([x - 3, 13.5], [x + 3, 13.5]), L([x, 13.5], [x, 16]),
    ]


def breaker_pole(x):
    return bars(x) + [A([x, 8], 4.6, 135, 225)]


def heater_pole(x):
    return [
        L([x, 0], [x, 4]),
        PL([[x, 4], [x - 2, 5], [x + 2, 7], [x - 2, 9], [x + 2, 11], [x, 12]]),
        L([x, 12], [x, 16]),
    ]


def coil(text, size=2.5):
    def build(src):
        g = [L([0, 0], [0, 4]), C([0, 8], 4), L([0, 12], [0, 16])]
        if text:
            g.append(T([0, 8], text, size))
        return g
    return build


def motor(src):
    return [g for g in src["graphics"]
            if not (g["kind"] == "text" and ("~" in g["text"] or "=" in g["text"]))]


def transformer_1ph(src):
    return [
        L([0, 0], [0, 10]), L([8, 0], [8, 10]),
        A([2, 10], 2, 180, 360), A([6, 10], 2, 180, 360),
        L([0, 15.5], [8, 15.5]), L([0, 16.5], [8, 16.5]),
        L([0, 22], [0, 32]), L([8, 22], [8, 32]),
        A([2, 22], 2, 0, 180), A([6, 22], 2, 0, 180),
    ]


# ----------------------------------------------------------------- the pack

REDRAW = {}


def add(path, fn):
    REDRAW[path] = fn


add("contacts/no", contact(False))
add("contacts/nc", contact(True))
add("contacts/changeover", contact_co())
add("contacts/no_delay_on", contact(False, lambda: delay_head(True)))
add("contacts/no_delay_off", contact(False, lambda: delay_head(False)))
add("contacts/nc_delay_on", contact(True, lambda: delay_head(True)))
add("contacts/nc_delay_off", contact(True, lambda: delay_head(False)))
add("contacts/position_no", contact(False, roller_head))
add("contacts/position_nc", contact(True, roller_head))
add("contacts/thermal_no", contact(False, thermal_head))
add("contacts/thermal_nc", contact(True, thermal_head))

add("operators/pushbutton_no", contact(False, flat_head))
add("operators/pushbutton_nc", contact(True, flat_head))
add("operators/pushbutton_changeover", contact_co(flat_head))
add("operators/emergency_stop", contact(True, mushroom_head))
add("operators/selector_no", contact(False, knob_head))
add("operators/selector_nc", contact(True, knob_head))
add("operators/keyswitch_no", contact(False, key_head))
add("operators/keyswitch_nc", contact(True, key_head))

add("sensors/float_no", contact(False, float_head))
add("sensors/float_nc", contact(True, float_head))
add("sensors/pressure_no", contact(False, lambda: letter_head("P")))
add("sensors/pressure_nc", contact(True, lambda: letter_head("P")))
add("sensors/temperature_no", contact(False, lambda: letter_head("T")))
add("sensors/temperature_nc", contact(True, lambda: letter_head("T")))
add("sensors/proximity_no", contact(False, diamond_head))
add("sensors/proximity_nc", contact(True, diamond_head))

def foot_head():
    return flat_head() + [L([-8, 11], [-11, 11])]


def pull_head():
    return stem(-8) + [PL([[-10, 6], [-8, 6], [-8, 10], [-10, 10]])]


for kind, nc in (("no", False), ("nc", True)):
    add(f"operators/pushbutton_mushroom_{kind}", contact(nc, mushroom_head))
    add(f"operators/footswitch_{kind}", contact(nc, foot_head))
    add(f"operators/pullcord_{kind}", contact(nc, pull_head))
    add(f"sensors/flow_{kind}", contact(nc, lambda: letter_head("F")))

add("coils/brake", coil("BR"))
add("coils/solenoid", coil("S"))
add("coils/relay", coil("CR"))
add("coils/contactor", coil("M"))
add("coils/solenoid_valve", coil("SOL", 2.2))
add("coils/timer_on_delay", coil("TON", 2.2))
add("coils/timer_off_delay", coil("TOF", 2.2))

for n in (1, 2, 3, 4):
    add(f"contactors/main_{n}p", multi(bars))
    add(f"switches/disconnector_{n}p", multi(blade))
    add(f"switches/switch_disconnector_{n}p", multi(blade_arc))
    add("protection/fuse" + ("" if n == 1 else f"_{n}p"), multi(fuse_s, link=False))
    add(f"protection/mcb_{n}p", multi(breaker_pole))
for n in (3, 4):
    add(f"switches/main_switch_{n}p",
        multi(blade_arc, head=lambda: [L([-1.7, 8], [-8, 8]), L([-8, 5], [-8, 11])]))
add("protection/fuse_switch_1p", multi(fuse_switch_pole))
add("protection/fuse_switch_3p", multi(fuse_switch_pole))
add("protection/motor_circuit_protector",
    multi(breaker_pole, head=lambda: [T([-8, 8], "I>", 2.2, "right")]))
add("protection/motor_protection_breaker",
    multi(breaker_pole, head=lambda: [T([-8, 8], "TM", 2.2, "right")]))
add("protection/overload_relay", multi(heater_pole, link=False))

for p in ("motors/motor_1ph", "motors/motor_3ph", "motors/motor_3ph_6t", "motors/motor_dc",
          "motors/generator_3ph"):
    add(p, motor)
add("transformers/transformer_1ph", transformer_1ph)


# ---- breadth: passives, signalling, protection with N or earth leakage, transformers


def zig(x, y0=4, y1=12):
    """The zigzag NFPA draws for a resistance or heating element."""
    h = y1 - y0
    return PL([[x, y0], [x - 2, y0 + h / 8], [x + 2, y0 + 3 * h / 8], [x - 2, y0 + 5 * h / 8],
               [x + 2, y0 + 7 * h / 8], [x, y1]])


def resistive(src, drop_inner=False):
    """The IEC graphics with every resistor rectangle replaced by a zigzag."""
    g = []
    for item in src["graphics"]:
        if item["kind"] == "rect":
            g.append(zig(item["at"][0] + item["size"][0] / 2, item["at"][1],
                         item["at"][1] + item["size"][1]))
        elif drop_inner and item["kind"] == "line" and item["from"][1] == 4 and item["to"][1] == 12:
            continue
        else:
            g.append(item)
    return g


def potentiometer(src):
    return [
        L([0, 0], [0, 4]), zig(0), L([0, 12], [0, 16]),
        L([8, 0], [8, 8]), L([8, 8], [1, 8]), PL([[1, 8], [2.6, 7.2], [2.6, 8.8]], True, True),
    ]


def round_alarm(text, size):
    def build(src):
        return [L([0, 0], [0, 4]), C([0, 8], 4), T([0, 8], text, size), L([0, 12], [0, 16])]
    return build


def surge(src):
    return [
        L([0, 0], [0, 6]), PL([[-1.8, 6], [1.8, 6], [0, 7.7]], True, True),
        PL([[-1.8, 10], [1.8, 10], [0, 8.3]], True, True), L([0, 10], [0, 16]),
    ]


def inductor_core(src):
    return [g for g in src["graphics"] if not (g["kind"] == "line" and g["from"][0] == 2.5)] + [
        L([2.5, 4.5], [2.5, 11.5]), L([3.3, 4.5], [3.3, 11.5])]


def leakage(src):
    """Breaker poles with the IEC sensing ring, link and text; solid pole drawing replaced."""
    g = [i for i in src["graphics"] if not (i["kind"] == "line" and i.get("stroke") != "dashed")]
    for i in g:
        if i["kind"] == "text" and i["text"] == "IΔ":
            i["text"] = "GFI"
    for x in xs_of(src):
        g += breaker_pole(x) + [L([x, 16], [x, 24])]
    return g


def emc(src):
    return [i for i in src["graphics"] if i["kind"] != "text"] + [T([12, 12], "FILTER", 2.2)]


def transformer_3ph(src):
    g = []
    for x in (0, 8, 16):
        g += [L([x, 0], [x, 6]), L([x, 34], [x, 40])]
        for y in (7, 9, 11, 13):
            g.append(A([x, y], 1, -90, 90))
        for y in (27, 29, 31, 33):
            g.append(A([x, y], 1, -90, 90))
    g += [L([-2, 19], [18, 19]), L([-2, 21], [18, 21])]
    return g


for path in ("passive/resistor", "passive/thermistor_ntc", "passive/thermistor_ptc",
             "passive/photoresistor", "passive/resistor_variable", "passive/varistor",
             "passive/heater_3ph"):
    add(path, resistive)
add("passive/heater", lambda src: resistive(src, True))
add("passive/potentiometer", potentiometer)
add("passive/inductor_core", inductor_core)
add("signalling/horn", round_alarm("HORN", 1.8))
add("signalling/bell", round_alarm("BELL", 1.8))
add("signalling/buzzer", round_alarm("BZ", 2.5))
add("protection/surge_arrester", surge)
add("protection/mcb_1p_n", multi(breaker_pole))
add("protection/mcb_3p_n", multi(breaker_pole))
for kind in ("rcd", "rcbo"):
    for n in ("2p", "4p"):
        add(f"protection/{kind}_{n}", leakage)
add("motors/motor_3ph_dahlander", motor)
add("transformers/emc_filter", emc)
add("transformers/transformer_3ph", transformer_3ph)

# Same artwork in both standards: copied unchanged.
SAME = [
    "connectivity/*", "earth/*", "terminals/*", "measuring/*", "sources/*", "supplies/*",
    "drives/*", "plc/*", "sensors/pt100", "sensors/transmitter",
    "passive/bridge_rectifier", "passive/capacitor", "passive/capacitor_polarized",
    "passive/capacitor_variable", "passive/diode", "passive/led", "passive/photodiode",
    "passive/zener_diode", "passive/thyristor", "passive/triac", "passive/transistor_npn",
    "passive/transistor_pnp", "passive/inductor", "transformers/reactor_3ph",
    "switches/changeover_switch_3p",
    "sensors/inductive_*", "sensors/capacitive_*", "sensors/photoelectric_*", "sensors/encoder",
    "signalling/beacon", "motors/fan_1ph", "motors/fan_dc",
]

# New NFPA symbols: pilot lights carry the colour letter in the circle.
LAMPS = [("red", "R", "lr"), ("green", "G", "lg"), ("amber", "A", "la"),
         ("white", "W", "lw"), ("blue", "B", "lb")]


# ----------------------------------------------------------------- writing


def derive(path, src, graphics):
    sym = {k: src[k] for k in ("name", "description", "category", "function", "keywords",
                               "connections", "behavior")}
    sym["id"] = "nfpa/" + path
    sym["standard"] = STANDARD
    sym["keywords"] = list(src["keywords"]) + [k for k in ("nfpa", "ansi") if k not in src["keywords"]]
    for k in ("aliases", "favorite", "bridges"):
        if k in src:
            sym[k] = src[k]
    sym["graphics"] = graphics
    sym["anchors"] = clear_tag(src["anchors"], graphics)
    return sym


def pilot(src, colour, letter, alias):
    g = [L([0, 0], [0, 4]), C([0, 8], 4), L([0, 12], [0, 16])]
    if letter:
        g.append(T([0, 8], letter, 3))
    sym = derive("signalling/lamp" + (f"_{colour}" if colour else ""), src, g)
    sym["name"] = f"Pilot light, {colour}" if colour else "Pilot light"
    sym["function"] = "signal.lamp" + (f".{colour}" if colour else "")
    sym["description"] = ("NFPA pilot light" + (f", {colour}; the letter in the circle names the colour."
                                                   if colour else " (circle)."))
    sym["keywords"] = ["lamp", "pilot light", "indicator", colour or "pilot", "nfpa", "ansi"]
    sym["aliases"] = [alias]
    sym.pop("favorite", None)
    return sym


def main():
    out = os.path.join(ROOT, "nfpa")
    written = set()
    for path, fn in REDRAW.items():
        if path not in IEC:
            sys.exit(f"no IEC counterpart for {path}")
        write_symbol(os.path.join(out, path + ".toml"), derive(path, IEC[path], fn(IEC[path])))
        written.add(path)
    for path, src in IEC.items():
        if path not in written and any(fnmatch.fnmatch(path, pat) for pat in SAME):
            write_symbol(os.path.join(out, path + ".toml"), derive(path, src, src["graphics"]))
            written.add(path)
    lamp = IEC["signalling/lamp"]
    # The plain pilot light replaces the IEC lamp: circle, no X.
    sym = pilot(lamp, "", "", "lamp")
    sym["aliases"], sym["favorite"] = list(lamp.get("aliases", [])), lamp.get("favorite", False)
    write_symbol(os.path.join(out, "signalling/lamp.toml"), sym)
    written.add("signalling/lamp")
    for colour, letter, alias in LAMPS:
        path = f"signalling/lamp_{colour}"
        write_symbol(os.path.join(out, path + ".toml"), pilot(lamp, colour, letter, alias))
        written.add(path)
    for f in glob.glob(os.path.join(out, "*", "*.toml")):
        if os.path.relpath(f, out)[:-5] not in written:
            os.remove(f)
    print(f"{len(written)} NFPA symbols written")


main()
