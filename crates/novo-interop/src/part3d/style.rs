//! Colours and names that a STEP file carries for its solids.
//!
//! - **Names**: a solid sits in a shape representation that belongs to a
//!   `PRODUCT_DEFINITION`; that product's name (`door`, `mounting plate`) is
//!   the solid's name. Without one, the name of the B-rep itself.
//! - **Colours**: `STYLED_ITEM` points at a solid (or one of its faces, or a
//!   mapped assembly item) and at a presentation style whose chain ends in a
//!   `COLOUR_RGB`. The chain is followed through a fixed list of style
//!   entities only, so it never wanders into geometry.
//!
//! Pure reading; nothing here changes a file or touches the network.

use std::collections::{HashMap, HashSet};

use super::parse::{Param, StepFile};

const SOLIDS: [&str; 6] = [
    "MANIFOLD_SOLID_BREP",
    "BREP_WITH_VOIDS",
    "FACETED_BREP",
    "SHELL_BASED_SURFACE_MODEL",
    "TESSELLATED_SOLID",
    "TESSELLATED_SHELL",
];

const REPS: [&str; 6] = [
    "SHAPE_REPRESENTATION",
    "ADVANCED_BREP_SHAPE_REPRESENTATION",
    "FACETED_BREP_SHAPE_REPRESENTATION",
    "MANIFOLD_SURFACE_SHAPE_REPRESENTATION",
    "TESSELLATED_SHAPE_REPRESENTATION",
    "GEOMETRICALLY_BOUNDED_SURFACE_SHAPE_REPRESENTATION",
];

/// Style entities a colour chain may pass through.
const STYLE_CHAIN: [&str; 9] = [
    "PRESENTATION_STYLE_ASSIGNMENT",
    "PRESENTATION_STYLE_BY_CONTEXT",
    "SURFACE_STYLE_USAGE",
    "SURFACE_SIDE_STYLE",
    "SURFACE_STYLE_FILL_AREA",
    "FILL_AREA_STYLE",
    "FILL_AREA_STYLE_COLOUR",
    "SURFACE_STYLE_RENDERING",
    "SURFACE_STYLE_RENDERING_WITH_PROPERTIES",
];

/// One solid with what the file says about it.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidStyle {
    /// Entity id of the solid in the file.
    pub id: u32,
    pub name: String,
    pub colour: Option<[u8; 3]>,
}

/// Names and colours read from a STEP file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepStyle {
    /// Every product name, in file order, without repeats.
    pub product_names: Vec<String>,
    pub solids: Vec<SolidStyle>,
    /// The colour most solids have, if any has one.
    pub dominant: Option<[u8; 3]>,
}

impl StepStyle {
    /// The part's name: the first product that is not a generic wrapper
    /// (the file's `_ASM` top level is skipped when something else exists).
    pub fn part_name(&self) -> Option<&str> {
        self.product_names
            .iter()
            .find(|n| !n.to_ascii_uppercase().ends_with("_ASM"))
            .or_else(|| self.product_names.first())
            .map(String::as_str)
    }

    pub fn coloured_solids(&self) -> usize {
        self.solids.iter().filter(|s| s.colour.is_some()).count()
    }
}

/// `#rrggbb` of a colour.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn colour_entity(file: &StepFile, id: u32) -> Option<[u8; 3]> {
    let e = file.entity(id)?;
    match e.name() {
        "COLOUR_RGB" => {
            let a = e.args();
            Some([
                byte(a.get(1)?.as_f64()?),
                byte(a.get(2)?.as_f64()?),
                byte(a.get(3)?.as_f64()?),
            ])
        }
        "DRAUGHTING_PRE_DEFINED_COLOUR" => {
            match e.args().first()?.as_text()?.to_lowercase().as_str() {
                "red" => Some([255, 0, 0]),
                "green" => Some([0, 255, 0]),
                "blue" => Some([0, 0, 255]),
                "yellow" => Some([255, 255, 0]),
                "black" => Some([0, 0, 0]),
                "white" => Some([255, 255, 255]),
                "cyan" => Some([0, 255, 255]),
                "magenta" => Some([255, 0, 255]),
                _ => None,
            }
        }
        _ => None,
    }
}

