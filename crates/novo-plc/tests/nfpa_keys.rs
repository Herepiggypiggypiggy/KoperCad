//! Channel symbols of the NFPA pack are known to the PLC engine.

use novo_core::SymbolKey;
use novo_plc::{FromKey, Functions};
use novo_symbols::{Library, Pack};

#[test]
fn nfpa_channel_symbols_have_the_function_of_their_iec_twin() {
    for name in [
        "digital_input",
        "digital_output",
        "relay_output",
        "analog_input",
        "analog_output",
    ] {
        let nfpa = SymbolKey(format!("nfpa/plc/{name}"));
        let iec = SymbolKey(format!("iec/plc/{name}"));
        let expected = FromKey.function(&iec).unwrap();
        assert_eq!(FromKey.function(&nfpa), Some(expected), "{name}");
        // The library agrees, so a project can use either source.
        let library = Library::builtin_pack(Pack::Nfpa).unwrap();
        assert_eq!(library.function(&nfpa), Some(expected), "{name}");
    }
}
