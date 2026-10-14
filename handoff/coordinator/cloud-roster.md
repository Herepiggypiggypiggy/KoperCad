# Cloud session roster (repo copy)

The project-mount roster is `/mnt/project-files/coordination/threads.md`; cloud sessions can't
read it. These sessions were started from the coordinator cloud session on 2026-10-14. All run in
environment `env_01AL913YkMhxzrWWVepJ3KQH` (Default). Message them with
`mcp__claude-code-remote__send_message` (session id below).

| Role | Session | Branch | Notes |
| --- | --- | --- | --- |
| Coordinator / integration lead | session_01DZ68tTULUv7Uhe8ecKbC47 | claude/youthful-franklin-aey18d | Merges on reviewer READY; owns handoff/MAIN-HANDOFF.md |
| Main reviewer | session_01XEdxpvQFh4HWmurh72tKwp | (none) | Correctness, core contract, local CI while Actions is down |
| Design & workflow reviewer | session_01NLERJkVHpsVcABrarTR3CF | (none) | UI PRs against docs/DESIGN.md; UI PRs need both verdicts |
| WS-13 reports | session_01834PmEwNeusq2TnHgA5ZyG | claude/ws13-reports | crate novo-reports |
| WS-12 cables | session_019U7n53LMsCesbK7taAj8hD | claude/ws12-cables | crate novo-cables |
| WS-28 Python API + CLI | session_011qgUKo3T6sBPNTVxKy9zZF | claude/ws28-python-cli | crates novo-cli, novo-py |

Coordinator helper agents (inside the coordinator session) also did: WS-09 PDF (#49), WS-14
design checks (claude/ws14-design-checks), and the fixes pushed to #28-#31, #36, #38, #41.

The original project threads (coordination/threads.md) resume at 23:20 UTC and keep their own
branches; the cloud workers above took workstreams no project thread owns.
