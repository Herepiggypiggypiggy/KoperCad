# Release and builds: roadmap
Scope: experimental build, CI load, Windows installer, launcher and desktop shortcut, local build. No admin rights ever.
Done: experimental pipeline, main Windows CI, KoperCAD rename of names, installer, data-folder migration, kopercad.exe.
Next: build on Tadjiro's request only; hands-on Windows checks; retire the novoelectric.exe copy; tagged release flow (v* -> installer) when Gate 1 lands.
Done since: latest-build branch for laptops, Python module rename.
Open questions: whether #511 merged; STEP-button release hold (#506/#508); drop the novoelectric.exe compat copy once launchers are refreshed.
Done 2026-10-19: v0.1.0-rc1 draft release, Tag a release workflow (#1033), Windows CI green. Next: publish only when Tadjiro says so; guard step in tag-release.yml; Windows novo-cfd smoke; drop novoelectric.exe copy.
