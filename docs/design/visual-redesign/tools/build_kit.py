"""Builds the mockup kit for docs/design/visual-redesign.md.

    python3 build_kit.py <phosphor-assets/regular dir> <out dir> [--map name=url ...]

Writes into <out dir>:
  kc.css            tokens, components (kc.base.css) + one mask class per icon:
                    .i-<phosphor name>, .ki-<koper icon>, .sy-<iec symbol>
  canvas-dark.svg   the Gate 1 motor starter page drawn from the real IEC symbols
  canvas-light.svg  the same in the light theme
  iso-dark.svg / iso-light.svg   a cabinet in isometric view for the 3D mockups

`--map canvas-dark=/_blob/abc` replaces the url the css uses for an asset (the design canvas
serves uploads from /_blob/<id>); by default the css points at the file next to it.
Phosphor icons are MIT (github.com/phosphor-icons/core); install them with
`npm pack @phosphor-icons/core` and pass assets/regular.
"""
import json
import math
import os
import re
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from koper_icons import ICONS, ADDON_ICONS, HEAD, body  # noqa: E402

PHOSPHOR = """file-plus folder-open floppy-disk file-pdf printer arrow-counter-clockwise arrow-clockwise scissors copy clipboard trash
magnifying-glass magnifying-glass-plus magnifying-glass-minus corners-out grid-four magnet check push-pin arrows-left-right seal-check play
terminal-window gear-six question cursor circuitry line-segment plugs text-t arrow-elbow-right-down arrow-up-right cube file-text
clipboard-text sidebar-simple files warning-diamond git-diff info dots-six-vertical package lightbulb warning warning-octagon check-circle
list caret-down caret-right caret-up caret-left x dots-three dots-three-vertical house magic-wand sparkle thermometer-hot wind fan ruler
download-simple upload-simple export file-arrow-down file-xls file-csv table lightning graduation-cap book-open keyboard eye eye-slash stack
crosshair arrows-out-cardinal selection hand cube-transparent align-left align-right align-top align-bottom door camera bookmark gauge tag
hash folder clock-counter-clockwise user sidebar layout square-split-horizontal corners-in arrows-out browsers columns rows chat-circle
robot wrench funnel sliders-horizontal globe share link git-branch plus minus broom pencil-simple file-dashed file-code file-zip lock lock-open
shield-check circle push-pin-simple arrow-square-out frame-corners cursor-click number-circle-one dots-nine squares-four file-dxf  file-doc
arrows-clockwise tray archive file-arrow-up image selection-all bounding-box toggle-left wave-sine flag note-pencil table  printer
git-commit circle-notch sun moon palette rows-plus-bottom open-ai-logo hard-drives map-trifold  list-checks list-bullets textbox
resize arrows-horizontal dock-simple monitor app-window picture-in-picture arrow-line-right arrow-line-left dots-six  cpu plug-charging
wall power battery-charging lightning-a gear toolbox target fast-forward rewind pause stop timer clock puzzle-piece shield-warning toggle-right lock-key game-controller broadcast certificate fingerprint prohibit seal-warning plug record""".split()

# the schematic and palette use these library symbols
SUBSET = json.load(open(os.path.join(HERE, "symbols-subset.json")))


def data_uri(svg):
    return "url(\"data:image/svg+xml," + urllib.parse.quote(svg, safe="/:=,;()-._ ") .replace('"', "%22") + "\")"


def icon_css(phos_dir):
    css = []
    miss = []
    seen = set()
    for n in PHOSPHOR:
        if n in seen:
            continue
        seen.add(n)
        p = os.path.join(phos_dir, n + ".svg")
        if not os.path.exists(p):
            miss.append(n)
            continue
        svg = open(p).read().strip()
        css.append(f".kc .i-{n}{{--m:{data_uri(svg)}}}")
    for n, (_, _, b) in {**ICONS, **ADDON_ICONS}.items():
        svg = HEAD + body(b).replace('fill="currentColor"', 'fill="#000"') + "</svg>"
        svg = svg.replace('stroke="currentColor"', 'stroke="#000"')
        css.append(f".kc .ki-{n.replace('.', '-')}{{--m:{data_uri(svg)}}}")
    for sid, s in SUBSET.items():
        svg = s["svg"]
        svg = re.sub(r"<title>.*?</title>", "", svg)
        svg = svg.replace("currentColor", "#000").replace('stroke-width="0.45"', 'stroke-width="0.62"')
        svg = re.sub(r' width="[\d.]+" height="[\d.]+"', "", svg, count=1)
        css.append(f".kc .sy-{sid[4:].replace('/', '-').replace('_', '-')}{{--m:{data_uri(svg)}}}")
    return "\n".join(css), miss


