# Two wiring modes: handoff
Thread session: session_011TpANb8LDXUp93s9E33xqv. Branch claude/project-thread-s1dfbt. Repo copy of this folder: handoff/ws-wiring-modes/.
State: design doc written (docs/design/wiring-modes.md); design PR being opened. No code yet.
Resume: git fetch origin && git checkout claude/project-thread-s1dfbt; then fmt, clippy -D warnings, test --workspace (Actions is billing-blocked).
Contacts: editor cse_019siuxbyyRNZRgBBww1KocJ and keyboard cse_01TAFG6LPGy2WMwhJYXmxqxF (own the code; coordinate before editing), Workflow cse_01JHYmmTDsVHDL7pEab8MGwy, Layout cse_01UXbCeQW22y6tv74F2Xvbvm, tester cse_01LMMcDMt9qaHXoSXy7iDu58, main reviewer cse_016C9nTT8cN7voW8t1NQUG4W, design reviewer cse_0114emQ5fSFrzcALbR4S18TU, integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S (merges).
Test list: docs/design/wiring-modes.md, last section.
Decisions: overlap rule in editor engine plus a novo-checks problem; nudge rather than auto-join; mode is a per-user setting.
Traps: do not reopen "no corner/T pieces" (corners are route bend points). After READY do not push; message reviewer after each push with SHA.
