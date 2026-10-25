#!/usr/bin/env python3
"""Makes emc-cabinet-two-ducts.novo from emc-cabinet.novo (run from the repo root).

The motor wire and the analog wire share the duct at y=300 (EMC-01). This adds a second 60 x 60 duct
at y=100 below the devices, so either wire can reach another duct and the "Route the wire in another
duct" fix has somewhere to go.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/emc-cabinet.novo"
OUT = "qa/fixtures/emc-cabinet-two-ducts.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
rail = {"record": "rail", "id": 18, "structure": 2, "x": 20.0, "y": 100.0, "length": 760.0,
        "kind": "duct", "width": 60.0, "height": 60.0, "properties": {}}
c.execute("insert into records (id, kind, page, data) values (18, 'rail', NULL, ?)", (json.dumps(rail),))
c.execute("update meta set value='19' where key='next_id'")
c.commit()
c.execute("vacuum")
c.close()
