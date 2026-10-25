#!/usr/bin/env python3
"""Makes emc-cabinet-keep-clear.novo from emc-cabinet-two-ducts.novo (run from the repo root).

The ducts at y=300 and y=100 are 140 mm apart edge to edge. The planning distance for a motor wire
beside an analog wire is 300 mm, which no pair of ducts here can give, so this project states 100 mm
for that pair (project property emc.distance.motor.analog_mm): the "Route the two wires in ducts at
least 100 mm apart" fix can then be met. emc-cabinet-two-ducts.novo (300 mm) shows the refusal.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/emc-cabinet-two-ducts.novo"
OUT = "qa/fixtures/emc-cabinet-keep-clear.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
(info,) = c.execute("select value from meta where key='info'").fetchone()
info = json.loads(info)
info["properties"]["emc.distance.motor.analog_mm"] = 100.0
info["properties"]["emc.distance.analog.motor_mm"] = 100.0
c.execute("update meta set value=? where key='info'", (json.dumps(info),))
c.commit()
c.execute("vacuum")
c.close()
