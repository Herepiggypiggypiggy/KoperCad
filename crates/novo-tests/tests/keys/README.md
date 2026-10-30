# Test-only keys

`TEST-ONLY-addon-signing.key` is a throw-away Ed25519 key made for the signing tests
(`addons_signing_cli.rs`). Its seed is a repeated byte; anybody can read it here. It signs nothing
real: no build of KoperCAD trusts `test-only-addon-key`. The real KoperVonk key is never in this
repository (the test `no_other_private_key_is_in_the_repository` fails if one is).
