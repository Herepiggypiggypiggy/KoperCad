# Safety, PLC and network symbols: roadmap

Scope: findings RM-04 and RM-20 of the real-machine build (`audit/real-machine/findings.md`): no safety relay, door interlock, light curtain, PLC CPU, network switch or fieldbus symbols in `novo-symbols`.

Done (PR 1):
- 16 IEC symbols: `safety/` relay, relay_expander, emergency_stop_2ch, door_switch_nc, door_switch_no, light_curtain_tx, light_curtain_rx; `plc/` cpu, remote_io; `network/` ethernet_switch_5, ethernet_switch_8, port_ethernet, port_ethernet_2p, port_rs485, port_can, bus_terminator.
- Letter codes, standards classes, EPLAN-word table rows (own words), search words, aliases.
- Starter macros "Emergency stop with safety relay" and "PLC network with switch".
- QA scenario `qa/scenarios/70-safety-plc-network-symbols.qa`.

Next, in order:
1. NFPA pack counterparts (`handoff/ws25/generator/gen_nfpa.py` entries).
2. Network-aware checks (a port wired to a non-port pin; safety relay channel left open; both channels of an e-stop on one wire).
3. Cable plan: network cables as proposals between `P` pins (Cat 5e, M12), RS-485 cable with shield.
4. Safety relay as device with its contacts as separate symbols for cross references (today the contacts live inside the block).
5. More parts: enabling switch, safety mat, two-hand control, safety PLC input, media converter, IO-Link master.

Open: the simulation does not evaluate safety functions; `novo-sim` could treat `safety.relay` as a coil with safety contacts.
