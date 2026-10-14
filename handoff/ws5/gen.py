"""One-off generator for the first pass of the IEC symbol set.

The TOML files it writes are the source of truth from then on; this script is
only a drafting aid for the repetitive geometry.
"""
import math
import os
import sys

OUT = sys.argv[1]

# ---------------------------------------------------------------- emitting


def num(v):
    v = round(v, 3)
    if abs(v - round(v)) < 1e-9:
        return str(int(round(v)))
    return repr(v)


def pt(p):
    return f"[{num(p[0])}, {num(p[1])}]"


def tstr(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def gfx_line(g):
    k = g["kind"]
    parts = [f'kind = "{k}"']
    for key, val in g.items():
        if key == "kind":
            continue
        if key in ("from", "to", "at", "center", "size") and isinstance(val, list):
            parts.append(f"{key} = {pt(val)}")
        elif key == "points":
            parts.append("points = [" + ", ".join(pt(p) for p in val) + "]")
        elif isinstance(val, bool):
            parts.append(f"{key} = {'true' if val else 'false'}")
        elif isinstance(val, str):
            parts.append(f"{key} = {tstr(val)}")
        else:
            parts.append(f"{key} = {num(val)}")
    return "  { " + ", ".join(parts) + " },"


def tval(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return tstr(v)
    if isinstance(v, (int, float)):
        return num(v)
    if isinstance(v, list):
        return "[" + ", ".join(tval(x) for x in v) + "]"
    raise TypeError(v)


ALL = []


def gbox(g):
    k = g["kind"]
    if k == "line":
        pts = [g["from"], g["to"]]
    elif k == "polyline":
        pts = g["points"]
    elif k == "rect":
        pts = [g["at"], [g["at"][0] + g["size"][0], g["at"][1] + g["size"][1]]]
    elif k in ("circle", "arc"):
        c, r = g["center"], g["r"]
        pts = [[c[0] - r, c[1] - r], [c[0] + r, c[1] + r]]
    elif k == "ellipse":
        c = g["center"]
        pts = [[c[0] - g["rx"], c[1] - g["ry"]], [c[0] + g["rx"], c[1] + g["ry"]]]
    else:
        size = g.get("size", 2.5)
        w = len(g["text"]) * size * 0.6
        x = g["at"][0]
        x0 = {"left": x, "center": x - w / 2, "right": x - w}[g.get("align", "center")]
        pts = [[x0, g["at"][1] - size / 2], [x0 + w, g["at"][1] + size / 2]]
    xs = [q[0] for q in pts]
    ys = [q[1] for q in pts]
    return min(xs), min(ys), max(xs), max(ys)


def clear_tag(anchors, graphics):
    """Move right-aligned tag anchors left until they clear the artwork."""
    out = []
    for role, at, align in anchors:
        if align == "right":
            y = at[1]
            left = [gbox(g)[0] for g in graphics
                    if gbox(g)[1] <= y + 1.6 and gbox(g)[3] >= y - 1.6]
            if left:
                at = [min(at[0], math.floor((min(left) - 1.5) * 2) / 2), y]
        out.append((role, at, align))
    return out


exec(open("aliases.py").read())


def emit(path, name, description, function, keywords, conns, anchors, graphics, behavior, bridges=None):
    anchors = clear_tag(anchors, graphics)
    sid = "iec/" + path
    category = path.split("/")[0]
    lines = [
        "format = 1",
        f"id = {tstr(sid)}",
        f"name = {tstr(name)}",
        f"description = {tstr(description)}",
        'standard = "IEC 60617"',
        f"category = {tstr(category)}",
        f"function = {tstr(function)}",
        "keywords = [" + ", ".join(tstr(k) for k in keywords) + "]",
    ]
    extra = ALIASES.get(path, ())
    names = [a for a in extra if a != "*"]
    if names:
        lines.append("aliases = [" + ", ".join(tstr(a) for a in names) + "]")
    if "*" in extra:
        lines.append("favorite = true")
    lines += ["", "connections = ["]
    for c in conns:
        role = f", role = {tstr(c[3])}" if len(c) > 3 and c[3] else ""
        lines.append(f"  {{ id = {tstr(c[0])}, at = {pt(c[1])}, dir = \"{c[2]}\"{role} }},")
    lines += ["]", "", "anchors = ["]
    for a in anchors:
        lines.append(f"  {{ role = {tstr(a[0])}, at = {pt(a[1])}, align = \"{a[2]}\" }},")
    lines += ["]", ""]
    if bridges:
        lines.append("bridges = " + tval(bridges))
        lines.append("")
    lines += ["graphics = ["]
    lines += [gfx_line(g) for g in graphics]
    lines += ["]", "", "[behavior]"]
    for k, v in behavior.items():
        lines.append(f"{k} = {tval(v)}")
    full = os.path.join(OUT, "iec", path + ".toml")
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "w") as f:
        f.write("\n".join(lines) + "\n")
    ALL.append(sid)


# ---------------------------------------------------------------- primitives


def L(a, b, dashed=False):
    g = {"kind": "line", "from": a, "to": b}
    if dashed:
        g["stroke"] = "dashed"
    return g


def PL(points, closed=False, fill=False, dashed=False):
    g = {"kind": "polyline", "points": points}
    if closed:
        g["closed"] = True
    if fill:
        g["fill"] = True
    if dashed:
        g["stroke"] = "dashed"
    return g


def R(at, size, fill=False):
    g = {"kind": "rect", "at": at, "size": size}
    if fill:
        g["fill"] = True
    return g


def C(center, r, fill=False):
    g = {"kind": "circle", "center": center, "r": r}
    if fill:
        g["fill"] = True
    return g


def E(center, rx, ry, dashed=False):
    g = {"kind": "ellipse", "center": center, "rx": rx, "ry": ry}
    if dashed:
        g["stroke"] = "dashed"
    return g


def A(center, r, start, end):
    return {"kind": "arc", "center": center, "r": r, "start": start, "end": end}


def T(at, text, size=2.5, align="center"):
    g = {"kind": "text", "at": at, "text": text}
    if size != 2.5:
        g["size"] = size
    if align != "center":
        g["align"] = align
    return g


TAG_LEFT = lambda x=0, y=8: ("tag", [x, y], "right")

# ---------------------------------------------------------------- contacts
# A contact runs top to bottom over 16 mm: fixed contact lead on top
# (y 0..5), moving blade pivoting at y = 11, bottom lead y 11..16.

BLADE_Y = 8  # height where actuator links attach to the blade


def blade(x, kind):
    """Lines for one pole and the point on the blade where actuators attach."""
    g = [L([x, 16], [x, 11])]
    if kind == "no":
        tip = [x - 4, 4]
    else:  # nc: blade crosses to the right and rests on a stub
        tip = [x + 4, 4]
    g.append(L([x, 11], tip))
    t = (11 - BLADE_Y) / 7
    attach = [x + (tip[0] - x) * t, BLADE_Y]
    return g, attach


def fixed(x, kind, mark=None):
    """Upper lead plus the function mark at the fixed contact (y = 5)."""
    g = []
    top = 5
    if mark == "contactor":
        top = 3.5
        g.append(A([x, 5], 1.5, 180, 360))
    elif mark == "breaker":
        g += [L([x - 1.2, 3.8], [x + 1.2, 6.2]), L([x - 1.2, 6.2], [x + 1.2, 3.8])]
    elif mark == "disconnector":
        g.append(L([x - 1.5, 5], [x + 1.5, 5]))
    elif mark == "switch_disconnector":
        g.append(L([x - 1.5, 5], [x + 1.5, 5]))
        g.append(A([x, 3.5], 1.5, 180, 360))
        top = 2
    g.insert(0, L([x, 0], [x, top]))
    if kind == "nc":
        g.append(L([x, 5], [x + 3.5, 5]))
    return g


def actuator(kind, attach, ax=-8):
    """Actuator glyph left of the contact, joined to the blade by a dashed link."""
    y = BLADE_Y
    g = []
    if kind is None:
        return g
    if kind == "position":
        bx = attach[0]
        return [PL([[bx, y], [bx - 2, y - 1.2], [bx - 2, y + 1.2]], closed=True)]
    if kind in ("delay_on", "delay_off"):
        bx = attach[0]
        g += [L([bx, y - 0.8], [bx - 4, y - 0.8]), L([bx, y + 0.8], [bx - 4, y + 0.8])]
        # Movement towards the arc's centre is delayed. Operating moves the
        # blade to +x, so an on-delay arc opens to the right.
        if kind == "delay_on":
            g.append(A([bx - 4, y], 1.5, 90, 270))
        else:
            g.append(A([bx - 5.5, y], 1.5, -90, 90))
        return g
    g.append(L(attach, [ax, y], dashed=True))
    if kind == "push":
        g.append(PL([[ax + 1.5, y - 2], [ax, y - 2], [ax, y + 2], [ax + 1.5, y + 2]]))
    elif kind == "estop":
        g.append(PL([[ax + 1.5, y - 2], [ax, y - 2], [ax, y + 2], [ax + 1.5, y + 2]]))
        g.append(A([ax, y], 2, 90, 270))
    elif kind == "turn":
        g.append(PL([[ax + 1.2, y - 2], [ax, y - 2], [ax, y + 2], [ax - 1.2, y + 2]]))
    elif kind == "thermal":
        g += thermal_glyph(ax, y)
    elif kind == "breaker_trip":
        g += thermal_glyph(ax, y)
        g.append(T([ax - 6, y], "I>", 2.2, "right"))
    elif kind == "proximity":
        g.append(PL([[ax, y], [ax - 2, y - 2], [ax - 4, y], [ax - 2, y + 2]], closed=True))
        g.append(PL([[ax - 6, y - 2], [ax - 6, y + 2], [ax - 4.5, y + 2]]))
    elif kind == "pressure":
        g.append(T([ax - 1.5, y], "p", 3, "center"))
    elif kind == "temperature":
        g.append(T([ax - 1.5, y], "θ", 3, "center"))
    elif kind == "key":
        g += [PL([[ax + 1.2, y - 2], [ax, y - 2], [ax, y + 2], [ax - 1.2, y + 2]]),
              L([ax - 1.2, y], [ax - 3.5, y]), C([ax - 4.7, y], 1.2), L([ax - 2.5, y], [ax - 2.5, y + 1.2])]
    elif kind == "magnetic":
        g.append(T([ax - 0.5, y], "I>", 2.2, "right"))
    elif kind == "float":
        g.append(L([ax, y], [ax - 2, y + 2]))
        g.append(C([ax - 3, y + 3], 1.4))
    return g


def thermal_glyph(ax, y):
    return [PL([[ax, y], [ax, y - 2], [ax - 2, y - 2], [ax - 2, y + 2], [ax - 4, y + 2], [ax - 4, y]])]


def single_contact(path, name, desc, function, kind, ids, keywords, actuator_kind=None,
                   mark=None, behavior=None):
    g, attach = blade(0, kind)
    g = fixed(0, kind, mark) + g + actuator(actuator_kind, attach)
    conns = [(ids[0], [0, 0], "up"), (ids[1], [0, 16], "down")]
    tag_x = -10 if actuator_kind not in (None, "position", "delay_on", "delay_off") else -6
    if actuator_kind in ("proximity", "breaker_trip"):
        tag_x = -16
    if actuator_kind in ("thermal", "float"):
        tag_x = -14
    if actuator_kind in ("delay_on", "delay_off"):
        tag_x = -9
    beh = behavior or {"kind": "contact", "normally": "open" if kind == "no" else "closed"}
    beh.setdefault("poles", [[ids[0], ids[1]]])
    emit(path, name, desc, function, keywords, conns, [TAG_LEFT(tag_x)], g, beh)


def multipole(path, name, desc, function, n, keywords, mark=None, actuator_kind=None,
              behavior=None, pitch=8, extra=None, n_pole=False, length=16):
    g = []
    attaches = []
    conns = []
    for i in range(n):
        x = i * pitch
        ff = fixed(x, "no", mark)
        bl, at = blade(x, "no")
        g += ff + bl
        attaches.append(at)
        top = str(2 * i + 1)
        bot = str(2 * i + 2)
        if n_pole and i == n - 1:
            top, bot = "N", "N'"
        conns.append((top, [x, 0], "up", "n" if n_pole and i == n - 1 else None))
        conns.append((bot, [x, length], "down", "n" if n_pole and i == n - 1 else None))
        if length > 16:
            g.append(L([x, 16], [x, length]))
    if n > 1:
        g.append(L(attaches[0], attaches[-1], dashed=True))
    g += actuator(actuator_kind, attaches[0])
    if extra:
        g += extra
    tag_x = -10 if actuator_kind else -6
    if actuator_kind == "breaker_trip":
        tag_x = -16
    beh = dict(behavior or {"kind": "contact", "normally": "open"})
    beh["poles"] = [[conns[2 * i][0], conns[2 * i + 1][0]] for i in range(n)]
    emit(path, name, desc, function, keywords, conns, [TAG_LEFT(tag_x)], g, beh)


# ---------------------------------------------------------------- contacts

single_contact("contacts/no", "Make contact (NO)",
               "Normally open contact, used for auxiliary contacts of relays and contactors.",
               "contact.no", "no", ("13", "14"), ["no", "make", "normally open", "auxiliary"])
single_contact("contacts/nc", "Break contact (NC)",
               "Normally closed contact, used for auxiliary contacts of relays and contactors.",
               "contact.nc", "nc", ("21", "22"), ["nc", "break", "normally closed", "auxiliary"])

# changeover: common 11 at bottom, NC 12 top right, NO 14 top left
g = [L([0, 16], [0, 11]), L([0, 11], [4, 4]), L([-4, 0], [-4, 5]), L([4, 0], [4, 3]),
     L([4, 3], [4, 5]), L([4, 5], [2.5, 5]), L([-4, 5], [-4, 5.0001])]
g = [L([0, 16], [0, 11]), L([0, 11], [3.4, 4.5]),
     PL([[-4, 0], [-4, 5], [-1.5, 5]]),
     PL([[4, 0], [4, 5], [1.5, 5]])]
emit("contacts/changeover", "Changeover contact",
     "Changeover (CO) contact with common 11, break side 12 and make side 14.",
     "contact.co", ["co", "changeover", "spdt", "auxiliary"],
     [("11", [0, 16], "down"), ("12", [4, 0], "up"), ("14", [-4, 0], "up")],
     [TAG_LEFT(-8)], g,
     {"kind": "contact", "normally": "changeover", "poles": [["11", "12", "14"]]})

for kind, act, fn, nm, ids, desc in [
    ("no", "delay_on", "contact.no.delay_on", "Make contact, delayed on closing", ("17", "18"),
     "Timer contact that closes after the on-delay of its timer relay."),
    ("no", "delay_off", "contact.no.delay_off", "Make contact, delayed on opening", ("17", "18"),
     "Timer contact that opens after the off-delay of its timer relay."),
    ("nc", "delay_on", "contact.nc.delay_on", "Break contact, delayed on opening", ("15", "16"),
     "Timer contact that opens after the on-delay of its timer relay."),
    ("nc", "delay_off", "contact.nc.delay_off", "Break contact, delayed on closing", ("15", "16"),
     "Timer contact that closes again after the off-delay of its timer relay."),
]:
    short = fn.split(".", 1)[1].replace(".", "_")
    single_contact(f"contacts/{short}", nm, desc, fn, kind, ids,
                   ["timer", "delay", kind, act.replace("_", " ")], act,
                   behavior={"kind": "contact", "normally": "open" if kind == "no" else "closed",
                             "delay": act.replace("delay_", "")})

single_contact("contacts/position_no", "Position switch, make contact",
               "Limit switch with a normally open contact, operated mechanically by the machine.",
               "contact.no.position", "no", ("13", "14"), ["limit switch", "position", "no"], "position")
single_contact("contacts/position_nc", "Position switch, break contact",
               "Limit switch with a normally closed contact, operated mechanically by the machine.",
               "contact.nc.position", "nc", ("21", "22"), ["limit switch", "position", "nc"], "position")
single_contact("contacts/thermal_nc", "Thermal overload contact, break",
               "Auxiliary break contact of an overload relay (95-96); opens when the relay trips.",
               "contact.nc.thermal", "nc", ("95", "96"), ["overload", "thermal", "trip", "95", "96"], "thermal")
single_contact("contacts/thermal_no", "Thermal overload contact, make",
               "Auxiliary make contact of an overload relay (97-98); closes when the relay trips.",
               "contact.no.thermal", "no", ("97", "98"), ["overload", "thermal", "trip", "97", "98", "signal"], "thermal")

# ---------------------------------------------------------------- operators

single_contact("operators/pushbutton_no", "Push button, make contact",
               "Manually operated push button with a normally open contact, such as a start button.",
               "contact.no.push", "no", ("13", "14"), ["push button", "start", "no", "button"], "push")
single_contact("operators/pushbutton_nc", "Push button, break contact",
               "Manually operated push button with a normally closed contact, such as a stop button.",
               "contact.nc.push", "nc", ("11", "12"), ["push button", "stop", "nc", "button"], "push")
single_contact("operators/emergency_stop", "Emergency stop, break contact",
               "Mushroom-head emergency stop button with a latching normally closed contact.",
               "contact.nc.estop", "nc", ("11", "12"), ["emergency stop", "e-stop", "mushroom", "nc"], "estop",
               behavior={"kind": "contact", "normally": "closed", "latching": True})
single_contact("operators/selector_no", "Selector switch, make contact",
               "Rotary selector or key switch with a normally open contact that stays in position.",
               "contact.no.turn", "no", ("13", "14"), ["selector", "rotary", "switch", "key", "no"], "turn",
               behavior={"kind": "contact", "normally": "open", "latching": True})
single_contact("operators/selector_nc", "Selector switch, break contact",
               "Rotary selector or key switch with a normally closed contact that stays in position.",
               "contact.nc.turn", "nc", ("11", "12"), ["selector", "rotary", "switch", "key", "nc"], "turn",
               behavior={"kind": "contact", "normally": "closed", "latching": True})

# ---------------------------------------------------------------- sensors

single_contact("sensors/proximity_no", "Proximity switch, make contact",
               "Inductive or capacitive proximity sensor with a normally open switching output.",
               "contact.no.proximity", "no", ("13", "14"), ["proximity", "sensor", "inductive", "no"], "proximity")
single_contact("sensors/proximity_nc", "Proximity switch, break contact",
               "Inductive or capacitive proximity sensor with a normally closed switching output.",
               "contact.nc.proximity", "nc", ("21", "22"), ["proximity", "sensor", "inductive", "nc"], "proximity")
single_contact("sensors/pressure_no", "Pressure switch, make contact",
               "Pressure switch with a normally open contact.",
               "contact.no.pressure", "no", ("13", "14"), ["pressure", "sensor", "no"], "pressure")
single_contact("sensors/float_no", "Float switch, make contact",
               "Level float switch with a normally open contact.",
               "contact.no.float", "no", ("13", "14"), ["float", "level", "sensor", "no"], "float")
single_contact("sensors/temperature_nc", "Temperature switch, break contact",
               "Thermostat or thermal switch with a normally closed contact that opens when hot.",
               "contact.nc.temperature", "nc", ("21", "22"), ["temperature", "thermostat", "sensor", "nc"], "temperature")

# ---------------------------------------------------------------- coils


def coil(path, name, desc, function, keywords, extra=None, behavior=None):
    g = [L([0, 0], [0, 5]), R([-4, 5], [8, 6]), L([0, 11], [0, 16])]
    if extra:
        g += extra
    emit(path, name, desc, function, keywords,
         [("A1", [0, 0], "up"), ("A2", [0, 16], "down")], [TAG_LEFT(-6)], g,
         behavior or {"kind": "coil", "poles": [["A1", "A2"]]})


coil("coils/relay", "Relay or contactor coil",
     "Operating coil of a relay or contactor; its contacts carry the same device tag.",
     "coil.relay", ["coil", "relay", "contactor", "a1", "a2"])
coil("coils/timer_on_delay", "Timer coil, on-delay",
     "Coil of an on-delay timer relay: its delayed contacts switch after the set time once energised.",
     "coil.timer.on_delay", ["timer", "on delay", "coil"],
     [R([4, 5], [4, 6]), L([4, 5], [8, 11]), L([4, 11], [8, 5])],
     {"kind": "coil", "poles": [["A1", "A2"]], "delay": "on"})
coil("coils/timer_off_delay", "Timer coil, off-delay",
     "Coil of an off-delay timer relay: its delayed contacts return after the set time once de-energised.",
     "coil.timer.off_delay", ["timer", "off delay", "coil"],
     [R([4, 5], [4, 6], fill=True)],
     {"kind": "coil", "poles": [["A1", "A2"]], "delay": "off"})
coil("coils/solenoid_valve", "Solenoid valve coil",
     "Coil of a solenoid valve.",
     "coil.valve", ["valve", "solenoid", "coil", "pneumatic"],
     [L([4, 8], [8, 8], dashed=True), PL([[8, 6], [12, 10], [12, 6], [8, 10]], closed=True)],
     {"kind": "coil", "poles": [["A1", "A2"]]})

# ---------------------------------------------------------------- contactors

for n in (1, 3, 4):
    multipole(f"contactors/main_{n}p", f"Contactor main contacts, {n}-pole",
              f"{n}-pole main (power) contacts of a contactor, switched by the coil with the same tag.",
              "contact.main", n, ["contactor", "main contact", "power", f"{n}p"], mark="contactor",
              behavior={"kind": "contact", "normally": "open"})

# ---------------------------------------------------------------- protection

for n in (1, 2, 3, 4):
    multipole(f"protection/mcb_{n}p", f"Miniature circuit breaker, {n}-pole",
              f"{n}-pole miniature circuit breaker (MCB) with thermal and magnetic trip.",
              "protection.mcb", n, ["mcb", "circuit breaker", "breaker", f"{n}p", "b curve", "c curve"],
              mark="breaker", actuator_kind="breaker_trip",
              behavior={"kind": "breaker", "trip": ["thermal", "magnetic"]})

multipole("protection/mcb_1p_n", "Miniature circuit breaker, 1-pole + N",
          "1-pole + neutral miniature circuit breaker with a switched, unprotected neutral.",
          "protection.mcb", 2, ["mcb", "circuit breaker", "1p+n", "neutral"],
          mark="breaker", actuator_kind="breaker_trip", n_pole=True,
          behavior={"kind": "breaker", "trip": ["thermal", "magnetic"]})

multipole("protection/motor_protection_breaker", "Motor protection circuit breaker, 3-pole",
          "3-pole motor protection circuit breaker (MPCB) with adjustable thermal and magnetic trip.",
          "protection.mpcb", 3, ["mpcb", "motor protection", "motor circuit breaker", "pkz", "gv2"],
          mark="breaker", actuator_kind="breaker_trip",
          behavior={"kind": "breaker", "trip": ["thermal", "magnetic"], "adjustable": True})


def rcd(path, name, desc, n, n_last, keywords, trip):
    attach = -4 * 3 / 7
    extra = [E([(n - 1) * 4, 20], (n - 1) * 4 + 3, 1.6),
             PL([[-3, 20], [-8, 20], [-8, 8], [attach, 8]], dashed=True),
             T([-9, 20], "IΔ", 2.2, "right")]
    if "magnetic" in trip:
        extra += thermal_glyph(-8, 12) + [T([-14, 12], "I>", 2.2, "right")]
    # attach trip link from the sensor to the blade link
    multipole(path, name, desc, "protection.rcd" if "magnetic" not in trip else "protection.rcbo",
              n, keywords, mark="breaker", actuator_kind=None, extra=extra, n_pole=n_last, length=24,
              behavior={"kind": "breaker", "trip": trip})


rcd("protection/rcd_2p", "Residual current device, 2-pole",
    "2-pole residual current circuit breaker (RCCB) that trips on earth leakage.",
    2, True, ["rcd", "rccb", "residual current", "earth leakage", "2p", "fi"], ["residual"])
rcd("protection/rcd_4p", "Residual current device, 4-pole",
    "4-pole residual current circuit breaker (RCCB) that trips on earth leakage.",
    4, True, ["rcd", "rccb", "residual current", "earth leakage", "4p", "fi"], ["residual"])
rcd("protection/rcbo_2p", "RCBO, 1-pole + N",
    "Residual current breaker with overcurrent protection (RCBO), 1-pole + neutral.",
    2, True, ["rcbo", "residual current", "breaker", "1p+n"], ["residual", "thermal", "magnetic"])


def fuse_pole(x, top=0, bottom=16):
    return [L([x, top], [x, 4]), R([x - 1.5, 4], [3, 8]), L([x, 4], [x, 12]), L([x, 12], [x, bottom])]


emit("protection/fuse", "Fuse",
     "Fuse (general symbol), for cartridge fuses and fuse links.",
     "protection.fuse", ["fuse", "f", "cartridge"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-4)], fuse_pole(0),
     {"kind": "fuse", "poles": [["1", "2"]]})
