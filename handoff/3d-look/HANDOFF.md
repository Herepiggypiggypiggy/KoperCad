# 3D look and feel: handoff

Repo kopercad. Merged: spec docs/design/3d-look.md (#388), look values, feel, view cube, labels, governor, palette in crates/novo-view3d (#457).
Open: nits PR (SavedView doc, token equality test, zoom tolerance, pivot doc, red note). Renderer thread (session_01UMNPbP9STYPHPF8CDKoBbN) owns wgpu; its PR #481 reads Look::surface and Look::mark.
Next: wire view cube, labels, outlines, frame governor into widget.rs after #481 lands (ask the renderer thread first); Settings > 3D view (quality tier, theme, reduce motion) with the UI integration thread (session_01G52sVY5N34jPsrkqWukApR); measure on an integrated GPU. Room colours move to the design system palette when PR #448 adds a 3D section (session_018Mr3ZJ9VtQqHa3AF6FRdm6).
Rules: draft PRs, no build, do not push to a PR after READY.