fn param_refs(p: &Param, out: &mut Vec<u32>) {
    match p {
        Param::Ref(r) => out.push(*r),
        Param::List(l) | Param::Typed(_, l) => l.iter().for_each(|x| param_refs(x, out)),
        _ => {}
    }
}

/// The colour at the end of a style chain starting at `id`.
fn chain_colour(file: &StepFile, id: u32, depth: u32, seen: &mut HashSet<u32>) -> Option<[u8; 3]> {
    if depth > 10 || !seen.insert(id) {
        return None;
    }
    if let Some(c) = colour_entity(file, id) {
        return Some(c);
    }
    let e = file.entity(id)?;
    if !e
        .parts
        .iter()
        .any(|(n, _)| STYLE_CHAIN.contains(&n.as_str()))
    {
        return None;
    }
    let mut next = Vec::new();
    for (_, args) in &e.parts {
        args.iter().for_each(|p| param_refs(p, &mut next));
    }
    next.into_iter()
        .find_map(|r| chain_colour(file, r, depth + 1, seen))
}

fn first_text(args: &[Param], idx: &[usize]) -> Option<String> {
    idx.iter()
        .filter_map(|&i| args.get(i)?.as_text())
        .map(str::trim)
        .find(|t| !t.is_empty() && *t != "NOT SPECIFIED")
        .map(str::to_owned)
}

