Source at git ff732d978b (full ff732d978b668f660032a9973be1051390c84321, branch claude/controller-addon-signing). Binary: debug kopercad and kopercad-cli built from that tree at 12:31 UTC, after the 12:30 head commit. Scenario and fixtures: scenario/ (not in the repository).

# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/claude-0/-home-claude/211515c3-4ae8-5ebf-a81a-bcf96f18388f/scratchpad/qa1793/app-wrapper.sh`, built 1793363183 (unix time), source at git ff732d978b.

> **WARNING: the program is older than the newest source file by 128 s, it was built before the latest change** The pictures may not show the current code.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| PR 1793: File > Add-ons shows the five trust states (signed, unsigned, unknown key, revoked, tampered) (1793-addon-trust-states) | Add-ons | 1366x768 | pass | - | 6 |
| PR 1793: File > Add-ons shows the five trust states (signed, unsigned, unknown key, revoked, tampered) (1793-addon-trust-states) | Add-ons | 1920x1080 | pass | - | 6 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
