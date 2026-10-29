//! Descriptions written into a project from part data by recipe.

use novo_core::{Command, DeviceTag, Document, Project, Value};
use novo_parts::{
    AssignMode, CatalogPart, RecipeSet, assign_part, describe_devices, describe_parts,
};

fn contactor() -> CatalogPart {
    let mut p = CatalogPart::new("Acme", "C-09");
    p.function = "contactor".into();
    p.description = "Contactor 9A".into();
    p.data.insert("poles".into(), Value::Int(3));
    p.data.insert("ac3_current_a".into(), Value::Float(9.0));
    p.data.insert("aux_contacts_no".into(), Value::Int(1));
    p.data.insert("aux_contacts_nc".into(), Value::Int(0));
    p
}

#[test]
fn devices_get_their_description_in_one_undo_step() {
    let mut doc = Document::new(Project::new(""));
    let (k1, k2) = doc
        .transact("Add", |tx| {
            let a = tx.add_device(DeviceTag::parse("-K1").unwrap(), "")?;
            let b = tx.add_device(DeviceTag::parse("-K2").unwrap(), "Hand written")?;
            Ok((a, b))
        })
        .unwrap()
        .0;
    let assign = assign_part(doc.project(), &[k1, k2], &contactor(), AssignMode::Replace).unwrap();
    doc.execute(assign).unwrap();

    let set = RecipeSet::english();
    let command = describe_devices(doc.project(), &[k1, k2], &set, false).unwrap();
    doc.execute(command).unwrap();
    assert_eq!(
        doc.project().device(k1).unwrap().description,
        "3-pole contactor; 9 A; 3M/1m/0b"
    );
    // Text somebody wrote stays unless Replace is chosen.
    assert_eq!(
        doc.project().device(k2).unwrap().description,
        "Hand written"
    );
    // Nothing left to write.
    assert!(describe_devices(doc.project(), &[k1, k2], &set, false).is_none());

    let all = describe_devices(doc.project(), &[k1, k2], &set, true).unwrap();
    doc.execute(all).unwrap();
    assert_eq!(
        doc.project().device(k2).unwrap().description,
        "3-pole contactor; 9 A; 3M/1m/0b"
    );
    doc.undo().unwrap();
    assert_eq!(
        doc.project().device(k2).unwrap().description,
        "Hand written"
    );
}

#[test]
fn project_parts_get_descriptions_for_the_bom() {
    let mut doc = Document::new(Project::new(""));
    let (k1, _) = doc
        .transact("Add", |tx| {
            tx.add_device(DeviceTag::parse("-K1").unwrap(), "")
        })
        .unwrap();
    doc.execute(assign_part(doc.project(), &[k1], &contactor(), AssignMode::Replace).unwrap())
        .unwrap();
    let set = RecipeSet::english();
    // The maker's own text is kept unless asked.
    assert!(describe_parts(doc.project(), &set, false).is_none());
    let command: Command = describe_parts(doc.project(), &set, true).unwrap();
    doc.execute(command).unwrap();
    let part = doc.project().parts().next().unwrap();
    assert_eq!(part.description, "3-pole contactor; 9 A; 3M/1m/0b");
}
