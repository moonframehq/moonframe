use super::*;

#[test]
fn a_video_reads_its_source_view_and_guides() {
    use crate::{VideoColor, VideoFilter};
    let source = r##"return mod { view = function()
        return ui.video {
            source = "nova-1",
            id = "preview",
            zoom = 2,
            center = { 0.25, 0.75 },
            filter = "nearest",
            color = "display-p3",
            white = 4095,
            gamma = 2.2,
            probe = { 0.5, 0.5 },
            stats = { stats = true },
            guides = { { x = 0.1, y = 0.2, width = 0.5, height = 0.5, color = "#ff000080" } },
        }
    end }"##;
    let dir = app(&[("scan", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("scan").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Video(video)) = slot.view() else {
        panic!("expected a video");
    };
    assert_eq!(
        (video.source.as_str(), video.id.as_str()),
        ("nova-1", "preview")
    );
    assert_eq!(video.view.zoom, Some(2.));
    assert_eq!(video.view.center, [0.25, 0.75]);
    assert_eq!(video.view.filter, VideoFilter::Nearest);
    assert_eq!(video.view.color, VideoColor::DisplayP3);
    assert_eq!(video.view.white, Some(4095.));
    assert_eq!(video.view.probe, Some([0.5, 0.5]));
    assert!(video.stats.is_some());
    let guide = video.view.guides[0];
    assert_eq!(guide.color[0], 1.);
    assert!((guide.color[3] - 128. / 255.).abs() < 1e-6);
}

#[test]
fn a_string_is_a_videos_source_and_the_defaults_change_nothing() {
    use crate::VideoColor;
    let source = r#"return mod { view = function() return ui.video "camera" end }"#;
    let dir = app(&[("scan", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let Some(Node::Video(video)) = workspace.slot("scan").unwrap().view() else {
        panic!("expected a video");
    };
    assert_eq!(video.source, "camera");
    assert_eq!(video.view.color, VideoColor::Srgb);
    assert_eq!((video.view.gain, video.view.gamma), (1., 1.));
    assert_eq!(video.view.white, None);
}

#[test]
fn what_a_video_cannot_use_is_refused() {
    for (view, expected) in [
        ("ui.video {}", "source"),
        (r#"ui.video "../secret""#, "a video's `source`"),
        (r#"ui.video "/etc/passwd""#, "a video's `source`"),
        (r#"ui.video { source = "a", color = "cmyk" }"#, "srgb"),
        (
            r#"ui.video { source = "a", center = { 1 } }"#,
            "two fractions",
        ),
        (
            r#"ui.video { source = "a", guides = { { color = "red" } } }"#,
            "#rrggbb",
        ),
    ] {
        let source = format!("return mod {{ view = function() return {view} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace
            .slot("bad")
            .unwrap()
            .error()
            .map(ToString::to_string);
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{view}: {error:?}"
        );
    }
}