g = []
for i in range(3):
    g += fuse_pole(8 * i)
emit("protection/fuse_3p", "Fuse, 3-pole",
     "Three fuses of one fuse base or fuse holder sharing a device tag.",
     "protection.fuse", ["fuse", "3p", "fuse base"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down"), ("3", [8, 0], "up"), ("4", [8, 16], "down"),
      ("5", [16, 0], "up"), ("6", [16, 16], "down")], [TAG_LEFT(-4)], g,
     {"kind": "fuse", "poles": [["1", "2"], ["3", "4"], ["5", "6"]]})


def overload_pole(x):
    # series thermal element drawn inline in the conductor
    return [L([x, 0], [x, 5]), PL([[x, 5], [x - 1.5, 5], [x - 1.5, 8], [x + 1.5, 8], [x + 1.5, 11], [x, 11]]),
            L([x, 11], [x, 16])]


g = []
for i in range(3):
    g += overload_pole(8 * i)
g += [R([-3.5, 4], [21, 8], ), ]
g = []
for i in range(3):
    g += overload_pole(8 * i)
g.append(R([-3, 3.5], [22, 9]))
emit("protection/overload_relay", "Thermal overload relay, 3-pole",
     "Main circuit of a 3-pole thermal overload relay; its auxiliary contacts are 95-96 and 97-98.",
     "protection.overload", ["overload", "thermal relay", "bimetal", "ol"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down"), ("3", [8, 0], "up"), ("4", [8, 16], "down"),
      ("5", [16, 0], "up"), ("6", [16, 16], "down")], [TAG_LEFT(-5)], g,
     {"kind": "overload", "poles": [["1", "2"], ["3", "4"], ["5", "6"]]})

