#!/usr/bin/env python3
"""Writes a small synthetic overhead crane as a STEP file (boxes and cylinders, each its own
named solid, millimetres, z up). Used by the QA scenarios of the crane model picker; no customer
data. Usage: tools/make_crane_step.py OUT.stp"""
import sys

out = ["ISO-10303-21;", "HEADER;", "ENDSEC;", "DATA;",
       "#1=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
       "#2=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#1))REPRESENTATION_CONTEXT('',''));"]
n = [100]
solids = []

def nid():
    n[0] += 1
    return n[0]

def f(v):
    return "(" + ",".join("%.6f" % c for c in v) + ")"

def placement(o, z, x):
    p, dz, dx = nid(), nid(), nid()
    out.append("#%d=CARTESIAN_POINT('',%s);" % (p, f(o)))
    out.append("#%d=DIRECTION('',%s);" % (dz, f(z)))
    out.append("#%d=DIRECTION('',%s);" % (dx, f(x)))
    a = nid()
    out.append("#%d=AXIS2_PLACEMENT_3D('',#%d,#%d,#%d);" % (a, p, dz, dx))
    return a

def finish(name, faces):
    shell, brep = nid(), nid()
    out.append("#%d=CLOSED_SHELL('',(%s));" % (shell, ",".join("#%d" % x for x in faces)))
    out.append("#%d=MANIFOLD_SOLID_BREP('%s',#%d);" % (brep, name, shell))
    solids.append(brep)

def box(name, lo, hi):
    (x0, y0, z0), (x1, y1, z1) = lo, hi
    c = [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
         (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
    pt, vx = [], []
    for p in c:
        a = nid(); out.append("#%d=CARTESIAN_POINT('',%s);" % (a, f(p))); pt.append(a)
        b = nid(); out.append("#%d=VERTEX_POINT('',#%d);" % (b, a)); vx.append(b)
    pairs = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)]
    ec = []
    for a, b in pairs:
        d, v, l, e = nid(), nid(), nid(), nid()
        out.append("#%d=DIRECTION('',(1.,0.,0.));" % d)
        out.append("#%d=VECTOR('',#%d,1.);" % (v, d))
        out.append("#%d=LINE('',#%d,#%d);" % (l, pt[a], v))
        out.append("#%d=EDGE_CURVE('',#%d,#%d,#%d,.T.);" % (e, vx[a], vx[b], l))
        ec.append(e)
    faces_def = [
        ([(3,0),(2,0),(1,0),(0,0)], c[0], (0,0,-1), (1,0,0)),
        ([(4,1),(5,1),(6,1),(7,1)], c[4], (0,0,1), (1,0,0)),
        ([(0,1),(9,1),(4,0),(8,0)], c[0], (0,-1,0), (1,0,0)),
        ([(1,1),(10,1),(5,0),(9,0)], c[1], (1,0,0), (0,1,0)),
        ([(2,1),(11,1),(6,0),(10,0)], c[3], (0,1,0), (-1,0,0)),
        ([(3,1),(8,1),(7,0),(11,0)], c[0], (-1,0,0), (0,-1,0)),
    ]
    faces = []
    for loop, o, z, x in faces_def:
        oes = []
        for e, fwd in loop:
            i = nid(); out.append("#%d=ORIENTED_EDGE('',*,*,#%d,%s);" % (i, ec[e], ".T." if fwd else ".F.")); oes.append(i)
        lp, bd = nid(), nid()
        out.append("#%d=EDGE_LOOP('',(%s));" % (lp, ",".join("#%d" % i for i in oes)))
        out.append("#%d=FACE_OUTER_BOUND('',#%d,.T.);" % (bd, lp))
        pl, s, fc = nid(), nid(), nid()
        out.append("#%d=PLANE('',#%d);" % (s, placement(o, z, x)))
        out.append("#%d=ADVANCED_FACE('',(#%d),#%d,.T.);" % (fc, bd, s))
        faces.append(fc)
    finish(name, faces)