/// Reads names and colours.
pub fn read_style(file: &StepFile) -> StepStyle {
    let mut style = StepStyle::default();
    let mut seen_names = HashSet::new();
    for &id in file.ids_of("PRODUCT") {
        if let Some(e) = file.entity(id)
            && let Some(n) = first_text(e.args(), &[1, 0])
            && seen_names.insert(n.clone())
        {
            style.product_names.push(n);
        }
    }

    // Representation -> product name, through PRODUCT_DEFINITION_SHAPE.
    let product_of_definition = |def: u32| -> Option<String> {
        let pd = file.entity(def)?;
        let formation = pd.args().get(2)?.as_ref()?;
        let f = file.entity(formation)?;
        let product = f.args().get(2)?.as_ref()?;
        let p = file.entity(product)?;
        first_text(p.args(), &[1, 0])
    };
    let mut rep_name: HashMap<u32, String> = HashMap::new();
    for &id in file.ids_of("SHAPE_DEFINITION_REPRESENTATION") {
        let Some(e) = file.entity(id) else { continue };
        let (Some(pds), Some(rep)) = (
            e.args().first().and_then(Param::as_ref),
            e.args().get(1).and_then(Param::as_ref),
        ) else {
            continue;
        };
        let name = file
            .entity(pds)
            .and_then(|p| p.args().get(2).and_then(Param::as_ref))
            .and_then(product_of_definition);
        if let Some(n) = name {
            rep_name.insert(rep, n);
        }
    }
    // A B-rep representation tied to the named shape representation.
    for &id in file.ids_of("REPRESENTATION_RELATIONSHIP") {
        let Some(e) = file.entity(id) else { continue };
        let (Some(a), Some(b)) = (
            e.args().get(2).and_then(Param::as_ref),
            e.args().get(3).and_then(Param::as_ref),
        ) else {
            continue;
        };
        if let Some(n) = rep_name.get(&a).cloned() {
            rep_name.entry(b).or_insert(n);
        } else if let Some(n) = rep_name.get(&b).cloned() {
            rep_name.entry(a).or_insert(n);
        }
    }
    // Solid -> representation holding it.
    let mut rep_of: HashMap<u32, u32> = HashMap::new();
    let mut items_of_rep: HashMap<u32, Vec<u32>> = HashMap::new();
    for n in REPS {
        for &rid in file.ids_of(n) {
            if let Some(e) = file.entity(rid) {
                let items = e.args().get(1).map(Param::refs).unwrap_or_default();
                for &i in &items {
                    rep_of.entry(i).or_insert(rid);
                }
                items_of_rep.insert(rid, items);
            }
        }
    }

    // Faces of each solid, for colours given per face.
    let faces_of = |solid: u32| -> Vec<u32> {
        let Some(e) = file.entity(solid) else {
            return Vec::new();
        };
        let mut shells = e.args().get(1).map(|p| match p {
            Param::Ref(r) => vec![*r],
            other => other.refs(),
        });
        let mut faces = Vec::new();
        for s in shells.get_or_insert_with(Vec::new).drain(..) {
            if let Some(sh) = file.entity(s) {
                faces.extend(sh.args().get(1).map(Param::refs).unwrap_or_default());
            }
        }
        faces
    };

    // Colour given to each styled item.
    let mut item_colour: HashMap<u32, [u8; 3]> = HashMap::new();
    for name in ["STYLED_ITEM", "OVER_RIDING_STYLED_ITEM"] {
        for &id in file.ids_of(name) {
            let Some(e) = file.entity(id) else { continue };
            let (Some(styles), Some(item)) =
                (e.args().get(1), e.args().get(2).and_then(Param::as_ref))
            else {
                continue;
            };
            let mut refs = Vec::new();
            param_refs(styles, &mut refs);
            let colour = refs
                .into_iter()
                .find_map(|r| chain_colour(file, r, 0, &mut HashSet::new()));
            if let Some(c) = colour {
                // The overriding style wins over the plain one.
                if name == "OVER_RIDING_STYLED_ITEM" || !item_colour.contains_key(&item) {
                    item_colour.insert(item, c);
                }
            }
        }
    }
    // A colour on a mapped item reaches every solid of the mapped shape.
    let mut mapped_colour: HashMap<u32, [u8; 3]> = HashMap::new();
    for (&item, &c) in &item_colour {
        if let Some(e) = file.entity(item)
            && e.name() == "MAPPED_ITEM"
            && let Some(map) = e
                .args()
                .get(1)
                .and_then(Param::as_ref)
                .and_then(|m| file.entity(m))
            && let Some(rep) = map.args().get(1).and_then(Param::as_ref)
        {
            for &i in items_of_rep.get(&rep).into_iter().flatten() {
                mapped_colour.entry(i).or_insert(c);
            }
        }
    }

    let mut counts: HashMap<[u8; 3], usize> = HashMap::new();
    let mut order: Vec<[u8; 3]> = Vec::new();
    for n in SOLIDS {
        for &id in file.ids_of(n) {
            let Some(e) = file.entity(id) else { continue };
            let name = rep_of
                .get(&id)
                .and_then(|r| rep_name.get(r).cloned())
                .or_else(|| first_text(e.args(), &[0]))
                .unwrap_or_default();
            let mut colour = item_colour
                .get(&id)
                .or_else(|| mapped_colour.get(&id))
                .copied();
            if colour.is_none() && !item_colour.is_empty() {
                let mut face_counts: Vec<([u8; 3], usize)> = Vec::new();
                for f in faces_of(id) {
                    if let Some(&c) = item_colour.get(&f) {
                        match face_counts.iter_mut().find(|(k, _)| *k == c) {
                            Some((_, n)) => *n += 1,
                            None => face_counts.push((c, 1)),
                        }
                    }
                }
                colour = face_counts
                    .into_iter()
                    .max_by_key(|(_, n)| *n)
                    .map(|(c, _)| c);
            }
            if let Some(c) = colour {
                if !counts.contains_key(&c) {
                    order.push(c);
                }
                *counts.entry(c).or_default() += 1;
            }
            style.solids.push(SolidStyle { id, name, colour });
        }
    }
    style.dominant = order.into_iter().max_by_key(|c| counts[c]);
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str =
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n";
    const TAIL: &str = "ENDSEC;\nEND-ISO-10303-21;\n";

    fn style_of(body: &str) -> StepStyle {
        let text = format!("{HEAD}{body}{TAIL}");
        read_style(&StepFile::parse(&text).unwrap())
    }

    #[test]
    fn colour_chain_and_product_name() {
        let s = style_of(
            "#1=MANIFOLD_SOLID_BREP('body',#2);\n#2=CLOSED_SHELL('',());\n\
             #10=PRODUCT('P1','Mounting plate','',(#11));\n\
             #12=PRODUCT_DEFINITION_FORMATION('','',#10);\n\
             #13=PRODUCT_DEFINITION('design','',#12,#14);\n\
             #15=PRODUCT_DEFINITION_SHAPE('','',#13);\n\
             #16=SHAPE_REPRESENTATION('',(#1),#17);\n\
             #18=SHAPE_DEFINITION_REPRESENTATION(#15,#16);\n\
             #20=STYLED_ITEM('color',(#21),#1);\n\
             #21=PRESENTATION_STYLE_ASSIGNMENT((#22));\n\
             #22=SURFACE_STYLE_USAGE(.BOTH.,#23);\n\
             #23=SURFACE_SIDE_STYLE('',(#24));\n\
             #24=SURFACE_STYLE_FILL_AREA(#25);\n\
             #25=FILL_AREA_STYLE('',(#26));\n\
             #26=FILL_AREA_STYLE_COLOUR('',#27);\n\
             #27=COLOUR_RGB('',0.5,0.25,1.0);\n",
        );
        assert_eq!(s.solids.len(), 1);
        assert_eq!(s.solids[0].name, "Mounting plate");
        assert_eq!(s.solids[0].colour, Some([128, 64, 255]));
        assert_eq!(s.dominant, Some([128, 64, 255]));
        assert_eq!(s.part_name(), Some("Mounting plate"));
        assert_eq!(hex([128, 64, 255]), "#8040ff");
    }

    #[test]
    fn face_colours_give_the_solid_its_most_common_one() {
        let s = style_of(
            "#1=MANIFOLD_SOLID_BREP('body',#2);\n#2=CLOSED_SHELL('',(#3,#4,#5));\n\
             #3=ADVANCED_FACE('',(),#9,.T.);\n#4=ADVANCED_FACE('',(),#9,.T.);\n#5=ADVANCED_FACE('',(),#9,.T.);\n\
             #30=STYLED_ITEM('',(#40),#3);\n#31=STYLED_ITEM('',(#40),#4);\n#32=STYLED_ITEM('',(#41),#5);\n\
             #40=PRESENTATION_STYLE_ASSIGNMENT((#50));\n#41=PRESENTATION_STYLE_ASSIGNMENT((#51));\n\
             #50=SURFACE_STYLE_RENDERING(.NORMAL_SHADING.,#60);\n#51=SURFACE_STYLE_RENDERING(.NORMAL_SHADING.,#61);\n\
             #60=COLOUR_RGB('',1.,0.,0.);\n#61=COLOUR_RGB('',0.,1.,0.);\n",
        );
        assert_eq!(s.solids[0].colour, Some([255, 0, 0]));
        assert_eq!(s.solids[0].name, "body");
    }

    #[test]
    fn no_style_means_no_colour() {
        let s = style_of("#1=MANIFOLD_SOLID_BREP('',#2);\n#2=CLOSED_SHELL('',());\n");
        assert_eq!(s.solids[0].colour, None);
        assert_eq!(s.dominant, None);
        assert_eq!(s.part_name(), None);
    }

    #[test]
    fn asm_wrapper_is_not_the_part_name() {
        let s =
            style_of("#1=PRODUCT('A_ASM','A_ASM','',(#9));\n#2=PRODUCT('A','A rail','',(#9));\n");
        assert_eq!(s.part_name(), Some("A rail"));
    }
}
