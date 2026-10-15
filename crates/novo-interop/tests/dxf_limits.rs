//! DXF reader hardening (WS-24).

use novo_interop::dxf::read_dxf;

fn file(blocks: &str, entities: &str) -> String {
    format!(
        "0\nSECTION\n2\nBLOCKS\n{blocks}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    )
}

#[test]
fn insert_arrays_warn() {
    let block =
        "0\nBLOCK\n2\nB\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n";
    let ins = "0\nINSERT\n8\n0\n2\nB\n70\n3\n71\n2\n";
    let r = read_dxf(file(block, ins).as_bytes()).unwrap();
    assert_eq!(r.drawing.entities.len(), 1);
    assert!(r.warnings.iter().any(|w| w.contains("array (3 x 2)")));
}

#[test]
fn exploding_nested_inserts_is_capped() {
    // Each level inserts the next ten times: 10^8 lines without a cap.
    let mut blocks = String::from(
        "0\nBLOCK\n2\nL0\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n",
    );
    for level in 1..8 {
        blocks.push_str(&format!("0\nBLOCK\n2\nL{level}\n10\n0\n20\n0\n"));
        for _ in 0..10 {
            blocks.push_str(&format!("0\nINSERT\n8\n0\n2\nL{}\n", level - 1));
        }
        blocks.push_str("0\nENDBLK\n");
    }
    let r = read_dxf(file(&blocks, "0\nINSERT\n8\n0\n2\nL7\n").as_bytes()).unwrap();
    assert!(r.drawing.entities.len() <= 2_000_000 + 10);
    assert!(r.warnings.iter().any(|w| w.contains("entities")));
}
