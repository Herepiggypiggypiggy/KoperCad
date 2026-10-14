# Second batch of symbols, executed inside gen.py's namespace.

# ---------------------------------------------------------------- operators

single_contact("operators/keyswitch_no", "Key switch, make contact",
               "Key-operated selector switch with a normally open contact.",
               "contact.no.key", "no", ("13", "14"), ["key switch", "key", "selector", "no"], "key",
               behavior={"kind": "contact", "normally": "open", "latching": True})
single_contact("operators/keyswitch_nc", "Key switch, break contact",
               "Key-operated selector switch with a normally closed contact.",
               "contact.nc.key", "nc", ("11", "12"), ["key switch", "key", "selector", "nc"], "key",
               behavior={"kind": "contact", "normally": "closed", "latching": True})

g = [L([0, 16], [0, 11]), L([0, 11], [3.4, 4.5]),
     PL([[-4, 0], [-4, 5], [-1.5, 5]]),
     PL([[4, 0], [4, 5], [1.5, 5]])]
co_attach = [3.4 * 3 / 6.5, 8]
g += [L(co_attach, [-8, 8], dashed=True),
      PL([[-6.5, 6], [-8, 6], [-8, 10], [-6.5, 10]])]
emit("operators/pushbutton_changeover", "Push button, changeover contact",
     "Push button with a changeover contact: common 11, break 12, make 14.",
     "contact.co.push", ["push button", "changeover", "co", "button"],
     [("11", [0, 16], "down"), ("12", [4, 0], "up"), ("14", [-4, 0], "up")],
     [TAG_LEFT(-10)], g,
     {"kind": "contact", "normally": "changeover", "poles": [["11", "12", "14"]]})

# ---------------------------------------------------------------- sensors

single_contact("sensors/pressure_nc", "Pressure switch, break contact",
               "Pressure switch with a normally closed contact.",
               "contact.nc.pressure", "nc", ("21", "22"), ["pressure", "sensor", "nc"], "pressure")
single_contact("sensors/float_nc", "Float switch, break contact",
               "Level float switch with a normally closed contact.",
               "contact.nc.float", "nc", ("21", "22"), ["float", "level", "sensor", "nc"], "float")
single_contact("sensors/temperature_no", "Temperature switch, make contact",
               "Thermostat or thermal switch with a normally open contact that closes when hot.",
               "contact.no.temperature", "no", ("13", "14"), ["temperature", "thermostat", "sensor", "no"],
               "temperature")
two_lead("sensors/pt100", "Resistance temperature sensor",
         "Resistance temperature detector such as a Pt100 or Pt1000.",
         "sensor.temperature.rtd", ["pt100", "pt1000", "rtd", "temperature", "sensor"],
         [R([-1.5, 4], [3, 8]), L([-3, 11], [3, 5]), T([4, 10], "θ", 2.5, "left")],
         {"kind": "passive", "type": "resistor"})
box("sensors/transmitter", "Transmitter, 2-wire",
    "Two-wire measuring transmitter with a 4-20 mA loop output, such as a pressure transmitter.",
    "sensor.transmitter", ["transmitter", "4-20ma", "pressure transmitter", "sensor", "loop"],
    ["+", "-"], [], [((0.5, 0.5), "4-20 mA", 2.2)],
    {"kind": "load", "type": "transmitter", "poles": [["+", "-"]]}, diagonal=False, height=16)

# ---------------------------------------------------------------- contactors & protection

multipole("contactors/main_2p", "Contactor main contacts, 2-pole",
          "2-pole main (power) contacts of a contactor, switched by the coil with the same tag.",
          "contact.main", 2, ["contactor", "main contact", "power", "2p"], mark="contactor",
          behavior={"kind": "contact", "normally": "open"})
multipole("protection/mcb_3p_n", "Miniature circuit breaker, 3-pole + N",
          "3-pole + neutral miniature circuit breaker with a switched, unprotected neutral.",
          "protection.mcb", 4, ["mcb", "circuit breaker", "3p+n", "neutral"],
          mark="breaker", actuator_kind="breaker_trip", n_pole=True,
          behavior={"kind": "breaker", "trip": ["thermal", "magnetic"]})
