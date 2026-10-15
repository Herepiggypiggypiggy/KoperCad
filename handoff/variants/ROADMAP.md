# Variants and generated engineering: roadmap

Brief 6 of docs/AMBITION-REVIEW.md (gap 6). Design: docs/design/variants.md.

Done (PR "Variants and generated engineering"):
- Expression language, parametric macros (typed, calculated, optional parts, repeat), options and variants with
  configuration diff and variant project, spec-sheet generator with round trip, app windows and commands.

Next, in order:
1. Parameter form in the macro ghost/inspector (`parametric::instantiate`), with the calculated values shown live.
2. Variant name in the title block and on reports; BOM and wire list per variant; `kopercad-cli generate`/`variant`.
3. Move the starter specs into starter.rs and .kcmacro files (Macro library thread), update from library for
   parametric definitions (`kc.base`).
4. Cabinet placement and layout carry-over for generated devices; heat/cable sizing as derived values (needs
   the protection and cable engineering lanes).
5. A configuration matrix view (every combination of options, problems per combination: `options::combinations`).
