use super::*;

fn demands(lens: &[f64]) -> Vec<Demand> {
    lens.iter()
        .enumerate()
        .map(|(id, &length)| Demand { id, length })
        .collect()
}

fn mat(catalogue: &[f64], kerf: f64) -> Material {
    Material::new(catalogue, kerf, 100.0)
}

/// Every cut fits its bar: pieces plus kerf between them within the length.
fn assert_valid(plan: &Plan, lens: &[f64]) {
    let kerf = plan.material.kerf;
    let mut seen = vec![0.0; lens.len()];
    for bin in &plan.bins {
        let used: f64 = bin.cuts.iter().map(|c| c.length).sum::<f64>()
            + kerf * bin.cuts.len().saturating_sub(1) as f64;
        assert!(used <= bin.source.length() + 1e-6, "overfull bar {bin:?}");
        assert!(bin.left >= 0.0);
        for c in &bin.cuts {
            seen[c.id] += c.length;
        }
    }
    for (id, want) in lens.iter().enumerate() {
        assert!(
            (seen[id] - want).abs() < 1e-6,
            "piece {id} cut {} of {want}",
            seen[id]
        );
    }
}

/// Exhaustive answer for few pieces: (purchased length, bars bought).
fn brute(material: &Material, lens: &[f64], held: &[f64]) -> (f64, usize) {
    fn rec(
        m: &Material,
        lens: &[f64],
        k: usize,
        held: &[f64],
        held_load: &mut Vec<f64>,
        new_load: &mut Vec<f64>,
        best: &mut (f64, usize),
    ) {
        if k == lens.len() {
            let cost: f64 = new_load.iter().map(|&l| m.shrink(l)).sum();
            if cost < best.0 - 1e-9 || ((cost - best.0).abs() < 1e-9 && new_load.len() < best.1) {
                *best = (cost, new_load.len());
            }
            return;
        }
        let w = lens[k] + m.kerf;
        for h in 0..held.len() {
            if held_load[h] + w <= held[h] + m.kerf + 1e-9 {
                held_load[h] += w;
                rec(m, lens, k + 1, held, held_load, new_load, best);
                held_load[h] -= w;
            }
        }
        for b in 0..new_load.len() {
            if new_load[b] + w <= m.longest() + m.kerf + 1e-9 {
                new_load[b] += w;
                rec(m, lens, k + 1, held, held_load, new_load, best);
                new_load[b] -= w;
            }
        }
        new_load.push(w);
        rec(m, lens, k + 1, held, held_load, new_load, best);
        new_load.pop();
    }
    let mut best = (f64::INFINITY, usize::MAX);
    rec(
        material,
        lens,
        0,
        held,
        &mut vec![0.0; held.len()],
        &mut Vec::new(),
        &mut best,
    );
    best
}

#[test]
fn perfect_pairs_fill_the_bars() {
    let lens = [600.0, 400.0, 500.0, 500.0, 700.0, 300.0];
    let p = plan(&mat(&[1000.0], 0.0), &demands(&lens), &[]);
    assert_valid(&p, &lens);
    assert_eq!(p.bars_bought(), 3);
    assert_eq!(p.bought_mm, 3000.0);
    assert!(p.proven_optimal());
    assert!(p.bins.iter().all(|b| b.left == 0.0));
}

#[test]
fn the_search_beats_best_fit_decreasing() {
    // Best-fit decreasing needs four bars of 1000 for these, three is possible.
    let lens = [
        290.0, 390.0, 280.0, 240.0, 420.0, 210.0, 470.0, 300.0, 300.0,
    ];
    let p = plan(&mat(&[1000.0], 0.0), &demands(&lens), &[]);
    assert_valid(&p, &lens);
    assert_eq!(p.bars_bought(), 3);
    assert!(!p.budget_hit);
}

#[test]
fn the_kerf_is_paid_between_pieces() {
    // 495 + 495 + one 5 mm cut = 995 fits 1000; 496 + 496 + 5 = 997 fits;
    // 498 + 498 + 5 = 1001 does not.
    let m = mat(&[1000.0], 5.0);
    assert_eq!(plan(&m, &demands(&[496.0, 496.0]), &[]).bars_bought(), 1);
    assert_eq!(plan(&m, &demands(&[498.0, 498.0]), &[]).bars_bought(), 2);
    // Exactly full: 497.5 + 497.5 + 5 = 1000.
    assert_eq!(plan(&m, &demands(&[497.5, 497.5]), &[]).bars_bought(), 1);
}

