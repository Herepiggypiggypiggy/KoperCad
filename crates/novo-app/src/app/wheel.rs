//! The place wheel (DESIGN.md §4, "Place wheel"): a radial picker at the
//! pointer for the common parts. Five fixed wedges on the inner ring
//! (Terminal, Switch, Relay, Breaker, Points) around a centre, Other…, that
//! opens the place surface; the sector under the pointer fans its items
//! out on the outer ring, the ones used most in the project first. Click, flick or keys (1 to 5, then 1 to 8) pick; Backspace goes
//! back a ring and Esc closes. A picked symbol is attached to the pointer
//! exactly as the place mode does.

use std::f32::consts::TAU;

use eframe::egui::{self, Key, Modifiers, Pos2, Vec2};
use novo_workflow::palette::Mode;

use super::NovoApp;

/// Inner ring: from `HUB` to `RING`. Outer ring: from `RING` to `REACH`.
const HUB: f32 = 38.0;
const RING: f32 = 110.0;
const REACH: f32 = 200.0;
/// How long a held key has to stay down before it opens the wheel.
pub(super) const HOLD: f64 = 0.25;
/// How long the place key must be seen up before a new press counts.
pub(super) const REARM: f64 = 0.08;
/// How long the pointer must stay on another wedge before the outer ring
/// follows it, so a path from one wedge's items that clips a neighbouring
/// wedge does not change the choice.
const DWELL: f64 = 0.18;

/// What a wedge puts down: the library key and the short name drawn.
type Item = (&'static str, &'static str);

/// The categories are fixed so muscle memory forms; the items inside
/// follow use. The centre of the wheel (Other…) opens the place surface.
const CATEGORIES: [(&str, &[Item]); 5] = [
    (
        "Terminal",
        &[
            ("iec/terminals/terminal_end", "Terminal"),
            ("iec/terminals/terminal_pe_end", "PE"),
            ("iec/terminals/terminal_n_end", "N"),
            ("iec/terminals/terminal", "Through"),
            ("iec/terminals/terminal_2level", "2-level"),
            ("iec/terminals/terminal_fused", "Fused"),
            ("iec/terminals/terminal_disconnect", "Disconnect"),
        ],
    ),
    (
        "Switch",
        &[
            ("iec/contacts/no", "NO"),
            ("iec/contacts/nc", "NC"),
            ("iec/contacts/changeover", "Changeover"),
            ("iec/operators/pushbutton_no", "Push NO"),
            ("iec/operators/pushbutton_nc", "Push NC"),
            ("iec/operators/emergency_stop", "E-stop"),
            ("iec/operators/selector_no", "Selector"),
            ("iec/contacts/position_no", "Limit"),
        ],
    ),
    (
        "Relay",
        &[
            ("iec/coils/relay", "Coil"),
            ("iec/contacts/no", "NO"),
            ("iec/contacts/nc", "NC"),
            ("iec/contacts/changeover", "Changeover"),
            ("iec/coils/timer_on_delay", "Timer on"),
            ("iec/coils/timer_off_delay", "Timer off"),
            ("iec/coils/contactor", "Contactor"),
        ],
    ),
    (
        "Breaker",
        &[
            ("iec/protection/mcb_1p", "MCB 1P"),
            ("iec/protection/mcb_3p", "MCB 3P"),
            ("iec/protection/motor_protection_breaker", "Motor prot."),
            ("iec/protection/motor_circuit_protector", "MCP"),
            ("iec/protection/fuse", "Fuse"),
            ("iec/protection/rcbo_2p", "RCBO"),
            ("iec/protection/overload_relay", "Overload"),
            ("iec/protection/surge_arrester", "Surge"),
        ],
    ),
    (
        "Points",
        &[
            ("iec/connectivity/potential_feed", "Feed"),
            ("iec/connectivity/interruption_point", "Interruption"),
            ("iec/earth/protective_earth", "PE"),
            ("iec/earth/earth", "Earth"),
            ("iec/earth/chassis", "Chassis"),
        ],
    ),
];

/// "Other…", the wheel's centre: it opens the place surface. Not a wedge
/// index; it travels in `sector` beside them.
const OTHER: usize = usize::MAX;

