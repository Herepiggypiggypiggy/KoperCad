"""Turns the drawn icon set of koper_icons.py into Rust data for novo-theme.

    python3 icons_to_rust.py > ../../../../crates/novo-theme/src/drawn/set.rs

Run by hand; the output is committed. koper_icons.py stays the source of truth, and a
test in novo-theme compares the number of shapes of every icon with the SVG files in
../icons, so the two cannot drift silently.

Mapping (see docs/design/visual-redesign.md section 9.3):
  <rect>                 -> Shape::Rect
  <circle>               -> Shape::Circle (filled for class="dot")
  path, one M + L/H/V    -> Shape::Line (one segment) or Shape::Poly (more, or closed)
  path with a/c/s        -> Shape::Path with Seg::Arc / Seg::Cubic. SVG endpoint arcs are
                            converted to centre form so the Rust side samples them exactly
  stroke-dasharray       -> Shape::Dashed around the shape
"""
import math
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from koper_icons import ICONS  # noqa: E402

GROUPS = {
    "workspace": "Workspace",
    "component": "Component",
    "wiring": "Wiring",
    "cabinet": "Cabinet",
    "production": "Production",
    "workflow": "Workflow",
    "analysis": "Analysis",
}


def num(v):
    s = f"{v:.4f}".rstrip("0").rstrip(".")
    if s in ("-0", ""):
        s = "0"
    if "." not in s:
        s += ".0"
    return s


def attrs(tag):
    return dict(re.findall(r'([\w-]+)="([^"]*)"', tag))


def tokens(d):
    for m in re.finditer(r"([MmLlHhVvAaCcSsZz])|(-?\d*\.?\d+(?:e-?\d+)?)", d):
        yield m.group(1) or float(m.group(2))


def arc_to_centre(x1, y1, rx, ry, phi, fa, fs, x2, y2):
    """SVG endpoint arc -> (cx, cy, rx, ry, theta1, delta). phi must be 0."""
    assert phi == 0
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    lam = dx * dx / (rx * rx) + dy * dy / (ry * ry)
    if lam > 1:
        s = math.sqrt(lam)
        rx, ry = rx * s, ry * s
    num_ = rx * rx * ry * ry - rx * rx * dy * dy - ry * ry * dx * dx
    den = rx * rx * dy * dy + ry * ry * dx * dx
    k = math.sqrt(max(0.0, num_ / den))
    if fa == fs:
        k = -k
    cxp, cyp = k * rx * dy / ry, -k * ry * dx / rx
    cx, cy = cxp + (x1 + x2) / 2, cyp + (y1 + y2) / 2

    def ang(ux, uy, vx, vy):
        a = math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)
        return a

    t1 = ang(1, 0, (dx - cxp) / rx, (dy - cyp) / ry)
    dt = ang((dx - cxp) / rx, (dy - cyp) / ry, (-dx - cxp) / rx, (-dy - cyp) / ry)
    if not fs and dt > 0:
        dt -= 2 * math.pi
    elif fs and dt < 0:
        dt += 2 * math.pi
    return cx, cy, rx, ry, t1, dt


def subpaths(d):
    """Yield (start, segs, closed); segs are ('L', x, y) | ('A', centre form) | ('C', ...)."""
    it = list(tokens(d))
    i = 0
    cmd = None
    cur = (0.0, 0.0)
    start = None
    segs = []
    closed = False
    last_c2 = None
    out = []

    def flush():
        nonlocal start, segs, closed
        if start is not None and segs:
            out.append((start, segs, closed))
        start, segs, closed = None, [], False

    def take(n):
        nonlocal i
        vals = it[i:i + n]
        i += n
        return vals

    while i < len(it):
        t = it[i]
        if isinstance(t, str):
            cmd = t
            i += 1
            if cmd in "Zz":
                closed = True
                cur = start
                flush()
                continue
        rel = cmd.islower()
        c = cmd.upper()
        ox, oy = cur if rel else (0.0, 0.0)
        if c == "M":
            flush()
            x, y = take(2)
            cur = (ox + x, oy + y)
            start = cur
            cmd = "l" if rel else "L"  # implicit lineto
            last_c2 = None
        elif c == "L":
            x, y = take(2)
            cur = (ox + x, oy + y)
            segs.append(("L", *cur))
            last_c2 = None
        elif c == "H":
            (x,) = take(1)
            cur = ((cur[0] if rel else 0.0) + x, cur[1])
            segs.append(("L", *cur))
            last_c2 = None
        elif c == "V":
            (y,) = take(1)
            cur = (cur[0], (cur[1] if rel else 0.0) + y)
            segs.append(("L", *cur))
            last_c2 = None
        elif c == "A":
            rx, ry, phi, fa, fs, x, y = take(7)
            end = (ox + x, oy + y)
            segs.append(("A", *arc_to_centre(cur[0], cur[1], rx, ry, phi, int(fa), int(fs), *end)))
            cur = end
            last_c2 = None
        elif c in "CS":
            if c == "C":
                x1, y1, x2, y2, x, y = take(6)
                p1 = (ox + x1, oy + y1)
            else:
                x2, y2, x, y = take(4)
                p1 = (2 * cur[0] - last_c2[0], 2 * cur[1] - last_c2[1]) if last_c2 else cur
            p2 = (ox + x2, oy + y2)
            end = (ox + x, oy + y)
            segs.append(("C", *p1, *p2, *end))
            last_c2 = p2
            cur = end
        else:
            raise ValueError(f"path command {cmd}")
    flush()
    return out


