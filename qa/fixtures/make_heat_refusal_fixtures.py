#!/usr/bin/env python3
"""Makes cabinet-heat-absurd-loss.novo and cabinet-heat-tiny-enclosure.novo (run from the repo root).

Both start from cabinet-cooling-advice.novo. The first gives T1 a loss of 1e12 W, the second shrinks
the enclosure to 110 x 110 x 100 mm with a 100 mm plate so the devices lie outside it.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"


def edit(out, change):
    shutil.copyfile(BASE, out)
    c = sqlite3.connect(out)
    change(c)
    c.commit()
    c.execute("vacuum")
    c.close()


def absurd(c):
    dev = json.loads(c.execute("select data from records where id=43").fetchone()[0])
    dev["properties"]["power_loss_w"] = 1e12
    c.execute("update records set data=? where id=43", (json.dumps(dev),))


def tiny(c):
    row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
    row["plate"] = {"width": 100.0, "height": 100.0, "margin": 5.0}
    row["properties"].update(
        {"interior_width_mm": 110.0, "interior_height_mm": 110.0, "interior_depth_mm": 100.0}
    )
    c.execute("update records set data=? where id=1", (json.dumps(row),))


edit("qa/fixtures/cabinet-heat-absurd-loss.novo", absurd)
edit("qa/fixtures/cabinet-heat-tiny-enclosure.novo", tiny)
