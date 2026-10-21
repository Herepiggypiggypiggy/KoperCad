# Roadmap: device shapes in the 3D view
Scope: devices drawn with the typical form of their family instead of plain boxes, then real maker models.
Done: typical shapes for breaker, contactor, terminal block, PLC, drive, supply, connector (`crates/novo-view3d/src/device_shape.rs`), Layers > Device shapes.
Next: (1) imported STEP models from the Parts window GeometryService into the 3D MeshLibrary; (2) shapes for motors, pushbuttons, transformers; (3) pin-accurate terminals on shapes.
Open: whether families should be chosen by part category instead of tag letter once the catalog carries one.
