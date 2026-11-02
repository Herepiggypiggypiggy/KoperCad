//! The body of the Place rules panel: the rule set as plain-language cards.
//!
//! Every setting of `place-rules.txt` has a control here with its explanation
//! behind a "?" and one sentence that says what the rule does. Edits apply to
//! `Place all` at once and can be undone; the text file stays available under
//! "Edit as text" for people who want it.

use eframe::egui;
use novo_layout::heat::HeatPlace;
use novo_layout::rules::{Class, HotRule, KindRule, RailUse, RuleSet, Zone};
use novo_layout::surface_rules::{SurfaceRule, SurfaceRules};

use super::dialog_ui;

/// The name of one letter code (IEC 81346 kind letters) in plain words.
pub(super) fn code_name(code: &str) -> &'static str {
    match code.to_ascii_uppercase().as_str() {
        "Q" => "circuit breakers and switches",
        "F" => "fuses and protection",
        "G" => "power supplies",
        "A" => "controllers and modules",
        "K" => "relays and contactors",
        "T" => "drives and transformers",
        "U" => "converters",
        "X" => "terminals",
        "S" => "operator switches",
        "H" => "lamps and signals",
        "M" => "motors",
        "P" => "meters",
        "B" => "sensors",
        "Y" => "valves and actuators",
        "W" => "cables",
        "R" => "resistors",
        "C" => "capacitors",
        "*" => "everything else",
        _ => "devices",
    }
}

/// The title of a rule card: the names of its codes, e.g. "Circuit breakers
/// and switches, fuses and protection".
pub(super) fn rule_title(rule: &KindRule) -> String {
    let mut names: Vec<&str> = Vec::new();
    for code in &rule.codes {
        let name = code_name(code);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let joined = names.join(", ");
    let mut chars = joined.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "New kind".to_owned(),
    }
}

fn zone_words(zone: Zone) -> &'static str {
    match zone {
        Zone::Top => "the top of the plate",
        Zone::Middle => "the middle of the plate",
        Zone::Bottom => "the bottom of the plate",
    }
}

/// One sentence for a rule, the way a person would say it.
pub(super) fn rule_sentence(rule: &KindRule) -> String {
    let codes = rule.codes.join(", ");
    let rail = match rule.rail {
        RailUse::Own => "on rails of their own",
        RailUse::Shared => "on rails shared with other shared kinds",
    };
    let class = match rule.class {
        Class::Power => " They carry power, so their wiring duct is kept apart from signal ducts.",
        Class::Signal => {
            " They carry signals, so their wiring duct is kept apart from power ducts."
        }
        Class::Mixed => " They fit next to a power or a signal duct.",
    };
    let mut spacing = String::new();
    if let Some(g) = rule.gap {
        spacing.push_str(&format!(" {g:.0} mm between neighbours."));
    }
    match (rule.above, rule.below) {
        (Some(a), Some(b)) if (a - b).abs() < 0.01 => {
            spacing.push_str(&format!(" {a:.0} mm of room above and below."));
        }
        (a, b) => {
            if let Some(a) = a {
                spacing.push_str(&format!(" {a:.0} mm of room above."));
            }
            if let Some(b) = b {
                spacing.push_str(&format!(" {b:.0} mm of room below."));
            }
        }
    }
    format!(
        "Devices with letter {codes} go to {}, {rail}.{class}{spacing}",
        zone_words(rule.zone)
    )
}

fn mm_drag(ui: &mut egui::Ui, value: &mut f64, max: f64) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .range(0.0..=max)
            .speed(0.5)
            .suffix(" mm"),
    )
    .changed()
}

/// A setting that is either the default or a number: a check box and, when on,
/// the number.
fn optional_mm(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut Option<f64>,
    default: f64,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut own = value.is_some();
        if ui.checkbox(&mut own, label).changed() {
            *value = own.then_some(default);
            changed = true;
        }
        if let Some(v) = value {
            changed |= mm_drag(ui, v, 500.0);
        } else {
            ui.weak(format!("standard ({default:.0} mm)"));
        }
        dialog_ui::help(ui, help);
    });
    changed
}

/// A setting with its choices on one row: name and ?, then the choices.
fn row_choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    ui.horizontal(|ui| {
        ui.add_sized([86.0, 20.0], egui::Label::new(label));
        let mut index = options.iter().position(|(v, _)| v == value).unwrap_or(0);
        let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
        if novo_theme::widgets::segmented(ui, &mut index, &labels) {
            *value = options[index].0;
        }
        dialog_ui::help(ui, help);
    });
}

