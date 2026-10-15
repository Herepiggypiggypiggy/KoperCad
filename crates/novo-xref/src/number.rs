//! Automatic names for interruption points.
//!
//! A new interruption point takes the lowest whole number no other
//! interruption point uses, so the first pair on a project is `1`, the next
//! `2`. The partner on another page takes the name when it is linked
//! ([`crate::link_command`]), so numbering never has to be typed.

use std::collections::HashSet;

use novo_core::{Project, Result, SymbolId, SymbolRole, Tx};

/// The lowest whole number no interruption point is named with yet.
pub fn next_interruption_name(project: &Project) -> String {
    let used: HashSet<u64> = project
        .symbols()
        .filter_map(|s| match &s.role {
            SymbolRole::Interruption { name } => name.parse().ok(),
            _ => None,
        })
        .collect();
    (1..).find(|n| !used.contains(n)).unwrap().to_string()
}

/// Names `symbol` with the next free number when it is an interruption point
/// without a name. Returns the new name, or `None` when nothing changed.
pub fn auto_name_interruption(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<Option<String>> {
    let unnamed = matches!(
        tx.project().symbol(symbol).map(|s| &s.role),
        Some(SymbolRole::Interruption { name }) if name.is_empty()
    );
    if !unnamed {
        return Ok(None);
    }
    let name = next_interruption_name(tx.project());
    tx.set_role(symbol, SymbolRole::Interruption { name: name.clone() })?;
    Ok(Some(name))
}

/// Names every unnamed interruption point, in symbol order. Returns the
/// symbols named.
pub fn auto_name_interruptions(tx: &mut Tx<'_>) -> Result<Vec<SymbolId>> {
    let mut unnamed: Vec<SymbolId> = tx
        .project()
        .symbols()
        .filter(|s| matches!(&s.role, SymbolRole::Interruption { name } if name.is_empty()))
        .map(|s| s.id)
        .collect();
    unnamed.sort();
    for &id in &unnamed {
        auto_name_interruption(tx, id)?;
    }
    Ok(unnamed)
}
