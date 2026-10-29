#!/usr/bin/env python3
"""Makes coil-open-return.novo from the golden motor starter (run from the repo root).

The control circuit with the wire from coil K1's second pin to the return feed removed, so the coil can
never pull in: Live's Why? on the coil has to say the return is open (LSC-03).
"""
import json
import shutil
import sqlite3

BASE = "crates/novo-tests/tests/golden/v0.1-motor-starter.novo"
OUT = "qa/fixtures/coil-open-return.novo"
COIL = 57

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
gone = 0
for rid, data in c.execute("select id, data from records where kind='wire'").fetchall():
    d = json.loads(data)
    ends = (d["from"], d["to"])
    if any(e.get("symbol") == COIL and e.get("pin") == 1 for e in ends):
        c.execute("delete from records where id=?", (rid,))
        gone += 1
assert gone == 1, gone
c.commit()
c.execute("vacuum")
c.close()
