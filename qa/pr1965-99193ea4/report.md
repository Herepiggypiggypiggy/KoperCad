# PR 1965 QA, source at git 99193ea4aff9a8172f06eb809d835e9cc09d3e39

No screen changes, so no screenshots: this PR adds `kopercad-cli parts step-parts <file>`. Run with the built-in synthetic block (block-plain.stp) with its solid renamed to 'Wheel A' (exit 0):

```
| Part | Role | Size mm (x y z) | Triangles |
| --- | --- | --- | --- |
| Wheel A | wheel | 10 x 20 x 30 | 12 |

- wheel: 1

```

Tests: cargo test -p novo-interop --lib part3d (83 pass), cargo test -p novo-cli --test parts (5 pass), clippy -D warnings clean. The tip commit adds only this evidence.
