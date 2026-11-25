# Crane export v68b roadmap
Scope: bring a private crane configurator export (version folder v68b, 192 pages) into KoperCAD through the existing crane importer (File > Import > Crane configurator project, `kopercad-cli import crane`). No new importer was needed.
Done: importer crash on mixed core numbers fixed; names with `/` (-K12/3) now import as -K12_3 instead of being dropped.
Next: 52 cores whose cable is missing; 226 symbols drawn as boxes (Black box, PLC box, PLC connection points); free texts check; Windows run of the opt-in real test.
Open: should the original EPLAN name be kept as a property so reports can show -K12/3 again?