multipole("protection/motor_circuit_protector", "Motor circuit protector, 3-pole",
          "3-pole circuit breaker with magnetic (short-circuit) trip only, used with a separate overload relay.",
          "protection.mcp", 3, ["mcp", "magnetic only", "motor circuit protector", "short circuit"],
          mark="breaker", actuator_kind="magnetic",
          behavior={"kind": "breaker", "trip": ["magnetic"]})
rcd("protection/rcbo_4p", "RCBO, 3-pole + N",
    "Residual current breaker with overcurrent protection (RCBO), 3-pole + neutral.",
    4, True, ["rcbo", "residual current", "breaker", "3p+n"], ["residual", "thermal", "magnetic"])
g = []
for i in range(2):
    g += fuse_pole(8 * i)
emit("protection/fuse_2p", "Fuse, 2-pole",
     "Two fuses of one fuse base or fuse holder sharing a device tag.",
     "protection.fuse", ["fuse", "2p", "fuse base"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down"), ("3", [8, 0], "up"), ("4", [8, 16], "down")],
     [TAG_LEFT(-4)], g, {"kind": "fuse", "poles": [["1", "2"], ["3", "4"]]})


def fuse_on_blade(x):
    # fuse body drawn along the moving blade from (x, 11) to (x - 4, 4)
    d = [-4 / math.hypot(4, 7), -7 / math.hypot(4, 7)]
    nrm = [-d[1], d[0]]
    c = [x - 2, 7.5]
    half, w = 2.4, 1.1
    pts = [[c[0] + d[0] * sl * half + nrm[0] * sw * w, c[1] + d[1] * sl * half + nrm[1] * sw * w]
           for sl, sw in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    return [PL(pts, closed=True)]


g = []
for i in range(3):
    x = 8 * i
    g += fixed(x, "no", "disconnector") + blade(x, "no")[0] + fuse_on_blade(x)
g.append(L([-4 * 3 / 7 + 0, 8], [16 - 4 * 3 / 7, 8], dashed=True))
emit("protection/fuse_switch_3p", "Fuse switch-disconnector, 3-pole",
     "3-pole switch-disconnector whose moving contacts carry the fuse links.",
     "protection.fuse_switch", ["fuse switch", "fuse disconnector", "nh", "3p"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down"), ("3", [8, 0], "up"), ("4", [8, 16], "down"),
      ("5", [16, 0], "up"), ("6", [16, 16], "down")], [TAG_LEFT(-6)], g,
     {"kind": "fuse", "switched": True, "poles": [["1", "2"], ["3", "4"], ["5", "6"]]})
two_lead("protection/surge_arrester", "Surge arrester",
         "Surge protective device (SPD) between a conductor and earth.",
         "protection.surge", ["surge arrester", "spd", "overvoltage", "lightning"],
         [R([-1.5, 4], [3, 8]), L([0, 5.5], [0, 10]), PL([[0, 10], [-0.8, 8.6], [0.8, 8.6]], closed=True, fill=True)],
         {"kind": "passive", "type": "surge_arrester"}, ids=("1", "PE"))

# ---------------------------------------------------------------- switches

multipole("switches/disconnector_3p", "Disconnector, 3-pole",
          "3-pole disconnector (isolator) for isolating a circuit without load.",
          "switch.disconnector", 3, ["disconnector", "isolator", "3p"], mark="disconnector",
          behavior={"kind": "contact", "normally": "open", "latching": True})
for n in (1, 2):
    multipole(f"switches/switch_disconnector_{n}p", f"Switch-disconnector, {n}-pole",
              f"{n}-pole load-break switch-disconnector.",
              "switch.switch_disconnector", n, ["switch disconnector", "load break", "isolator", f"{n}p"],
              mark="switch_disconnector",
              behavior={"kind": "contact", "normally": "open", "latching": True})
multipole("switches/main_switch_4p", "Main switch, 4-pole",
          "4-pole main switch (rotary switch-disconnector) with handle, switching the neutral too.",
          "switch.main", 4, ["main switch", "rotary", "isolator", "repair switch", "4p"],
          mark="switch_disconnector", actuator_kind="turn",
          behavior={"kind": "contact", "normally": "open", "latching": True})

# ---------------------------------------------------------------- motors

motor3("motors/motor_3ph_dahlander", "Three-phase motor, two speeds (Dahlander)",
       "Pole-changing Dahlander motor: 1U 1V 1W for low speed, 2U 2V 2W for high speed.",
       "motor.ac3", ["motor", "dahlander", "two speed", "pole changing"], six=True,
       ids1=("1U", "1V", "1W"), ids2=("2U", "2V", "2W"),
       behavior={"kind": "motor", "phases": 3, "speeds": 2})
motor3("motors/generator_3ph", "Three-phase generator",
       "Three-phase AC generator with terminals U1, V1, W1 and protective earth.",
       "source.generator", ["generator", "genset", "alternator", "g"], letter="G",
       behavior={"kind": "source", "type": "ac", "phases": 3})

# ---------------------------------------------------------------- transformers & supplies


def coil_body(x, top=4):
    return [A([x, top + 1 + 2 * i], 1, -90, 90) for i in range(4)]


g = []
for i in range(3):
    x = 8 * i
    g += [L([x, 0], [x, 4])] + coil_body(x) + [L([x, 12], [x, 16])]
g.append(L([-2, 3], [18, 3]))
emit("transformers/reactor_3ph", "Line reactor, 3-phase",
     "Three-phase line or motor reactor (choke).",
     "passive.inductor.reactor", ["reactor", "choke", "line reactor", "motor reactor", "3 phase"],
     [("1U1", [0, 0], "up"), ("1U2", [0, 16], "down"), ("1V1", [8, 0], "up"), ("1V2", [8, 16], "down"),
      ("1W1", [16, 0], "up"), ("1W2", [16, 16], "down")], [TAG_LEFT(-3)], g,
     {"kind": "passive", "type": "inductor"},
     [["1U1", "1U2"], ["1V1", "1V2"], ["1W1", "1W2"]])
box("transformers/emc_filter", "EMC filter, 3-phase",
    "Three-phase mains (EMC) filter in front of a drive.",
    "passive.filter", ["emc filter", "line filter", "rfi", "mains filter"],
    ["L1", "L2", "L3", "PE"], ["L1'", "L2'", "L3'", "PE'"],
    [((0.5, 0.3), "~", 3), ((0.5, 0.55), "~", 3), ((0.35, 0.8), "~", 3)],
    {"kind": "passive", "type": "filter"},
    diagonal=False, roles={"PE": "pe", "PE'": "pe"},
    bridges=[["L1", "L1'"], ["L2", "L2'"], ["L3", "L3'"], ["PE", "PE'"]])
box("supplies/dc_dc_converter", "DC/DC converter",
    "DC/DC converter, for example 24 V to 12 V or an isolated supply.",
    "converter.dc_dc", ["dc/dc", "converter", "isolated", "dc"],
    ["+IN", "-IN"], ["+OUT", "-OUT"], [((0.25, 0.25), "=", 3.5), ((0.75, 0.75), "=", 3.5)],
    {"kind": "converter", "input": "dc", "output": "dc", "ports": [["+IN", "-IN"], ["+OUT", "-OUT"]]})
two_lead("sources/dc_source", "DC voltage source", "DC voltage source, such as a 24 V DC supply rail.",
         "source.dc", ["dc source", "supply", "voltage source", "24v"],
         [C([0, 8], 4), T([0, 8], "=", 4)],
         {"kind": "source", "type": "dc"}, ids=("+", "-"), top=4, bottom=12)

# ---------------------------------------------------------------- terminals & connectors

emit("terminals/terminal_n", "Neutral terminal",
     "Neutral (N) terminal of a terminal strip, usually blue.",
     "terminal.n", ["terminal", "n", "neutral", "blue"],
     [("1", [0, 0], "up", "n"), ("2", [0, 8], "down", "n")], [TAG_LEFT(-3, 4)],
     [L([0, 0], [0, 2.8]), C([0, 4], 1.2), L([0, 5.2], [0, 8]), T([2, 4], "N", 2, "left")],
     {"kind": "conductor", "poles": [["1", "2"]]})
emit("terminals/terminal_2level", "Double-level terminal",
     "Two-level terminal: two independent terminals stacked in one block.",
     "terminal.multilevel", ["terminal", "double level", "two level", "2 tier"],
     [("1", [0, 0], "up"), ("2", [0, 8], "down"), ("3", [8, 0], "up"), ("4", [8, 8], "down")],
     [TAG_LEFT(-3, 4)],
     [L([0, 0], [0, 2.8]), C([0, 4], 1.2), L([0, 5.2], [0, 8]),
      L([8, 0], [8, 2.8]), C([8, 4], 1.2), L([8, 5.2], [8, 8]),
      {"kind": "rect", "at": [-2, 1.8], "size": [12, 4.4], "stroke": "dashed"}],
     {"kind": "conductor", "poles": [["1", "2"], ["3", "4"]]})
emit("terminals/plug_socket", "Plug and socket, mated",
     "A mated contact pair of a plug connector.",
     "connector.pair", ["plug", "socket", "connector", "mated"],
     [("1", [0, 0], "up"), ("2", [0, 12], "down")], [TAG_LEFT(-3, 5)],
     [L([0, 0], [0, 2]), A([0, 4], 2, 180, 360), R([-0.8, 3.5], [1.6, 3], fill=True), L([0, 6.5], [0, 12])],
     {"kind": "conductor", "poles": [["1", "2"]]})

# ---------------------------------------------------------------- passive & semiconductors

two_lead("passive/thermistor_ptc", "PTC thermistor",
         "Thermistor with positive temperature coefficient, as used for motor winding protection.",
         "passive.resistor.ptc", ["ptc", "thermistor", "motor protection", "temperature"],
         [R([-1.5, 4], [3, 8]), PL([[-3.5, 12], [-2.5, 12], [3, 5]]), T([4, 10.5], "θ", 2.2, "left")],
         {"kind": "passive", "type": "thermistor"}, ids=("T1", "T2"))
two_lead("passive/varistor", "Varistor",
         "Voltage-dependent resistor (VDR) for overvoltage suppression.",
         "passive.resistor.varistor", ["varistor", "vdr", "mov", "suppressor"],
         [R([-1.5, 4], [3, 8]), PL([[-3.5, 12], [-2.5, 12], [3, 5]]), T([4, 10.5], "U", 2.2, "left")],
         {"kind": "passive", "type": "varistor"})
two_lead("passive/inductor_core", "Inductor with core",
         "Inductor with a magnetic core.",
         "passive.inductor", ["inductor", "choke", "core", "ferrite"],
         coil_body(0) + [L([2.5, 4.5], [2.5, 11.5])],
         {"kind": "passive", "type": "inductor"})


def transistor(path, name, desc, pnp):
    g = [L([0, 0], [0, 4]), L([-2, 6.5], [0, 4]), L([-2, 9.5], [0, 12]), L([0, 12], [0, 16]),
         L([-2, 5], [-2, 11]), L([-8, 8], [-2, 8]), C([-1, 8], 5)]
    if pnp:
        tip, base = [-1.8, 9.7], [-0.6, 11.4]
    else:
        tip, base = [0, 12], [-1.2, 10.3]
    d = [tip[0] - base[0], tip[1] - base[1]]
    nrm = [-d[1] * 0.45, d[0] * 0.45]
    g.append(PL([tip, [base[0] + nrm[0], base[1] + nrm[1]], [base[0] - nrm[0], base[1] - nrm[1]]],
                closed=True, fill=True))
    emit(path, name, desc, "passive.transistor." + ("pnp" if pnp else "npn"),
         ["transistor", "pnp" if pnp else "npn", "bipolar"],
         [("C", [0, 0], "up"), ("E", [0, 16], "down"), ("B", [-8, 8], "left")],
         [("tag", [4.5, 8], "left")], g, {"kind": "passive", "type": "transistor"})


transistor("passive/transistor_npn", "NPN transistor", "Bipolar NPN transistor.", False)
transistor("passive/transistor_pnp", "PNP transistor", "Bipolar PNP transistor.", True)

g = [L([0, 0], [0, 4]), L([8, 0], [8, 4]), R([-4, 4], [16, 12]), L([0, 16], [0, 20]), L([8, 16], [8, 20]),
     PL([[1.5, 8], [6.5, 8], [4, 12]], closed=True), L([1.5, 12], [6.5, 12])]
emit("passive/bridge_rectifier", "Bridge rectifier",
     "Single-phase bridge rectifier: AC in on the two ~ terminals, DC out on + and -.",
     "passive.rectifier", ["bridge rectifier", "rectifier", "diode bridge", "graetz"],
     [("~1", [0, 0], "up"), ("~2", [8, 0], "up"), ("+", [0, 20], "down"), ("-", [8, 20], "down")],
     [TAG_LEFT(-6, 10)], g,
     {"kind": "converter", "input": "ac", "output": "dc", "ports": [["~1", "~2"], ["+", "-"]]})

# ---------------------------------------------------------------- measuring

for path, name, desc, fn, text, kw in [
    ("measuring/frequency_meter", "Frequency meter", "Frequency-measuring instrument.", "meter.frequency",
     "Hz", ["frequency", "hz", "meter"]),
    ("measuring/wattmeter", "Wattmeter", "Active power measuring instrument.", "meter.power", "W",
     ["wattmeter", "power", "meter"]),
    ("measuring/power_factor_meter", "Power factor meter", "Power factor (cos φ) measuring instrument.",
     "meter.power_factor", "cosφ", ["power factor", "cos phi", "meter"]),
]:
    two_lead(path, name, desc, fn, kw, [C([0, 8], 4), T([0, 8], text, 2.8 if len(text) < 3 else 2)],
             {"kind": "meter", "quantity": fn.split(".")[1]}, top=4, bottom=12)
two_lead("measuring/hour_counter", "Hour counter", "Operating hours counter.", "meter.hours",
         ["hour counter", "hours", "operating time", "counter"],
         [R([-4, 4], [8, 8]), T([0, 8], "h", 3)],
         {"kind": "meter", "quantity": "hours"})

# ---------------------------------------------------------------- connectivity

emit("connectivity/interruption_point", "Interruption point",
     "Off-page connector: every interruption point with the same name is one electrical node.",
     "connectivity.interruption", ["interruption point", "off page", "continuation", "arrow", "link"],
     [("1", [0, 0], "up")], [("tag", [3, 6], "left")],
     [L([0, 0], [0, 4]), PL([[-2, 4], [0, 7], [2, 4]])],
     {"kind": "none"})
emit("connectivity/potential_feed", "Potential feed",
     "Potential definition point: names the potential (L1, N, 24V, ...) of the net it touches.",
     "connectivity.potential", ["potential", "feed", "supply", "net label", "l1", "24v"],
     [("1", [0, 8], "down")], [("tag", [3, 2], "left")],
     [PL([[-2, 0], [2, 0], [0, 3]], closed=True, fill=True), L([0, 3], [0, 8])],
     {"kind": "none"})
emit("connectivity/bridge_2", "Terminal bridge, 2 positions",
     "Jumper bridge linking two adjacent terminals.",
     "connectivity.bridge", ["bridge", "jumper", "link", "terminal bridge"],
     [("1", [0, 0], "up"), ("2", [8, 0], "up")], [TAG_LEFT(-3, 3)],
     [L([0, 0], [0, 3]), L([8, 0], [8, 3]), L([0, 3], [8, 3]), C([0, 3], 0.6, fill=True), C([8, 3], 0.6, fill=True)],
     {"kind": "conductor", "poles": [["1", "2"]]})
