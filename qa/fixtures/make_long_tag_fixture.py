#!/usr/bin/env python3
"""Makes cabinet-long-tag.novo, cabinet-huge-enclosure.novo and cabinet-small-plate.novo from cabinet-cooling-advice.novo (run from the repo root).

T1 gets a tag of 220 letters after the structure parts, as a pasted or imported name might.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"
OUT = "qa/fixtures/cabinet-long-tag.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
dev = json.loads(c.execute("select data from records where id=43").fetchone()[0])
dev["tag"] = "=A1+CAB1-" + "L" * 220
c.execute("update records set data=? where id=43", (json.dumps(dev),))
c.commit()
c.execute("vacuum")
c.close()


def structure(out, change, base=BASE):
    shutil.copyfile(base, out)
    c = sqlite3.connect(out)
    row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
    change(row)
    c.execute("update records set data=? where id=1", (json.dumps(row),))
    c.commit()
    c.execute("vacuum")
    c.close()


def huge(row):
    row["properties"].update(
        {"interior_width_mm": 20001.0, "interior_height_mm": 25000.0, "interior_depth_mm": 300.0}
    )


def small_plate(row):
    row["plate"] = {"width": 450.0, "height": 450.0, "margin": 10.0}


# The 20 m heat limit (CH-02) and a plate too small for the unplaced devices of heat-place-all.novo (CH-04).
structure("qa/fixtures/cabinet-huge-enclosure.novo", huge)
structure("qa/fixtures/cabinet-small-plate.novo", small_plate, "qa/fixtures/heat-place-all.novo")
