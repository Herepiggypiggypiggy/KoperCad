//! Pin designations: EPLAN pin text against a KoperCAD symbol's connections.

use std::collections::BTreeMap;

use novo_symbols::Symbol;

/// How an EPLAN device's pins land on a symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinMap {
    /// `(EPLAN pin, KoperCAD connection id)` in the EPLAN order.
    pub pairs: Vec<(String, String)>,
    /// EPLAN pins that found no connection; they are reported, never dropped.
    pub unmapped: Vec<String>,
    /// The pins were matched by position because no designation agreed.
    pub positional: bool,
    /// Connections of the symbol no EPLAN pin reached.
    pub spare: Vec<String>,
}

impl PinMap {
    pub fn is_complete(&self) -> bool {
        self.unmapped.is_empty()
    }

    /// The connection id for an EPLAN pin.
    pub fn connection(&self, eplan_pin: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(p, _)| p == eplan_pin)
            .map(|(_, c)| c.as_str())
    }
}

/// Matches `pins` to the connections of `symbol`: the same designation first
/// (case and spaces ignored), then the alias set, then a repeated name takes
/// the symbol's `<name>2` (the second PE of a drive), and when nothing agrees
/// but the counts do, by position.
pub fn match_pins(
    pins: &[String],
    symbol: &Symbol,
    aliases: Option<&BTreeMap<String, String>>,
) -> PinMap {
    let key = |s: &str| s.split_whitespace().collect::<String>().to_uppercase();
    let conns: Vec<String> = symbol.connections.iter().map(|c| c.id.clone()).collect();
    let mut used = vec![false; conns.len()];
    let find = |wanted: &str, used: &[bool]| {
        conns
            .iter()
            .enumerate()
            .position(|(i, c)| !used[i] && key(c) == key(wanted))
    };

    let mut map = PinMap::default();
    for pin in pins {
        let hit = find(pin, &used)
            .or_else(|| {
                aliases
                    .and_then(|a| a.get(&key(pin)))
                    .and_then(|t| find(t, &used))
            })
            .or_else(|| find(&format!("{}2", key(pin)), &used));
        match hit {
            Some(i) => {
                used[i] = true;
                map.pairs.push((pin.clone(), conns[i].clone()));
            }
            None => map.unmapped.push(pin.clone()),
        }
    }
    if map.pairs.is_empty() && !pins.is_empty() && pins.len() == conns.len() {
        map.pairs = pins.iter().cloned().zip(conns.iter().cloned()).collect();
        map.unmapped.clear();
        map.positional = true;
        used.iter_mut().for_each(|u| *u = true);
    }
    map.spare = conns
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(c, _)| c.clone())
        .collect();
    map
}
