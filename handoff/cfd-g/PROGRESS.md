2026-10-16 08:50 UTC first validation PR: conduction + cavity study tests and report
2026-10-16 08:55 UTC #585 merged. Follow-up: novo_cfd::accuracy::flow_core_statement() + test; stub.rs still has its placeholder statement (CFD-A to switch if wanted).
2026-10-16 11:10 UTC #585 and #598 merged. Plate test (Churchill-Chu, open boundaries from C2) added: +7.8% -> +3.9%.
2026-10-16 12:25 UTC #675 merged; #668 superseded by docs-only PR: enclosure numbers, CFD-B turbulence limits, limit lines in accuracy.rs.
2026-10-17 10:00 UTC Report PR: vented-box, device and full-cabinet rows, sealed numbers re-measured, solid-mask caveat updated (#679 merged), known-difference section, accuracy limit line. Owed next: chimney check, cabinet-with-vents validation.
2026-10-17 10:30 UTC #945 merged. Chimney sweep test (validation_chimney.rs): -14.5% coarse, -10.8% fine vs ideal stack law, scaling exact; R1 wording fix (20% here, up to 50% in CFD-B) included. Owed: cabinet-with-vents.
2026-10-17 10:50 UTC #984 READY (45e1a91, chimney default test 2 powers, fine mesh ignored). Cabinet-with-vents test (3 devices) written: +4 to +5.5% vs 2D, hottest device matches. All owed items from the sweep nudge are now in PRs.
