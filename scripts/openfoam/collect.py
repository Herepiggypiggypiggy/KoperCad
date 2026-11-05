#!/usr/bin/env python3
"""Reads the postProcessing output of the OpenFOAM cases written by
make_cases.py and prints one JSON row per case (same keys as ours.json).

    python collect.py cases.json case_root > of.json
"""
import glob
import json
import os
import sys

KELVIN = 273.15


def last_row(case_dir, func, fname):
    files = sorted(glob.glob(os.path.join(case_dir, "postProcessing", func, "*", fname)))
    if not files:
        return None
    rows = [l.split() for l in open(files[-1]) if l.strip() and not l.lstrip().startswith("#")]
    if not rows:
        return None
    return [float(x.strip("()")) for x in rows[-1][1:]]


def main():
    spec = json.load(open(sys.argv[1]))
    root = sys.argv[2]
    out = []
    for case in spec["cases"]:
        for d in sorted(glob.glob(os.path.join(root, case["name"] + "_n*"))):
            info = json.load(open(os.path.join(d, "case-info.json")))
            W, H, D = case["domain_mm"]
            mean = last_row(d, "meanT", "volFieldValue.dat")
            mx = last_row(d, "maxT", "volFieldValue.dat")
            pr = last_row(d, "probes", "T")
            row = dict(case=case["name"], n_per_50mm=info["n_per_50mm"], cell_mm=info["cell_mm"],
                       mean_air_c=mean[0] - KELVIN if mean else None, max_air_c=mx[0] - KELVIN if mx else None,
                       probes_c=[t - KELVIN for t in pr] if pr else None)
            flows, exh = [], []
            for vi in range(len(case["vents"])):
                f = last_row(d, "ventFlow%d" % vi, "surfaceFieldValue.dat")
                t = last_row(d, "ventT%d" % vi, "surfaceFieldValue.dat")
                flows.append(f[0] if f else None)
                exh.append(t[0] - KELVIN if (t and f and f[0] > 0) else None)
            row["vent_flow_out_m3s"] = flows
            row["exhaust_c"] = exh
            if case["wall"]["kind"] == "film":
                h, tout = case["wall"]["h"], case["wall"]["t_out_c"]
                areas = {"xmin": H * D, "xmax": H * D, "ymin": W * D, "ymax": W * D, "zmin": W * H, "zmax": W * H}
                vent_a = {"ymin": 0.0, "ymax": 0.0}
                for v in case["vents"]:
                    vent_a["ymax" if v["face"] == "top" else "ymin"] += v["width"] * v["depth"]
                q = []
                for wn in ("xmin", "xmax", "ymax", "ymin", "zmin", "zmax"):  # ours: x-, x+, top(y-), bottom(y+), z-, z+
                    t = last_row(d, "wallT_" + wn, "surfaceFieldValue.dat")
                    a = (areas[wn] - vent_a.get(wn, 0.0)) * 1e-6
                    q.append(h * a * (t[0] - KELVIN - tout) if t else None)
                row["wall_heat_w"] = q
            out.append(row)
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
