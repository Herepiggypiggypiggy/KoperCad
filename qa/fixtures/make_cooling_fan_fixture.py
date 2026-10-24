#!/usr/bin/env python3
"""Makes cabinet-cooling-fans.novo from cabinet-cooling-advice.novo (run from the repo root).

The hot cabinet plus three fan parts in the project catalog (QA stand-ins, not real articles), two of
them with the same free-air flow, so the Cooling advice window names the article behind each fan.
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/cabinet-cooling-advice.novo"
OUT = "qa/fixtures/cabinet-cooling-fans.novo"
PARTS = [
    ("QA-FF-150", "QA filter fan 150", 150.0, "Filter fan"),
    ("QA-FF-300", "QA filter fan 300", 300.0, "Filter fan"),
    ("QA-FN-300", "QA axial fan 300", 300.0, "Fan"),
    ("QA-FF-500", "QA filter fan 500", 500.0, "Filter fan"),
]

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
next_id = int(c.execute("select value from meta where key='next_id'").fetchone()[0])
for order, desc, flow, kind in PARTS:
    rec = {"record": "part", "id": next_id, "manufacturer": "QA", "order_number": order,
           "type_number": order, "description": desc,
           "properties": {"airflow_m3h": flow, "cooling_kind": kind}}
    c.execute("insert into records(id, kind, page, data) values (?, 'part', null, ?)",
              (next_id, json.dumps(rec)))
    next_id += 1
c.execute("update meta set value=? where key='next_id'", (str(next_id),))
c.commit()
c.execute("vacuum")
c.close()
