# Heat flow modes and the moving cloud: roadmap

Goal (Tadjiro, 2026-11-02): heat flow is hard to evaluate; give it modes like airflow, and let heat flow and airflow be read alongside a cloud that visibly moves with the flow.

Done in the first PR
- Heat flow menu in the 3D Heat strip: Off, Carried by air, Through solids, Air and solids, Flux map.
- Paths: each hot device's split into air, plate and radiation, pinned on the devices and listed in the Heat panel.
- Cloud > Moves: the solved air flow carries the heat from cold air (frozen-flow transport model, `novo-heatview::advect`).

Next
- Left rail rows for Heat flow mode and Moves (the 3D rail has one Heat row; its flyout is not built).
- Moving cloud for the transient (heat-up) frames; settle detection that stops the repaint loop.
- Radiation as its own arrows once the solver publishes per-face radiation flux.