#[test]
fn a_piece_the_length_of_the_bar_needs_no_cut() {
    let p = plan(&mat(&[2000.0], 3.0), &demands(&[2000.0]), &[]);
    assert_eq!(p.bars_bought(), 1);
    assert_eq!(p.bins[0].left, 0.0);
}

#[test]
fn the_offcut_is_the_bar_minus_pieces_and_every_cut() {
    // 2000 - 600 - 700 = 700, minus 2 mm between the pieces and 2 mm for the
    // cut that frees the offcut.
    let m = mat(&[2000.0], 2.0);
    let p = plan(&m, &demands(&[600.0, 700.0]), &[]);
    assert_eq!(p.bins.len(), 1);
    assert!((p.bins[0].left - 696.0).abs() < 1e-9);
    assert_eq!(p.offcuts(), vec![696.0]);
    // A leftover under the minimum is scrap.
    let p = plan(&m, &demands(&[950.0, 950.0]), &[]);
    assert!((p.bins[0].left - 96.0).abs() < 1e-9);
    assert!(p.offcuts().is_empty());
    assert!((p.scrap_mm() - 96.0).abs() < 1e-9);
}

#[test]
fn a_short_load_is_bought_in_the_short_bar() {
    let m = mat(&[1000.0, 2000.0], 0.0);
    let p = plan(&m, &demands(&[400.0, 300.0]), &[]);
    assert_eq!(p.bought, vec![(1000.0, 1)]);
    let p = plan(&m, &demands(&[1900.0]), &[]);
    assert_eq!(p.bought, vec![(2000.0, 1)]);
}

#[test]
fn mixed_lengths_buy_the_least_material() {
    // 1500 + 900: a 2000 bar holds 1500 only, 900 goes in a 1000 bar (3000
    // bought); one 2000 + one 1000 beats two 2000.
    let m = mat(&[1000.0, 2000.0], 0.0);
    let p = plan(&m, &demands(&[1500.0, 900.0]), &[]);
    assert_eq!(p.bought_mm, 3000.0);
    // 1100 + 900 fit one 2000 bar exactly, one bar beats 2000+1000.
    let p = plan(&m, &demands(&[1100.0, 900.0]), &[]);
    assert_eq!(p.bought, vec![(2000.0, 1)]);
}

#[test]
fn equal_length_prefers_fewer_bars() {
    // Two 1000 bars and one 2000 bar are both 2000 mm; fewer bars wins.
    let m = mat(&[1000.0, 2000.0], 0.0);
    let p = plan(&m, &demands(&[1000.0, 1000.0]), &[]);
    assert_eq!(p.bars_bought(), 1);
}

#[test]
fn a_held_offcut_is_used_before_buying() {
    let m = mat(&[2000.0], 2.0);
    let p = plan(&m, &demands(&[600.0]), &[700.0]);
    assert_eq!(p.bars_bought(), 0);
    assert_eq!(p.bins[0].source, Source::Held(700.0));
    assert!((p.bins[0].left - 98.0).abs() < 1e-9);
}

#[test]
fn held_pieces_reduce_the_purchase_and_unused_ones_are_listed() {
    let m = mat(&[1000.0], 0.0);
    let lens = [900.0, 900.0, 900.0];
    let p = plan(&m, &demands(&lens), &[1000.0]);
    assert_valid(&p, &lens);
    assert_eq!(p.bars_bought(), 2);
    let p = plan(&m, &demands(&[300.0]), &[1000.0, 500.0]);
    assert_eq!(p.bars_bought(), 0);
    assert_eq!(p.unused_held.len(), 1);
}

