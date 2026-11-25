# Crane verify handoff
Branch claude/project-thread-n4naqu (kopercad). Customer files stay private; only counts are written here.
Run (needs the private .epj and the PDF EPLAN printed):
  KOPERCAD_CRANE_EPJ=<x.epj> KOPERCAD_VERIFY_OUT=<dir> cargo test -p novo-tests --test eplan_verify -- --nocapture
  python tools/eplan_verify/pdf_compare.py <x.pdf> <dir>/geometry.json <dir> [--pages 20,156] [--render 20,156]   (pip install pymupdf; dev tool only)
Outputs: verify.md (check counts, free pins by EPLAN view, pin offsets by symbol key), verify.jsonl, geometry.json, pdf_compare.md, pages.json, diff.json, diff-page-N.png (green ours, magenta EPLAN wire we lack, orange ours EPLAN lacks).
Facts: PDF page number = file page index + 1; EPLAN printed the A3 sheet fitted to A4 (scale by width 420 mm). EPLAN wires are solid coloured lines, busbars thick black, symbols blue.
v70b numbers (main at 3bb592af1): 2309 of 7471 pins >0.5 mm off the file's points (1689 terminal pins); 2164 pins without wire: 1113 open in EPLAN too, 533 touch a pin EPLAN keeps in another net, 518 bridged terminal sides; vs PDF over 191 pages: 49% of EPLAN wire length covered, 75.6% of ours on EPLAN lines.
