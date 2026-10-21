# Wire list with cut lengths: roadmap

Scope: the wire list's Route length mm / Length basis columns and the cut list report in `novo-reports`. Routing engine and `route_connections` belong to the Wire routing thread (WS-16, PR #381).

## Done
- Length columns on the wire list (routed / free air / estimate), design in docs/design/cut-list.md.
- `CutList` report: summary, groups by size and colour, end allowances, CSV/xlsx through the standard reports.

## Next
1. Rebase on #381 once merged; draft PR for this branch.
2. Allowances and the estimate constants as project rules (needs WS-02).
3. Add the cut list to a Generated pages menu entry in the app (WS-01).
