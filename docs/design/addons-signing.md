# Signing add-ons: keys, the release workflow, rotation and revocation

Lane: Controller add-on signing. Code: `crates/novo-addons/src/{trust,keyfile,pack}.rs`, `crates/novo-cli/src/addon_keys.rs`, the Manage add-ons window (`crates/novo-app/src/app/addons.rs`). Wrappers: `scripts/sign-addon.ps1` and `.sh`. Release job: `.github/workflows/package-windows.yml`, job `controllers-addon`. Background on the package format and the trust table: [addons.md](addons.md).

## What this gives, in one paragraph

Today every install of the Controllers add-on asks the person to tick "I trust this file", because KoperCAD ships with no publisher key. With a KoperVonk signing key, the release job signs the `.kcaddon`, KoperCAD carries the **public** half, and the file installs without the question and shows "Signed by KoperVonk". The private half exists only on Tadjiro's PC (and as a GitHub secret). **Until Tadjiro does the steps below, nothing changes for users: `BUILTIN_KEYS` and `addons/keys/kopervonk.pub` hold no key, so the add-on is published unsigned and the job summary says so.** This lane cannot create the real key; it builds the tools, the workflow and the checks.

## What Tadjiro does, once (about 10 minutes)

You need a build that contains this change (the next `experimental` build or release; the portable zip has `kopercad-cli.exe`). Everything runs as your normal Windows user. No admin rights.

1. **Make the key.** Open PowerShell in the folder with `kopercad-cli.exe` (or in the repository, using the wrapper `scripts\sign-addon.ps1 keygen`) and run:

   ```powershell
   .\kopercad-cli.exe addons keygen --id koper-vonk-2026 --owner KoperVonk --out "$env:LOCALAPPDATA\KoperCAD\keys\koper-vonk-2026.key"
   ```

   - The **private key** is the file you named after `--out`: `%LOCALAPPDATA%\KoperCAD\keys\koper-vonk-2026.key`. It is created readable by you only (Windows: `icacls` drops the inherited access; Linux/macOS: mode 600). It is never printed, never written anywhere else, never overwritten (a second run refuses), and the tool refuses a path inside a git work tree.
   - Do **not** put it in Documents or on OneDrive (those sync to a cloud). `%LOCALAPPDATA%` does not sync.
   - **Back it up** once to somewhere only you control (a password manager attachment or an encrypted USB stick). If the key is lost you cannot sign updates with it; you make a new key (see Rotation) and ship a new KoperCAD that trusts it. Nothing already installed breaks.
   - Optional: add `--valid-from 2026-11-01` to refuse signatures dated before that day.

2. **Put the key in the GitHub secret.** Run `.\scripts\sign-addon.ps1 secret`. It puts the secret's value on the clipboard (base64 of the key file) and prints nothing. In GitHub: repository **Settings > Secrets and variables > Actions > New repository secret**, name **`ADDON_SIGNING_KEY`**, paste, save. Then copy anything else so the clipboard no longer holds the key.

3. **Commit the public key (one line).** The `keygen` output ended with a block that starts "Public key, to commit: add this ONE line". Add that line, as printed, at the end of `addons/keys/kopervonk.pub`:

   ```
   key koper-vonk-2026 <64 hex digits> 2026-11-01 KoperVonk     (a - instead of the date if you gave no --valid-from)
   ```

   Commit it as its own data-only change (no code). The public key is not secret; the same line is also in `koper-vonk-2026.key.pub` next to the private key. Ask Claude to do the commit if you paste the line into the chat. A test (`the_embedded_key_file_is_valid`) fails the build if the line is malformed, so a typo cannot ship.

4. **Release as usual** (push a `vX.Y.Z` tag). On a tag build the job signs `controllers-*.kcaddon`. Its **job summary** says "Add-on signing: signed" and whether the public key is already in the program. Releases made before step 3 are signed too, but KoperCAD builds from before the key was committed still ask for the tick (they do not know the key) and show "Signed with an unknown key".

5. **Check it** on any PC: `kopercad-cli addons verify controllers-0.1.0.kcaddon` says "Signed by KoperVonk" and exits 0 when this KoperCAD trusts the key; with `--keys addons\keys\kopervonk.pub` it checks against exactly the committed key.

