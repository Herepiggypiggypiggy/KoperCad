#!/usr/bin/env python3
"""Compare KoperCAD's imported wires with the wires EPLAN drew in its PDF.

Usage: pdf_compare.py <eplan.pdf> <geometry.json> <out-dir> [--pages 3,20]

`geometry.json` comes from the opt-in test `novo-tests/tests/eplan_verify.rs`
(KOPERCAD_CRANE_EPJ=... KOPERCAD_VERIFY_OUT=<dir>). The PDF page index of a
drawn page is its `source` (file page order). Both sides are reduced to
axis-aligned wire segments in page millimetres (y down) and compared by
covered length and by end points. Nothing is filtered: every unmatched EPLAN
segment and every extra KoperCAD segment is written to `diff.json`.

Needs PyMuPDF (`pip install pymupdf`); it is a developer tool, never shipped
and never linked into KoperCAD. The PDF and all outputs are private data.
"""
import json
import sys
from collections import defaultdict

import pymupdf

SHEET_W_MM = 420.0  # A3 sheet; EPLAN printed it fitted to A4, so scale by the page width
PT = None  # set per page from its width
TOL = 0.35  # mm: two lines this close are the same line
INNER = (10.0, 410.0, 10.0, 270.0)  # x0, x1, y0, y1 mm: outside is border, ticks, title block


def is_blue(c):
    return c is not None and len(c) == 3 and c[2] > 0.9 and c[0] < 0.1 and c[1] < 0.1


def is_frame(c):
    return c is not None and len(c) == 3 and abs(c[0] - 0.49) < 0.02 and abs(c[1] - 0.65) < 0.02


def segs_of_pdf(page):
    """Line segments of the page, classified.

    Returns (wires, busbars, other, dots). In EPLAN's default look wires are
    coloured solid lines (red here), busbars are thick solid black lines,
    symbols and boxes are blue or dashed. Page margin, tick marks and title
    block are cut off so they never count as wires.
    """
    PT = SHEET_W_MM / page.rect.width
    wires, busbars, other, dots = [], [], [], []
    for d in page.get_drawings():
        col = d.get("color")
        fill = d.get("fill")
        dashed = bool(d.get("dashes")) and d["dashes"].strip() not in ("[] 0", "")
        dashed = dashed and "[]" not in d["dashes"]
        for it in d["items"]:
            if it[0] != "l":
                continue
            a, b = it[1], it[2]
            seg = (a.x * PT, a.y * PT, b.x * PT, b.y * PT, tuple(col or ()))
            xs, ys = (seg[0], seg[2]), (seg[1], seg[3])
            if min(xs) < INNER[0] or max(xs) > INNER[1] or min(ys) < INNER[2] or max(ys) > INNER[3]:
                continue
            if is_frame(col):
                continue
            if is_blue(col) or dashed:
                other.append(seg)
            elif col is not None and tuple(col) == (0.0, 0.0, 0.0):
                (busbars if (d.get("width") or 0) >= 0.9 else other).append(seg)
            else:
                wires.append(seg)
        if fill is not None and d.get("rect") is not None:
            r = d["rect"]
            if r.width * PT < 2.0 and r.height * PT < 2.0 and not is_blue(fill):
                dots.append(((r.x0 + r.x1) / 2 * PT, (r.y0 + r.y1) / 2 * PT))
    return wires, busbars, other, dots


def normalize(segs):
    """-> (horizontals {y: [(x0,x1)]}, verticals {x: [(y0,y1)]}, skew count)."""
    hs, vs, skew = defaultdict(list), defaultdict(list), 0
    for x0, y0, x1, y1, *_ in segs:
        if abs(y0 - y1) < TOL and abs(x0 - x1) >= TOL:
            hs[round((y0 + y1) / 2, 2)].append((min(x0, x1), max(x0, x1)))
        elif abs(x0 - x1) < TOL and abs(y0 - y1) >= TOL:
            vs[round((x0 + x1) / 2, 2)].append((min(y0, y1), max(y0, y1)))
        elif abs(x0 - x1) >= TOL and abs(y0 - y1) >= TOL:
            skew += 1
    return hs, vs, skew


