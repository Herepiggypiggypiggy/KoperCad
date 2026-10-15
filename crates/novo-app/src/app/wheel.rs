//! The place wheel (DESIGN.md §4, "Place wheel"): a radial picker at the
//! pointer for the common parts. Five fixed wedges on the inner ring
//! (Terminal, Switch, Relay, Breaker, More…); the sector under the pointer
//! fans its items out on the outer ring, the ones used most in the project
//! first. Click, flick or keys (1 to 5, then 1 to 8) pick; Backspace goes
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

/// What a wedge puts down: the library key and the short name drawn.
type Item = (&'static str, &'static str);

/// The categories are fixed so muscle memory forms; the items inside
/// follow use. The last wedge has no items: it opens the place surface.
const CATEGORIES: [(&str, &[Item]); 5] = [
    (
        "Terminal",
        &[
            ("iec/terminals/terminal", "Terminal"),
            ("iec/terminals/terminal_pe", "PE"),
            ("iec/terminals/terminal_n", "N"),
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
    ("More…", &[]),
];

/// The wedge that opens the place surface.
const MORE: usize = CATEGORIES.len() - 1;

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

/// The open wheel.
pub(crate) struct Wheel {
    centre: Pos2,
    /// The wedge hovered last, whose items the outer ring shows.
    hot: Option<usize>,
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
    pub(super) fn arm_place_hold(&mut self, ctx: &egui::Context) {
        let (time, down) = ctx.input(|i| (i.time, i.keys_down.iter().copied().collect::<Vec<_>>()));
        self.wb.place_hold = Some((time, down));
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD));
    }

    pub(super) fn place_hold_tick(&mut self, ctx: &egui::Context) {
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
            self.run_command(ctx, "tool.place", None);
        } else if now - since >= HOLD {
            self.wb.place_hold = None;
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
            locked: None,
            armed: false,
            held,
            items: Vec::new(),
        });
    }

    /// The items of wedge `sector` that the library has, the symbols used
    /// most in this project first, the defaults after them in order.
    fn wheel_items(&self, sector: usize) -> Vec<Item> {
        let Some(view) = self.canvas.schematic_ref() else {
            return Vec::new();
        };
        let used = |key: &str| {
            self.backend.document().map_or(0, |d| {
                d.project().symbols().filter(|s| s.symbol.0 == key).count()
            })
        };
        let mut items: Vec<Item> = CATEGORIES[sector]
            .1
            .iter()
            .copied()
            .filter(|(key, _)| view.editor.catalog.get(key).is_some())
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
        let shown_before = wheel.hot;

        // What the outer ring shows is the sector hovered, until one is
        // chosen with a click or a key.
        let hover = match region(offset, 1) {
            Region::Wedge(i) => Some(i),
            _ => shown_before,
        };
        let shown = locked.or(hover).filter(|&i| i != MORE);
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
                Region::Hub => sector = Some(MORE),
                Region::Outside => close = true,
            }
        } else if key_released && let Region::Item(j) = at {
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
            w.armed = armed && !released;
            w.items = items.clone();
            if back {
                w.locked = None;
            }
            if let Some(i) = sector.filter(|&i| i != MORE) {
                w.locked = Some(i);
            }
        }
        self.draw_wheel(ctx, centre, shown, &items, at);

        if close {
            self.wb.wheel = None;
        } else if sector == Some(MORE) {
            self.wb.wheel = None;
            self.wb.field.open(Mode::Place, pointer);
        } else if let Some(j) = pick
            && let Some(&(key, _)) = items.get(j)
        {
            self.wb.wheel = None;
            if let Some(view) = self.canvas.schematic() {
                view.editor.begin_place(key, None);
            }
        }
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
            "Place",
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
