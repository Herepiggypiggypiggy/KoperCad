#!/usr/bin/env python3
"""Makes cabinet-heat-spec-small.novo and cabinet-heat-spec-large.novo from cabinet-cooling-advice.novo
(run from the repo root). Same devices and plate; only the `encl.` record (E1, novo_layout::spec) differs:
a synthetic catalogue enclosure of 1000x1400x300 mm or of 1400x1800x500 mm, wall 2 mm.
No maker data: the article numbers are invented."""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"
for name, article, (w, h, d) in [
    ("small", "SYNTH.SMALL", (1000.0, 1400.0, 300.0)),
    ("large", "SYNTH.LARGE", (1400.0, 1800.0, 500.0)),
]:
    out = f"qa/fixtures/cabinet-heat-spec-{name}.novo"
    shutil.copyfile(BASE, out)
    c = sqlite3.connect(out)
    row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
    p = row["properties"]
    p["encl.article"] = article
    p["encl.series"] = "Synthetic"
    for key, v in [("width_mm", w), ("height_mm", h), ("depth_mm", d), ("wall_mm", 2.0)]:
        p[f"encl.{key}"] = v
        p[f"encl.{key}.src"] = "catalogue"
    c.execute("update records set data=? where id=1", (json.dumps(row),))
    c.commit()
    c.execute("vacuum")
    c.close()