def merge(iv):
    iv.sort()
    out = []
    for a, b in iv:
        if out and a <= out[-1][1] + TOL:
            out[-1][1] = max(out[-1][1], b)
        else:
            out.append([a, b])
    return out


def group(lines):
    """Merge intervals of lines within TOL of each other into one family."""
    out = []  # (coord, merged intervals)
    for k in sorted(lines):
        if out and k - out[-1][0] <= TOL:
            out[-1][1].extend(lines[k])
        else:
            out.append((k, list(lines[k])))
    return [(k, merge(v)) for k, v in out]


def covered(interval, family):
    a, b = interval
    got = 0.0
    for c, ivs in family:
        for x, y in ivs:
            lo, hi = max(a, x), min(b, y)
            if hi > lo:
                got += hi - lo
    return min(got, b - a)


def compare_axis(theirs, ours):
    """theirs, ours: list of (coord, intervals) -> total, covered, unmatched list."""
    tot = cov = 0.0
    miss = []
    for coord, ivs in theirs:
        fam = [f for f in ours if abs(f[0] - coord) <= TOL]
        for a, b in ivs:
            tot += b - a
            c = covered((a, b), fam)
            cov += c
            if b - a - c > 0.5:
                miss.append((coord, a, b, round(b - a - c, 2)))
    return tot, cov, miss


def near(p, pts, r=0.6):
    return any(abs(p[0] - q[0]) <= r and abs(p[1] - q[1]) <= r for q in pts)


def ours_segs(poly_list):
    out = []
    for poly in poly_list:
        for a, b in zip(poly, poly[1:]):
            out.append((a[0], a[1], b[0], b[1]))
    return out


def classify_missing(seg, ours_h, ours_v, pins, junctions):
    """Why EPLAN has this wire stretch and KoperCAD does not."""
    axis, c, a, b, length = seg
    p0, p1 = ((a, c), (b, c)) if axis == "h" else ((c, a), (c, b))
    fam = ours_h if axis == "h" else ours_v
    if length <= 2.5:
        return "pin stub (symbol artwork)"
    for coord, ivs in fam:
        if abs(coord - c) <= TOL:
            for x, y in ivs:
                if y >= a - 12 and x <= b + 12:
                    return "collinear KoperCAD wire nearby: pin or route offset"
    if near(p0, pins, 1.0) or near(p1, pins, 1.0):
        return "pin there, no wire (connection missing)"
    if near(p0, junctions, 1.0) or near(p1, junctions, 1.0):
        return "junction there, no wire (connection missing)"
    return "nothing of KoperCAD near (no pin, no wire)"


def classify_extra(seg, eh, ev):
    axis, c, a, b, length = seg
    fam = eh if axis == "h" else ev
    for coord, ivs in fam:
        if abs(coord - c) <= 8.0:
            for x, y in ivs:
                if y >= a - 2 and x <= b + 2:
                    return "parallel EPLAN wire within 8 mm: route differs"
    return "no EPLAN line near (extra wire)"


def render_diff(doc, pg, diff_page, path):
    """One PNG: EPLAN's page, KoperCAD's wires (green) and pins (circles) laid
    over it, EPLAN wire stretches KoperCAD lacks in magenta, KoperCAD
    stretches EPLAN does not have in orange."""
    page = doc[pg["source"]]
    k = page.rect.width / SHEET_W_MM  # sheet mm -> pt
    sh = page.new_shape()
    for poly in pg["wires"]:
        pts = [pymupdf.Point(x * k, y * k) for x, y in poly]
        for a, b in zip(pts, pts[1:]):
            sh.draw_line(a, b)
    sh.finish(color=(0, 0.6, 0), width=1.6)
    for x, y in pg["pins"]:
        sh.draw_circle(pymupdf.Point(x * k, y * k), 1.6)
    sh.finish(color=(0, 0.5, 0), width=0.6)
    for key, color in (("eplan_missing_in_ours", (1, 0, 1)), ("busbar_missing_in_ours", (0.6, 0.3, 0.9)), ("ours_not_in_eplan", (1, 0.55, 0))):
        for axis, c, a, b, _ in diff_page.get(key, []):
            p0, p1 = ((a, c), (b, c)) if axis == "h" else ((c, a), (c, b))
            sh.draw_line(pymupdf.Point(p0[0] * k, p0[1] * k), pymupdf.Point(p1[0] * k, p1[1] * k))
        sh.finish(color=color, width=2.2)
    sh.commit()
    page.get_pixmap(matrix=pymupdf.Matrix(2, 2)).save(path)