def ang(v):
    """An angle in radians; multiples of a quarter turn use the exact constants."""
    q = v / (math.pi / 2)
    if abs(q - round(q)) < 1e-4:
        k = int(round(q))
        if k == 0:
            return "0.0"
        sign = "-" if k < 0 else ""
        k = abs(k)
        return f"{sign}FRAC_PI_2" if k == 1 else f"{sign}{k}.0 * FRAC_PI_2"
    return num(v)


def fmt_pt(x, y):
    return f"[{num(x)}, {num(y)}]"


def rust_subpath(start, segs, closed):
    if all(s[0] == "L" for s in segs):
        pts = [start] + [(s[1], s[2]) for s in segs]
        if len(pts) == 2 and not closed:
            return f"Shape::Line([{num(pts[0][0])}, {num(pts[0][1])}, {num(pts[1][0])}, {num(pts[1][1])}])"
        body = ", ".join(fmt_pt(*p) for p in pts)
        return f"Shape::Poly(&[{body}], {'true' if closed else 'false'})"
    parts = []
    for s in segs:
        if s[0] == "L":
            parts.append(f"Seg::Line({fmt_pt(s[1], s[2])})")
        elif s[0] == "A":
            _, cx, cy, rx, ry, t1, dt = s
            parts.append(
                f"Seg::Arc {{ cx: {num(cx)}, cy: {num(cy)}, rx: {num(rx)}, ry: {num(ry)}, "
                f"from: {ang(t1)}, to: {ang(t1 + dt)} }}")
        else:
            _, a, b, c, d_, e, f = s
            parts.append(f"Seg::Cubic([{fmt_pt(a, b)}, {fmt_pt(c, d_)}, {fmt_pt(e, f)}])")
    return (f"Shape::Path {{ start: {fmt_pt(*start)}, segs: &[{', '.join(parts)}], "
            f"closed: {'true' if closed else 'false'} }}")


def shapes_of(body):
    out = []
    for tag in re.findall(r"<(?:path|rect|circle)\b[^>]*/>", body):
        a = attrs(tag)
        dash = a.get("stroke-dasharray")
        if tag.startswith("<rect"):
            r = float(a.get("rx", 0))
            shapes = [f"Shape::Rect {{ x: {num(float(a['x']))}, y: {num(float(a['y']))}, "
                      f"w: {num(float(a['width']))}, h: {num(float(a['height']))}, r: {num(r)} }}"]
        elif tag.startswith("<circle"):
            filled = a.get("class") == "dot"
            shapes = [f"Shape::Circle {{ cx: {num(float(a['cx']))}, cy: {num(float(a['cy']))}, "
                      f"r: {num(float(a['r']))}, filled: {'true' if filled else 'false'} }}"]
        else:
            shapes = [rust_subpath(*sp) for sp in subpaths(a["d"])]
        for s in shapes:
            if dash:
                on, off = (float(v) for v in dash.split())
                s = f"Shape::Dashed {{ shape: &{s}, on: {num(on)}, off: {num(off)} }}"
            out.append(s)
    return out


def const_name(name):
    return name.upper().replace("-", "_")


def main():
    lines = [
        "//! The drawn icon set as data. Generated by",
        "//! `docs/design/visual-redesign/tools/icons_to_rust.py` from `koper_icons.py`;",
        "//! do not edit by hand (a test compares it with the SVG files).",
        "",
        "use super::{Group, Icon, Seg, Shape};",
        "use std::f32::consts::FRAC_PI_2;",
        "",
    ]
    for name, (group, title, body) in ICONS.items():
        shapes = shapes_of(body)
        lines.append(f"pub const {const_name(name)}: Icon = Icon {{")
        lines.append(f'    name: "{name}",')
        lines.append(f"    group: Group::{GROUPS[group]},")
        lines.append(f'    title: "{title}",')
        lines.append("    shapes: &[")
        for s in shapes:
            lines.append(f"        {s},")
        lines.append("    ],")
        lines.append("};")
        lines.append("")
    lines.append(f"/// Every drawn icon, in the order of the source.")
    lines.append(f"pub const ALL: [&Icon; {len(ICONS)}] = [")
    for name in ICONS:
        lines.append(f"    &{const_name(name)},")
    lines.append("];")
    sys.stdout.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
