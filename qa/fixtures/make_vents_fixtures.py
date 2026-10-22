#!/usr/bin/env python3
"""Makes the vent fixtures from cabinet-cooling-advice.novo (run from the repo root).

  cabinet-vents-small-grille.novo  the hot cabinet with one 30 x 30 mm grille in the right wall
  cabinet-vents-at-limit.novo      the hot cabinet with the proposed filter fan and its grille, T1 limit (max_ambient_c) raised so it ends about at its limit

Usage: make_vents_fixtures.py [T1 max_ambient_c for the at-limit file]
"""
import json
import shutil
import sqlite3
import sys

BASE = "qa/fixtures/cabinet-cooling-advice.novo"
GRILLE = dict(kind="Grille", wall="Right", exhaust=False, u_mm=130.0, v_mm=0.0, width_mm=147.0,
              height_mm=147.0, free_fraction=0.6)
FAN = dict(kind="FilterFan", wall="Door", exhaust=False, u_mm=500.0, v_mm=1245.0, width_mm=150.0,
           height_mm=150.0, flow_m3h=70.0)


def make(name, openings, t1_limit=None):
    out = f"qa/fixtures/{name}.novo"
    shutil.copyfile(BASE, out)
    c = sqlite3.connect(out)
    row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
    row["properties"]["heat.openings"] = json.dumps(openings)
    c.execute("update records set data=? where id=1", (json.dumps(row),))
    if t1_limit is not None:
        dev = json.loads(c.execute("select data from records where id=43").fetchone()[0])
        dev["properties"]["max_ambient_c"] = t1_limit
        c.execute("update records set data=? where id=43", (json.dumps(dev),))
    c.commit()
    c.execute("vacuum")
    c.close()


make("cabinet-vents-small-grille", [dict(GRILLE, width_mm=30.0, height_mm=30.0)])
make("cabinet-vents-at-limit", [FAN, GRILLE], float(sys.argv[1]) if len(sys.argv) > 1 else 43.0)
