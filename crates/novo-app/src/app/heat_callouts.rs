//! Where the hot-spot cards of the Cabinet heat view go. A card hangs off
//! its peak; it takes the first free place around it that covers no badge
//! and no other card, keeps clear of the devices where it can, and stays
//! inside the view.

use eframe::egui::{Pos2, Rect, Vec2};

/// Size of a card, points.
pub(super) const SIZE: Vec2 = Vec2::new(210.0, 34.0);
/// Gap kept around other cards and badges.
const GAP: f32 = 2.0;

/// Top left of the card relative to its peak, nearest places first: right and
/// above, right and below, left, then straight above and below, then farther
/// out.
fn offsets() -> Vec<Vec2> {
    let (w, h) = (SIZE.x, SIZE.y);
    let mut v = vec![
        Vec2::new(14.0, -h + 4.0),
        Vec2::new(14.0, 10.0),
        Vec2::new(-w - 14.0, -h + 4.0),
        Vec2::new(-w - 14.0, 10.0),
        Vec2::new(-w * 0.5, -h - 14.0),
        Vec2::new(-w * 0.5, 16.0),
    ];
    for k in 1..=4 {
        let d = k as f32 * (h + 6.0);
        v.push(Vec2::new(14.0, -h + 4.0 - d));
        v.push(Vec2::new(14.0, 10.0 + d));
        v.push(Vec2::new(-w - 14.0, -h + 4.0 - d));
        v.push(Vec2::new(-w - 14.0, 10.0 + d));
    }
    v
}

/// The card for a peak at `at`. `hard` are rectangles it must not cover
/// (badges, other cards), `soft` ones it covers only when nothing else is
/// free (devices). With no clean place the card goes where it covers the
/// least.
pub(super) fn place(at: Pos2, bounds: Rect, hard: &[Rect], soft: &[Rect]) -> Rect {
    let mut best: Option<(f32, Rect)> = None;
    for (rank, off) in offsets().into_iter().enumerate() {
        let card = Rect::from_min_size(at + off, SIZE);
        let mut cost = rank as f32 * 0.01;
        if !bounds.contains_rect(card) {
            cost += 5000.0;
        }
        cost += 1000.0
            * hard
                .iter()
                .filter(|q| q.intersects(card.expand(GAP)))
                .count() as f32;
        cost += 10.0 * soft.iter().filter(|q| q.intersects(card)).count() as f32;
        if best.is_none_or(|(c, _)| cost < c) {
            best = Some((cost, card));
        }
        if cost < 1.0 {
            break;
        }
    }
    best.map_or_else(|| Rect::from_min_size(at + offsets()[0], SIZE), |(_, r)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))
    }

    #[test]
    fn a_card_sits_right_and_above_its_peak_when_nothing_is_in_the_way() {
        let at = Pos2::new(300.0, 300.0);
        let c = place(at, view(), &[], &[]);
        assert_eq!(c.min, at + Vec2::new(14.0, -SIZE.y + 4.0));
    }

    #[test]
    fn it_never_covers_a_badge_or_another_card() {
        let at = Pos2::new(300.0, 300.0);
        let first = place(at, view(), &[], &[]);
        let second = place(at, view(), &[first], &[]);
        assert!(!second.intersects(first.expand(GAP)));
        let badge = Rect::from_min_size(Pos2::new(310.0, 290.0), Vec2::new(90.0, 20.0));
        let c = place(at, view(), &[badge], &[]);
        assert!(!c.intersects(badge.expand(GAP)));
    }

    #[test]
    fn it_keeps_off_a_device_when_there_is_room_and_stays_in_the_view() {
        let at = Pos2::new(20.0, 20.0);
        // A device right of and above the peak, and the view's own edge.
        let device = Rect::from_min_size(Pos2::new(30.0, 0.0), Vec2::new(300.0, 60.0));
        let c = place(at, view(), &[], &[device]);
        assert!(view().contains_rect(c), "{c:?}");
        assert!(!c.intersects(device), "{c:?}");
    }

    #[test]
    fn a_crowd_of_peaks_gets_separate_cards() {
        let mut cards: Vec<Rect> = Vec::new();
        for k in 0..5 {
            let at = Pos2::new(100.0 + 6.0 * k as f32, 120.0);
            let c = place(at, view(), &cards, &[]);
            assert!(cards.iter().all(|q| !q.intersects(c)), "card {k}: {c:?}");
            cards.push(c);
        }
    }
}