def main():
    pdf, geo, outdir = sys.argv[1:4]
    only = None
    if "--pages" in sys.argv:
        only = {int(x) for x in sys.argv[sys.argv.index("--pages") + 1].split(",")}
    doc = pymupdf.open(pdf)
    pages = json.load(open(geo))
    rows, diff = [], {}
    total_classes = defaultdict(lambda: [0, 0.0])
    total_extra = defaultdict(lambda: [0, 0.0])
    for pg in pages:
        src = pg["source"]
        if only is not None and src + 1 not in only:
            continue
        if src >= len(doc):
            continue
        wires, busbars, other, dots = segs_of_pdf(doc[src])
        eh, ev, eskew = normalize(wires)
        eh_raw, ev_raw = dict(eh), dict(ev)
        oh, ov, oskew = normalize(ours_segs(pg["wires"]))
        ah, av, _ = normalize(wires + busbars + other)  # everything EPLAN drew
        bh, bv, _ = normalize(busbars)
        eh, ev, oh, ov = group(eh), group(ev), group(oh), group(ov)
        ah, av, bh, bv = group(ah), group(av), group(bh), group(bv)
        th, ch, mh = compare_axis(eh, oh)
        tv, cv, mv = compare_axis(ev, ov)
        xh, xch, xmh = compare_axis(oh, ah)  # ours vs all of EPLAN's drawing
        xv, xcv, xmv = compare_axis(ov, av)
        bth, bch, bmh = compare_axis(bh, oh)
        btv, bcv, bmv = compare_axis(bv, ov)
        e_len, e_cov = th + tv, ch + cv
        o_len, o_cov = xh + xv, xch + xcv
        # End points: EPLAN dots (joints) and pins must meet our pins/junctions.
        ours_pts = [tuple(p) for p in pg["pins"]] + [tuple(p) for p in pg["junctions"]]
        ours_pts_pins = [tuple(p) for p in pg["pins"]]
        ours_pts_j = [tuple(p) for p in pg["junctions"]]
        dots_ok = sum(1 for d in dots if near(d, ours_pts))
        rows.append(
            dict(
                pdf_page=src + 1,
                name=pg["name"],
                title=pg["title"],
                eplan_mm=round(e_len),
                eplan_covered_pct=round(100 * e_cov / e_len, 1) if e_len else None,
                ours_mm=round(o_len),
                ours_on_eplan_pct=round(100 * o_cov / o_len, 1) if o_len else None,
                busbar_mm=round(bth + btv),
                busbar_covered_pct=round(100 * (bch + bcv) / (bth + btv), 1) if bth + btv else None,
                eplan_dots=len(dots),
                eplan_dots_at_our_points=dots_ok,
                eplan_skew=eskew,
            )
        )
        classes = defaultdict(lambda: [0, 0.0])
        for axis, items in (("h", mh), ("v", mv)):
            for coord, a, b, miss in items:
                cl = classify_missing((axis, coord, a, b, miss), oh, ov, ours_pts_pins, ours_pts_j)
                classes[cl][0] += 1
                classes[cl][1] += miss
        xclasses = defaultdict(lambda: [0, 0.0])
        eh_all, ev_all = group(eh_raw), group(ev_raw)
        for axis, items in (("h", xmh), ("v", xmv)):
            for coord, a, b, miss in items:
                cl = classify_extra((axis, coord, a, b, miss), eh_all, ev_all)
                xclasses[cl][0] += 1
                xclasses[cl][1] += miss
        for k, (n, mm) in classes.items():
            total_classes[k][0] += n
            total_classes[k][1] += mm
        for k, (n, mm) in xclasses.items():
            total_extra[k][0] += n
            total_extra[k][1] += mm
        diff[src + 1] = dict(
            eplan_missing_in_ours=[("h", *m) for m in mh] + [("v", *m) for m in mv],
            busbar_missing_in_ours=[("h", *m) for m in bmh] + [("v", *m) for m in bmv],
            ours_not_in_eplan=[("h", *m) for m in xmh] + [("v", *m) for m in xmv],
        )
    rows.sort(key=lambda r: (r["eplan_covered_pct"] is None, r["eplan_covered_pct"] or 0))
    if "--render" in sys.argv:
        want = {int(x) for x in sys.argv[sys.argv.index("--render") + 1].split(",")}
        by_src = {pg["source"] + 1: pg for pg in pages}
        for n in want:
            if n in by_src and n in diff:
                render_diff(doc, by_src[n], diff[n], f"{outdir}/diff-page-{n}.png")
    json.dump(rows, open(f"{outdir}/pages.json", "w"), indent=1)
    json.dump(diff, open(f"{outdir}/diff.json", "w"))
    et = sum(r["eplan_mm"] for r in rows)
    ec = sum(r["eplan_mm"] * (r["eplan_covered_pct"] or 0) / 100 for r in rows)
    ot = sum(r["ours_mm"] for r in rows)
    oc = sum(r["ours_mm"] * (r["ours_on_eplan_pct"] or 0) / 100 for r in rows)
    dt = sum(r["eplan_dots"] for r in rows)
    dk = sum(r["eplan_dots_at_our_points"] for r in rows)
    with open(f"{outdir}/pdf_compare.md", "w") as f:
        f.write("# Wires: KoperCAD vs EPLAN PDF\n\n")
        f.write(f"pages compared {len(rows)}\n\n")
        f.write(f"- EPLAN wire length {et:.0f} mm, covered by KoperCAD wires {100*ec/max(et,1):.1f}%\n")
        f.write(f"- KoperCAD wire length {ot:.0f} mm, lying on an EPLAN wire {100*oc/max(ot,1):.1f}%\n")
        bt = sum(r["busbar_mm"] for r in rows)
        bc = sum(r["busbar_mm"] * (r["busbar_covered_pct"] or 0) / 100 for r in rows)
        f.write(f"- EPLAN busbar length {bt:.0f} mm, covered by a KoperCAD wire {100*bc/max(bt,1):.1f}% (busbars are a symbol in KoperCAD, lane B)\n")
        f.write(f"- EPLAN joint dots {dt}, at a KoperCAD pin or junction {100*dk/max(dt,1):.1f}%\n\n")
        f.write("## EPLAN wire stretches KoperCAD lacks, by cause (ranked by length)\n\n")
        for k, (n, mm) in sorted(total_classes.items(), key=lambda kv: -kv[1][1]):
            f.write(f"- {mm:.0f} mm in {n} stretches: {k}\n")
        f.write("\n## KoperCAD wire stretches EPLAN does not have, by cause\n\n")
        for k, (n, mm) in sorted(total_extra.items(), key=lambda kv: -kv[1][1]):
            f.write(f"- {mm:.0f} mm in {n} stretches: {k}\n")
        f.write("\n## Worst pages by EPLAN length not covered\n\n| pdf page | name | EPLAN mm | covered % | ours mm | on EPLAN % | dots | dots matched |\n|---|---|---|---|---|---|---|---|\n")
        for r in sorted(rows, key=lambda r: -(r["eplan_mm"] * (100 - (r["eplan_covered_pct"] or 0))))[:40]:
            f.write(f"| {r['pdf_page']} | {r['name']} | {r['eplan_mm']} | {r['eplan_covered_pct']} | {r['ours_mm']} | {r['ours_on_eplan_pct']} | {r['eplan_dots']} | {r['eplan_dots_at_our_points']} |\n")
    print(open(f"{outdir}/pdf_compare.md").read())


if __name__ == "__main__":
    main()