# ---------------------------------------------------------------- switches

multipole("switches/disconnector_1p", "Disconnector, 1-pole",
          "1-pole disconnector (isolator) for isolating a circuit without load.",
          "switch.disconnector", 1, ["disconnector", "isolator", "1p"], mark="disconnector",
          behavior={"kind": "contact", "normally": "open", "latching": True})
for n in (3, 4):
    multipole(f"switches/switch_disconnector_{n}p", f"Switch-disconnector, {n}-pole",
              f"{n}-pole load-break switch-disconnector.",
              "switch.switch_disconnector", n, ["switch disconnector", "load break", "isolator", f"{n}p"],
              mark="switch_disconnector",
              behavior={"kind": "contact", "normally": "open", "latching": True})
multipole("switches/main_switch_3p", "Main switch, 3-pole",
          "3-pole main switch (rotary switch-disconnector) with handle, usually the cabinet's main isolator.",
          "switch.main", 3, ["main switch", "rotary", "isolator", "repair switch", "3p"],
          mark="switch_disconnector", actuator_kind="turn",
          behavior={"kind": "contact", "normally": "open", "latching": True})

# ---------------------------------------------------------------- motors


def circle_hit(cx, cy, r, x, from_top=True):
    dy = math.sqrt(max(r * r - (x - cx) ** 2, 0))
    return cy - dy if from_top else cy + dy


