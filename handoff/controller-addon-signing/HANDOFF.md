# Controller add-on signing: handoff

Branch `claude/controller-addon-signing`, one PR from main. Status and head SHA are in the PR; this file says how to continue.

## State

Everything in the roadmap's "Done" list is written and tested on Linux. **BUILTIN_KEYS and `addons/keys/kopervonk.pub` are empty on purpose**: the real KoperVonk key must be made by Tadjiro (a private key never goes through a cloud session). Until then every build behaves as before: unsigned package, tick needed.

## Not verified

- `scripts/sign-addon.ps1` and the Windows `icacls` branch of `keygen` were not run (no PowerShell or Windows here). The `.sh` wrapper and the CLI were run on Linux.
- The release job was not run (needs a tag push and, to sign, the secret). The YAML parses; the sign step is plain shell around the CLI.

## Where things are

- Library: `crates/novo-addons/src/trust.rs` (judge, TrustStore, Trust), `keyfile.rs` (private key text, base64), `pack.rs` (`sign_package`), `build.rs` (embeds the key file).
- CLI: `crates/novo-cli/src/addon_keys.rs`, subcommands in `addons.rs`.
- Window: `trust_look` and `tick_warning` in `crates/novo-app/src/app/addons.rs`; icon `UNKNOWN_KEY` in `novo-theme`.
- Tests: see the roadmap. Test-only key: `crates/novo-tests/tests/keys/TEST-ONLY-addon-signing.key`.

## Resume

```
export CARGO_TARGET_DIR=/tmp/kc-target-sign CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p novo-addons -p novo-cli
cargo test -p novo-tests --test addons_signing_cli --test addons_fixtures
cargo test -p novo-app --lib addons
```

## Decisions and traps

- Revoked beats trusted; judged before the signature is checked, so the message says "revoked", not "mismatch".
- A mismatch (`Trust::Tampered`) is an error when a package is read (`AddonError::Tampered`) but a state for an installed add-on (it shows Failed with the reason).
- Revocation is offline by design; the guide says what to do when a key leaks.
- The secret is only read by the `Sign the add-on package` step, which runs a prebuilt binary, not cargo.
- Disk: sessions share the volume; delete `$CARGO_TARGET_DIR/debug/incremental` and use `CARGO_INCREMENTAL=0` if a write fails with "no space left".