def cyl(name, o, axis, ref, r, h):
    """Cylinder from point o along axis (unit), radius r, length h; ref is a unit vector across the axis."""
    top = tuple(o[i] + axis[i] * h for i in range(3))
    neg = tuple(-a for a in axis)
    p0 = tuple(o[i] + ref[i] * r for i in range(3))
    p1 = tuple(top[i] + ref[i] * r for i in range(3))
    pa, pb = placement(o, axis, ref), placement(top, axis, ref)
    pc = placement(o, neg, ref)
    v = []
    for p in (p0, p1):
        a = nid(); out.append("#%d=CARTESIAN_POINT('',%s);" % (a, f(p)))
        b = nid(); out.append("#%d=VERTEX_POINT('',#%d);" % (b, a)); v.append((a, b))
    c0, c1 = nid(), nid()
    out.append("#%d=CIRCLE('',#%d,%.6f);" % (c0, pa, r))
    out.append("#%d=CIRCLE('',#%d,%.6f);" % (c1, pb, r))
    d, vec, ln = nid(), nid(), nid()
    out.append("#%d=DIRECTION('',%s);" % (d, f(axis)))
    out.append("#%d=VECTOR('',#%d,1.);" % (vec, d))
    out.append("#%d=LINE('',#%d,#%d);" % (ln, v[0][0], vec))
    e0, e1, e2 = nid(), nid(), nid()
    out.append("#%d=EDGE_CURVE('',#%d,#%d,#%d,.T.);" % (e0, v[0][1], v[0][1], c0))
    out.append("#%d=EDGE_CURVE('',#%d,#%d,#%d,.T.);" % (e1, v[1][1], v[1][1], c1))
    out.append("#%d=EDGE_CURVE('',#%d,#%d,#%d,.T.);" % (e2, v[0][1], v[1][1], ln))
    def oe(e, fwd):
        i = nid(); out.append("#%d=ORIENTED_EDGE('',*,*,#%d,%s);" % (i, e, ".T." if fwd else ".F.")); return i
    o0, o1, o2, o3, o4, o5 = oe(e0, 1), oe(e1, 1), oe(e2, 1), oe(e2, 0), oe(e1, 0), oe(e0, 0)
    def loop(ids):
        lp, bd = nid(), nid()
        out.append("#%d=EDGE_LOOP('',(%s));" % (lp, ",".join("#%d" % i for i in ids)))
        out.append("#%d=FACE_OUTER_BOUND('',#%d,.T.);" % (bd, lp))
        return bd
    b0, b1, b2 = loop([o5]), loop([o1]), loop([o0, o2, o4, o3])
    s0, s1, s2 = nid(), nid(), nid()
    out.append("#%d=PLANE('',#%d);" % (s0, pc))
    out.append("#%d=PLANE('',#%d);" % (s1, pb))
    out.append("#%d=CYLINDRICAL_SURFACE('',#%d,%.6f);" % (s2, pa, r))
    faces = []
    for bd, s in ((b0, s0), (b1, s1), (b2, s2)):
        fc = nid(); out.append("#%d=ADVANCED_FACE('',(#%d),#%d,.T.);" % (fc, bd, s)); faces.append(fc)
    finish(name, faces)

X, Y, Z = (1, 0, 0), (0, 1, 0), (0, 0, 1)
# Runway: two rails on supports, 12 m long, 8 m apart (y), rail head at z = 7060.
for tag, y in (("A", 0), ("B", 8000)):
    box("Runway support " + tag, (0, y - 150, 6700), (12000, y + 150, 7000))
    box("Crane rail " + tag, (0, y - 30, 7000), (12000, y + 30, 7060))
# Bridge: an end truck over each rail with two wheels, the girder across.
for tag, y in (("A", 0), ("B", 8000)):
    box("End truck " + tag, (1700, y - 200, 7220), (2800, y + 200, 7400))
    for k, x in enumerate((1900, 2600), 1):
        cyl("Wheel %s%d" % (tag, k), (x, y - 40, 7220), Y, Z, 160, 80)
box("Main girder", (2100, -200, 7400), (2400, 8200, 8000))
# Trolley on the girder: rails, frame, four wheels, drum with motor and gearbox.
for k, x in enumerate((2150, 2350), 1):
    box("Trolley track %d" % k, (x - 15, 200, 8000), (x + 15, 7800, 8040))
box("Trolley frame", (1950, 3400, 8130), (2550, 4600, 8200))
for k, (x, y) in enumerate(((2150, 3500), (2150, 4500), (2350, 3500), (2350, 4500)), 1):
    cyl("Trolley wheel %d" % k, (x - 30, y, 8040 + 45), X, Y, 45, 60)
cyl("Rope drum", (2250, 3600, 8300), Y, X, 220, 800)
box("Hoist motor", (2050, 4500, 8200), (2450, 4900, 8500))
box("Hoist gearbox", (2100, 4400, 8200), (2400, 4500, 8450))
# The hook hangs under the drum.
box("Hook block", (2100, 3900, 6700), (2400, 4100, 7100))
box("Hook", (2200, 3950, 6300), (2300, 4050, 6700))

out.append("#%d=ADVANCED_BREP_SHAPE_REPRESENTATION('',(%s),#2);" % (nid(), ",".join("#%d" % s for s in solids)))
out += ["ENDSEC;", "END-ISO-10303-21;"]
open(sys.argv[1], "w").write("\n".join(out) + "\n")