def motor3(path, name, desc, function, keywords, six=False, ids1=("U1", "V1", "W1"),
           ids2=("U2", "V2", "W2"), letter="M", behavior=None):
    cx, cy, r = 8, 16, 7
    g = []
    conns = []
    xs = [0, 8, 16]
    ids = list(ids1)
    for x, cid in zip(xs, ids):
        y = circle_hit(cx, cy, r, x)
        g.append(L([x, 0], [x, 4]))
        # bend inwards so the lead meets the circle radially enough
        if x != cx:
            sx = cx + (x - cx) * 0.55
            sy = circle_hit(cx, cy, r, sx)
            g.append(L([x, 4], [sx, sy]))
        else:
            g.append(L([x, 4], [x, y]))
        conns.append((cid, [x, 0], "up"))
    g.append(C([cx, cy], r))
    g.append(T([cx, cy - 1.5], letter, 4))
    g.append(T([cx, cy + 3], "3~", 2.5))
    if six:
        for x, cid in zip(xs, list(ids2)):
            if x != cx:
                sx = cx + (x - cx) * 0.55
                sy = circle_hit(cx, cy, r, sx, from_top=False)
                g.append(L([sx, sy], [x, 28]))
            else:
                g.append(L([x, cy + r], [x, 28]))
            g.append(L([x, 28], [x, 32]))
            conns.append((cid, [x, 32], "down"))
    # PE on the right
    g.append(PL([[24, 0], [24, cy], [cx + r, cy]]))
    conns.append(("PE", [24, 0], "up", "pe"))
    poles = [list(ids1)] + ([list(ids2)] if six else [])
    beh = dict(behavior or {"kind": "motor", "phases": 3})
    beh["ports"] = poles
    emit(path, name, desc, function, keywords, conns, [TAG_LEFT(-3, cy)], g, beh)