# ---------------------------------------------------------------- schematic page
THEMES = {
    "dark": dict(ground="#0E0E10", paper="#17171A", line="#3B3B41", sym="#E3E1DC", wire="#6F9BD8", text="#B9B7B2",
                 dots="#2C2C31", sel="#C88A5E", wirenum="#7FA7E0", mark="#D6A55C"),
    "light": dict(ground="#E7E5E1", paper="#FFFFFF", line="#B5B1AA", sym="#1E1D1B", wire="#2F5FA8", text="#4A4842",
                  dots="#D6D3CD", sel="#9A5427", wirenum="#2F5FA8", mark="#7F5408"),
}


def sym_g(sid, x, y, k, col, rot=0, ox=0, oy=0):
    """The library symbol `sid` with its origin at (x, y) mm, scaled k px/mm."""
    s = SUBSET["iec/" + sid]["svg"]
    inner = re.search(r"<g stroke=.*</g>", s, re.S).group(0)
    inner = inner.replace("currentColor", col).replace('stroke-width="0.45"', 'stroke-width="0.4"')
    t = f"translate({ox + x * k:.2f} {oy + y * k:.2f}) scale({k})"
    if rot:
        t += f" rotate({rot})"
    return f'<g transform="{t}">{inner}</g>'


def schematic(theme, W=1070, H=706):
    c = THEMES[theme]
    k = 3.3
    ox, oy = 250, 96  # page origin of the drawing in px
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="Inter,Arial,sans-serif">']
    # sheet
    out.append(f'<rect x="22" y="14" width="{W-44}" height="{H-30}" fill="{c["paper"]}" stroke="{c["line"]}"/>')
    out.append(f'<rect x="38" y="48" width="{W-76}" height="{H-78}" fill="none" stroke="{c["line"]}"/>')
    cols = 8
    cw = (W - 76) / cols
    for i in range(cols):
        out.append(f'<text x="{38 + cw*(i+.5):.1f}" y="35" font-size="11" fill="{c["text"]}" text-anchor="middle">{i+1}</text>')
        if i:
            out.append(f'<path d="M{38+cw*i:.1f} 22V48" stroke="{c["line"]}"/>')
    # grid dots inside frame
    out.append(f'<pattern id="g" width="13.2" height="13.2" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r=".7" fill="{c["dots"]}"/></pattern>')
    out.append(f'<rect x="39" y="49" width="{W-78}" height="{H-80}" fill="url(#g)"/>')

    def P(x, y):
        return (ox + x * k, oy + y * k)

    def wire(pts, col=None):
        d = " ".join(f"{'M' if i == 0 else 'L'}{ox + x*k:.1f} {oy + y*k:.1f}" for i, (x, y) in enumerate(pts))
        out.append(f'<path d="{d}" fill="none" stroke="{col or c["wire"]}" stroke-width="1.3" stroke-linejoin="round"/>')

    def dot(x, y):
        out.append(f'<circle cx="{ox + x*k:.1f}" cy="{oy + y*k:.1f}" r="2.1" fill="{c["wire"]}"/>')

    def text(x, y, t, size=9.5, anchor="end", col=None, weight=400):
        out.append(f'<text x="{ox + x*k:.1f}" y="{oy + y*k + 3:.1f}" font-size="{size}" fill="{col or c["text"]}" text-anchor="{anchor}" font-weight="{weight}">{t}</text>')

    def wnum(x, y, t):
        out.append(f'<text x="{ox + x*k + 5:.1f}" y="{oy + y*k + 3:.1f}" font-size="8.5" fill="{c["wirenum"]}">{t}</text>')

    def sym(sid, x, y, rot=0):
        out.append(sym_g(sid, x, y, k, c["sym"], rot, ox, oy))

    # ---- circuit 1: -F1 -K1 -F2 -X1 -M1 (the Gate 1 starter), poles at x 0 8 16, PE at 24
    for i, x in enumerate((0, 8, 16)):
        sym("connectivity/potential_feed", x, -8)
        text(x, -10, f"L{i+1}", 9.5, "middle", c["text"])
    sym("connectivity/potential_feed_pe", 24, -8)
    text(24, -10, "PE", 9.5, "middle")
    y = 0
    steps = [("protection/fuse_3p", "-F1", 8, 0), ("contactors/main_3p", "-K1", 36, 0), ("protection/overload_relay", "-F2", 64, 0)]
    for sid, tag, y0, _ in steps:
        sym(sid, 0, y0)
        text(-6, y0 + 8, tag, 9.5, "end", c["sym"], 500)
    # wires between poles
    for x in (0, 8, 16):
        wire([(x, 0), (x, 8)])
        wire([(x, 24), (x, 36)])
        wire([(x, 52), (x, 64)])
        wire([(x, 80), (x, 88)])
    wire([(24, 0), (24, 88)])
    wire([(24, 88), (24, 88)])
    for x in (0, 8, 16, 24):
        sym("terminals/terminal", x, 88)
    text(-6, 92, "-X1", 9.5, "end", c["sym"], 500)
    for x in (0, 8, 16, 24):
        wire([(x, 96), (x, 108)])
    sym("motors/motor_3ph", 0, 108)
    text(-3, 124, "=A1+FLD-M1", 9.5, "end", c["sym"], 500)
    for i, x in enumerate((0, 8, 16)):
        wnum(x, 18, str(1 + i * 4))
        wnum(x, 30, str(2 + i * 4))
        wnum(x, 58, str(3 + i * 4))
        wnum(x, 100, str(4 + i * 4))
    # selection on -K1 (accent box with corner handles)
    sx0, sy0 = ox + (-3) * k, oy + 34 * k
    sw, sh = 22 * k, 20 * k
    out.append(f'<rect x="{sx0:.1f}" y="{sy0:.1f}" width="{sw:.1f}" height="{sh:.1f}" fill="{c["sel"]}" fill-opacity=".10" stroke="{c["sel"]}" stroke-width="1.2" stroke-dasharray="4 3"/>')
    for hx, hy in ((sx0, sy0), (sx0 + sw, sy0), (sx0, sy0 + sh), (sx0 + sw, sy0 + sh)):
        out.append(f'<rect x="{hx-3:.1f}" y="{hy-3:.1f}" width="6" height="6" fill="{c["paper"]}" stroke="{c["sel"]}" stroke-width="1.2"/>')

    # ---- circuit 2: control (x 52): feed, F2 aux (NC), S0 (stop), S1 || K1 aux, coil K1, 0 V
    cx = 56
    sym("connectivity/potential_feed", cx, -8)
    text(cx, -10, "L+", 9.5, "middle")
    wire([(cx, 0), (cx, 8)])
    sym("contacts/thermal_nc", cx, 8)
    text(cx - 11, 16, "-F2", 9.5, "end", c["sym"], 500)
    wire([(cx, 24), (cx, 28)])
    sym("operators/pushbutton_nc", cx, 28)
    text(cx - 11, 36, "-S0", 9.5, "end", c["sym"], 500)
    wire([(cx, 44), (cx, 48)])
    # parallel hold-in
    dot(cx, 48)
    wire([(cx, 48), (cx + 14, 48), (cx + 14, 52)])
    sym("operators/pushbutton_no", cx, 52)
    text(cx - 11, 60, "-S1", 9.5, "end", c["sym"], 500)
    sym("contacts/no", cx + 14, 52)
    text(cx + 19, 60, "-K1", 9.5, "start", c["sym"], 500)
    wire([(cx, 68), (cx, 72), (cx + 14, 72), (cx + 14, 68)])
    dot(cx, 72)
    wire([(cx, 72), (cx, 76)])
    sym("coils/contactor", cx, 76)
    text(cx - 6, 84, "-K1", 9.5, "end", c["sym"], 500)
    wire([(cx, 92), (cx, 100)])
    out.append(f'<path d="M{ox + (cx-3)*k:.1f} {oy + 100*k:.1f}h{6*k:.1f}" stroke="{c["wire"]}" stroke-width="1.3"/>')
    text(cx, 104, "0 V", 9.5, "middle")
    for t, y in (("11", 22), ("12", 40), ("13", 62), ("14", 76), ("15", 88)):
        wnum(cx, y, t)
    # cross reference under coil
    out.append(f'<text x="{ox + (cx+6)*k:.1f}" y="{oy + 91*k:.1f}" font-size="8.5" fill="{c["wirenum"]}">1.1</text>')

    # ---- circuit 3: a second feeder (motor protection breaker -> contactor -> motor)
    bx = 118
    for i, x in enumerate((0, 8, 16)):
        sym("connectivity/potential_feed", bx + x, -8)
        text(bx + x, -10, f"L{i+1}", 9.5, "middle")
    sym("protection/motor_protection_breaker", bx, 8)
    text(bx - 24, 16, "-Q2", 9.5, "end", c["sym"], 500)
    sym("contactors/main_3p", bx, 36)
    text(bx - 6, 44, "-K2", 9.5, "end", c["sym"], 500)
    for x in (0, 8, 16):
        wire([(bx + x, 0), (bx + x, 8)])
        wire([(bx + x, 24), (bx + x, 36)])
        wire([(bx + x, 52), (bx + x, 80)])
        sym("terminals/terminal", bx + x, 80)
        wire([(bx + x, 88), (bx + x, 104)])
    text(bx - 6, 84, "-X2", 9.5, "end", c["sym"], 500)
    sym("motors/motor_1ph", bx + 4, 104)
    text(bx + 1, 118, "=A1+FLD-M2", 9.5, "end", c["sym"], 500)

    # a proposal mark (lightbulb dot) and a warning mark near X1 PE, as the canvas draws them
    out.append(f'<circle cx="{ox + 30*k:.1f}" cy="{oy + 90*k:.1f}" r="7" fill="{c["mark"]}" fill-opacity=".16" stroke="{c["mark"]}"/>')
    out.append(f'<text x="{ox + 30*k:.1f}" y="{oy + 90*k + 3.6:.1f}" font-size="10" fill="{c["mark"]}" text-anchor="middle" font-weight="700">!</text>')

    # title block
    tx, ty, tw, th = W - 38 - 300, H - 30 - 74, 300, 74
    out.append(f'<rect x="{tx}" y="{ty}" width="{tw}" height="{th}" fill="{c["paper"]}" stroke="{c["line"]}"/>')
    for i in range(1, 4):
        out.append(f'<path d="M{tx} {ty + i*th/4}h{tw}" stroke="{c["line"]}"/>')
    out.append(f'<path d="M{tx+80} {ty}v{th}" stroke="{c["line"]}"/>')
    for i, (a, b) in enumerate((("Project", "Motor starter (example)"), ("Page", "1 Main circuit"), ("Structure", "=A1+CAB"), ("Rev", "A  2026-10-17  TV"))):
        out.append(f'<text x="{tx+6}" y="{ty + i*th/4 + 13}" font-size="9" fill="{c["text"]}">{a}</text>')
        out.append(f'<text x="{tx+88}" y="{ty + i*th/4 + 13}" font-size="9.5" fill="{c["sym"]}">{b}</text>')
    out.append("</svg>")
    return "\n".join(out)


