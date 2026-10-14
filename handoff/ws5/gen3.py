# Third batch, executed inside gen.py's namespace.

# ---------------------------------------------------------------- switches & protection

for n in (2, 4):
    multipole(f"switches/disconnector_{n}p", f"Disconnector, {n}-pole",
              f"{n}-pole disconnector (isolator) for isolating a circuit without load.",
              "switch.disconnector", n, ["disconnector", "isolator", f"{n}p"], mark="disconnector",
              behavior={"kind": "contact", "normally": "open", "latching": True})

g = []
for i in range(4):
    g += fuse_pole(8 * i)
emit("protection/fuse_4p", "Fuse, 4-pole",
     "Four fuses of one fuse base or fuse holder sharing a device tag.",
     "protection.fuse", ["fuse", "4p", "fuse base"],
     [(str(2 * i + k), [8 * i, 16 * (k - 1)], "up" if k == 1 else "down") for i in range(4) for k in (1, 2)],
     [TAG_LEFT(-4)], g,
     {"kind": "fuse", "poles": [[str(2 * i + 1), str(2 * i + 2)] for i in range(4)]})

g = fixed(0, "no", "disconnector") + blade(0, "no")[0] + fuse_on_blade(0)
emit("protection/fuse_switch_1p", "Fuse switch-disconnector, 1-pole",
     "1-pole switch-disconnector whose moving contact carries the fuse link.",
     "protection.fuse_switch", ["fuse switch", "fuse disconnector", "1p"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-6)], g,
     {"kind": "fuse", "switched": True, "poles": [["1", "2"]]})

# I-0-II changeover switch: three changeover poles 12 mm apart
g = []
conns = []
poles = []
attaches = []
for k in range(3):
    x = 12 * k
    g += [L([x, 16], [x, 11]), L([x, 11], [x + 3.4, 4.5]),
          PL([[x - 4, 0], [x - 4, 5], [x - 1.5, 5]]),
          PL([[x + 4, 0], [x + 4, 5], [x + 1.5, 5]])]
    attaches.append([x + 3.4 * 3 / 6.5, 8])
    c, b, m = f"{k + 1}1", f"{k + 1}2", f"{k + 1}4"
    conns += [(c, [x, 16], "down"), (b, [x + 4, 0], "up"), (m, [x - 4, 0], "up")]
    poles.append([c, b, m])
g.append(L(attaches[0], attaches[-1], dashed=True))
g.append(L(attaches[0], [-8, 8], dashed=True))
g.append(PL([[-6.8, 6], [-8, 6], [-8, 10], [-9.2, 10]]))
emit("switches/changeover_switch_3p", "Changeover switch, 3-pole (I-0-II)",
     "3-pole changeover switch, for example a manual mains/generator transfer switch.",
     "switch.changeover", ["changeover switch", "transfer switch", "i-0-ii", "3p", "selector"],
     conns, [TAG_LEFT(-10)], g,
     {"kind": "contact", "normally": "changeover", "latching": True, "center_off": True, "poles": poles})

# ---------------------------------------------------------------- terminals & connectivity

g = []
for i in range(3):
    x = 8 * i
    g += [L([x, 0], [x, 2.8]), C([x, 4], 1.2), L([x, 5.2], [x, 8])]
g.append({"kind": "rect", "at": [-2, 1.8], "size": [20, 4.4], "stroke": "dashed"})
emit("terminals/terminal_3level", "Triple-level terminal",
     "Three-level terminal: three independent terminals stacked in one block.",
     "terminal.multilevel", ["terminal", "triple level", "three level", "3 tier"],
     [(str(2 * i + k), [8 * i, 8 * (k - 1)], "up" if k == 1 else "down") for i in range(3) for k in (1, 2)],
     [TAG_LEFT(-3, 4)], g,
     {"kind": "conductor", "poles": [["1", "2"], ["3", "4"], ["5", "6"]]})

for n in (3, 4):
    xs = [8 * i for i in range(n)]
    g = [L([x, 0], [x, 3]) for x in xs] + [L([0, 3], [xs[-1], 3])] + [C([x, 3], 0.6, fill=True) for x in xs]
    emit(f"connectivity/bridge_{n}", f"Terminal bridge, {n} positions",
         f"Jumper bridge linking {n} adjacent terminals.",
         "connectivity.bridge", ["bridge", "jumper", "link", "terminal bridge", f"{n}"],
         [(str(i + 1), [x, 0], "up") for i, x in enumerate(xs)], [TAG_LEFT(-3, 3)], g,
         {"kind": "conductor", "poles": [[str(i + 1) for i in range(n)]]})

# ---------------------------------------------------------------- passive & semiconductors

two_lead("passive/thermistor_ntc", "NTC thermistor",
         "Thermistor with negative temperature coefficient, for temperature sensing or inrush limiting.",
         "passive.resistor.ntc", ["ntc", "thermistor", "temperature", "inrush"],
         [R([-1.5, 4], [3, 8]), PL([[-3.5, 12], [-2.5, 12], [3, 5]]), T([4, 10.5], "-θ", 2.2, "left")],
         {"kind": "passive", "type": "thermistor"})


