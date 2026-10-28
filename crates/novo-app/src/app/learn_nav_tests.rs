//! The navigation of the lessons: one way home in the Learn window, one way
//! off the lesson card, and a list button that leads back to the lessons.
//! Names come from the accessibility tree, icon glyphs dropped.

use super::lesson_verify_cables::Lv;

fn plain(label: &str) -> String {
    label
        .chars()
        .filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c))
        .collect::<String>()
        .trim()
        .to_lowercase()
}

/// The visible names of the last frame: icon glyphs dropped, lower case.
fn names(lv: &mut Lv) -> Vec<String> {
    lv.frames(2);
    lv.widgets().into_iter().map(|(l, _)| plain(&l)).collect()
}

/// Clicks the control with this name, as a mouse would.
fn click(lv: &mut Lv, name: &str) {
    lv.frames(2);
    let rect = lv
        .widgets()
        .into_iter()
        .find(|(l, _)| plain(l) == name)
        .unwrap_or_else(|| panic!("no control named `{name}`"))
        .1;
    lv.click_screen(rect.center());
}

fn count(names: &[String], label: &str) -> usize {
    names.iter().filter(|n| n.as_str() == label).count()
}

fn open_window(lv: &mut Lv, path: Option<String>, track: Option<String>) {
    lv.d.app.learn.window = true;
    lv.d.app.learn.only_path = path;
    lv.d.app.learn.only_track = track;
}

#[test]
fn the_learn_window_has_one_way_home_on_every_page() {
    let mut lv = Lv::start("placing-parts");
    let path = lv.d.app.learn.catalog.paths[0].id.clone();
    let track = lv.d.app.learn.catalog.tracks[0].id.clone();
    for (path, track) in [(None, None), (Some(path), None), (None, Some(track))] {
        open_window(&mut lv, path.clone(), track.clone());
        let n = names(&mut lv);
        assert_eq!(
            count(&n, "home"),
            1,
            "one Home on {path:?} {track:?}: {n:?}"
        );
        assert!(
            !n.iter().any(|l| l.contains("back to home")),
            "no second way home: {n:?}"
        );
    }
}

#[test]
fn the_lesson_card_has_one_close_one_list_and_a_back() {
    let mut lv = Lv::start("placing-parts");
    let n = names(&mut lv);
    assert_eq!(count(&n, "close"), 1, "{n:?}");
    assert_eq!(count(&n, "all lessons"), 1, "{n:?}");
    assert!(n.iter().any(|l| l == "back"), "{n:?}");
    assert!(n.iter().any(|l| l.starts_with("restart")), "{n:?}");
}

#[test]
fn the_list_button_opens_the_learn_window_at_the_lessons_topic() {
    let mut lv = Lv::start("placing-parts");
    let track =
        lv.d.app
            .learn
            .run
            .as_ref()
            .map(|r| r.lesson().track.clone())
            .unwrap();
    assert!(!lv.d.app.learn.window);
    click(&mut lv, "all lessons");
    assert!(lv.d.app.learn.window, "the window opened");
    assert_eq!(lv.d.app.learn.only_track.as_deref(), Some(track.as_str()));
    assert!(
        lv.d.app.learn.run.is_some(),
        "the lesson itself is untouched"
    );
}

#[test]
fn the_cross_hides_the_card_and_continue_brings_it_back() {
    let mut lv = Lv::start("placing-parts");
    click(&mut lv, "close");
    assert!(!lv.d.app.learn.card_on());
    assert!(lv.d.app.learn.run.is_some(), "the lesson waits");
    lv.d.app.continue_lesson();
    assert!(lv.d.app.learn.card_on());
}

#[test]
fn a_finished_card_ends_in_one_close_and_one_list() {
    let mut lv = Lv::start("placing-parts");
    {
        let run = lv.d.app.learn.run.as_mut().unwrap();
        while run.phase() != novo_learn::Phase::Finished {
            assert!(run.skip(), "skipping reaches the end");
        }
    }
    let n = names(&mut lv);
    assert_eq!(count(&n, "close"), 1, "the cross only: {n:?}");
    assert_eq!(count(&n, "all lessons"), 1, "{n:?}");
}
