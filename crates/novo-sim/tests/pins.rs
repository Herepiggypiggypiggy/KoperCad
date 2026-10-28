//! Pins by their printed names and the poles behind them (used by controller bindings).

use novo_core::{DeviceTag, Document, Pin, Placement, Point, SymbolId, SymbolRole};
use novo_sim::PoleState;
use novo_sim::pins::{
    PinProblem, PoleRef, force_pole, operate_poles, pole_of_pin, poles_of_device,
};
use novo_sim::rest_state;

struct Drawing {
    doc: Document,
    receiver: SymbolId,
    coil: SymbolId,
    nc: SymbolId,
}

fn place(
    tx: &mut novo_core::Tx<'_>,
    page: novo_core::PageId,
    key: &str,
    pins: &[&str],
    groups: Vec<Vec<u16>>,
    role: SymbolRole,
) -> SymbolId {
    let id = tx
        .place_symbol(
            page,
            key,
            Placement::at(Point::new(0.0, 0.0)),
            pins.iter()
                .enumerate()
                .map(|(i, n)| Pin::new(*n, 0.0, 8.0 * i as f64))
                .collect(),
            role,
        )
        .unwrap();
    tx.update::<novo_core::SymbolInstance>(id, |s| s.pass_through = groups)
        .unwrap();
    id
}

fn drawing() -> Drawing {
    let mut doc = Document::default();
    let ((receiver, coil, nc), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let a1 = tx.add_device(DeviceTag::parse("-A1").unwrap(), "")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1").unwrap(), "")?;
            let s1 = tx.add_device(DeviceTag::parse("-S1").unwrap(), "")?;
            let dev = |d| SymbolRole::Device {
                device: d,
                main: true,
            };
            // A receiver block: a common and two outputs, each output a pole [COM, n].
            let receiver = place(
                tx,
                page,
                "iec/controllers/outputs_6",
                &["COM", "1", "2"],
                vec![vec![0, 1], vec![0, 2]],
                dev(a1),
            );
            let coil = place(
                tx,
                page,
                "iec/coils/contactor",
                &["A1", "A2"],
                vec![],
                dev(k1),
            );
            let nc = place(
                tx,
                page,
                "iec/operators/pushbutton_nc",
                &["11", "12"],
                vec![vec![0, 1]],
                dev(s1),
            );
            Ok((receiver, coil, nc))
        })
        .unwrap();
    Drawing {
        doc,
        receiver,
        coil,
        nc,
    }
}

#[test]
fn an_output_pin_leads_to_its_pole() {
    let d = drawing();
    let p = d.doc.project();
    assert_eq!(
        pole_of_pin(p, "-A1:1"),
        Ok(PoleRef {
            symbol: d.receiver,
            group: 0
        })
    );
    assert_eq!(
        pole_of_pin(p, " -A1 : 2 "),
        Ok(PoleRef {
            symbol: d.receiver,
            group: 1
        })
    );
}

#[test]
fn a_common_is_refused_with_the_count_of_poles_it_serves() {
    let d = drawing();
    assert_eq!(
        pole_of_pin(d.doc.project(), "-A1:COM"),
        Err(PinProblem::Shared {
            device: "-A1".into(),
            pin: "COM".into(),
            poles: 2
        })
    );
}

#[test]
fn names_that_lead_nowhere_say_why() {
    let d = drawing();
    let p = d.doc.project();
    assert!(matches!(pole_of_pin(p, "-A1"), Err(PinProblem::NotAPin(_))));
    assert!(matches!(
        pole_of_pin(p, "-Z9:1"),
        Err(PinProblem::NoDevice(_))
    ));
    assert!(matches!(
        pole_of_pin(p, "-A1:99"),
        Err(PinProblem::NoPin { .. })
    ));
    // A coil pin exists but is not a switched pole.
    assert!(matches!(
        pole_of_pin(p, "-K1:A1"),
        Err(PinProblem::NotSwitched { .. })
    ));
    for text in ["-A1:99", "-K1:A1", "-A1:COM", "-Z9:1"] {
        assert!(!pole_of_pin(p, text).unwrap_err().to_string().is_empty());
    }
}

#[test]
fn all_poles_of_a_device_and_none_for_a_coil() {
    let d = drawing();
    let p = d.doc.project();
    assert_eq!(poles_of_device(p, "-A1").unwrap().len(), 2);
    assert_eq!(poles_of_device(p, "-S1").unwrap().len(), 1);
    assert!(matches!(
        poles_of_device(p, "-K1"),
        Err(PinProblem::NoPoles(_))
    ));
    let _ = d.coil;
}

#[test]
fn forcing_and_operating_poles() {
    let d = drawing();
    let rest = rest_state(d.doc.project());
    let mut state = rest.clone();
    let out = pole_of_pin(d.doc.project(), "-A1:1").unwrap();
    assert!(!state.is_closed(out.symbol, out.group));
    force_pole(&mut state, out, true);
    assert!(state.is_closed(out.symbol, out.group));
    force_pole(&mut state, out, false);
    assert!(!state.is_closed(out.symbol, out.group));

    // A break pushbutton rests closed; operated it opens, released it returns to rest.
    let button = poles_of_device(d.doc.project(), "-S1").unwrap();
    assert!(rest.is_closed(d.nc, 0));
    let mut state = PoleState::all_open();
    operate_poles(&mut state, &rest, &button, false);
    assert!(state.is_closed(d.nc, 0));
    operate_poles(&mut state, &rest, &button, true);
    assert!(!state.is_closed(d.nc, 0));
}
