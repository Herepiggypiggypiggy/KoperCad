#!/usr/bin/env python3
"""Developer-side OpenFOAM case writer for the KoperCAD cross-check.

Reads cases.json (written by crates/novo-cfd/tests/openfoam_reference.rs) and
writes one OpenFOAM case folder per case and mesh level, from scratch (no
OpenFOAM tutorial or source text is copied). Separate-process use only:
OpenFOAM is GPL v3, KoperCAD is MIT; nothing of OpenFOAM is linked or shipped.

Target solver: ESI OpenFOAM v2606 buoyantBoussinesqSimpleFoam (laminar,
constant properties, same numbers as the KoperCAD cabinet solver). Keywords
must be checked against the installed version; see research/openfoam/.

    python make_cases.py cases.json out_dir [--n 2 3 4] [--solver NAME]

Frame: OpenFOAM x = KoperCAD x (width), y = UP (KoperCAD y runs down from the
top), z = KoperCAD z (rear plate to door). Metres, kelvin.
Mesh: a block per 50 mm cube (all geometry sits on the 50 mm lattice), n cells
per block edge, so device boxes are cut out exactly and no cut-cell logic is
needed. Faces towards a device go to patch devN (uniform heat flux), faces
in a vent footprint to ventN, the rest to the six wall patches.
"""
import json
import os
import sys

LATTICE = 50.0
HDR = "FoamFile\n{\n    version 2.0;\n    format ascii;\n    class %s;\n    object %s;\n}\n\n"
KELVIN = 273.15


def hdr(cls, obj, loc=None):
    s = HDR % (cls, obj)
    if loc:
        s = s.replace("    object", '    location "%s";\n    object' % loc)
    return s


def on_lattice(*vals):
    for v in vals:
        r = v / LATTICE
        assert abs(r - round(r)) < 1e-9, "value %s is not a multiple of %s mm" % (v, LATTICE)


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", newline="\n") as f:
        f.write(text)


