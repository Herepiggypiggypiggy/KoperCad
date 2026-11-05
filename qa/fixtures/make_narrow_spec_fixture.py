#!/usr/bin/env python3
"""Makes cabinet-spec-narrow.novo from cabinet-spec-a.novo (run from the repo root): the same cabinet with a
synthetic catalogue enclosure 400 mm wide (article SYN-narrow), so the 450 mm plate does not fit inside it.
No maker data: the article number is invented."""
import json
import shutil
import sqlite3

shutil.copyfile("qa/fixtures/cabinet-spec-a.novo", "qa/fixtures/cabinet-spec-narrow.novo")
c = sqlite3.connect("qa/fixtures/cabinet-spec-narrow.novo")
row = json.loads(c.execute("select data from records where id=1").fetchone()[0])
row["properties"]["encl.article"] = "SYN-narrow"
row["properties"]["encl.width_mm"] = 400.0
c.execute("update records set data=? where id=1", (json.dumps(row),))
c.commit()
c.execute("vacuum")
c.close()
