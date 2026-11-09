use super::*;

fn frame(x: f32, y: f32, width: f32, height: f32) -> Frame {
    Frame {
        x,
        y,
        width,
        height,
    }
}

const SCREEN: Frame = Frame {
    x: 0.,
    y: 25.,
    width: 1440.,
    height: 875.,
};

#[test]
fn a_frame_on_the_screen_reopens_where_it_was() {
    let kept = frame(100., 80., 1200., 760.);
    assert_eq!(fit(kept, SCREEN), Some(kept));
    // Exactly the screen.
    assert_eq!(fit(SCREEN, SCREEN), Some(SCREEN));
}

#[test]
fn a_frame_partly_off_the_screen_is_moved_onto_it() {
    // Hanging off the right and the bottom.
    assert_eq!(
        fit(frame(1000., 600., 800., 500.), SCREEN),
        Some(frame(640., 400., 800., 500.))
    );
    // Hanging off the left, above the menu bar.
    assert_eq!(
        fit(frame(-300., 10., 800., 500.), SCREEN),
        None,
        "its title bar is under the menu bar"
    );
    assert_eq!(
        fit(frame(-300., 40., 800., 500.), SCREEN),
        Some(frame(0., 40., 800., 500.))
    );
}

#[test]
fn a_frame_larger_than_the_screen_shrinks_to_it() {
    // Kept on a larger screen, reopened on a laptop's.
    assert_eq!(fit(frame(200., 100., 2400., 1300.), SCREEN), Some(SCREEN));
}

#[test]
fn a_frame_off_the_screen_falls_back_to_the_default() {
    // On a screen to the right that is gone.
    assert_eq!(fit(frame(1600., 100., 1200., 800.), SCREEN), None);
    // Only a sliver left on the screen to grab.
    assert_eq!(fit(frame(1400., 100., 1200., 800.), SCREEN), None);
    // Below the screen.
    assert_eq!(fit(frame(100., 890., 800., 500.), SCREEN), None);
    // Not a frame.
    assert_eq!(fit(frame(f32::NAN, 0., 800., 500.), SCREEN), None);
    assert_eq!(fit(frame(0., 100., 0., 500.), SCREEN), None);
}

#[test]
fn a_screen_away_from_the_origin_keeps_its_frames() {
    let left = frame(-1920., 0., 1920., 1080.);
    assert_eq!(
        fit(frame(-1800., 50., 1000., 700.), left),
        Some(frame(-1800., 50., 1000., 700.))
    );
    assert_eq!(fit(frame(100., 50., 1000., 700.), left), None);
}

#[test]
fn the_place_is_written_and_read_back() {
    let saved = Saved {
        state: State::Maximized,
        frame: frame(10., 20., 1280., 820.),
        display: Some("8f2c3e1a-0000-4000-8000-000000000001".to_owned()),
    };
    let text = serialize(&saved).unwrap();
    assert!(text.contains("\"maximized\""), "{text}");
    assert_eq!(parse(&text).unwrap(), saved);
    // Without a screen's id, as some systems give none.
    let anywhere = Saved {
        display: None,
        state: State::Windowed,
        ..saved
    };
    let text = serialize(&anywhere).unwrap();
    assert!(!text.contains("display"), "{text}");
    assert_eq!(parse(&text).unwrap(), anywhere);
}

#[test]
fn an_unreadable_place_is_none() {
    assert!(parse("").is_err());
    assert!(parse("{\"state\": \"minimized\"}").is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("window.json");
    assert_eq!(load(&path), None, "nothing kept yet");
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load(&path), None);
    let saved = Saved {
        state: State::Fullscreen,
        frame: frame(0., 0., 900., 600.),
        display: None,
    };
    write(&path, &saved);
    assert_eq!(load(&path), Some(saved));
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn each_profile_keeps_its_own_place() {
    assert_eq!(file(None).file_name().unwrap(), "window.json");
    assert_eq!(
        file(Some("notes")).file_name().unwrap(),
        "window-notes.json"
    );
    assert_eq!(file(None).parent(), crate::paths::state(None).parent());
}
