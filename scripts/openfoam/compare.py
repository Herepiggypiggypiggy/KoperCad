#!/usr/bin/env python3
"""Prints the KoperCAD versus OpenFOAM table from ours.json and of.json.

    python compare.py ours.json of.json
"""
import json
import sys

ours = json.load(open(sys.argv[1]))
theirs = json.load(open(sys.argv[2]))
AMB = 25.0


def pct(a, b):
    return "%+.1f%%" % (100.0 * (a - b) / b) if (a is not None and b not in (None, 0)) else "-"


print("| case | KoperCAD cells | OpenFOAM cells(n) | mean air rise K ours / OF | diff | max air rise K ours / OF | vent flow m3/h ours / OF | exhaust K ours / OF | heat balance ours |")
print("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
for o in ours:
    for t in theirs:
        if t["case"] != o["case"]:
            continue
    rows = [t for t in theirs if t["case"] == o["case"]]
    for i, t in enumerate(sorted(rows, key=lambda r: r["n_per_50mm"])):
        # pair coarse-coarse, mid-mid, fine-fine by order
        os_ = [x for x in ours if x["case"] == o["case"]]
        if o is not os_[min(i, len(os_) - 1)]:
            continue
        mo, mt = o["mean_air_c"] - AMB, (t["mean_air_c"] - AMB) if t["mean_air_c"] is not None else None
        xo, xt = o["max_air_c"] - AMB, (t["max_air_c"] - AMB) if t["max_air_c"] is not None else None
        fo = ", ".join("%.1f" % (v * 3600) for v in o["vent_flow_out_m3s"]) or "-"
        ft = ", ".join("%.1f" % (v * 3600) if v is not None else "-" for v in t["vent_flow_out_m3s"]) or "-"
        eo = ", ".join("%.1f" % (v - AMB) if v is not None else "-" for v in o["exhaust_c"]) or "-"
        et = ", ".join("%.1f" % (v - AMB) if v is not None else "-" for v in t["exhaust_c"]) or "-"
        print("| %s | %d | n=%d (%.1f mm) | %.2f / %s | %s | %.2f / %s | %s / %s | %s / %s | %+.3f |" % (
            o["case"], o["cells"], t["n_per_50mm"], t["cell_mm"], mo, "%.2f" % mt if mt is not None else "-",
            pct(mo, mt) if mt else "-", xo, "%.2f" % xt if xt is not None else "-", fo, ft, eo, et, o["imbalance"]))
