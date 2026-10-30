# Evidence: PR 1793, File > Add-ons trust states

Source at git ff732d978b (`ff732d978b668f660032a9973be1051390c84321`), branch `claude/controller-addon-signing`. This folder replaces an earlier run of `7937d06` that I deleted; its binary predated the one-line wording fix below. Binary: `kopercad` and `kopercad-cli`, debug, built from this exact tree after the head commit (see `HEAD.txt`, and `report.md` for the run). Window sizes 1366x768 and 1920x1080. Result: 2 runs, 0 failed, 0 findings (`summary.md`).

Screenshots are in `shots/` (kept on the project mount, not in the repository). The scenario, the wrapper, the trusted-keys file and the five test packages are in `scenario/`. The scenario file is not committed, to keep the PR head unchanged; it needs the packages and keys made by `scenario/make-fixtures.sh`. The private QA keys it creates stay in a scratch folder and are not kept anywhere.

## How the states were produced (nothing of the program was changed)

The five packages are copies of `qa/fixtures/addons/demo-tools-0.1.0.kcaddon`, signed with `kopercad-cli addons sign` using four throw-away keys made by `addons keygen` for this run. `app-wrapper.sh` is passed as `--app`: before it starts the real `kopercad` binary it writes `trusted-keys.toml` into the test profile, which is what a person would have in `%LOCALAPPDATA%\KoperCAD\addons`. That file trusts key `qa-publisher` and revokes key `qa-withdrawn` with the reason "the signing laptop was lost". The embedded key file (`addons/keys/kopervonk.pub`) is still empty, as in the PR.

| State | Package | What is on screen | Pictures |
| --- | --- | --- | --- |
| Unsigned | no signature | "Unsigned, from file: the package has no signature", warning "Nobody you trust signed this file", tick "I trust this file" | `*-01-1-unsigned.png` |
| Unknown key | signed with `qa-stranger` (not in any list) | "Signed with an unknown key (qa-stranger): this PC cannot tell who made it"; warning "signed with the key qa-stranger, which this PC does not know, so it could be anyone"; tick | `*-02-2-unknown-key.png` |
| Revoked | signed with `qa-withdrawn` (revoked) | refusal above the list: "Demo tools: it was signed with the key qa-withdrawn, which has been revoked (the signing laptop was lost)"; no prompt, no tick | `*-03-3-revoked.png` |
| Tampered | signed by an impostor holding another key under the id `qa-publisher` (which is trusted) | refusal: "the add-on was changed after it was signed, or was not signed by the holder of its key: its signature does not match its manifest for the key qa-publisher ... it was not installed"; no tick | `*-04-4-tampered.png` |
| Signed | signed with `qa-publisher` (trusted) | prompt "Signed by QA test publisher" with no tick; after Install, the details show "Signed by QA test publisher" | `*-05-5-signed-prompt.png`, `*-06-5-signed-installed.png` |

Revocation wording is shown (state 3, with the reason). Key rotation is not shown in the window: it is not a state, only several keys being trusted at once, and it is covered by tests in `crates/novo-addons/tests/signing.rs` and `trust.rs`, not by this QA.

## What the wording does and does not say

The request was to state that every state names the key and says the program does not know it. That is true only of the unknown-key state, and it would be wrong for the others, so I am not claiming it:

- **Unknown key:** names the key id and says this PC does not know it ("cannot tell who made it", "could be anyone"). The tick stays and the warning says to install only if you know where the file came from.
- **Signed:** names the owner ("Signed by QA test publisher") because the key is in a list this PC trusts. It says who made the file, not that the file is safe; the sandbox and the permission list are the protection.
- **Unsigned:** names no key (there is none) and says nobody you trust signed it.
- **Revoked and tampered:** name the key id and refuse the file. Tampered says the signature does not fit, either because the file changed or because the signer did not hold the key. A signature never proves a file is good.

None of the wording promises that a signed add-on is safe.

## Things seen in the pictures

- The tampered refusal repeats itself ("changed after it was signed" appears twice). It is correct but clumsy, and I left it; fixing it needs another push.
- A refused file shows its message above the list and no prompt. A file picked while another install prompt is still open leaves the first prompt up (the scenario presses Cancel between files); in the program the open prompt is modal, so this was only reachable through the test harness.
- Fixed in this head (`ff732d9`): the revoked refusal used to read "Demo tools it was signed with ...". It now reads "Demo tools: it was signed with ...".

## Not covered here

Windows, the `.ps1` wrapper, the release job, the real KoperVonk key (not yet generated, so nothing in these pictures is about it).