def light_arrows(x, y):
    # two arrows pointing in towards the component from the upper right
    out = []
    for dy in (0, 2):
        tip = [x, y + dy]
        out += [L([x + 2.5, y + dy - 2], tip),
                PL([tip, [x + 0.4, y + dy - 1.2], [x + 1.2, y + dy - 0.4]], closed=True, fill=True)]
    return out


two_lead("passive/photoresistor", "Light-dependent resistor",
         "Photoresistor (LDR) whose resistance falls with light.",
         "passive.resistor.ldr", ["ldr", "photoresistor", "light sensor", "twilight"],
         [R([-1.5, 4], [3, 8])] + light_arrows(2, 6),
         {"kind": "passive", "type": "resistor"})
two_lead("passive/capacitor_variable", "Variable capacitor", "Capacitor with adjustable value.",
         "passive.capacitor.variable", ["capacitor", "variable", "trimmer"],
         [L([-3, 7], [3, 7]), L([-3, 9], [3, 9]), L([-3, 11], [3, 5]),
          PL([[3, 5], [1.6, 5.4], [2.6, 6.4]], closed=True, fill=True)],
         {"kind": "passive", "type": "capacitor"}, top=7, bottom=9)
two_lead("passive/photodiode", "Photodiode", "Photodiode, conducting in reverse when lit.",
         "passive.diode.photo", ["photodiode", "light sensor", "diode"],
         DIODE + light_arrows(2.5, 6.5),
         {"kind": "passive", "type": "diode"}, ids=("A", "K"), top=6, bottom=10)

g = [L([0, 0], [0, 6])] + DIODE + [L([0, 10], [0, 16]), L([-8, 12], [-3, 12]), L([-3, 12], [-1.5, 10])]
emit("passive/thyristor", "Thyristor",
     "Thyristor (SCR): conducts from anode to cathode once triggered at the gate.",
     "passive.thyristor", ["thyristor", "scr", "semiconductor"],
     [("A", [0, 0], "up"), ("K", [0, 16], "down"), ("G", [-8, 12], "left")], [("tag", [4, 8], "left")], g,
     {"kind": "passive", "type": "thyristor", "poles": [["A", "K"]]})
g = [L([0, 0], [0, 6]), L([-3, 6], [3, 6]), L([-3, 10], [3, 10]),
     PL([[-3, 6], [0, 6], [-1.5, 10]], closed=True), PL([[0, 10], [3, 10], [1.5, 6]], closed=True),
     L([0, 10], [0, 16]), L([-8, 12], [-3, 12]), L([-3, 12], [-2, 10])]
emit("passive/triac", "Triac",
     "Triac: bidirectional thyristor for AC switching, triggered at the gate.",
     "passive.triac", ["triac", "ac switch", "semiconductor"],
     [("A1", [0, 0], "up"), ("A2", [0, 16], "down"), ("G", [-8, 12], "left")], [("tag", [4.5, 8], "left")], g,
     {"kind": "passive", "type": "triac", "poles": [["A1", "A2"]]})

g = []
for i in range(3):
    x = 8 * i
    g += [L([x, 0], [x, 4]), R([x - 1.5, 4], [3, 8]), L([x, 12], [x, 16])]
g.append(L([0, 16], [16, 16]))
emit("passive/heater_3ph", "Heating element, 3-phase",
     "Three-phase heater with its elements connected in star.",
     "load.heater", ["heater", "3 phase", "heating", "star"],
     [("1", [0, 0], "up"), ("2", [8, 0], "up"), ("3", [16, 0], "up")], [TAG_LEFT(-4)], g,
     {"kind": "load", "type": "resistive"})

# ---------------------------------------------------------------- sources & measuring

two_lead("sources/current_source", "Current source", "Ideal current source.",
         "source.current", ["current source", "source"],
         [C([0, 8], 4), L([0, 10.5], [0, 5.5]), PL([[0, 5.5], [-1, 7], [1, 7]], closed=True, fill=True)],
         {"kind": "source", "type": "current"}, top=4, bottom=12)
for path, name, desc, fn, text, kw in [
    ("measuring/thermometer", "Temperature indicator", "Temperature-measuring instrument.",
     "meter.temperature", "θ", ["thermometer", "temperature", "indicator", "meter"]),
    ("measuring/ohmmeter", "Ohmmeter", "Resistance-measuring instrument, such as an insulation monitor.",
     "meter.resistance", "Ω", ["ohmmeter", "resistance", "insulation", "meter"]),
]:
    two_lead(path, name, desc, fn, kw, [C([0, 8], 4), T([0, 8], text, 3.5)],
             {"kind": "meter", "quantity": fn.split(".")[1]}, top=4, bottom=12)

# ---------------------------------------------------------------- PLC

g, attach = blade(0, "no")
g = fixed(0, "no") + g + [R([-6, 2], [10, 12])]
emit("plc/relay_output", "PLC relay output",
     "One relay output channel of a PLC card: a potential-free make contact.",
     "plc.output.relay", ["plc", "relay output", "do", "contact", "i/o"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")],
     [TAG_LEFT(-8), ("address", [6, 8], "left")], g,
     {"kind": "contact", "normally": "open", "poles": [["1", "2"]]})
