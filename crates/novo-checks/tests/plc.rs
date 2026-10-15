//! PLC problems (novo-plc, plc.md §7) in the Issues list.

use novo_checks::{ProblemKind, Severity, check};
use novo_core::{AnyId, DeviceKind, DeviceTag, Document, Pin, Placement, Point, SymbolRole, Value};
use novo_plc::card::{CHANNEL, CHANNELS, SLOT, START_INPUT};

#[test]
fn a_channel_drawn_twice_is_an_error_and_unwired_channels_are_not_repeated() {
    let mut doc = Document::default();
    let ((a, b, unnumbered), _) = doc
        .transact("PLC", |tx| {
            let page = tx.add_page("1")?;
            let card = tx.add_device_of_kind(DeviceTag::parse("-K1")?, "", DeviceKind::Plc)?;
            tx.set_property(card, CHANNELS, Some(Value::from("DI8 DQ8")))?;
            tx.set_property(card, SLOT, Some(Value::Int(1)))?;
            tx.set_property(card, START_INPUT, Some(Value::Int(4)))?;
            tx.set_property(card, "plc.start.q", Some(Value::Int(0)))?;
            let mut place = |key: &str, channel: Option<&str>, x: f64| {
                let symbol = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 50.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: card,
                        main: false,
                    },
                )?;
                if let Some(c) = channel {
                    tx.set_property(symbol, CHANNEL, Some(Value::from(c)))?;
                }
                Ok::<_, novo_core::ModelError>(symbol)
            };
            let a = place("iec/plc/digital_input", Some("DI 1"), 10.0)?;
            let b = place("iec/plc/digital_input", Some("DI 1"), 20.0)?;
            let unnumbered = place("iec/plc/digital_output", None, 30.0)?;
            Ok((a, b, unnumbered))
        })
        .expect("plc");
    let problems = check(doc.project());
    let plc: Vec<_> = problems
        .iter()
        .filter(|p| p.kind == ProblemKind::Plc)
        .collect();
    assert_eq!(plc.len(), 2, "{plc:#?}");
    let twice = plc
        .iter()
        .find(|p| p.at == vec![AnyId::from(a), AnyId::from(b)])
        .expect("drawn twice");
    assert_eq!(twice.severity, Severity::Error);
    assert!(twice.message.contains("-K1"), "{}", twice.message);
    assert!(!twice.message.ends_with('.'));
    assert!(plc.iter().any(|p| p.at == vec![AnyId::from(unnumbered)]));
    // Unwired channels show once, as unconnected points, not again as PLC
    // problems.
    let unconnected = problems
        .iter()
        .filter(|p| p.kind == ProblemKind::UnconnectedPin)
        .count();
    assert_eq!(unconnected, 3, "{problems:#?}");
}