# ---------------------------------------------------------------- isometric cabinet
def iso(theme, W=760, H=700):
    dark = theme == "dark"
    base = dict(
        shell="#9AA3AD" if dark else "#7F8996",
        plate="#8C95A0" if dark else "#CFD3D8",
        rail="#C6CBD2" if dark else "#A7AEB7",
        duct="#6E7782" if dark else "#BFC4CB",
        dev=["#D9D4C8", "#C7C2B6", "#B7B2A6"] if dark else ["#E9E4D8", "#D8D3C6", "#C5C0B3"],
        line="#E3E1DC" if dark else "#3B3A36",
        chip="#17171A" if dark else "#FFFFFF",
        chiptx="#E3E1DC" if dark else "#1E1D1B",
    )
    kx, ky = 0.58, 0.30  # iso axes: x to the right-down, z to the left-down
    X0, Y0 = 400, 425
    S = 0.5

    def pr(x, y, z):
        return (X0 + (x - z) * kx * S * 1.7, Y0 - y * S * 1.0 + (x + z) * ky * S * 1.7)

    def poly(pts, fill, op=1.0, stroke=None, sw=1):
        d = " ".join(f"{px:.1f},{py:.1f}" for px, py in (pr(*p) for p in pts))
        s = f' stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round"' if stroke else ""
        return f'<polygon points="{d}" fill="{fill}" fill-opacity="{op}"{s}/>'

    def shade(hexc, f):
        r, g, b = (int(hexc[i:i+2], 16) for i in (1, 3, 5))
        return "#%02X%02X%02X" % tuple(min(255, int(v * f)) for v in (r, g, b))

    def box(x, y, z, w, h, d, col, lab=None):
        # top, left (facing -x... drawn as +z face) and right faces
        o = []
        o.append(poly([(x, y + h, z), (x + w, y + h, z), (x + w, y + h, z + d), (x, y + h, z + d)], shade(col, 1.08), 1, base["line"], .5))
        o.append(poly([(x, y, z + d), (x + w, y, z + d), (x + w, y + h, z + d), (x, y + h, z + d)], shade(col, .94), 1, base["line"], .5))
        o.append(poly([(x + w, y, z), (x + w, y, z + d), (x + w, y + h, z + d), (x + w, y + h, z)], shade(col, .78), 1, base["line"], .5))
        return "".join(o)

    W_, H_, D_ = 600, 800, 230
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="Inter,Arial,sans-serif">']
    # floor shadow
    o.append(poly([(-40, 0, -40), (W_ + 40, 0, -40), (W_ + 40, 0, D_ + 40), (-40, 0, D_ + 40)], "#000", .10 if dark else .06))
    # back plate (z = 0 plane) and side wall
    o.append(poly([(0, 0, 0), (W_, 0, 0), (W_, H_, 0), (0, H_, 0)], base["plate"], 1, base["line"], .8))
    # rails and ducts (z just in front of plate)
    for ry in (180, 360, 540, 700):
        o.append(box(30, ry, 0, W_ - 60, 4, 14, base["rail"]))
    for dy in (250, 450, 620):
        o.append(box(30, dy, 0, W_ - 60, 55, 30, base["duct"]))
    # devices
    devs = [(60, 540, 36, 90, "-F1"), (112, 540, 36, 90, "-F2"), (60, 360, 90, 120, "-K1"), (170, 360, 40, 90, "-S0"), (240, 360, 40, 90, "-S1"),
            (60, 180, 54, 110, "-Q2"), (130, 180, 90, 110, "-K2"), (240, 180, 54, 110, "-T1")]
    for i, (x, y, w, h, lab) in enumerate(devs):
        o.append(box(x, y + 4, 14, w, h, 70, base["dev"][i % 3]))
    # side wall (right) as translucent glass
    o.append(poly([(W_, 0, 0), (W_, 0, D_), (W_, H_, D_), (W_, H_, 0)], base["shell"], .16, base["line"], .8))
    o.append(poly([(0, H_, 0), (W_, H_, 0), (W_, H_, D_), (0, H_, D_)], base["shell"], .12, base["line"], .8))
    o.append(poly([(0, 0, D_), (W_, 0, D_), (W_, H_, D_), (0, H_, D_)], base["shell"], .06, base["line"], .6))
    # tag chips
    for i, (x, y, w, h, lab) in enumerate(devs):
        px, py = pr(x + w / 2, y + h * 0.55, 84)
        o.append(f'<rect x="{px-17:.1f}" y="{py-9:.1f}" width="34" height="18" rx="3" fill="{base["chip"]}" fill-opacity=".88"/>')
        o.append(f'<text x="{px:.1f}" y="{py+4:.1f}" font-size="11" font-weight="600" fill="{base["chiptx"]}" text-anchor="middle">{lab}</text>')
    o.append("</svg>")
    return "\n".join(o)


