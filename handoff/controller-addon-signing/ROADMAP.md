# Controller add-on signing: roadmap

Scope: let the Controllers add-on (and any add-on) be signed by KoperVonk so users stop needing "I trust this file". Guide: `docs/design/addons-signing.md`.

## Done in the first PR (branch `claude/controller-addon-signing`)

- Key tool: `kopercad-cli addons keygen | sign | verify`, wrappers `scripts/sign-addon.ps1` / `.sh`.
- `trust.rs`: five trust states (signed, unsigned, unknown key, revoked, tampered), key id / owner / `valid_from`, revocation list (by id or public key), dated signatures (format 2), one-line key format, embedded `addons/keys/kopervonk.pub` (build.rs, empty or absent is fine).
- Release job signs on tag builds when secret `ADDON_SIGNING_KEY` exists; otherwise unsigned with a warning and summary. PR CI signs a copy with the test-only key.
- Manage add-ons window: wording, mark and tone per state; revoked and tampered files are refused with the reason.
- Tests: `trust.rs`, `keyfile.rs`, `crates/novo-addons/tests/signing.rs`, `crates/novo-tests/tests/addons_signing_cli.rs`, `crates/novo-app/src/app/addons_trust_tests.rs`.

## Next, in order

1. Tadjiro generates the real key and sets the secret (guide, "What Tadjiro does, once").
2. One data-only commit: the printed `key ...` line into `addons/keys/kopervonk.pub`. Then change `the_embedded_key_file_is_valid` expectations if wanted (it already passes with a key).
3. Tag a release; confirm the job summary says "signed" and the public key is in the program.
4. Later: ship a second key before the first is retired (rotation section of the guide).

## Open questions

- Windows: the `icacls` restriction on the private key file is written but not run on Windows here; confirm on a real PC (the Security tab of the file lists only the user).
- Hardware key support is not planned.
