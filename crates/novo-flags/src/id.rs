use serde::{Deserialize, Serialize};

/// A stable identity: a hash of (source, rule key, subjects). The same problem
/// found again by a re-check gets the same id, so waivers, the "new flag" pulse
/// and the F8 position survive recomputes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FlagId(pub u64);

impl FlagId {
    /// Hash the parts with FNV-1a (fixed algorithm: ids may be stored in files).
    pub fn of<'a>(parts: impl IntoIterator<Item = &'a str>) -> FlagId {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for part in parts {
            for b in part.bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            // Separator so ("ab","c") differs from ("a","bc").
            h ^= 0xff;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        FlagId(h)
    }
}
