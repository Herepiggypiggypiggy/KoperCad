# HEAT-H roadmap (project to thermal scenario bridge)

1. Done: `novo_thermal::bridge::scenario_from_project` with report of missing data.
2. Next (CFD-F owns UI): call the bridge from the Cabinet heat view, show `report.messages()` ("3 devices have no loss data") in the strip, wire `heat.simulate`.
3. Next: vents and fans have no project model yet; the bridge returns none. Needs a model proposal to WS-02.
4. Next: store ambient, material, installation and wall thickness on the enclosure (today `BridgeOptions`).
5. When DOOR-1 exposes the shared device-depth function in `novo-layout`, replace `bridge::device_depth_mm`.