def main():
    phos, out = sys.argv[1], sys.argv[2]
    maps = dict(a.split("=", 1) for a in sys.argv[3:] if "=" in a and not a.startswith("-"))
    os.makedirs(out, exist_ok=True)
    assets = {
        "canvas-dark": schematic("dark"),
        "canvas-light": schematic("light"),
        "iso-dark": iso("dark"),
        "iso-light": iso("light"),
    }
    for n, svg in assets.items():
        open(os.path.join(out, n + ".svg"), "w").write(svg)
    icons, miss = icon_css(phos)
    if miss:
        print("missing phosphor icons:", " ".join(miss), file=sys.stderr)
    base = open(os.path.join(HERE, "kc.base.css")).read()
    art = []
    for n in assets:
        art.append(f".kc .art-{n}{{background-image:url({maps.get(n, n + '.svg')});background-position:center top;background-size:contain;background-repeat:no-repeat}}")
    rays = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><path d="M128 38 L136 104 L128 112 L120 104 Z M128 218 L120 152 L128 144 L136 152 Z '
            'M38 128 L104 120 L112 128 L104 136 Z M218 128 L152 136 L144 128 L152 120 Z"/>'
            '<circle cx="84" cy="84" r="5"/><circle cx="172" cy="84" r="5"/><circle cx="84" cy="172" r="5"/><circle cx="172" cy="172" r="5"/></svg>')
    brand = (".kc .brand{position:relative;display:inline-block;width:20px;height:20px;color:var(--tx)}"
             ".kc .brand::before{content:\"\";position:absolute;inset:0;background:currentColor;-webkit-mask:" + data_uri(rays) + " center/contain no-repeat;mask:" + data_uri(rays) + " center/contain no-repeat}"
             ".kc .brand::after{content:\"\";position:absolute;left:50%;top:50%;width:18%;height:18%;margin:-9% 0 0 -9%;border-radius:50%;background:var(--ac)}")
    open(os.path.join(out, "kc.css"), "w").write(base + "\n" + brand + "\n" + icons + "\n" + "\n".join(art) + "\n")
    print("kc.css", os.path.getsize(os.path.join(out, "kc.css")))


if __name__ == "__main__":
    main()