If step 2 is skipped, the release still goes out, **unsigned**, with a warning annotation, a job summary saying so, and release notes that still tell users to tick "I trust this file". Nothing fails.

## The tools

```
kopercad-cli addons keygen --id <id> [--owner KoperVonk] --out <file> [--valid-from YYYY-MM-DD]
kopercad-cli addons sign   <file.kcaddon> (--key <file> | --key-env <VARIABLE>) [-o out] [--date D | --no-date] [--replace]
kopercad-cli addons verify <file.kcaddon> [--keys <file>]... [--public-key <hex> --key-id <id>] [--root <dir>]
```

| Command | Notes |
| --- | --- |
| `keygen` | 32 random bytes from the operating system (no admin). Writes the private key file and `<file>.pub` (the public line). Prints the public key in three equivalent forms: the one `key ...` line, a `[[key]]` TOML block, and a `BUILTIN_KEYS` entry. |
| `sign` | Reads the package, checks every file against its hash (a damaged package is never signed), writes the `signature` entry and reads the result back with that key. Replaces the file (via a `.signing` temporary and a rename) unless `-o` is given. Refuses an already signed package without `--replace`. Deterministic: the same package, key and date give identical bytes. The date defaults to today (UTC); `SOURCE_DATE_EPOCH` overrides it for reproducible builds; `--no-date` writes the older undated format. |
| `verify` | Exit code 0 only when the package is signed with a trusted key. Without key options it uses what this KoperCAD trusts (built in plus your `trusted-keys.toml`). With `--keys` or `--public-key` it uses **only** those. States are printed in the same words as the window. |

`scripts/sign-addon.ps1` (Windows) and `scripts/sign-addon.sh` wrap the three plus `secret`; they find the CLI on `PATH`, in `target\release`, or through `cargo run`.

The key file is four lines of text (`kopercad-addon-key 1`, `id:`, `owner:`, `seed:`). The id travels with the seed so the one secret is all CI needs. `pack --key-id --seed-file` still works with a bare 64-digit seed file.

## What a signature covers

The signed bytes are a domain prefix (`kopercad-addon-manifest-v1\n`, or `-v2\ndate: YYYY-MM-DD\n` for a dated signature) and the exact bytes of `manifest.toml`. The manifest lists the SHA-256 of every other file, and the program checks each. So one signature covers the whole package; changing any byte of any file breaks either a hash ("has been changed") or the signature. A signature cannot be moved to another manifest, and cannot be turned into another purpose's signature. Ed25519 is verified strictly (`verify_strict`).

A signature says **who made the file**, not that it is good. The sandbox and the permission list stay the protection against a bad add-on.

## Trust states

Every state has its own words, its own mark and a different colour family; the words always carry the meaning, never the colour alone.

| State | Window wording | Mark | Install |
| --- | --- | --- | --- |
| Signed | "Signed by KoperVonk" | seal with check, green | no tick |
| Unsigned | "Unsigned, from file: the package has no signature" | shield with warning, amber | tick "I trust this file" |
| Unknown key | "Signed with an unknown key (id): this PC cannot tell who made it" | seal with question mark, amber | tick (the banner says the key is unknown, "it could be anyone") |
| Revoked | "Signed with a revoked key (id), reason: the publisher withdrew it" | prohibited, red | refused; no tick |
| Does not match | "Signature does not match (key id): the file was changed after it was signed" | octagon warning, red | refused; no tick |

Where it shows: the install prompt (File > Add-ons... > Install from file..., or drop a file), the add-on's details in the manager, the refusal message above the list, and `kopercad-cli addons inspect | verify | list --json` (`"state": "signed | unsigned | unknown-key | revoked | tampered"`).

An **installed** add-on is judged again at every start. If its key is revoked later, or its signature is swapped, it shows *Failed* with the exact reason and does not load; no tick brings it back. An add-on the person installed with the tick keeps working as before.

## Keys, rotation, several keys

Keys come from three places, merged, first id wins:

