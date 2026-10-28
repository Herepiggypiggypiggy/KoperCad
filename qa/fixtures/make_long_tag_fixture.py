#!/usr/bin/env python3
"""Makes cabinet-long-tag.novo from cabinet-cooling-advice.novo (run from the repo root).

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
