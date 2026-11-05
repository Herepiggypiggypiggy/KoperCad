"""Writes test-crane-parts.stp: a synthetic crane of plain boxes with named solids (hook block, hook,
drum, gearbox, motor, girder). Made from the 10 x 20 x 30 mm block of the interop test pack.
Run from the repository root: python3 qa/fixtures/crane-model/make_fixture.py"""
import re

SRC = "crates/novo-interop/tests/data/step-pack/block-plain.stp"
lines = open(SRC).read().split("\n")
head = [l for l in lines[:5]]
body = [l for l in lines if l.startswith("#") and not l.startswith(("#90=", "#9="))]
# (name, size x y z, lower corner x y z), millimetres, Z up
PARTS = [
    ("Hook block", (600, 300, 500), (-300, -150, 2500)),
    ("Hook", (180, 120, 800), (-90, -60, 1700)),
    ("Drum", (1600, 520, 520), (-800, -260, 4500)),
    ("Gearbox", (400, 400, 400), (800, -200, 4560)),
    ("Hoist motor", (300, 300, 560), (1200, -150, 4580)),
    ("Bridge girder", (9000, 500, 900), (-4500, -250, 5200)),
]
out = head[:]
out.append("#90=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));")
out.append("#9=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#90))REPRESENTATION_CONTEXT('',''));")
num = re.compile(r"#(\d+)")
pt = re.compile(r"CARTESIAN_POINT\(''\,\(([^)]*)\)\)")
for k, (name, size, lo) in enumerate(PARTS):
    off = (k + 1) * 1000
    for l in body:
        def renum(m):
            n = int(m.group(1))
            return m.group(0) if n == 9 else "#%d" % (n + off)
        l2 = num.sub(renum, l)
        def mv(m):
            v = [float(x) for x in m.group(1).split(",")]
            w = [v[0] / 10 * size[0] + lo[0], v[1] / 20 * size[1] + lo[1], v[2] / 30 * size[2] + lo[2]]
            return "CARTESIAN_POINT('',(%s))" % ",".join("%.3f" % x for x in w)
        l2 = pt.sub(mv, l2)
        l2 = l2.replace("MANIFOLD_SOLID_BREP('',", "MANIFOLD_SOLID_BREP('%s'," % name)
        out.append(l2)
out += ["ENDSEC;", "END-ISO-10303-21;", ""]
open("qa/fixtures/crane-model/test-crane-parts.stp", "w").write("\n".join(out))