fn zone_choice(ui: &mut egui::Ui, zone: &mut Zone) -> bool {
    let before = *zone;
    row_choice(
        ui,
        "Goes to",
        "Place all fills the plate from the top down. A kind in the top zone gets the first rows, \
         the bottom zone the last. Heavy and hot parts usually go low, protection high.",
        zone,
        &[
            (Zone::Top, "Top"),
            (Zone::Middle, "Middle"),
            (Zone::Bottom, "Bottom"),
        ],
    );
    *zone != before
}

fn card(ui: &mut egui::Ui, index: usize, count: usize, rule: &mut KindRule) -> Edit {
    let mut edit = Edit::None;
    let mut changed = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(egui::RichText::new(rule_title(rule)).strong()).wrap());
        ui.horizontal(|ui| {
            if ui
                .small_button("Delete")
                .on_hover_text("Remove this rule")
                .clicked()
            {
                edit = Edit::Delete(index);
            }
            let down = ui
                .add_enabled(index + 1 < count, egui::Button::new("Down").small())
                .on_hover_text(
                    "Later rules come later on a shared rail. Move a rule to change the order.",
                )
                .on_disabled_hover_text("This is the last rule");
            if down.clicked() {
                edit = Edit::Move(index, 1);
            }
            let up = ui
                .add_enabled(index > 0, egui::Button::new("Up").small())
                .on_hover_text("Move this rule earlier")
                .on_disabled_hover_text("This is the first rule");
            if up.clicked() {
                edit = Edit::Move(index, -1);
            }
        });
        dialog_ui::hint(ui, &rule_sentence(rule));
        ui.add_space(4.0);

        // The letter codes, as a short text field.
        let id = ui.id().with(("codes", index));
        let mut buffer: String = ui
            .data(|d| d.get_temp(id))
            .unwrap_or_else(|| rule.codes.join(", "));
        ui.horizontal(|ui| {
            ui.label("Letters");
            let r = ui.add(egui::TextEdit::singleline(&mut buffer).desired_width(120.0));
            if r.changed() {
                let codes: Vec<String> = buffer
                    .split([',', ' ', ';'])
                    .map(|c| c.trim().to_ascii_uppercase())
                    .filter(|c| !c.is_empty())
                    .collect();
                if !codes.is_empty() {
                    rule.codes = codes;
                    changed = true;
                }
            }
            if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, buffer.clone()));
            } else {
                ui.data_mut(|d| d.remove_temp::<String>(id));
            }
            dialog_ui::help(
                ui,
                "The first letters of the device tags this rule is for, e.g. Q for -Q1 and -Q12, \
                 QA for -QA3 only. Separate letters with commas. * is every kind not named by \
                 another rule. A device follows the rule with the longest matching letters.",
            );
        });

        changed |= zone_choice(ui, &mut rule.zone);
        let before = rule.rail;
        row_choice(
            ui,
            "Rail",
            "Own: this kind never shares a rail with another kind, every kind gets its own rows. \
             Shared: devices of shared kinds in the same zone fill rails together, in the order \
             of the rules below.",
            &mut rule.rail,
            &[(RailUse::Own, "Own rail"), (RailUse::Shared, "Shared rail")],
        );
        changed |= rule.rail != before;
        let before = rule.class;
        row_choice(
            ui,
            "Carries",
            "Power rows and signal rows are kept apart (see Wiring ducts below): a power duct \
             never serves a signal row. Choose Both for terminals, which take either.",
            &mut rule.class,
            &[
                (Class::Power, "Power"),
                (Class::Signal, "Signal"),
                (Class::Mixed, "Both"),
            ],
        );
        changed |= rule.class != before;

        egui::CollapsingHeader::new("Spacing for this kind")
            .id_salt(("spacing", index))
            .default_open(rule.gap.is_some() || rule.above.is_some() || rule.below.is_some())
            .show(ui, |ui| {
                changed |= optional_mm(
                    ui,
                    "Space between devices",
                    "Free room between neighbours along the rail. Left off, the standard space \
                     of the whole plate is used.",
                    &mut rule.gap,
                    20.0,
                );
                changed |= optional_mm(
                    ui,
                    "Room above the row",
                    "Free room above this kind's rows, e.g. for a drive's cooling air or for \
                     cables to bend. Never less than the standard space between rows.",
                    &mut rule.above,
                    40.0,
                );
                changed |= optional_mm(
                    ui,
                    "Room below the row",
                    "Free room below this kind's rows.",
                    &mut rule.below,
                    40.0,
                );
            });
    });
    ui.add_space(6.0);
    if changed && matches!(edit, Edit::None) {
        edit = Edit::Changed;
    }
    edit
}

