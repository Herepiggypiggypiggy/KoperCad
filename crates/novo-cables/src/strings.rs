//! Every user-facing string of the crate, in one place for translation
//! (Dutch first, DESIGN.md §3).

pub const SHIELDED_SUFFIX: &str = " shielded";
pub const SHIELD_ON_PE: &str = "PE";
pub const SPARE: &str = "spare ";
pub const FREE: &str = "free ";

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

pub fn cables_proposed(count: usize, list: &str) -> String {
    format!("{}: {list}", plural(count, "cable", "cables"))
}

pub fn spare_in_use(cable: &str, core: &str) -> String {
    format!("{cable}: core {core} is marked spare but carries a wire")
}

pub fn protective_core_misused(cable: &str, core: &str) -> String {
    format!("{cable}: green-yellow core {core} carries a conductor that is not PE")
}

pub fn protective_on_other_core(cable: &str, core: &str) -> String {
    format!("{cable}: PE runs on core {core}; it belongs on the green-yellow core")
}

pub fn core_count_exceeded(cable: &str, unassigned: usize, free: usize) -> String {
    format!(
        "{cable}: {} between its ends but only {}",
        plural(unassigned, "more wire runs", "more wires run"),
        plural(free, "free core", "free cores")
    )
}

pub fn more_cores_than_part(cable: &str, cores: usize, part_cores: usize) -> String {
    format!("{cable}: {cores} cores, but its part has {part_cores}")
}

pub fn shield_unconnected(cable: &str, core: &str) -> String {
    format!("{cable}: shield {core} is not connected")
}

pub fn undersized_core(cable: &str, core: &str) -> String {
    format!("{cable}: core {core} is thinner than its wire")
}
