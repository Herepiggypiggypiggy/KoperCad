# 3D look and feel: handoff

Repo kopercad. Spec: docs/design/3d-look.md (PR #388, branch claude/project-thread-rkbpit).
Code: PR stacked on #439 (renderer skeleton, branch claude/project-thread-8laqbm), branch claude/3d-look-view3d, in crates/novo-view3d: look.rs (spec values, Look::for_theme), palette.rs (materials, light rig, wire colours, tiers, theme colours), feel.rs (zoom notch, orbit speed, inertia, transitions, pivot, saved views), cube.rs (view cube hit test), labels.rs (label layout, fades, outline state), governor.rs (frame governor).
PR #420 (same ideas in novo-theme, own camera) is superseded: close it.
Renderer thread (session_01UMNPbP9STYPHPF8CDKoBbN) owns wgpu; it reads Look::surface and Look::mark only. Design system thread (session_018Mr3ZJ9VtQqHa3AF6FRdm6, PR #448) will add a 3D section to its palette; then palette.rs room colours read from it. UI entry points: spec section 8; UI thread session_01G52sVY5N34jPsrkqWukApR places widgets.
Next: wire label and outline passes with the renderer, Settings > 3D view, measure on an integrated GPU.
