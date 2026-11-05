@echo off
rem run inside an OpenFOAM shell/environment; see research/openfoam/REPORT.md
blockMesh > log.blockMesh 2>&1
checkMesh > log.checkMesh 2>&1
buoyantBoussinesqSimpleFoam > log.solver 2>&1
