# Visual test user: roadmap

Scope: let Claude test KoperCAD as a person with a screen (Tadjiro, 2026-10-17).

Done: control mode (`kopercad --control`), MCP server (`kopercad-cli test-user`),
`.mcp.json`, docs/testing/visual-test-user.md, docs/testing/tester-playbook.md,
first probe of scenario A on Linux.

Next, in order:
1. Merge PR #942; run scenarios A to I on `latest-build` via a fresh session that has the MCP natively; findings to `/mnt/project-files/qa/v0.1/`.
2. A `scene` command in control mode: dump the open page (symbols, tags, wires, selection) as text, so canvas content is readable without pixels.
3. Native dialogs: an app setting (control mode only) that routes Open/Save-as through an in-window path box, so scenario B can run.
4. Windows pass through Remote Control (docs section "Windows"), DPI 125/150 percent, real GPU.
5. Label icon-only buttons in the AccessKit tree (also an accessibility gain).

Open questions: where the Windows MCP server is registered (Remote Control session config).
