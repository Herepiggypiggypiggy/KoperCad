//! A platform-independent golden for the demo page: the shape of the scene
//! and its tessellated mesh, with no GPU and no pixels. A change that moves
//! these numbers changes what is drawn; review it, then update the numbers.

use novo_canvas::Tessellator;
use novo_canvas::model::{PageScene, PlaceholderArt, demo_document};

#[test]
fn demo_page_scene_is_stable() {
    let (doc, page) = demo_document().unwrap();
    let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
    let scene = ps.scene();
    let mesh = Tessellator::default().scene(scene);
    let b = scene.bounds();
    eprintln!(
        "items {} bounds {:?}..{:?} vertices {} indices {}",
        scene.iter().count(),
        b.min,
        b.max,
        mesh.mesh.vertices.len(),
        mesh.mesh.indices.len()
    );
    // Item count and bounds do not depend on the platform's float
    // rounding; exact vertex counts might (libm in curve tessellation).
    assert_eq!(scene.iter().count(), 22);
    assert_eq!((b.max.x - b.min.x).round(), 420.0);
    assert_eq!((b.max.y - b.min.y).round(), 297.0);
    assert!(!mesh.mesh.vertices.is_empty() && !mesh.mesh.indices.is_empty());
    // Run twice: the build is deterministic.
    let again = PageScene::new(doc.project(), page, &PlaceholderArt);
    let mesh2 = Tessellator::default().scene(again.scene());
    assert_eq!(mesh.mesh.vertices.len(), mesh2.mesh.vertices.len());
    assert_eq!(mesh.mesh.indices, mesh2.mesh.indices);
    assert_eq!(mesh.mesh.vertices, mesh2.mesh.vertices);
}