enum Edit {
    None,
    Changed,
    Delete(usize),
    Move(usize, i32),
}

fn number(ui: &mut egui::Ui, label: &str, help: &str, value: &mut f64, max: f64) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed = mm_drag(ui, value, max);
        dialog_ui::help(ui, help);
    });
    changed
}

fn plain_number(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    value: &mut f64,
    suffix: &str,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed = ui
            .add(
                egui::DragValue::new(value)
                    .range(range)
                    .speed(0.5)
                    .suffix(suffix),
            )
            .changed();
        dialog_ui::help(ui, help);
    });
    changed
}

/// A new rule, put before the "everything else" rule so that one stays last.
fn add_rule(rules: &mut RuleSet) {
    let rule = KindRule {
        codes: vec!["Z".to_owned()],
        zone: Zone::Middle,
        rail: RailUse::Shared,
        gap: None,
        above: None,
        below: None,
        class: Class::Mixed,
    };
    let at = rules
        .rules
        .iter()
        .position(|r| r.codes.iter().any(|c| c == "*"))
        .unwrap_or(rules.rules.len());
    rules.rules.insert(at, rule);
}

/// Draws the rules. True when the rule set changed.
pub(super) fn show(
    ui: &mut egui::Ui,
    rules: &mut RuleSet,
    surfaces: Option<&SurfaceRules>,
) -> (bool, Option<Option<String>>) {
    let mut changed = false;
    let mut surface_edit = None;
    dialog_ui::hint(
        ui,
        "Place all builds the plate from the top down, one row of devices per rail. \
         Each card says where one kind of device goes. Changes apply to the next \
         Place all at once; Save keeps them as your own defaults.",
    );

    dialog_ui::section(ui, "Where each kind goes", true);
    let count = rules.rules.len();
    let mut pending = Edit::None;
    for index in 0..count {
        let e = card(ui, index, count, &mut rules.rules[index]);
        if !matches!(e, Edit::None) {
            pending = e;
        }
    }
    match pending {
        Edit::None => {}
        Edit::Changed => changed = true,
        Edit::Delete(i) => {
            rules.rules.remove(i);
            changed = true;
        }
        Edit::Move(i, by) => {
            let j = i as i32 + by;
            if j >= 0 && (j as usize) < rules.rules.len() {
                rules.rules.swap(i, j as usize);
                changed = true;
            }
        }
    }
    if ui.button("+ Add a kind of device").clicked() {
        add_rule(rules);
        changed = true;
    }

    if let Some(surfaces) = surfaces {
        surface_edit = show_surfaces(ui, surfaces);
    }

    dialog_ui::section(ui, "Spacing", false);
    changed |= number(
        ui,
        "Space between devices",
        "The free room between two neighbours on a rail, unless a kind asks for its own.",
        &mut rules.gap,
        200.0,
    );
    changed |= number(
        ui,
        "Space between rows",
        "The least room between two rails. A wiring duct needs it, so very small values leave \
         no place for wires.",
        &mut rules.row_gap,
        400.0,
    );

    dialog_ui::section(ui, "Wiring ducts", false);
    changed |= number(
        ui,
        "Keep power and signal apart",
        "The distance kept between a power duct and a signal duct, to limit interference. \
         0 switches the separation off. When the plate is too small, Place all gives it up \
         before leaving a device off and says so.",
        &mut rules.separation,
        300.0,
    );
    ui.horizontal(|ui| {
        ui.label("Duct size");
        changed |= ui
            .add(
                egui::DragValue::new(&mut rules.duct_width)
                    .range(5.0..=200.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.label("wide");
        changed |= ui
            .add(
                egui::DragValue::new(&mut rules.duct_height)
                    .range(5.0..=200.0)
                    .suffix(" mm"),
            )
            .changed();
        ui.label("deep");
        dialog_ui::help(
            ui,
            "The cross-section of the ducts Place all draws. Route all checks how full they get.",
        );
    });

    dialog_ui::section(ui, "Hot devices", false);
    ui.horizontal(|ui| {
        let mut on = rules.hot.is_some();
        if ui.checkbox(&mut on, "Move hot devices").changed() {
            rules.hot = on.then_some(HotRule {
                min_loss_w: 25.0,
                zone: Zone::Middle,
            });
            changed = true;
        }
        dialog_ui::help(
            ui,
            "A device that gives off at least this much heat (power loss from its part data) goes \
             to the chosen zone whatever its kind, with extra room on each side.",
        );
    });
    if let Some(hot) = &mut rules.hot {
        changed |= plain_number(
            ui,
            "From a power loss of",
            "Devices at or above this loss count as hot.",
            &mut hot.min_loss_w,
            " W",
            0.0..=2000.0,
        );
        changed |= zone_choice(ui, &mut hot.zone);
        changed |= number(
            ui,
            "Extra room each side",
            "Added on each side of a hot device so it does not heat its neighbours.",
            &mut rules.hot_gap,
            200.0,
        );
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut on = rules.heat.is_some();
        if ui.checkbox(&mut on, "Heat-aware placement").changed() {
            rules.heat = on.then(HeatPlace::default);
            changed = true;
        }
        dialog_ui::help(
            ui,
            "Spreads hot devices, keeps devices that mind heat away from them and below them. \
             Needs power loss and a maximum ambient on the parts. A planning score, not a \
             temperature: check the result in the Heat view.",
        );
    });
    if let Some(heat) = &mut rules.heat {
        changed |= plain_number(
            ui,
            "Air around the cabinet",
            "Ambient temperature the devices are judged against.",
            &mut heat.ambient_c,
            " °C",
            -40.0..=90.0,
        );
        changed |= plain_number(
            ui,
            "Limit when a part has none",
            "Used only for weighing; a device is never called sensitive because of this guess.",
            &mut heat.assumed_limit_c,
            " °C",
            0.0..=150.0,
        );
        changed |= plain_number(
            ui,
            "Sensitive within",
            "A device whose own limit is within this many kelvin of the air around is sensitive \
             to heat.",
            &mut heat.sensitive_margin_k,
            " K",
            0.0..=100.0,
        );
        changed |= plain_number(
            ui,
            "Counts as a heat source from",
            "The smallest power loss that makes the heat search run.",
            &mut heat.min_source_w,
            " W",
            0.0..=500.0,
        );
    }
    (changed, surface_edit)
}

/// The surface rules as the property text (`door: H S P`).
pub(super) fn surface_text(rules: &SurfaceRules) -> String {
    let mut out = String::new();
    for r in &rules.rules {
        out.push_str(&format!("{}: {}\n", r.surface, r.codes.join(" ")));
    }
    out
}

/// One sentence for a surface rule.
pub(super) fn surface_sentence(rule: &SurfaceRule) -> String {
    let kinds: Vec<String> = rule
        .codes
        .iter()
        .map(|c| match code_name(c) {
            "devices" => format!("marked with letter {}", c.to_ascii_uppercase()),
            n => n.to_owned(),
        })
        .collect();
    format!(
        "Devices that are {} go to the {} instead of the mounting plate.",
        kinds.join(", "),
        rule.surface
    )
}

/// The "Door and other surfaces" cards. Returns the new property text when
/// the person changed something (`Some(None)`: back to KoperCAD's default).
pub(super) fn show_surfaces(ui: &mut egui::Ui, rules: &SurfaceRules) -> Option<Option<String>> {
    let mut edited = rules.clone();
    let mut changed = false;
    let mut reset = false;
    dialog_ui::section(ui, "Door and other surfaces", false);
    dialog_ui::hint(
        ui,
        if rules.standard {
            "This cabinet uses KoperCAD's default: pilot devices (push buttons, lamps, meters) go on the door, everything else on the plate. Change a card to make your own rules for this cabinet."
        } else {
            "These rules are saved with this cabinet. A device can also name its own surface, which wins."
        },
    );
    let mut delete = None;
    for (i, rule) in edited.rules.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            dialog_ui::hint(ui, &surface_sentence(rule));
            ui.horizontal(|ui| {
                ui.label("Goes to");
                let id = ui.id().with(("surface", i));
                let mut name: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| rule.surface.clone());
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(90.0));
                if r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, name.clone()));
                } else {
                    ui.data_mut(|d| d.remove_temp::<String>(id));
                }
                if r.lost_focus() && !name.trim().is_empty() && name.trim().to_ascii_lowercase() != rule.surface {
                    rule.surface = name.trim().to_ascii_lowercase();
                    changed = true;
                }
                dialog_ui::help(ui, "The name of a surface of this cabinet: door, side, roof... Press Enter to keep it. A name the cabinet does not have leaves its devices on the plate and says so.");
            });
            ui.horizontal(|ui| {
                ui.label("Letters");
                let id = ui.id().with(("surface_codes", i));
                let mut text: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| rule.codes.join(", "));
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(120.0));
                if r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, text.clone()));
                } else {
                    ui.data_mut(|d| d.remove_temp::<String>(id));
                }
                if r.lost_focus() {
                    let codes: Vec<String> = text
                        .split([',', ' ', ';'])
                        .map(|c| c.trim().to_ascii_uppercase())
                        .filter(|c| !c.is_empty())
                        .collect();
                    if !codes.is_empty() && codes != rule.codes {
                        rule.codes = codes;
                        changed = true;
                    }
                }
                dialog_ui::help(ui, "The first letters of the device tags, e.g. H for lamps, S for switches and buttons, P for meters. Press Enter to keep them.");
                if ui.small_button("Delete").on_hover_text("Remove this rule").clicked() {
                    delete = Some(i);
                }
            });
        });
    }
    if let Some(i) = delete {
        edited.rules.remove(i);
        changed = true;
    }
    ui.horizontal(|ui| {
        if ui.button("+ Add a surface rule").clicked() {
            edited.rules.push(SurfaceRule {
                surface: "door".to_owned(),
                codes: vec!["Z".to_owned()],
            });
            changed = true;
        }
        if !rules.standard
            && novo_theme::widgets::ghost_button(ui, "Back to KoperCAD's default").clicked()
        {
            reset = true;
        }
    });
    if reset {
        return Some(None);
    }
    changed.then(|| Some(surface_text(&edited)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_is_named_and_said_in_plain_words() {
        let standard = RuleSet::standard();
        let q = &standard.rules[0];
        assert_eq!(
            rule_title(q),
            "Circuit breakers and switches, fuses and protection, power supplies"
        );
        let said = rule_sentence(q);
        assert!(
            said.contains("top of the plate") && said.contains("shared"),
            "{said}"
        );
        let t = standard
            .rules
            .iter()
            .find(|r| r.codes == ["T"])
            .expect("a drive rule");
        let said = rule_sentence(t);
        assert!(
            said.contains("bottom")
                && said.contains("20 mm between")
                && said.contains("80 mm of room above and below"),
            "{said}"
        );
    }

    #[test]
    fn surface_rules_read_back_from_their_text() {
        let standard = SurfaceRules::standard();
        let text = surface_text(&standard);
        assert_eq!(text, "door: H S P\n");
        let (back, problems) = SurfaceRules::parse(&text);
        assert!(problems.is_empty());
        assert_eq!(back.rules, standard.rules);
        let said = surface_sentence(&standard.rules[0]);
        assert!(
            said.contains("lamps and signals") && said.contains("door"),
            "{said}"
        );
    }

    #[test]
    fn a_new_rule_goes_before_the_everything_else_rule() {
        let mut rules = RuleSet::standard();
        let n = rules.rules.len();
        add_rule(&mut rules);
        assert_eq!(rules.rules.len(), n + 1);
        assert_eq!(rules.rules.last().unwrap().codes, ["*"]);
        assert_eq!(rules.rules[n - 1].codes, ["Z"]);
        // Still a rule set the file format reads back.
        assert_eq!(RuleSet::parse(&rules.to_text()).unwrap(), rules);
    }

    #[test]
    fn drawing_the_panel_changes_nothing_by_itself() {
        let ctx = egui::Context::default();
        novo_theme::apply(&ctx);
        // Fonts load on the first pass.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        let mut rules = RuleSet::standard();
        let before = rules.clone();
        let mut changed = true;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            changed = show(ui, &mut rules, None).0;
        })
        .textures_delta
        .clear();
        assert!(!changed);
        assert_eq!(rules, before);
    }
}
