#!/usr/bin/env python3
"""Text-on-text overlaps in exported DXF pages (TEXT and MTEXT, group codes 10/20 position,
40 height, 1 string, 72/73 alignment ignored: the box is estimated from the insertion
point, glyph advance 0.6 x height). A DXF holds no glyph widths, so this is the same
estimate the canvas uses; it shows whether the DXF carries the collisions of the page.

  python3 -I tools/text-conflicts/dxf_text_overlaps.py FOLDER_OR_FILE.dxf [...]

Not run in CI.
"""
import glob
import os
import sys


def texts(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        raw = [l.rstrip("\n") for l in f]
    pairs = [(raw[i].strip(), raw[i + 1]) for i in range(0, len(raw) - 1, 2)]
    out, cur = [], None
    for code, val in pairs:
        if code == "0":
            if cur and cur.get("s"):
                out.append(cur)
            cur = {} if val in ("TEXT", "MTEXT") else None
        elif cur is not None:
            if code == "10": cur["x"] = float(val)
            elif code == "20": cur["y"] = float(val)
            elif code == "40": cur["h"] = float(val)
            elif code == "1": cur["s"] = val
            elif code == "50": cur["rot"] = float(val)
    if cur and cur.get("s"):
        out.append(cur)
    return [t for t in out if "x" in t and "y" in t and "h" in t]


def box(t):
    w, h = len(t["s"]) * t["h"] * 0.6, t["h"]
    if abs(t.get("rot", 0.0)) % 180 == 90:
        w, h = h, w
    return (t["x"], t["y"], t["x"] + w, t["y"] + h)


def main():
    files = []
    for a in sys.argv[1:]:
        files += sorted(glob.glob(os.path.join(a, "*.dxf"))) if os.path.isdir(a) else [a]
    total = 0
    for f in files:
        ts = texts(f)
        bx = [box(t) for t in ts]
        n = 0
        for i in range(len(ts)):
            for j in range(i + 1, len(ts)):
                a, b = bx[i], bx[j]
                if min(a[2], b[2]) - max(a[0], b[0]) > 0.3 and min(a[3], b[3]) - max(a[1], b[1]) > 0.3:
                    n += 1
        if n:
            print(f"{os.path.basename(f)}: {n} overlapping text pairs of {len(ts)} texts")
        total += n
    print(f"total {total} overlapping text pairs in {len(files)} files")


if __name__ == "__main__":
    main()
