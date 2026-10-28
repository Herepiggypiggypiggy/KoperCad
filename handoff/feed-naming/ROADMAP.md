# Feed naming roadmap

Scope: a new potential feed must take the potential of what it is attached to and never join an unrelated conductor by its default name (cooling tower report, findings 2 and 12).

## Done (PR 2)
- `novo-editor/src/feed_name.rs`: a feed placed without a potential gets a provisional name no potential in the project has (`L1`, `L2`, ...), then takes: the potential its net already carries; PE or N from a `PE` / `N` pin or a PE / N terminal; 24 V and 0 V from `L+` / `M` pins; the return of a coil (0 V on a DC coil, N on a line coil); `L1`..`L3` from a device's own phase pins. The mark `feed.provisional` keeps it from touching a name a person set.
- Retargeting runs at the end of place, settle, accept (facing wire) and draw_wire.
- A feed dropped on a pin that already has a wire joins it (finding 12).
- The `potentials` lesson follows: a feed on a `PE` pin is earth at once, the extra "make it earth" step is gone.

## Next
- Wire tool and macro paths other than the four above (a macro that places feeds names them itself).
- A visible hint when a feed was named from its pin ("named 0V from the coil return").