motor3("motors/motor_3ph", "Three-phase motor",
       "Three-phase induction motor with terminals U1, V1, W1 and protective earth.",
       "motor.ac3", ["motor", "3 phase", "induction", "asynchronous", "m"])
motor3("motors/motor_3ph_6t", "Three-phase motor, 6 terminals",
       "Three-phase motor with both winding ends brought out (U1-W2), for star-delta starting.",
       "motor.ac3", ["motor", "star delta", "6 terminals", "wye delta"], six=True)


def motor2(path, name, desc, function, keywords, ids, text2, behavior):
    cx, cy, r = 4, 16, 7
    g = []
    for x in (0, 8):
        sx = cx + (x - cx) * 0.8
        g += [L([x, 0], [x, 4]), L([x, 4], [sx, circle_hit(cx, cy, r, sx)])]
    g += [C([cx, cy], r), T([cx, cy - 1.5], "M", 4), T([cx, cy + 3], text2, 2.5)]
    conns = [(ids[0], [0, 0], "up"), (ids[1], [8, 0], "up")]
    if behavior.get("pe"):
        del behavior["pe"]
        g.append(PL([[16, 0], [16, cy], [cx + r, cy]]))
        conns.append(("PE", [16, 0], "up", "pe"))
    behavior["ports"] = [list(ids)]
    emit(path, name, desc, function, keywords, conns, [TAG_LEFT(-5, cy)], g, behavior)


motor2("motors/motor_1ph", "Single-phase motor",
       "Single-phase AC motor with protective earth.", "motor.ac1",
       ["motor", "single phase", "1 phase", "m"], ("U1", "U2"), "1~",
       {"kind": "motor", "phases": 1, "pe": True})
motor2("motors/motor_dc", "DC motor",
       "Direct-current motor with armature terminals A1 and A2.", "motor.dc",
       ["motor", "dc", "direct current", "m"], ("A1", "A2"), "=",
       {"kind": "motor", "phases": 0})

# ---------------------------------------------------------------- transformers & supplies

g = [L([0, 0], [0, 9]), L([8, 0], [8, 9]), C([4, 12], 5), C([4, 20], 5),
     L([0, 23], [0, 32]), L([8, 23], [8, 32])]
emit("transformers/transformer_1ph", "Single-phase transformer",
     "Single-phase transformer with two separate windings, such as a control transformer.",
     "transformer.1ph", ["transformer", "control transformer", "1 phase", "isolation"],
     [("1.1", [0, 0], "up"), ("1.2", [8, 0], "up"), ("2.1", [0, 32], "down"), ("2.2", [8, 32], "down")],
     [TAG_LEFT(-3, 16)], g, {"kind": "transformer", "ports": [["1.1", "1.2"], ["2.1", "2.2"]]})

g = []
r = 9.5
for x in (0, 8, 16):
    yt = circle_hit(8, 14, r, x)
    yb = circle_hit(8, 26, r, x, from_top=False)
    g += [L([x, 0], [x, yt]), L([x, yb], [x, 40])]
g += [C([8, 14], r), C([8, 26], r)]
emit("transformers/transformer_3ph", "Three-phase transformer",
     "Three-phase transformer with two windings.",
     "transformer.3ph", ["transformer", "3 phase", "power transformer"],
     [("1U", [0, 0], "up"), ("1V", [8, 0], "up"), ("1W", [16, 0], "up"),
      ("2U", [0, 40], "down"), ("2V", [8, 40], "down"), ("2W", [16, 40], "down")],
     [TAG_LEFT(-3, 20)], g, {"kind": "transformer", "ports": [["1U", "1V", "1W"], ["2U", "2V", "2W"]]})


def box(path, name, desc, function, keywords, top, bottom, texts, behavior, diagonal=True,
        height=24, pitch=8, roles=None, bridges=None):
    roles = roles or {}
    n = max(len(top), len(bottom))
    width = (n - 1) * pitch + 8
    x0 = -4
    g = []
    conns = []
    for i, cid in enumerate(top):
        x = i * pitch
        g.append(L([x, 0], [x, 4]))
        conns.append((cid, [x, 0], "up", roles.get(cid)))
    for i, cid in enumerate(bottom):
        x = i * pitch
        g.append(L([x, height - 4], [x, height]))
        conns.append((cid, [x, height], "down", roles.get(cid)))
    g.append(R([x0, 4], [width, height - 8]))
    if diagonal:
        g.append(L([x0, height - 4], [x0 + width, 4]))
    for (fx, fy), text, size in texts:
        g.append(T([x0 + width * fx, 4 + (height - 8) * fy], text, size))
    emit(path, name, desc, function, keywords, conns, [TAG_LEFT(-6, height / 2)], g, behavior, bridges)