1. `BUILTIN_KEYS` in `trust.rs` (code; empty).
2. `addons/keys/kopervonk.pub`, embedded at build time by `crates/novo-addons/build.rs`. The file may be empty or missing: the program then trusts nobody and everything asks for the tick. Adding the real key is a data-only commit.
3. The person's `%LOCALAPPDATA%\KoperCAD\addons\trusted-keys.toml` (their own publishers).

A key has an **id** (3 to 48 characters of `a-z 0-9 - _ .`), an **owner** (shown in the window), the **public** key (64 hex digits) and an optional **valid_from** date. A signature file names the id it was made with. Several keys can be valid at once, so rotation overlaps:

1. Make the new key (`keygen --id koper-vonk-2027 ... --valid-from 2027-01-01`).
2. Add its line to `addons/keys/kopervonk.pub` next to the old one and ship a KoperCAD release. Both keys are trusted now.
3. Change the GitHub secret to the new key. New packages are signed with it; packages signed with the old key keep verifying.
4. When the old key is no longer needed, add it to the revocation list (below) in a later release, or just leave it.

`valid_from` works on **dated** signatures (signature format 2, which `sign` writes by default; the date is inside the signed bytes): a signature dated before the key's `valid_from` is refused. The date is what the signer claims, so it catches a mistake or an early test, not a thief. Format 1 (undated) signatures still verify.

## Revocation

A revocation says "never trust this key again", whichever list the key came from. In `addons/keys/kopervonk.pub` (or the person's `trusted-keys.toml`):

```toml
[[revoked]]
id = "koper-vonk-2026"
reason = "the signing laptop was lost"
# public = "<64 hex digits>"   # optional: also revokes the same key under any other id
```

- A revoked key **wins over** a trusted entry, wherever either was written. A package signed with it shows "Signed with a revoked key" even if the key is no longer in any list, never "unsigned".
- It is judged before the signature is checked, so the person hears "revoked", not "mismatch".
- Installed add-ons signed with it stop loading at the next start; rolling back to a version signed with it is refused.
- **Revocation is offline.** It reaches a PC when that PC runs a KoperCAD that carries the entry, or when the person adds it to their own `trusted-keys.toml`. There is no server check. A stolen key therefore remains accepted by every installed KoperCAD that was not updated, so: revoke in the repository, ship a release, and tell users to update. This is a real limit of an offline program, not a bug.
- If the GitHub secret leaks: delete the secret first, then make a new key, revoke the old id (with `public =` so renaming cannot help), release.

## Versions and downgrades

Signing does not change how versions are handled (see [addons.md](addons.md)): an older version than the installed one is refused even if validly signed (roll back or remove first), the same version with other content is refused, and a signed update keeps the on/off state and the previous version for roll-back. These are tested with signed packages in `crates/novo-addons/tests/signing.rs`.

## The release job

`package-windows.yml`, job `controllers-addon`:

1. Builds the package (`scripts/build-controllers-addon.sh`) unsigned. No secret is in the environment of this step; Cargo build scripts run here.
2. **Sign step.** Runs only the prebuilt `target/debug/kopercad-cli` with `ADDON_SIGNING_KEY` in its environment, on tag pushes. The key is decoded in memory, never written to disk, never echoed, and the tool never prints it. Pull requests and forks get no secret, and this job attaches nothing to a release outside tag pushes.
3. Without the secret (or not a tag): publishes unsigned, `::warning`, and a job summary "Add-on signing: UNSIGNED" or "skipped". With it: a summary naming the key and whether the program built from this commit already trusts it.
4. The `release` job reads `signed` from the job outputs and writes the truth into the release notes.

Pull request CI (`ci.yml`, job `controllers add-on`) signs a copy with the committed **test-only** key every time, so the signing path cannot rot.

## Test key

`crates/novo-tests/tests/keys/TEST-ONLY-addon-signing.key` is a throw-away key (id `test-only-addon-key`) for tests and CI. It is public by design, trusted by no build, and signs nothing real. A test (`no_other_private_key_is_in_the_repository`) fails if any other file in the repository starts with the key-file header.

## Not built, listed so nobody looks for it

- Online revocation or a key server. Revocation ships with the program.
- Windows code signing of the installer (SmartScreen). Separate and not this lane.
- Time stamping by a third party: dates are the signer's claim.
- Hardware keys (a YubiKey) for the private half. The key file format leaves room; not needed yet.
