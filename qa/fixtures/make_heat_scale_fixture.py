#!/usr/bin/env python3
"""Makes cabinet-heat-scale-hot.novo from cabinet-cooling-advice.novo (run from the repo root).

The hot cabinet with T1 at 600 W and no openings, so the sealed cabinet runs to about
285 degrees and the temperature scale has to label a range of over 250 K.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"
OUT = "qa/fixtures/cabinet-heat-scale-hot.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
row["properties"]["heat.openings"] = json.dumps([])
c.execute("update records set data=? where id=1", (json.dumps(row),))
dev = json.loads(c.execute("select data from records where id=43").fetchone()[0])
dev["properties"]["power_loss_w"] = 600.0
c.execute("update records set data=? where id=43", (json.dumps(dev),))
c.commit()
c.execute("vacuum")
c.close()
