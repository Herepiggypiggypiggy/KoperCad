#!/usr/bin/env python3
"""Makes emc-cabinet-door.novo from emc-cabinet.novo (run from the repo root).

The enclosure gets an interior, a left-hinged door and `emc.door_bonded = false` (stated as not
bonded). The analog transmitter -B1 moves to the inside of the door (surface `door`), so the analog
wire -A1 to -B1 crosses the hinge unshielded: EMC-12 asks for the strap (Preview/Apply) and gives
guidance for the wire.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/emc-cabinet.novo"
OUT = "qa/fixtures/emc-cabinet-door.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)


def edit(rid, change):
    (data,) = c.execute("select data from records where id=?", (rid,)).fetchone()
    d = json.loads(data)
    change(d)
    c.execute("update records set data=? where id=?", (json.dumps(d), rid))


def structure(d):
    d["properties"].update({
        "interior_width_mm": 850.0, "interior_height_mm": 450.0, "interior_depth_mm": 250.0,
        "plate_setback_mm": 20.0, "door_hinge": "left", "emc.door_bonded": False,
    })


def b1(d):
    d["x"], d["y"] = 100.0, 100.0
    d["properties"]["surface"] = "door"


edit(2, structure)
edit(14, b1)
c.commit()
c.execute("vacuum")
c.close()