/// Where the pointer is on the wheel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    Hub,
    Wedge(usize),
    Item(usize),
    Outside,
}

/// Which slice of `count` equal slices, the first centred on twelve
/// o'clock and running clockwise, an offset from the centre points into.
fn slice(offset: Vec2, count: usize) -> usize {
    let turn = offset.x.atan2(-offset.y).rem_euclid(TAU);
    let width = TAU / count as f32;
    ((turn + width / 2.0) / width) as usize % count
}

/// The region of `offset` (from the centre) when the outer ring has
/// `items` slices.
fn region(offset: Vec2, items: usize) -> Region {
    let d = offset.length();
    if d < HUB {
        Region::Hub
    } else if d < RING {
        Region::Wedge(slice(offset, CATEGORIES.len()))
    } else if d < REACH && items > 0 {
        Region::Item(slice(offset, items))
    } else {
        Region::Outside
    }
}

/// The wedge whose items are shown, given the wedge under the pointer
/// (`under`, `None` when it is not on a wedge) and the wedge it has been
/// resting on since `pending.1`. The first wedge is taken at once; a
/// different one only after [`DWELL`].
fn settle(
    shown: Option<usize>,
    pending: Option<(usize, f64)>,
    under: Option<usize>,
    now: f64,
) -> (Option<usize>, Option<(usize, f64)>) {
    let Some(under) = under else {
        return (shown, None);
    };
    if shown.is_none() || shown == Some(under) {
        return (Some(under), None);
    }
    match pending {
        Some((w, since)) if w == under => {
            if now - since >= DWELL {
                (Some(under), None)
            } else {
                (shown, pending)
            }
        }
        _ => (shown, Some((under, now))),
    }
}

/// The open wheel.
pub(crate) struct Wheel {
    centre: Pos2,
    /// The wedge hovered last, whose items the outer ring shows.
    hot: Option<usize>,
    /// A different wedge the pointer is resting on, and since when.
    pending: Option<(usize, f64)>,
    /// The wedge chosen with a click or a key: its items stay up.
    locked: Option<usize>,
    /// A press happened on the wheel, so a release means something.
    armed: bool,
    /// Opened by holding a key: letting go of it over an item picks it.
    held: Vec<Key>,
    /// The items of the wedge shown, most used first, that the library has.
    items: Vec<Item>,
}

