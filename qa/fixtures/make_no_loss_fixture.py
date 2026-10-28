#!/usr/bin/env python3
"""Makes cabinet-no-loss-data.novo from cabinet-cooling-advice.novo (run from the repo root).

The same cabinet with no device carrying a power loss, as a project whose parts were never
assigned: the Heat view has no data to colour (it used to show a 35 to 50 degree gradient and
hot-spot cards built from planning figures).
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"
OUT = "qa/fixtures/cabinet-no-loss-data.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
removed = 0
for rid, data in c.execute("select id, data from records").fetchall():
    row = json.loads(data)
    props = row.get("properties") if isinstance(row, dict) else None
    if isinstance(props, dict) and "power_loss_w" in props:
        del props["power_loss_w"]
        c.execute("update records set data=? where id=?", (json.dumps(row), rid))
        removed += 1
c.commit()
c.execute("vacuum")
c.close()
print(f"removed power_loss_w from {removed} records")
