# 3D view (WS-29): roadmap

Scope: architecture and renderer of the 3D cabinet view, crate `novo-view3d`. Design: `docs/design/3d-view.md` (section 5 is the contract the three sibling threads build against).

## Done
- Design document (this PR).

## Next, in order
1. Doc merged, then the coordinator registers `novo-view3d` (CLAUDE.md row and one `[workspace.dependencies]` line).
2. Crate with scene, camera, pick, cull and headless tests.
3. Renderer spike (offscreen colour + depth, instanced boxes, edges, orbit example, PNG path), measured numbers into the doc.
4. egui widget and a Cabinet canvas face behind a flag.
5. Door group, hidden layers, wires from `novo-routing`, dimensions.

## Open questions
- Where the 3D face sits in the window and its key (proposal in the doc, section 12).
- Whether maker meshes are wanted (geometry thread decides with screenshots).
- Door, hinge and enclosure interior in the model (WS-02, R2 of the Pro Panel gap analysis).
