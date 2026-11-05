#!/usr/bin/env python3
"""Makes the enclosure-record cooling fixtures from cabinet-cooling-advice.novo (run from the repo root).

  cabinet-cooling-spec-catalogue.novo   record with catalogue outside size 1000 x 1500 x 400: "Enclosure size: catalogue" and the estimate notice
  cabinet-cooling-spec-no-depth.novo    the same record without the outside depth: "The enclosure has no outside depth."

Synthetic enclosure, no maker file or article.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"


def make(name, props):
    out = f"qa/fixtures/{name}.novo"
    shutil.copyfile(BASE, out)
    c = sqlite3.connect(out)
    row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
    row["properties"].update(props)
    c.execute("update records set data=? where id=1", (json.dumps(row),))
    c.commit()
    c.execute("vacuum")
    c.close()


record = {
    "encl.article": "SYNTH.1",
    "encl.width_mm": 1000.0, "encl.width_mm.src": "catalogue",
    "encl.height_mm": 1500.0, "encl.height_mm.src": "catalogue",
    "encl.depth_mm": 400.0, "encl.depth_mm.src": "catalogue",
    "encl.wall_mm": 1.5, "encl.wall_mm.src": "catalogue",
}
make("cabinet-cooling-spec-catalogue", record)
make("cabinet-cooling-spec-no-depth", {k: v for k, v in record.items() if not k.startswith("encl.depth_mm")})