box("supplies/power_supply_dc", "Power supply, AC/DC",
    "Switched-mode power supply converting mains AC to a DC control voltage such as 24 V DC.",
    "converter.ac_dc", ["power supply", "psu", "24v", "dc", "rectifier", "smps"],
    ["L", "N", "PE"], ["L+", "M"], [((0.25, 0.25), "~", 3.5), ((0.75, 0.75), "=", 3.5)],
    {"kind": "converter", "input": "ac", "output": "dc", "ports": [["L", "N"], ["L+", "M"]]},
    roles={"PE": "pe", "N": "n"})
box("drives/frequency_converter", "Frequency converter",
    "Variable frequency drive (VFD) feeding a three-phase motor.",
    "converter.vfd", ["vfd", "frequency converter", "inverter", "drive", "speed"],
    ["L1", "L2", "L3", "PE"], ["U", "V", "W", "PE2"],
    [((0.25, 0.25), "~", 3.5), ((0.75, 0.75), "~", 3.5), ((0.7, 0.3), "f", 3)],
    {"kind": "converter", "input": "ac", "output": "ac", "ports": [["L1", "L2", "L3"], ["U", "V", "W"]]},
    roles={"PE": "pe", "PE2": "pe"}, bridges=[["PE", "PE2"]])
box("drives/soft_starter", "Soft starter",
    "Soft starter that ramps the voltage of a three-phase motor during start and stop.",
    "converter.soft_starter", ["soft starter", "softstarter", "ramp"],
    ["1L1", "3L2", "5L3"], ["2T1", "4T2", "6T3"],
    [((0.5, 0.5), "U ↗", 3)],
    {"kind": "converter", "input": "ac", "output": "ac", "ports": [["1L1", "3L2", "5L3"], ["2T1", "4T2", "6T3"]]},
    diagonal=False)

# ---------------------------------------------------------------- signalling

g = [L([0, 0], [0, 4]), C([0, 8], 4), L([-2.828, 5.172], [2.828, 10.828]),
     L([-2.828, 10.828], [2.828, 5.172]), L([0, 12], [0, 16])]
emit("signalling/lamp", "Indicator lamp",
     "Signal or indicator lamp; set the colour as a device property.",
     "signal.lamp", ["lamp", "indicator", "pilot light", "signal", "h"],
     [("X1", [0, 0], "up"), ("X2", [0, 16], "down")], [TAG_LEFT(-6)], g,
     {"kind": "lamp", "poles": [["X1", "X2"]]})
g = [L([0, 0], [0, 4]), R([-2, 4], [4, 4]), PL([[-2, 8], [-5, 12], [5, 12], [2, 8]]),
     L([0, 12], [0, 16])]
g = [L([0, 0], [0, 5]), PL([[-2, 5], [2, 5], [2, 8], [5, 11], [-5, 11], [-2, 8]], closed=True),
     L([0, 11], [0, 16])]
emit("signalling/horn", "Horn",
     "Horn or siren for audible alarms.",
     "signal.horn", ["horn", "siren", "alarm", "audible"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-7)], g,
     {"kind": "signal", "poles": [["1", "2"]]})
g = [L([0, 0], [0, 5]), A([0, 9], 4, 180, 360), L([-4, 9], [4, 9]), L([0, 9], [0, 16])]
g = [L([0, 0], [0, 4.5]), L([-4, 4.5], [4, 4.5]), A([0, 4.5], 4, 0, 180), L([0, 8.5], [0, 16])]
emit("signalling/buzzer", "Buzzer",
     "Buzzer for audible signals.",
     "signal.buzzer", ["buzzer", "alarm", "audible"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-6)], g,
     {"kind": "signal", "poles": [["1", "2"]]})
g = [L([0, 0], [0, 4]), A([0, 8], 4, 90, 270), L([0, 4], [0, 12]), L([0, 12], [0, 16])]
g = [L([0, 0], [0, 4]), L([0, 4], [0, 12]), A([0, 8], 4, 90, 270), L([-4, 8], [-4, 8.001]),
     L([0, 12], [0, 16])]
g = [L([0, 0], [0, 4]), A([1, 8], 4, 90, 270), L([1, 4], [1, 12]), L([0, 4], [1, 4]),
     L([0, 12], [1, 12]), L([0, 12], [0, 16])]
g = [L([0, 0], [0, 4]), L([0, 4], [0, 12]), A([0, 8], 4, 90, 270), L([0, 12], [0, 16])]
emit("signalling/bell", "Bell",
     "Electric bell.",
     "signal.bell", ["bell", "alarm", "audible"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-6)], g,
     {"kind": "signal", "poles": [["1", "2"]]})

# ---------------------------------------------------------------- terminals & connectors

emit("terminals/terminal", "Terminal",
     "Terminal of a terminal strip; both sides are the same conductor.",
     "terminal", ["terminal", "x", "clamp", "feed-through"],
     [("1", [0, 0], "up"), ("2", [0, 8], "down")], [TAG_LEFT(-3, 4)],
     [L([0, 0], [0, 2.8]), C([0, 4], 1.2), L([0, 5.2], [0, 8])],
     {"kind": "conductor", "poles": [["1", "2"]]})
emit("terminals/terminal_pe", "PE terminal",
     "Protective earth terminal of a terminal strip.",
     "terminal.pe", ["terminal", "pe", "earth", "ground", "protective earth"],
     [("1", [0, 0], "up", "pe"), ("2", [0, 8], "down", "pe")], [TAG_LEFT(-3, 4)],
     [L([0, 0], [0, 2.8]), C([0, 4], 1.2), L([0, 5.2], [0, 8]),
      L([1.2, 4], [4, 4]), L([4, 2.4], [4, 5.6]), L([4.8, 2.9], [4.8, 5.1]), L([5.6, 3.4], [5.6, 4.6])],
     {"kind": "conductor", "poles": [["1", "2"]]})
emit("terminals/terminal_fused", "Fuse terminal",
     "Terminal with a fuse holder.",
     "terminal.fuse", ["terminal", "fuse", "fuse terminal"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-3, 8)],
     [L([0, 0], [0, 1.8]), C([0, 3], 1.2), L([0, 4.2], [0, 11.8]), R([-1.2, 6], [2.4, 4]),
      C([0, 13], 1.2), L([0, 14.2], [0, 16])],
     {"kind": "fuse", "poles": [["1", "2"]]})
