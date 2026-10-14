"""WS-28 done-criterion, headless: a script places a DOL motor circuit, renames
its tags, saves, reopens and checks the netlist.

Run after installing the wheel (see crates/novo-py/README.md):

    python -m pytest crates/novo-py/python/tests
"""

import pytest

import novoelectric as ne


def build_dol(p):
    """Main circuit L1/L2/L3 -> -Q1 -> -K1 -> -F2 -> -M1 (PE straight to the
    motor); control circuit L -> -F3 -> -S1 -> -F2 95/96 -> -K1 A1/A2 -> N."""
    main = p.add_page("1", "Main circuit")
    ctrl = p.add_page("2", "Control circuit")

    pots = {
        name: p.add_potential(name, kind, voltage)
        for name, kind, voltage in [
            ("L1", "line", 400.0),
            ("L2", "line", 400.0),
            ("L3", "line", 400.0),
            ("PE", "protective_earth", None),
            ("L", "line", 230.0),
            ("N", "neutral", None),
        ]
    }

    q1 = p.add_device("-Q1", "Motor feeder breaker")
    k1 = p.add_device("-K1", "Motor contactor")
    f2 = p.add_device("-F2", "Overload relay")
    m1 = p.add_device("-M1", "Pump motor 2.2 kW")
    f3 = p.add_device("-F3", "Control circuit breaker")
    s1 = p.add_device("-S1", "Start")

    # Main circuit, one column per phase, 8 mm apart.
    feeds = {
        name: p.place(main, "feed", x, 0, potential=pots[name])
        for name, x in [("L1", 40), ("L2", 48), ("L3", 56), ("PE", 72)]
    }
    q1_s = p.place(main, "mcb3", 40, 20, device=q1, main=True)
    k1_main = p.place(main, "km3", 40, 60, device=k1)
    f2_main = p.place(main, "ol", 40, 100, device=f2, main=True)
    m1_s = p.place(main, "m3", 40, 140, device=m1, main=True)

    for feed, pin in [("L1", "1"), ("L2", "3"), ("L3", "5")]:
        p.connect((feeds[feed], "1"), (q1_s, pin))
    for a, b in [(q1_s, k1_main), (k1_main, f2_main)]:
        for out_pin, in_pin in [("2", "1"), ("4", "3"), ("6", "5")]:
            p.connect((a, out_pin), (b, in_pin))
    for out_pin, motor_pin in [("2", "U1"), ("4", "V1"), ("6", "W1")]:
        p.connect((f2_main, out_pin), (m1_s, motor_pin))
    p.connect((feeds["PE"], "1"), (m1_s, "PE"))

    # Control circuit.
    l_feed = p.place(ctrl, "feed", 20, 0, potential=pots["L"])
    n_feed = p.place(ctrl, "feed", 20, 160, potential=pots["N"])
    f3_s = p.place(ctrl, "mcb", 20, 20, device=f3, main=True)
    s1_s = p.place(ctrl, "no", 20, 50, device=s1, main=True)
    f2_aux = p.place(ctrl, "iec/contacts/thermal_nc", 20, 80, device=f2)
    k1_coil = p.place(ctrl, "coil", 20, 120, device=k1, main=True)
    p.connect((l_feed, "1"), (f3_s, "1"))
    p.connect((f3_s, "2"), (s1_s, "13"))
    p.connect((s1_s, "14"), (f2_aux, "95"))
    p.connect((f2_aux, "96"), (k1_coil, "A1"))
    p.connect((k1_coil, "A2"), (n_feed, "1"))

    return {"Q1": q1, "K1": k1, "F2": f2, "M1": m1, "F3": f3, "S1": s1}


def net_of(nets, pin):
    found = [n for n in nets if pin in n["pins"]]
    assert len(found) == 1, f"{pin} is in {len(found)} nets"
    return found[0]


def test_dol_motor_circuit_round_trip(tmp_path):
    path = tmp_path / "dol.novo"
    p = ne.Project.create(path, name="DOL starter")
    devices = build_dol(p)

    # Rename every tag into =A1+CAB, one undo step each.
    for short, device in devices.items():
        p.set_tag(device, f"=A1+CAB-{short}")
    assert p.undo_label == "Rename device"
    assert p.undo() and p.device("=A1+CAB-S1") is None
    assert p.device("-S1") == devices["S1"]
    assert p.redo() and p.device("=A1+CAB-S1") == devices["S1"]

    assert p.dirty
    p.save()
    assert not p.dirty
    before = p.nets()

    # The file is locked while open: a second opener is refused.
    with pytest.raises(ne.NovoError, match="already open"):
        ne.Project.open(path)
    p.close()

    with ne.Project.open(path) as q:
        assert q.info()["name"] == "DOL starter"
        assert [pg["title"] for pg in q.pages()] == ["Main circuit", "Control circuit"]
        assert sorted(d["tag"] for d in q.devices()) == sorted(
            f"=A1+CAB-{t}" for t in devices
        )
        nets = q.nets()
        assert nets == before

        # Each phase passes the breaker and contactor and reaches the motor.
        for phase, q_out, motor_pin in [
            ("L1", "2", "U1"),
            ("L2", "4", "V1"),
            ("L3", "6", "W1"),
        ]:
            assert net_of(nets, f"=A1+CAB-Q1:{q_out}")["potentials"] == [phase]
            motor = net_of(nets, f"=A1+CAB-M1:{motor_pin}")
            # The overload heater conducts: its in and out are one net.
            assert f"=A1+CAB-F2:{q_out}" in motor["pins"]
            assert motor["potentials"] == [phase]
        assert net_of(nets, "=A1+CAB-M1:PE")["potentials"] == ["PE"]

        # Control circuit: the coil sits between the overload contact and N.
        assert net_of(nets, "=A1+CAB-K1:A1")["pins"] == ["=A1+CAB-F2:96", "=A1+CAB-K1:A1"]
        assert net_of(nets, "=A1+CAB-K1:A2")["potentials"] == ["N"]
        assert net_of(nets, "=A1+CAB-F3:1")["potentials"] == ["L"]

        assert len(q.wires()) == 3 + 6 + 3 + 1 + 5
        assert not q.can_undo  # history starts fresh on open


def test_commands_as_json_and_undo():
    p = ne.Project("Scratch")
    created = p.execute({"command": "add_page", "name": "7", "title": "Lighting"})
    assert created[0]["kind"] == "page"
    k = p.execute('{"command": "add_device", "tag": "-K5"}')[0]["id"]
    assert p.device("-K5") == k
    assert p.undo() and p.device("-K5") is None
    with pytest.raises(ne.NovoError):
        p.execute({"command": "no_such_command"})
    with pytest.raises(ne.NovoError, match="no symbol"):
        p.place(created[0]["id"], "not-a-symbol", 0, 0)
    with pytest.raises(ne.NovoError, match="has no file"):
        p.save()


def test_assign_potentials_by_name():
    p = ne.Project()
    page = p.add_page("1")
    a = p.place(page, "fuse", 0, 0)
    b = p.place(page, "fuse", 0, 40)
    w = p.connect((a, "2"), (b, "1"))
    p.assign_potentials([w], ["L1"])
    assert [x["name"] for x in p.potentials()] == ["L1"]
    assert p.wires()[0]["potentials"] == ["L1"]


@pytest.mark.skip(reason="Excel BOM export waits for WS-13 (reports); see handoff/ws28/ROADMAP.md")
def test_dol_bom_to_excel(tmp_path):
    p = ne.Project()
    build_dol(p)
    p.export_bom(tmp_path / "bom.xlsx")  # API to be added on top of WS-13
