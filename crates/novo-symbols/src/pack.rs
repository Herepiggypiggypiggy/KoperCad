//! Symbol packs: one drawing standard per directory under `symbols/`.
//!
//! The same path below the pack root is the same device in every pack, so a
//! symbol and its counterpart agree on function, behavior and connection
//! designations and differ only in artwork (see `docs/design/symbol-packs.md`).

/// A drawing standard shipped with the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pack {
    /// IEC 60617, the default.
    Iec,
    /// NFPA 79 / ANSI Y32.2 / IEEE 315 (JIC style).
    Nfpa,
}

impl Pack {
    /// Every pack, in the order a picker offers them.
    pub const ALL: [Pack; 2] = [Pack::Iec, Pack::Nfpa];

    /// The directory and id prefix of the pack: `iec`, `nfpa`.
    pub fn id(self) -> &'static str {
        match self {
            Pack::Iec => "iec",
            Pack::Nfpa => "nfpa",
        }
    }

    /// The pack with directory name `id`.
    pub fn from_id(id: &str) -> Option<Pack> {
        Pack::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The standard the pack follows, as written in a symbol's `standard`.
    pub fn standard(self) -> &'static str {
        match self {
            Pack::Iec => "IEC 60617",
            Pack::Nfpa => "NFPA 79 / ANSI Y32.2",
        }
    }

    /// The pack a symbol id belongs to (`nfpa/contacts/no` is `Nfpa`).
    pub fn of(symbol_id: &str) -> Option<Pack> {
        symbol_id.split('/').next().and_then(Pack::from_id)
    }

    /// The id of the same device in `pack`: `counterpart("iec/contacts/no",
    /// Nfpa)` is `nfpa/contacts/no`. The symbol may not exist there; ask the
    /// library. `None` when `symbol_id` is in no known pack.
    pub fn counterpart(symbol_id: &str, pack: Pack) -> Option<String> {
        Pack::of(symbol_id)?;
        let (_, rest) = symbol_id.split_once('/')?;
        Some(format!("{}/{rest}", pack.id()))
    }

    /// The id below the pack root: `path_of("nfpa/contacts/no")` and
    /// `path_of("iec/contacts/no")` are both `contacts/no`. An id in no
    /// known pack (a company library) is returned unchanged. Engines that
    /// recognise a device by its key compare this, so a symbol of either
    /// pack is found.
    pub fn path_of(symbol_id: &str) -> &str {
        match symbol_id.split_once('/') {
            Some((first, rest)) if Pack::from_id(first).is_some() => rest,
            _ => symbol_id,
        }
    }

    /// Whether two ids are the same device drawn in the same or different
    /// packs (`iec/contacts/no` and `nfpa/contacts/no`).
    pub fn same_device(a: &str, b: &str) -> bool {
        Pack::path_of(a) == Pack::path_of(b)
    }
}