emit("terminals/terminal_disconnect", "Disconnect terminal",
     "Terminal with a knife disconnect for isolating a circuit during testing.",
     "terminal.disconnect", ["terminal", "disconnect", "knife", "test"],
     [("1", [0, 0], "up"), ("2", [0, 16], "down")], [TAG_LEFT(-3, 8)],
     [L([0, 0], [0, 1.8]), C([0, 3], 1.2), L([0, 4.2], [0, 6]), L([0, 10], [-2.5, 6.5]),
      L([0, 10], [0, 11.8]), C([0, 13], 1.2), L([0, 14.2], [0, 16])],
     {"kind": "contact", "normally": "closed", "latching": True, "poles": [["1", "2"]]})
emit("terminals/plug", "Plug (male contact)",
     "Male contact of a plug connector.",
     "connector.male", ["plug", "connector", "pin", "male"],
     [("1", [0, 0], "up"), ("2", [0, 8], "down")], [TAG_LEFT(-3, 4)],
     [L([0, 0], [0, 4]), R([-0.8, 4], [1.6, 3], fill=True), L([0, 7], [0, 8])],
     {"kind": "conductor", "poles": [["1", "2"]]})
emit("terminals/socket", "Socket (female contact)",
     "Female contact of a plug connector.",
     "connector.female", ["socket", "connector", "female", "receptacle"],
     [("1", [0, 0], "up"), ("2", [0, 8], "down")], [TAG_LEFT(-3, 4)],
     [L([0, 0], [0, 2]), A([0, 4], 2, 180, 360), L([0, 4], [0, 8])],
     {"kind": "conductor", "poles": [["1", "2"]]})
emit("terminals/socket_outlet", "Socket outlet with earth",
     "Single-phase socket outlet with protective earth contact.",
     "connector.outlet", ["socket outlet", "receptacle", "schuko", "power outlet"],
     [("L", [0, 0], "up"), ("N", [8, 0], "up", "n"), ("PE", [16, 0], "up", "pe")], [TAG_LEFT(-5, 10)],
     [L([0, 0], [0, 8]), L([8, 0], [8, 6]), L([16, 0], [16, 8]), A([8, 12], 6, 180, 360),
      L([2, 12], [14, 12]), L([0, 8], [3, 8]), L([16, 8], [13, 8])],
     {"kind": "load", "poles": [["L", "N"]]})

# ---------------------------------------------------------------- earth

emit("earth/earth", "Earth",
     "Earth (ground), general symbol.",
     "earth", ["earth", "ground", "gnd"],
     [("1", [0, 0], "up")], [TAG_LEFT(-5, 5)],
     [L([0, 0], [0, 4]), L([-3, 4], [3, 4]), L([-2, 5], [2, 5]), L([-1, 6], [1, 6])],
     {"kind": "none"})
emit("earth/protective_earth", "Protective earth",
     "Protective earth (PE) connection point.",
     "earth.pe", ["pe", "protective earth", "ground", "earth"],
     [("PE", [0, 0], "up", "pe")], [TAG_LEFT(-6, 6)],
     [L([0, 0], [0, 4]), C([0, 6], 3.8), L([-2.5, 5], [2.5, 5]), L([-1.6, 6.2], [1.6, 6.2]),
      L([-0.7, 7.4], [0.7, 7.4]), L([0, 4], [0, 5])],
     {"kind": "none"})
emit("earth/chassis", "Chassis (frame) connection",
     "Connection to the frame or chassis.",
     "earth.chassis", ["chassis", "frame", "mass", "functional earth"],
     [("1", [0, 0], "up")], [TAG_LEFT(-5, 5)],
     [L([0, 0], [0, 4]), L([-3, 4], [3, 4]), L([-3, 4], [-4.5, 6]), L([0, 4], [-1.5, 6]),
      L([3, 4], [1.5, 6])],
     {"kind": "none"})

# ---------------------------------------------------------------- passive components


def two_lead(path, name, desc, function, keywords, body, behavior, ids=("1", "2"), top=4, bottom=12,
             bridges=None):
    g = [L([0, 0], [0, top])] + body + [L([0, bottom], [0, 16])]
    behavior.setdefault("poles", [list(ids)])
    emit(path, name, desc, function, keywords,
         [(ids[0], [0, 0], "up"), (ids[1], [0, 16], "down")], [TAG_LEFT(-4)], g, behavior, bridges)


two_lead("passive/resistor", "Resistor", "Resistor, general symbol.", "passive.resistor",
         ["resistor", "r", "resistance"], [R([-1.5, 4], [3, 8])], {"kind": "passive", "type": "resistor"})
two_lead("passive/resistor_variable", "Variable resistor", "Resistor with adjustable value.",
         "passive.resistor.variable", ["resistor", "variable", "adjustable", "rheostat"],
         [R([-1.5, 4], [3, 8]), L([-3, 11], [3, 5]), PL([[3, 5], [1.6, 5.4], [2.6, 6.4]], closed=True, fill=True)],
         {"kind": "passive", "type": "resistor"})
g = [L([0, 0], [0, 4]), R([-1.5, 4], [3, 8]), L([0, 12], [0, 16]), L([8, 8], [1.5, 8]),
     PL([[1.5, 8], [3, 7.2], [3, 8.8]], closed=True, fill=True), L([8, 0], [8, 8])]
emit("passive/potentiometer", "Potentiometer",
     "Potentiometer with sliding contact (wiper).", "passive.potentiometer",
     ["potentiometer", "pot", "setpoint", "wiper"],
     [("1", [0, 0], "up"), ("3", [0, 16], "down"), ("2", [8, 0], "up")], [TAG_LEFT(-4)], g,
     {"kind": "passive", "type": "potentiometer", "poles": [["1", "3"]], "wiper": "2"})
two_lead("passive/capacitor", "Capacitor", "Capacitor, general symbol.", "passive.capacitor",
         ["capacitor", "c", "condenser"], [L([-3, 7], [3, 7]), L([-3, 9], [3, 9])],
         {"kind": "passive", "type": "capacitor"}, top=7, bottom=9)
two_lead("passive/capacitor_polarized", "Polarised capacitor", "Polarised (electrolytic) capacitor.",
         "passive.capacitor.polarized", ["capacitor", "electrolytic", "polarized", "polarised"],
         [L([-3, 7], [3, 7]), R([-3, 9], [6, 0.8], fill=True), T([3, 5], "+", 2.5)],
         {"kind": "passive", "type": "capacitor"}, ids=("+", "-"), top=7, bottom=9.8)
two_lead("passive/inductor", "Inductor", "Inductor, coil or reactor.", "passive.inductor",
         ["inductor", "choke", "reactor", "l"],
         [A([0, 5], 1, -90, 90), A([0, 7], 1, -90, 90), A([0, 9], 1, -90, 90), A([0, 11], 1, -90, 90)],
         {"kind": "passive", "type": "inductor"}, top=4, bottom=12)
DIODE = [PL([[-2.5, 6], [2.5, 6], [0, 10]], closed=True), L([-2.5, 10], [2.5, 10]), L([0, 6], [0, 10])]
two_lead("passive/diode", "Diode", "Semiconductor diode; current flows from anode to cathode.",
         "passive.diode", ["diode", "rectifier", "freewheeling"], DIODE,
         {"kind": "passive", "type": "diode"}, ids=("A", "K"), top=6, bottom=10)