#[test]
fn a_piece_too_long_for_the_bar_is_spliced() {
    let m = mat(&[2000.0], 0.0);
    let p = plan(&m, &demands(&[4500.0]), &[]);
    assert_eq!(p.bars_bought(), 3);
    let parts: Vec<_> = p
        .bins
        .iter()
        .flat_map(|b| b.cuts.iter().map(|c| c.part))
        .collect();
    assert!(parts.contains(&Some((1, 3))) && parts.contains(&Some((3, 3))));
    assert_valid(&p, &[4500.0]);
    // Exactly two bars long: two whole bars and nothing more.
    let p = plan(&m, &demands(&[4000.0]), &[]);
    assert_eq!(p.bars_bought(), 2);
}

#[test]
fn cuts_follow_build_order_and_say_how_much_bar_is_left() {
    let m = mat(&[2000.0], 2.0);
    let p = plan(&m, &demands(&[300.0, 500.0, 400.0]), &[]);
    let cuts = &p.bins[0].cuts;
    assert_eq!(cuts.iter().map(|c| c.id).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(cuts[0].from_left, 2000.0);
    assert_eq!(cuts[1].from_left, 2000.0 - 300.0 - 2.0);
    assert_eq!(cuts[2].from_left, 2000.0 - 300.0 - 2.0 - 500.0 - 2.0);
}

#[test]
fn nothing_to_cut_buys_nothing() {
    let p = plan(&mat(&[2000.0], 2.0), &[], &[500.0]);
    assert_eq!(p.bars_bought(), 0);
    assert!(p.bins.is_empty());
    assert_eq!(p.unused_held, vec![500.0]);
    assert!(p.proven_optimal());
}

#[test]
fn it_matches_an_exhaustive_search_on_small_cases() {
    // A small deterministic generator: no clock, no randomness.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    for case in 0..60 {
        let n = 2 + next(5) as usize;
        let lens: Vec<f64> = (0..n).map(|_| 100.0 + 50.0 * next(30) as f64).collect();
        let held: Vec<f64> = (0..next(3))
            .map(|_| 200.0 + 100.0 * next(10) as f64)
            .collect();
        let catalogue: &[f64] = if case % 2 == 0 {
            &[1500.0]
        } else {
            &[1000.0, 2000.0]
        };
        let kerf = [0.0, 2.0, 5.0][case % 3];
        let m = mat(catalogue, kerf);
        let p = plan(&m, &demands(&lens), &held);
        assert_valid(&p, &lens);
        let (cost, bars) = brute(&m, &lens, &held);
        assert!(!p.budget_hit);
        assert!(
            (p.bought_mm - cost).abs() < 1e-6 && p.bars_bought() == bars,
            "case {case}: lens {lens:?} held {held:?} {catalogue:?} kerf {kerf}: got {} in {} bars, best {cost} in {bars}",
            p.bought_mm,
            p.bars_bought()
        );
        assert!(p.bought_mm + 1e-6 >= p.lower_bound_mm);
    }
}

#[test]
fn a_big_job_ends_inside_the_budget_with_a_valid_plan() {
    let lens: Vec<f64> = (0..300).map(|i| 150.0 + (i * 37 % 800) as f64).collect();
    let p = plan(&mat(&[2000.0], 2.0), &demands(&lens), &[]);
    assert_valid(&p, &lens);
    let total: f64 = lens.iter().sum();
    assert!(p.bought_mm >= total);
    // Within a bar of the least that could possibly do.
    assert!(p.bought_mm <= total / 0.95 + 2000.0);
}

#[test]
fn a_shelf_of_thousands_of_equal_offcuts_does_not_drown_the_search() {
    let m = mat(&[2000.0], 3.0);
    let lens = [700.0, 650.0, 600.0, 550.0, 500.0, 450.0, 400.0, 350.0];
    let mut held = vec![300.0; 5000];
    held.extend(vec![900.0; 5000]);
    let started = std::time::Instant::now();
    let p = plan(&m, &demands(&lens), &held);
    assert!(started.elapsed().as_secs() < 5, "too slow");
    assert_valid(&p, &lens);
    assert!(!p.budget_hit);
    // Every offcut is still accounted for: used or listed as unused.
    let used = p
        .bins
        .iter()
        .filter(|b| matches!(b.source, Source::Held(_)))
        .count();
    assert_eq!(used + p.unused_held.len(), held.len());
}
