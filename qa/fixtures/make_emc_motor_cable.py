#!/usr/bin/env python3
"""Makes emc-cabinet-motor-cable.novo from emc-cabinet.novo (run from the repo root).

The motor wire of the drive -T1 gets a shielded cable of 80 m, screen all round at both ends and no
maker capacitance stated, so EMC-21 works out the capacitance from the coaxial formula and says the
insulation is assumed. 80 m x 211.7 pF/m = 16.9 nF, 1.2 mA at 230 V and 50 Hz (Info).
"""
import json
import shutil
import sqlite3

BASE = "qa/fixtures/emc-cabinet.novo"
OUT = "qa/fixtures/emc-cabinet-motor-cable.novo"

shutil.copyfile(BASE, OUT)
c = sqlite3.connect(OUT)
(new_id,) = c.execute("select max(id)+1 from records").fetchone()
cable = {
    "record": "cable", "id": new_id, "tag": "=A1+CAB-W1", "description": "", "part": None,
    "cores": [
        {"name": "1", "cross_section_mm2": None, "shield": False, "wire": 16},
        {"name": "SH", "cross_section_mm2": None, "shield": True, "wire": None},
    ],
    "length_m": 80.0,
    "properties": {"emc.shield_a": "360", "emc.shield_b": "360"},
}
c.execute("insert into records(id, kind, page, data) values (?, 'cable', NULL, ?)", (new_id, json.dumps(cable)))
c.commit()
c.close()
