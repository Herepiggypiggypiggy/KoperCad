//! Counts in words: "1 wire", "2 wires".

/// `1 wire`, `0 wires`, `3 wires`.
pub(super) fn wires(n: usize) -> String {
    format!("{n} {}", if n == 1 { "wire" } else { "wires" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_wire_is_singular() {
        assert_eq!(wires(1), "1 wire");
        assert_eq!(wires(0), "0 wires");
        assert_eq!(wires(2), "2 wires");
    }
}
