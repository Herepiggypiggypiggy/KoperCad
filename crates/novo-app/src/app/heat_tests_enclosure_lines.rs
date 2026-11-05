use super::heat::is_enclosure_line;
use novo_layout::spec::Source;
use novo_thermal::bridge::BridgeReport;

/// The lines the real bridge produces for a chosen enclosure: the app must
/// recognise exactly those as caveats and leave missing-data lines visible.
#[test]
fn enclosure_caveats_are_help_lines_but_missing_data_stays_visible() {
    let report = BridgeReport {
        enclosure_source: Some(Source::Catalogue),
        enclosure_article: "8455.600".into(),
        enclosure_estimate: true,
        ..BridgeReport::default()
    };
    let plain = report.messages();
    let all = report.messages_with_enclosure();
    let caveats: Vec<&String> = all.iter().filter(|m| !plain.contains(m)).collect();
    assert_eq!(
        caveats.len(),
        2,
        "source line and estimate sentence: {all:?}"
    );
    assert!(caveats.iter().all(|m| is_enclosure_line(m)));
    assert!(plain.iter().all(|m| !is_enclosure_line(m)));
    let missing = BridgeReport {
        placed: 1,
        without_loss: vec![novo_thermal::bridge::DeviceRef {
            id: 1,
            tag: "K1".into(),
        }],
        ..BridgeReport::default()
    };
    assert!(
        missing
            .messages_with_enclosure()
            .iter()
            .all(|m| !is_enclosure_line(m))
    );
}