two_lead("passive/zener_diode", "Zener diode", "Zener (voltage reference) diode.",
         "passive.diode.zener", ["zener", "diode", "voltage reference"],
         DIODE[:1] + [PL([[-3.5, 11], [-2.5, 10], [2.5, 10], [3.5, 9]]), L([0, 6], [0, 10])],
         {"kind": "passive", "type": "diode"}, ids=("A", "K"), top=6, bottom=10)
two_lead("passive/led", "Light-emitting diode", "Light-emitting diode (LED).",
         "passive.diode.led", ["led", "light emitting diode", "diode"],
         DIODE + [L([3, 7], [5.5, 5]), PL([[5.5, 5], [4.3, 5.2], [5, 6]], closed=True, fill=True),
                  L([3, 9], [5.5, 7]), PL([[5.5, 7], [4.3, 7.2], [5, 8]], closed=True, fill=True)],
         {"kind": "lamp"}, ids=("A", "K"), top=6, bottom=10)
two_lead("passive/heater", "Heating element", "Heating element or heater.", "load.heater",
         ["heater", "heating", "resistor", "element"],
         [R([-3, 4], [6, 8]), L([-1, 4], [-1, 12]), L([1, 4], [1, 12])],
         {"kind": "load", "type": "resistive"})

# ---------------------------------------------------------------- sources

two_lead("sources/battery", "Battery or DC cell", "Battery or DC cell; the longer plate is positive.",
         "source.dc", ["battery", "cell", "dc source", "accumulator"],
         [L([-3.5, 7], [3.5, 7]), L([-1.5, 9], [1.5, 9]), T([4, 5], "+", 2.5)],
         {"kind": "source", "type": "dc"}, ids=("+", "-"), top=7, bottom=9)
two_lead("sources/ac_source", "AC voltage source", "AC voltage source, such as a supply feed.",
         "source.ac", ["ac source", "supply", "voltage source", "mains"],
         [C([0, 8], 4), T([0, 8], "~", 4)],
         {"kind": "source", "type": "ac"}, top=4, bottom=12)

# ---------------------------------------------------------------- measuring

for path, name, desc, fn, text, kw in [
    ("measuring/ammeter", "Ammeter", "Current-measuring instrument.", "meter.current", "A",
     ["ammeter", "current", "meter", "measurement"]),
    ("measuring/voltmeter", "Voltmeter", "Voltage-measuring instrument.", "meter.voltage", "V",
     ["voltmeter", "voltage", "meter", "measurement"]),
]:
    # an ammeter sits in series and always conducts; a voltmeter is a load
    two_lead(path, name, desc, fn, kw, [C([0, 8], 4), T([0, 8], text, 3.5)],
             {"kind": "meter", "quantity": fn.split(".")[1]}, top=4, bottom=12,
             bridges=[["1", "2"]] if text == "A" else None)
two_lead("measuring/energy_meter", "Energy meter", "Electrical energy meter (kWh).", "meter.energy",
         ["energy meter", "kwh", "power meter", "metering"],
         [R([-5, 4], [10, 8]), L([-5, 6.5], [5, 6.5]), T([0, 9.3], "Wh", 2.5)],
         {"kind": "meter", "quantity": "energy"})
g = [L([0, 0], [0, 16]), C([0, 8], 3), L([3, 8], [8, 8]), L([8, 8], [8, 12]), L([3, 10], [4, 10]),
     L([4, 10], [4, 12])]
g = [L([0, 0], [0, 16]), C([0, 8], 3), PL([[2.6, 6.5], [8, 6.5], [8, 0]]), PL([[2.6, 9.5], [16, 9.5], [16, 0]])]
emit("measuring/current_transformer", "Current transformer",
     "Current transformer around a primary conductor, with secondary terminals S1 and S2.",
     "transformer.current", ["current transformer", "ct", "measuring"],
     [("P1", [0, 0], "up"), ("P2", [0, 16], "down"), ("S1", [8, 0], "up"), ("S2", [16, 0], "up")],
     [TAG_LEFT(-5)], g,
     {"kind": "transformer", "ports": [["P1", "P2"], ["S1", "S2"]]}, [["P1", "P2"]])

# ---------------------------------------------------------------- PLC


def plc_point(path, name, desc, fn, label, kw, behavior, down=False):
    if not down:
        g = [L([0, 0], [0, 4]), R([-4, 4], [8, 6]), T([0, 7], label, 2.2)]
        conns = [("1", [0, 0], "up")]
    else:
        g = [R([-4, 0], [8, 6]), T([0, 3], label, 2.2), L([0, 6], [0, 8])]
        conns = [("1", [0, 8], "down")]
    emit(path, name, desc, fn, kw, conns,
         [TAG_LEFT(-6, 7 if not down else 3), ("address", [6, 7 if not down else 3], "left")], g, behavior)


plc_point("plc/digital_input", "PLC digital input", "One digital input channel of a PLC card.",
          "plc.input.digital", "DI", ["plc", "input", "di", "digital", "i/o"],
          {"kind": "plc_input", "signal": "digital"})
plc_point("plc/digital_output", "PLC digital output", "One digital output channel of a PLC card.",
          "plc.output.digital", "DO", ["plc", "output", "do", "digital", "i/o"],
          {"kind": "plc_output", "signal": "digital"}, down=True)
plc_point("plc/analog_input", "PLC analog input", "One analog input channel of a PLC card (for example 4-20 mA).",
          "plc.input.analog", "AI", ["plc", "input", "ai", "analog", "4-20ma", "i/o"],
          {"kind": "plc_input", "signal": "analog"})
plc_point("plc/analog_output", "PLC analog output", "One analog output channel of a PLC card (for example 0-10 V).",
          "plc.output.analog", "AO", ["plc", "output", "ao", "analog", "0-10v", "i/o"],
          {"kind": "plc_output", "signal": "analog"}, down=True)
g = [L([0, 0], [0, 4]), L([8, 0], [8, 4]), R([-4, 4], [16, 6]), T([4, 7], "PLC  L+ / M", 2.2)]
g = [L([0, 0], [0, 4]), L([8, 0], [8, 4]), R([-4, 4], [16, 6]), T([4, 7], "24 V DC", 2.2)]
emit("plc/power_supply", "PLC power supply connection",
     "Supply connection of a PLC or I/O card (L+ and M).",
     "plc.supply", ["plc", "supply", "l+", "m", "24v"],
     [("L+", [0, 0], "up"), ("M", [8, 0], "up")], [TAG_LEFT(-6, 7)], g,
     {"kind": "load", "poles": [["L+", "M"]]})

exec(open("gen2.py").read())
exec(open("gen3.py").read())
exec(open("gen4.py").read())
print(len(ALL), "symbols")
