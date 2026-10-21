# Crane export (AutomationML drawing profile)

Scope: let the crane configurator (private repo, outside this one) hand KoperCAD a project through an open
standard, AutomationML (IEC 62714), with drawn pages and connected wires, without the EPLAN route.

Done
- Import of the drawing profile: pages, symbols with pins, nets to wires, interruption points across pages
  (`crates/novo-interop/src/aml/{read,draw,plan}.rs`, design `docs/design/automationml-drawing.md`).

Next, in order
1. Export of drawn pages from KoperCAD in the same profile (round trip).
2. Symbol kinds: motor, power poles, plug, thermal switch still draw as boxes with the file's pins.
3. Pin and wire numbers, wire colours and cross sections in the profile.

Open: whether other tools' AutomationML (not written by the configurator) should be offered a page layout
when it has nets but no symbols (generate pages from nets, migration slice 2).
