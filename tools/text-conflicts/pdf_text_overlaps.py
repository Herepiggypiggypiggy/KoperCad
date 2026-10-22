#!/usr/bin/env python3
"""Text-on-text overlaps in an exported PDF: per page, every pair of text runs whose
boxes share area. Reads the real glyph boxes the PDF holds (pdfminer.six), so it
measures what a printed sheet shows rather than the canvas estimate.

  python3 -I tools/text-conflicts/pdf_text_overlaps.py export.pdf [out.json]

Prints one line per page with overlaps and a total. Needs `pip install pdfminer.six`.
"""
import json
import sys

from pdfminer.high_level import extract_pages
from pdfminer.layout import LAParams, LTChar, LTTextLine, LTTextBox


def lines_of(page):
    out = []

    def walk(o):
        if isinstance(o, LTTextLine):
            t = o.get_text().strip()
            if t:
                out.append((t, o.bbox))
        elif hasattr(o, "__iter__"):
            for c in o:
                walk(c)

    walk(page)
    return out


def overlap(a, b, tol=0.25):
    # bbox = (x0, y0, x1, y1) in points; shrink by tol points so touching is not overlap.
    w = min(a[2], b[2]) - max(a[0], b[0])
    h = min(a[3], b[3]) - max(a[1], b[1])
    return (w - tol > 0 and h - tol > 0), max(0.0, w) * max(0.0, h)


def main():
    path = sys.argv[1]
    result = []
    total = 0
    # char_margin tiny so each run of glyphs is its own line, not merged with a neighbour.
    params = LAParams(char_margin=0.1, line_margin=0.05, word_margin=0.05, boxes_flow=None)
    for number, page in enumerate(extract_pages(path, laparams=params), 1):
        runs = lines_of(page)
        title = next((t for t, _ in runs if t.startswith("symbols") or t.startswith("terminals")), "")
        pairs = []
        for i, (ta, ba) in enumerate(runs):
            for tb, bb in runs[i + 1:]:
                hit, area = overlap(ba, bb)
                if hit:
                    pairs.append({"a": ta, "b": tb, "area_pt2": round(area, 1)})
        if pairs:
            print(f"page {number}: {len(pairs)} overlapping text pairs, e.g. {pairs[0]['a']!r} / {pairs[0]['b']!r}")
        total += len(pairs)
        result.append({"page": number, "texts": len(runs), "overlaps": pairs})
    print(f"total {total} overlapping text pairs in {len(result)} pages")
    if len(sys.argv) > 2:
        json.dump(result, open(sys.argv[2], "w"), indent=1)


if __name__ == "__main__":
    main()