impl NovoApp {
    /// A key that opens the wheel went down: a tap runs the command, a
    /// hold of [`HOLD`] seconds opens the wheel instead. Decided on later
    /// frames by [`Self::place_hold_tick`].
    ///
    /// A press is ignored while an earlier one is still being decided or
    /// the key has not been seen up since (see [`Self::place_hold_tick`]):
    /// Windows can deliver a held key's repeats as fresh presses, which
    /// used to re-arm the hold and open the part search under the wheel.
    pub(super) fn arm_place_hold(&mut self, ctx: &egui::Context) {
        if self.wb.place_hold.is_some() || self.wb.place_latch.is_some() {
            return;
        }
        let (time, down) = ctx.input(|i| (i.time, i.keys_down.iter().copied().collect::<Vec<_>>()));
        self.wb.place_hold = Some((time, down));
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD));
    }

    /// A fresh press of the place key while only the wheel is up. Looked at,
    /// not taken: the wheel reads its own keys.
    pub(super) fn wheel_place_press(&self, ctx: &egui::Context) -> bool {
        if self.wb.wheel.is_none() || self.modal_open_but_wheel() {
            return false;
        }
        ctx.input(|i| {
            i.events.iter().any(|e| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = e
                else {
                    return false;
                };
                crate::keys::chord_of(*key, *modifiers).is_some_and(|c| {
                    self.wb
                        .keymap
                        .resolve(c, novo_workflow::keymap::Focus::Canvas)
                        == Some("tool.place")
                })
            })
        })
    }

    pub(super) fn place_hold_tick(&mut self, ctx: &egui::Context) {
        // After a decision the keys stay latched until one has been up for
        // `REARM` seconds (as the hold itself ends when any of them is let go), so repeats cannot start a second decision.
        if let Some((keys, up_since)) = self.wb.place_latch.take() {
            let (now, any_down) =
                ctx.input(|i| (i.time, keys.iter().all(|k| i.keys_down.contains(k))));
            if any_down {
                self.wb.place_latch = Some((keys, None));
            } else {
                let since = up_since.unwrap_or(now);
                if now - since < REARM {
                    self.wb.place_latch = Some((keys, Some(since)));
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(REARM));
                }
            }
        }
        let Some((since, keys)) = &self.wb.place_hold else {
            return;
        };
        let (now, down) = ctx.input(|i| {
            (
                i.time,
                keys.iter().all(|k| i.keys_down.contains(k)) && !keys.is_empty(),
            )
        });
        let (since, keys) = (*since, keys.clone());
        if !down {
            self.wb.place_hold = None;
            // A tap with the wheel still up from an earlier hold replaces it.
            self.wb.wheel = None;
            self.run_command(ctx, "tool.place", None);
        } else if now - since >= HOLD {
            self.wb.place_hold = None;
            self.wb.place_latch = Some((keys.clone(), None));
            self.wb.field.close();
            self.open_wheel(ctx, keys);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD - (now - since)));
        }
    }

    /// Opens the wheel at the pointer, shifted to fit the window.
    pub(super) fn open_wheel(&mut self, ctx: &egui::Context, held: Vec<Key>) {
        if self.canvas.schematic_ref().is_none() || !self.is_open() {
            return;
        }
        self.tour_tick("place");
        self.learn_signal("wheel_opened");
        let screen = ctx.content_rect();
        let at = ctx.pointer_latest_pos().unwrap_or(screen.center());
        let room = screen.shrink(REACH + 8.0);
        let centre = if room.is_positive() {
            at.clamp(room.min, room.max)
        } else {
            screen.center()
        };
        self.wb.wheel = Some(Wheel {
            centre,
            hot: None,
            pending: None,
            locked: None,
            armed: false,
            held,
            items: Vec::new(),
        });
    }

    /// The items of wedge `sector` that the library has, the symbols used
    /// most in this project first, the defaults after them in order.
    pub(super) fn wheel_items(&self, sector: usize) -> Vec<Item> {
        let Some(view) = self.canvas.schematic_ref() else {
            return Vec::new();
        };
        // The items name IEC keys; the catalog's pack says which twin is drawn.
        let used = |key: &str| {
            let key = view.editor.catalog.in_pack(key);
            self.backend.document().map_or(0, |d| {
                d.project().symbols().filter(|s| s.symbol.0 == key).count()
            })
        };
        let mut items: Vec<Item> = CATEGORIES[sector]
            .1
            .iter()
            .copied()
            .filter(|(key, _)| {
                view.editor
                    .catalog
                    .get(&view.editor.catalog.in_pack(key))
                    .is_some()
            })
            .collect();
        items.sort_by_key(|(key, _)| std::cmp::Reverse(used(key)));
        items
    }

    /// Draws the wheel and takes its input.
    pub(super) fn place_wheel(&mut self, ctx: &egui::Context) {
        let Some(wheel) = &self.wb.wheel else {
            return;
        };
        let (centre, locked, held) = (wheel.centre, wheel.locked, wheel.held.clone());
        let pointer = ctx.pointer_latest_pos();
        let offset = pointer.map(|p| p - centre).unwrap_or(Vec2::splat(1e6));
        let (shown_before, pending_before) = (wheel.hot, wheel.pending);

        // What the outer ring shows is the sector rested on, until one is
        // chosen with a click or a key.
        let now = ctx.input(|i| i.time);
        let under = match region(offset, 1) {
            Region::Wedge(i) => Some(i),
            _ => None,
        };
        let (hover, pending) = settle(shown_before, pending_before, under, now);
        if pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(DWELL));
        }
        let shown = locked.or(hover);
        let items = shown.map_or_else(Vec::new, |i| self.wheel_items(i));
        let at = region(offset, items.len());

        let (pressed, released, key_released) = ctx.input(|i| {
            (
                i.pointer.any_pressed(),
                i.pointer.any_released(),
                !held.is_empty() && !held.iter().all(|k| i.keys_down.contains(k)),
            )
        });
        let mut pick: Option<usize> = None;
        let mut sector: Option<usize> = None;
        let mut close = false;
        let mut back = false;
        let armed = self.wb.wheel.as_ref().is_some_and(|w| w.armed) || pressed;
        if released && armed {
            match at {
                Region::Item(j) => pick = Some(j),
                Region::Wedge(i) => sector = Some(i),
                Region::Hub => sector = Some(OTHER),
                Region::Outside => close = true,
            }
        } else if key_released
            && locked.is_none()
            && let Region::Item(j) = at
        {
            // A wedge chosen with a click waits for a click on its item.
            pick = Some(j);
        }
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                close = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Backspace) {
                back = true;
            }
            let digits = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
            ];
            for (n, key) in digits.into_iter().enumerate() {
                if i.consume_key(Modifiers::NONE, key) {
                    if locked.is_none() && n < CATEGORIES.len() {
                        sector = Some(n);
                    } else if locked.is_some() && n < items.len() {
                        pick = Some(n);
                    }
                }
            }
        });

        if let Some(w) = &mut self.wb.wheel {
            w.hot = hover;
            w.pending = pending;
            w.armed = armed && !released;
            w.items = items.clone();
            if back {
                w.locked = None;
            }
            if let Some(i) = sector.filter(|&i| i != OTHER) {
                w.locked = Some(i);
            }
        }
        self.draw_wheel(ctx, centre, shown, &items, at);
        self.hidden_note(ctx, centre, shown, &items);

        if close {
            self.wb.wheel = None;
        } else if sector == Some(OTHER) {
            self.wb.wheel = None;
            self.wb.field.open(Mode::Place, pointer);
        } else if let Some(j) = pick
            && let Some(&(key, _)) = items.get(j)
        {
            self.wb.wheel = None;
            if let Some(view) = self.canvas.schematic() {
                // The catalog's pack (the project's drawing standard) says
                // which twin of the item is placed.
                let id = view.editor.catalog.in_pack(key);
                view.editor.begin_place(&id, None);
            }
        }
    }

    /// One weak line under the wheel when the project's standard has no
    /// drawing for some items of the sector shown; hovering lists them.
    fn hidden_note(&self, ctx: &egui::Context, centre: Pos2, shown: Option<usize>, items: &[Item]) {
        let (Some(sector), Some(view)) = (shown, self.canvas.schematic_ref()) else {
            return;
        };
        let hidden: Vec<&str> = CATEGORIES[sector]
            .1
            .iter()
            .filter(|(key, _)| !items.iter().any(|(k, _)| k == key))
            .map(|(_, label)| *label)
            .collect();
        if hidden.is_empty() {
            return;
        }
        let pack = view
            .editor
            .catalog
            .library()
            .pack()
            .map_or("this".to_owned(), |p| p.id().to_uppercase());
        egui::Area::new(egui::Id::new("place_wheel_hidden"))
            .order(egui::Order::Foreground)
            .fixed_pos(centre + Vec2::new(-90.0, REACH + 8.0))
            .show(ctx, |ui| {
                ui.set_width(180.0);
                ui.vertical_centered(|ui| {
                    ui.weak(format!(
                        "{} {} no {pack} version",
                        hidden.len(),
                        if hidden.len() == 1 {
                            "symbol has"
                        } else {
                            "symbols have"
                        }
                    ))
                    .on_hover_text(hidden.join(", "));
                });
            });
    }

    fn draw_wheel(
        &self,
        ctx: &egui::Context,
        centre: Pos2,
        shown: Option<usize>,
        items: &[Item],
        at: Region,
    ) {
        let visuals = ctx.global_style().visuals.clone();
        let accent = visuals.selection.stroke.color;
        let fill = visuals.window_fill();
        let hot_fill = visuals.selection.bg_fill;
        let text = visuals.text_color();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("place_wheel"),
        ));
        // Swallow the pointer so nothing underneath reacts.
        egui::Area::new(egui::Id::new("place_wheel_catch"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                ui.allocate_rect(ctx.content_rect(), egui::Sense::click_and_drag());
            });

        let ring = |count: usize, index: usize, inner: f32, outer: f32| {
            let width = TAU / count as f32;
            let mid = index as f32 * width;
            let (from, to) = (mid - width / 2.0, mid + width / 2.0);
            let steps = 16;
            let at = |r: f32, a: f32| centre + Vec2::new(a.sin(), -a.cos()) * r;
            let mut points: Vec<Pos2> = (0..=steps)
                .map(|s| at(outer, from + (to - from) * s as f32 / steps as f32))
                .collect();
            points.extend(
                (0..=steps)
                    .rev()
                    .map(|s| at(inner, from + (to - from) * s as f32 / steps as f32)),
            );
            (points, at((inner + outer) / 2.0, mid))
        };
        let wedge = |painter: &egui::Painter,
                     count,
                     index,
                     inner,
                     outer,
                     hot: bool,
                     label: &str,
                     on: bool| {
            let (points, mid) = ring(count, index, inner, outer);
            let stroke = if hot {
                egui::Stroke::new(2.0, accent)
            } else {
                egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color)
            };
            let fill = if hot || on { hot_fill } else { fill };
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
            painter.text(
                mid,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(13.0),
                text,
            );
        };

        if !items.is_empty() {
            for (j, (_, label)) in items.iter().enumerate() {
                let hot = at == Region::Item(j);
                wedge(
                    &painter,
                    items.len(),
                    j,
                    RING + 4.0,
                    REACH,
                    hot,
                    &format!("{} {label}", j + 1),
                    false,
                );
            }
        }
        for (i, (name, _)) in CATEGORIES.iter().enumerate() {
            let hot = at == Region::Wedge(i);
            wedge(
                &painter,
                CATEGORIES.len(),
                i,
                HUB,
                RING,
                hot,
                &format!("{} {name}", i + 1),
                shown == Some(i),
            );
        }
        painter.circle(
            centre,
            HUB - 4.0,
            if at == Region::Hub { hot_fill } else { fill },
            egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        );
        painter.text(
            centre,
            egui::Align2::CENTER_CENTER,
            "Other…",
            egui::FontId::proportional(13.0),
            text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(angle_deg: f32, radius: f32) -> Vec2 {
        let a = angle_deg.to_radians();
        Vec2::new(a.sin(), -a.cos()) * radius
    }

    #[test]
    fn the_outer_ring_follows_a_new_wedge_only_after_a_dwell() {
        // The first wedge is taken at once.
        assert_eq!(settle(None, None, Some(1), 0.0), (Some(1), None));
        // Clipping another wedge does not switch…
        let (shown, pending) = settle(Some(1), None, Some(0), 1.0);
        assert_eq!((shown, pending), (Some(1), Some((0, 1.0))));
        // …leaving it resets, and a short rest is not enough…
        assert_eq!(settle(shown, pending, None, 1.05), (Some(1), None));
        let (shown, pending) = settle(Some(1), Some((0, 1.0)), Some(0), 1.1);
        assert_eq!((shown, pending), (Some(1), Some((0, 1.0))));
        // …but resting there does.
        assert_eq!(
            settle(shown, pending, Some(0), 1.0 + DWELL + 0.01),
            (Some(0), None)
        );
    }

    #[test]
    fn wedges_run_clockwise_from_the_top() {
        assert_eq!(region(at(0.0, 70.0), 0), Region::Wedge(0));
        assert_eq!(region(at(72.0, 70.0), 0), Region::Wedge(1));
        assert_eq!(region(at(144.0, 70.0), 0), Region::Wedge(2));
        assert_eq!(region(at(-72.0, 70.0), 0), Region::Wedge(4));
        assert_eq!(region(at(10.0, 20.0), 0), Region::Hub);
    }

    #[test]
    fn items_fill_the_outer_ring_and_nothing_lies_beyond() {
        assert_eq!(region(at(0.0, 150.0), 4), Region::Item(0));
        assert_eq!(region(at(90.0, 150.0), 4), Region::Item(1));
        assert_eq!(region(at(0.0, 150.0), 0), Region::Outside);
        assert_eq!(region(at(0.0, 250.0), 4), Region::Outside);
    }

    #[test]
    fn every_wedge_is_wide_enough_at_its_outer_edge() {
        // DESIGN.md: at least 56 px wide at the outer edge.
        let width = TAU / CATEGORIES.len() as f32 * RING;
        assert!(width >= 56.0, "{width}");
        let eight = TAU / 8.0 * REACH;
        assert!(eight >= 56.0, "{eight}");
    }
}
