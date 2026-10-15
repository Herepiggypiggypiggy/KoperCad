"""Helpers for writing symbol TOML files (copied from the WS-05 generator)."""
import math


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
    parts = [f'kind = "{g["kind"]}"']
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
    for a in anchors:
        a = dict(a)
        if a.get("align") == "right":
            y = a["at"][1]
            left = [gbox(g)[0] for g in graphics
                    if gbox(g)[1] <= y + 1.6 and gbox(g)[3] >= y - 1.6]
            if left:
                a["at"] = [min(a["at"][0], math.floor((min(left) - 1.5) * 2) / 2), y]
        out.append(a)
    return out


def L(a, b, dashed=False):
    g = {"kind": "line", "from": list(a), "to": list(b)}
    if dashed:
        g["stroke"] = "dashed"
    return g


def PL(points, closed=False, fill=False):
    g = {"kind": "polyline", "points": [list(p) for p in points]}
    if closed:
        g["closed"] = True
    if fill:
        g["fill"] = True
    return g


def C(center, r, fill=False):
    g = {"kind": "circle", "center": list(center), "r": r}
    if fill:
        g["fill"] = True
    return g


def A(center, r, start, end):
    return {"kind": "arc", "center": list(center), "r": r, "start": start, "end": end}


def T(at, text, size=2.5, align="center"):
    g = {"kind": "text", "at": list(at), "text": text}
    if size != 2.5:
        g["size"] = size
    if align != "center":
        g["align"] = align
    return g


def write_symbol(path, sym):
    """Write one symbol dict (as parsed from TOML, plus our changes) to path."""
    lines = [
        "format = 1",
        f"id = {tstr(sym['id'])}",
        f"name = {tstr(sym['name'])}",
        f"description = {tstr(sym['description'])}",
        f"standard = {tstr(sym['standard'])}",
        f"category = {tstr(sym['category'])}",
        f"function = {tstr(sym['function'])}",
        "keywords = [" + ", ".join(tstr(k) for k in sym["keywords"]) + "]",
    ]
    if sym.get("aliases"):
        lines.append("aliases = [" + ", ".join(tstr(a) for a in sym["aliases"]) + "]")
    if sym.get("favorite"):
        lines.append("favorite = true")
    lines += ["", "connections = ["]
    for c in sym["connections"]:
        extra = ""
        if c.get("role"):
            extra += f", role = {tstr(c['role'])}"
        if c.get("optional"):
            extra += ", optional = true"
        lines.append(f"  {{ id = {tstr(c['id'])}, at = {pt(c['at'])}, dir = \"{c['dir']}\"{extra} }},")
    lines += ["]", "", "anchors = ["]
    for a in sym["anchors"]:
        lines.append(f"  {{ role = {tstr(a['role'])}, at = {pt(a['at'])}, align = \"{a.get('align', 'left')}\" }},")
    lines += ["]", ""]
    if sym.get("bridges"):
        lines += ["bridges = " + tval(sym["bridges"]), ""]
    lines += ["graphics = ["]
    lines += [gfx_line(g) for g in sym["graphics"]]
    lines += ["]", "", "[behavior]"]
    for k, v in sym["behavior"].items():
        lines.append(f"{k} = {tval(v)}")
    import os
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