def build(case, glob, n, outdir, solver):
    W, H, D = case["domain_mm"]
    on_lattice(W, H, D)
    nx, ny, nz = int(W / LATTICE), int(H / LATTICE), int(D / LATTICE)
    delta = LATTICE / n  # mm
    # devices as lattice index boxes (OpenFOAM y up)
    devs = []
    for d in case["devices"]:
        on_lattice(d["x"], d["y"], d["w"], d["h"], d["z0"], d["depth"])
        y_lo = H - (d["y"] + d["h"])
        devs.append(
            dict(
                lo=(round(d["x"] / LATTICE), round(y_lo / LATTICE), round(d["z0"] / LATTICE)),
                hi=(round((d["x"] + d["w"]) / LATTICE), round((y_lo + d["h"]) / LATTICE), round((d["z0"] + d["depth"]) / LATTICE)),
                loss=d["loss_w"],
            )
        )

    def dev_at(i, j, k):
        for n_, d in enumerate(devs):
            if all(d["lo"][a] <= (i, j, k)[a] < d["hi"][a] for a in range(3)):
                return n_
        return None

    # vents: face, x span, z span (centred in depth) in lattice indices
    vents = []
    for v in case["vents"]:
        on_lattice(v["x"] - v["width"] / 2, v["x"] + v["width"] / 2)
        z0 = D / 2 - v["depth"] / 2
        on_lattice(z0, z0 + v["depth"])
        vents.append(
            dict(
                top=v["face"] == "top",
                xs=(round((v["x"] - v["width"] / 2) / LATTICE), round((v["x"] + v["width"] / 2) / LATTICE)),
                zs=(round(z0 / LATTICE), round((z0 + v["depth"]) / LATTICE)),
            )
        )

    def vid(i, j, k):
        return (k * (ny + 1) + j) * (nx + 1) + i

    verts = []
    for k in range(nz + 1):
        for j in range(ny + 1):
            for i in range(nx + 1):
                verts.append("(%g %g %g)" % (i * LATTICE * 1e-3, j * LATTICE * 1e-3, k * LATTICE * 1e-3))

    def hexv(i, j, k):
        return [vid(i, j, k), vid(i + 1, j, k), vid(i + 1, j + 1, k), vid(i, j + 1, k),
                vid(i, j, k + 1), vid(i + 1, j, k + 1), vid(i + 1, j + 1, k + 1), vid(i, j + 1, k + 1)]

    face_idx = {  # axis, side -> hex vertex positions (outward normal)
        (0, 0): (0, 4, 7, 3), (0, 1): (1, 2, 6, 5),
        (1, 0): (0, 1, 5, 4), (1, 1): (3, 7, 6, 2),
        (2, 0): (0, 3, 2, 1), (2, 1): (4, 5, 6, 7),
    }
    wall_names = {(0, 0): "xmin", (0, 1): "xmax", (1, 0): "ymin", (1, 1): "ymax", (2, 0): "zmin", (2, 1): "zmax"}
    patches = {name: [] for name in wall_names.values()}
    for i in range(len(vents)):
        patches["vent%d" % i] = []
    for i in range(len(devs)):
        patches["dev%d" % i] = []
    blocks = []
    dims = (nx, ny, nz)
    for k in range(nz):
        for j in range(ny):
            for i in range(nx):
                if dev_at(i, j, k) is not None:
                    continue
                hv = hexv(i, j, k)
                blocks.append("    hex (%s) (%d %d %d) simpleGrading (1 1 1)" % (" ".join(map(str, hv)), n, n, n))
                ijk = (i, j, k)
                for (a, s), pos in face_idx.items():
                    nb = list(ijk)
                    nb[a] += 1 if s else -1
                    f = "(%s)" % " ".join(str(hv[p]) for p in pos)
                    if not (0 <= nb[a] < dims[a]):
                        name = wall_names[(a, s)]
                        if a == 1:
                            for vi, v in enumerate(vents):
                                if (v["top"] == bool(s)) and v["xs"][0] <= i < v["xs"][1] and v["zs"][0] <= k < v["zs"][1]:
                                    name = "vent%d" % vi
                        patches[name].append(f)
                    else:
                        dn = dev_at(*nb)
                        if dn is not None:
                            patches["dev%d" % dn].append(f)
    # device exposed area, m2
    face_area = (LATTICE * 1e-3) ** 2
    dev_area = [len(patches["dev%d" % i]) * face_area for i in range(len(devs))]

    # ---------- blockMeshDict
    bm = hdr("dictionary", "blockMeshDict") + "convertToMeters 1;\n\nvertices\n(\n" + "\n".join("    " + v for v in verts) + "\n);\n\nblocks\n(\n" + "\n".join(blocks) + "\n);\n\nedges ();\n\nboundary\n(\n"
    for name, faces in patches.items():
        if not faces:
            continue
        typ = "wall" if name in wall_names.values() or name.startswith("dev") else "patch"
        bm += "    %s\n    {\n        type %s;\n        faces\n        (\n%s\n        );\n    }\n" % (name, typ, "\n".join("            " + f for f in faces))
    bm += ");\n\nmergePatchPairs ();\n"
    write(os.path.join(outdir, "system/blockMeshDict"), bm)

    # ---------- topoSet for vent layers
    act = ""
    for vi, v in enumerate(vents):
        y0, y1 = (H - delta, H) if v["top"] else (0.0, delta)
        act += ("    {\n        name vent%d;\n        type cellSet;\n        action new;\n        source boxToCell;\n"
                "        box (%g %g %g) (%g %g %g);\n    }\n") % (
            vi, v["xs"][0] * LATTICE * 1e-3, y0 * 1e-3, v["zs"][0] * LATTICE * 1e-3,
            v["xs"][1] * LATTICE * 1e-3, y1 * 1e-3, v["zs"][1] * LATTICE * 1e-3)
    write(os.path.join(outdir, "system/topoSetDict"), hdr("dictionary", "topoSetDict") + "actions\n(\n" + act + ");\n")

    # ---------- constant
    t_k = glob["ambient_c"] + KELVIN
    pr = glob["nu"] / glob["alpha"]
    write(os.path.join(outdir, "constant/g"), hdr("uniformDimensionedVectorField", "g", "constant") + "dimensions [0 1 -2 0 0 0 0];\nvalue (0 -9.81 0);\n")
    write(os.path.join(outdir, "constant/transportProperties"), hdr("dictionary", "transportProperties", "constant") +
          "transportModel Newtonian;\nnu [0 2 -1 0 0 0 0] %g;\nbeta [0 0 0 -1 0 0 0] %g;\nTRef [0 0 0 1 0 0 0] %g;\nPr [0 0 0 0 0 0 0] %g;\nPrt [0 0 0 0 0 0 0] 0.85;\n" % (glob["nu"], glob["beta"], t_k, pr))
    turb = hdr("dictionary", "turbulenceProperties", "constant") + "simulationType laminar;\n"
    write(os.path.join(outdir, "constant/turbulenceProperties"), turb)
    write(os.path.join(outdir, "constant/momentumTransport"), hdr("dictionary", "momentumTransport", "constant") + "simulationType laminar;\n")

    # ---------- fvOptions (vent orifice loss as a Darcy-Forchheimer layer)
    cd, phi = glob["discharge_coefficient"], glob["vent_free_area"]
    kloss = 1.0 / (cd * phi) ** 2
    fo = hdr("dictionary", "fvOptions", "system")
    for vi, v in enumerate(vents):
        fo += ("vent%d\n{\n    type explicitPorositySource;\n    active yes;\n    explicitPorositySourceCoeffs\n    {\n"
               "        selectionMode cellSet;\n        cellSet vent%d;\n        type DarcyForchheimer;\n"
               "        DarcyForchheimerCoeffs\n        {\n            d (0 0 0);\n            f (0 %g 0);\n"
               "            coordinateSystem\n            {\n                type cartesian;\n                origin (0 0 0);\n"
               "                coordinateRotation { type axesRotation; e1 (1 0 0); e2 (0 1 0); }\n            }\n        }\n    }\n}\n") % (
            vi, vi, kloss / (delta * 1e-3))
    write(os.path.join(outdir, "system/fvOptions"), fo)

    # ---------- fields
    kappa = glob["conductivity"]
    wall = case["wall"]
    if wall["kind"] == "fixed":
        wallT = "type fixedValue; value uniform %g;" % (wall["t_c"] + KELVIN)
    else:
        h = wall["h"]
        f = h / (h + kappa * 2.0 / (delta * 1e-3))  # half cell conduction in series with the film
        wallT = "type mixed; refValue uniform %g; refGradient uniform 0; valueFraction uniform %g; value uniform %g;" % (wall["t_out_c"] + KELVIN, f, t_k)
    t_f = hdr("volScalarField", "T", "0") + "dimensions [0 0 0 1 0 0 0];\ninternalField uniform %g;\nboundaryField\n{\n" % t_k
    u_f = hdr("volVectorField", "U", "0") + "dimensions [0 1 -1 0 0 0 0];\ninternalField uniform (0 0 0);\nboundaryField\n{\n"
    p_f = hdr("volScalarField", "p_rgh", "0") + "dimensions [0 2 -2 0 0 0 0];\ninternalField uniform 0;\nboundaryField\n{\n"
    pp_f = hdr("volScalarField", "p", "0") + "dimensions [0 2 -2 0 0 0 0];\ninternalField uniform 0;\nboundaryField\n{\n"
    a_f = hdr("volScalarField", "alphat", "0") + "dimensions [0 2 -1 0 0 0 0];\ninternalField uniform 0;\nboundaryField\n{\n"
    for name, faces in patches.items():
        if not faces:
            continue
        if name.startswith("vent"):
            tt = "type inletOutlet; inletValue uniform %g; value uniform %g;" % (t_k, t_k)
            uu = "type pressureInletOutletVelocity; value uniform (0 0 0);"
            pr_ = "type fixedValue; value uniform 0;"
            ppp = "type calculated; value uniform 0;"
        elif name.startswith("dev"):
            q = devs[int(name[3:])]["loss"] / dev_area[int(name[3:])]
            tt = "type fixedGradient; gradient uniform %g;" % (q / kappa)
            uu = "type noSlip;"
            pr_ = "type fixedFluxPressure; value uniform 0;"
            ppp = "type calculated; value uniform 0;"
        else:
            tt = wallT
            uu = "type noSlip;"
            pr_ = "type fixedFluxPressure; value uniform 0;"
            ppp = "type calculated; value uniform 0;"
        t_f += "    %s { %s }\n" % (name, tt)
        u_f += "    %s { %s }\n" % (name, uu)
        p_f += "    %s { %s }\n" % (name, pr_)
        pp_f += "    %s { %s }\n" % (name, ppp)
        a_f += "    %s { type calculated; value uniform 0; }\n" % name
    for fname, txt in (("T", t_f), ("U", u_f), ("p_rgh", p_f), ("p", pp_f), ("alphat", a_f)):
        write(os.path.join(outdir, "0", fname), txt + "}\n")

    # ---------- system
    transient = solver.endswith("PimpleFoam")
    end = 4000 if not transient else 3600
    funcs = "functions\n{\n"
    funcs += ("    meanT { type volFieldValue; libs (fieldFunctionObjects); operation volAverage; regionType all; fields (T);"
              " writeFields no; writeControl timeStep; writeInterval 50; }\n")
    funcs += ("    maxT { type volFieldValue; libs (fieldFunctionObjects); operation max; regionType all; fields (T);"
              " writeFields no; writeControl timeStep; writeInterval 50; }\n")
    px = [tuple(c * 1e-3 for c in (p[0], H - p[1], p[2])) for p in case["probes_mm"]]
    funcs += "    probes { type probes; libs (sampling); fields (T); probeLocations (%s); writeControl timeStep; writeInterval 50; }\n" % " ".join("(%g %g %g)" % c for c in px)
    for vi in range(len(vents)):
        funcs += ("    ventFlow%d { type surfaceFieldValue; libs (fieldFunctionObjects); regionType patch; name vent%d; operation sum; fields (phi);"
                  " writeFields no; writeControl timeStep; writeInterval 50; }\n" % (vi, vi))
        funcs += ("    ventT%d { type surfaceFieldValue; libs (fieldFunctionObjects); regionType patch; name vent%d; operation weightedAverage;"
                  " weightField phi; fields (T); writeFields no; writeControl timeStep; writeInterval 50; }\n" % (vi, vi))
    if wall["kind"] == "film":
        for wn in wall_names.values():
            funcs += ("    wallT_%s { type surfaceFieldValue; libs (fieldFunctionObjects); regionType patch; name %s; operation areaAverage; fields (T);"
                      " writeFields no; writeControl timeStep; writeInterval 50; }\n" % (wn, wn))
    funcs += "}\n"
    cd_txt = hdr("dictionary", "controlDict", "system") + (
        "application %s;\nstartFrom startTime;\nstartTime 0;\nstopAt endTime;\nendTime %d;\ndeltaT %s;\n"
        "writeControl timeStep;\nwriteInterval %d;\npurgeWrite 2;\nwriteFormat ascii;\nwriteCompression off;\ntimeFormat general;\n"
        "timePrecision 6;\nrunTimeModifiable true;\n\n") % (solver, end, "1" if not transient else "0.5", end) + funcs
    write(os.path.join(outdir, "system/controlDict"), cd_txt)
    write(os.path.join(outdir, "system/fvSchemes"), hdr("dictionary", "fvSchemes", "system") + """ddtSchemes { default steadyState; }
gradSchemes { default Gauss linear; }
divSchemes
{
    default none;
    div(phi,U) bounded Gauss linearUpwind grad(U);
    div(phi,T) bounded Gauss limitedLinear 1;
    div((nuEff*dev2(T(grad(U))))) Gauss linear;
    div(((nuEff)*dev2(T(grad(U))))) Gauss linear;
}
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
wallDist { method meshWave; }
""" if not transient else hdr("dictionary", "fvSchemes", "system") + """ddtSchemes { default Euler; }
gradSchemes { default Gauss linear; }
divSchemes
{
    default none;
    div(phi,U) Gauss linearUpwind grad(U);
    div(phi,T) Gauss limitedLinear 1;
    div((nuEff*dev2(T(grad(U))))) Gauss linear;
    div(((nuEff)*dev2(T(grad(U))))) Gauss linear;
}
laplacianSchemes { default Gauss linear corrected; }
interpolationSchemes { default linear; }
snGradSchemes { default corrected; }
wallDist { method meshWave; }
""")
    write(os.path.join(outdir, "system/fvSolution"), hdr("dictionary", "fvSolution", "system") + """solvers
{
    p_rgh { solver GAMG; smoother GaussSeidel; tolerance 1e-8; relTol 0.01; }
    "(U|T)" { solver smoothSolver; smoother symGaussSeidel; tolerance 1e-9; relTol 0.1; }
    "(U|T)Final" { $U; relTol 0; }
    p_rghFinal { $p_rgh; relTol 0; }
}
SIMPLE
{
    momentumPredictor yes;
    nNonOrthogonalCorrectors 0;
    pRefCell 0;
    pRefValue 0;
    residualControl { p_rgh 1e-5; U 1e-6; T 1e-7; }
}
PIMPLE
{
    momentumPredictor yes;
    nOuterCorrectors 1;
    nCorrectors 2;
    nNonOrthogonalCorrectors 0;
    pRefCell 0;
    pRefValue 0;
}
relaxationFactors
{
    equations { U 0.7; T 0.7; }
    fields { p_rgh 0.3; }
}
""")
    meta = dict(case=case["name"], n_per_50mm=n, cell_mm=delta, solver=solver, blocks=len(blocks), dev_area_m2=dev_area, vents=len(vents), devices=len(devs))
    write(os.path.join(outdir, "case-info.json"), json.dumps(meta, indent=1))
    steps = ["blockMesh > log.blockMesh 2>&1"]
    if vents:
        steps.append("topoSet > log.topoSet 2>&1")
    steps.append("checkMesh > log.checkMesh 2>&1")
    steps.append("%s > log.solver 2>&1" % solver)
    write(os.path.join(outdir, "run.cmd"), "@echo off\r\nrem run inside an OpenFOAM shell/environment; see research/openfoam/REPORT.md\r\n" + "\r\n".join(steps) + "\r\n")
    write(os.path.join(outdir, "run.sh"), "#!/bin/sh\n" + "\n".join(steps) + "\n")
    return meta


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        print(__doc__)
        sys.exit(2)
    spec = json.load(open(args[0]))
    out = args[1]
    ns = [2, 3, 4]
    solver = "buoyantBoussinesqSimpleFoam"
    only = None
    if "--n" in args:
        i = args.index("--n") + 1
        ns = []
        while i < len(args) and not args[i].startswith("--"):
            ns.append(int(args[i]))
            i += 1
    if "--solver" in args:
        solver = args[args.index("--solver") + 1]
    if "--only" in args:
        only = args[args.index("--only") + 1].split(",")
    for case in spec["cases"]:
        if only and case["name"] not in only:
            continue
        for n in ns:
            meta = build(case, spec, n, os.path.join(out, "%s_n%d" % (case["name"], n)), solver)
            print(case["name"], n, meta["blocks"], "blocks")


if __name__ == "__main__":
    main()
