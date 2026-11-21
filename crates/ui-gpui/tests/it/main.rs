//! The GPUI frontend end to end: a real engine session behind a window drawn by GPUI's test
//! platform. Pointer and key input go through the same handlers a user's would.

use filmcraft_engine::Session;
use filmcraft_project::TrackKind;
use filmcraft_ui_gpui::{Tool, Workspace};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;

mod geometry;

fn demo_session() -> Session {
    let mut s = Session::default();
    s.execute("file.openDemoProject", json!({})).expect("demo project");
    s
}

fn clip_counts(s: &Session) -> (usize, usize) {
    let q = s.active_sequence().expect("an open sequence");
    (q.video_tracks.iter().map(|t| t.items.len()).sum(), q.audio_tracks.iter().map(|t| t.items.len()).sum())
}

fn window(cx: &mut TestAppContext, session: Session) -> (Entity<Workspace>, &mut VisualTestContext) {
    let (ws, cx) = cx.add_window_view(|window, cx| Workspace::new(session, window, cx));
    cx.run_until_parked();
    (ws, cx)
}

fn journal_has(ws: &Entity<Workspace>, cx: &VisualTestContext, id: &str) -> bool {
    ws.read_with(cx, |w, _| w.session.journal.iter().any(|(c, _)| c == id))
}

#[gpui::test]
fn opening_a_project_shows_its_sequence_and_clips(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("demo.fcproj").to_string_lossy().into_owned();
    let mut saved = demo_session();
    saved.execute("file.saveAs", json!({"path": path})).expect("save the demo project");
    let expected = clip_counts(&saved);

    let (ws, cx) = window(cx, Session::default());
    assert!(ws.read_with(cx, |w, _| w.session.active_sequence().is_none()), "starts with no sequence");
    let opened = ws.update(cx, |w, cx| w.open_project(&path, cx));
    cx.run_until_parked();

    assert!(opened.is_ok(), "{opened:?}");
    assert!(journal_has(&ws, cx, "file.open"), "opening goes through the registered command");
    assert_eq!(ws.read_with(cx, |w, _| clip_counts(&w.session)), expected);
    assert!(cx.debug_bounds("clip:V0:0").is_some(), "the first video clip is drawn on the Timeline");
    assert!(ws.read_with(cx, |w, _| w.shown_frame().is_some()), "the Program monitor shows a frame of the opened project");
}

#[gpui::test]
fn a_razor_click_on_a_clip_dispatches_the_razor_command(cx: &mut TestAppContext) {
    let (ws, cx) = window(cx, demo_session());
    let before = ws.read_with(cx, |w, _| clip_counts(&w.session).0);
    ws.update(cx, |w, cx| w.set_tool(Tool::Razor, cx));
    cx.run_until_parked();
    let clip = cx.debug_bounds("clip:V0:0").expect("the first video clip is drawn");
    cx.simulate_click(clip.center(), Modifiers::none());
    cx.run_until_parked();

    assert!(journal_has(&ws, cx, "timeline.razor"));
    let after = ws.read_with(cx, |w, _| clip_counts(&w.session).0);
    assert_eq!(after, before + 1, "the click cut one clip in two");
    let (first, second) = ws.read_with(cx, |w, _| {
        let t = &w.session.active_sequence().expect("sequence").video_tracks[0];
        (t.items[0].clone(), t.items[1].clone())
    });
    assert_eq!(first.start + first.duration, second.start, "the two halves meet at the cut");
}

#[gpui::test]
fn the_selection_tool_selects_the_clicked_clip(cx: &mut TestAppContext) {
    let (ws, cx) = window(cx, demo_session());
    let clip = cx.debug_bounds("clip:V0:0").expect("the first video clip is drawn");
    cx.simulate_click(clip.center(), Modifiers::none());
    cx.run_until_parked();

    assert!(journal_has(&ws, cx, "timeline.select"));
    let (selected, first) =
        ws.read_with(cx, |w, _| (w.session.state.selection.clone(), w.session.active_sequence().expect("sequence").video_tracks[0].items[0].id));
    assert!(selected.contains(&first), "the clicked clip is selected (with its linked clips)");
}

#[gpui::test]
fn scrubbing_the_ruler_moves_the_playhead_and_the_viewer_frame(cx: &mut TestAppContext) {
    let (ws, cx) = window(cx, demo_session());
    let (start, shown_before) = ws.read_with(cx, |w, _| (w.session.playhead(), w.shown_frame().expect("a frame at the start")));
    let ruler = cx.debug_bounds("ruler").expect("the ruler is drawn");
    let target = gpui::point(ruler.left() + ruler.size.width * 0.5, ruler.center().y);
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();

    assert!(journal_has(&ws, cx, "playhead.set"));
    let (now, shown) = ws.read_with(cx, |w, _| (w.session.playhead(), w.shown_frame().expect("a frame after the scrub")));
    assert_ne!(now, start, "the playhead moved");
    assert_eq!(shown.time, now, "the monitor shows the frame at the playhead");
    assert_ne!(shown, shown_before, "and it is a different frame than before");
}

#[gpui::test]
fn the_engines_shortcut_map_drives_the_window(cx: &mut TestAppContext) {
    let (ws, cx) = window(cx, demo_session());
    cx.update(|window, cx| {
        let focus = ws.read(cx).focus_handle_for_tests();
        window.focus(&focus, cx);
    });
    let start = ws.read_with(cx, |w, _| w.session.playhead());
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    assert!(journal_has(&ws, cx, "playhead.stepForward"), "Right runs the command the engine's map binds to it");
    assert!(ws.read_with(cx, |w, _| w.session.playhead()) > start);

    cx.simulate_keystrokes("c");
    assert_eq!(ws.read_with(cx, |w, _| w.tool), Tool::Razor, "C picks the razor, a frontend-owned tool");
}

#[gpui::test]
fn the_timeline_lists_every_track_of_the_sequence(cx: &mut TestAppContext) {
    let (ws, cx) = window(cx, demo_session());
    let (videos, audios) = ws.read_with(cx, |w, _| {
        let q = w.session.active_sequence().expect("sequence");
        (q.video_tracks.iter().filter(|t| t.kind == TrackKind::Video).count(), q.audio_tracks.len())
    });
    assert!(videos > 0 && audios > 0);
    for i in 0..videos {
        let rows = ws.read_with(cx, |w, _| w.session.active_sequence().expect("sequence").video_tracks[i].items.len());
        if rows > 0 {
            let selector: &'static str = Box::leak(format!("clip:V{i}:0").into_boxed_str());
            assert!(cx.debug_bounds(selector).is_some(), "{selector} is drawn");
        }
    }
}
