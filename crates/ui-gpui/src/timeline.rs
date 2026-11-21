//! The Timeline panel: ruler, track headers, clips and the playhead.

use filmcraft_project::{Track, TrackKind};
use filmcraft_time::{TICKS_PER_SECOND, Tick};
use gpui::{
    Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Role, ScrollEvent,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, px,
};

use crate::geometry::{HEADER_W, RULER_H, TRACK_H, ruler_step};
use crate::workspace::{Workspace, color};

/// Smallest space between two ruler labels.
const LABEL_GAP_PX: f64 = 84.0;

impl Workspace {
    pub(crate) fn render_timeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.tokens;
        let panel = div().id("timeline").role(Role::Group).aria_label("Timeline").h(px(RULER_H + TRACK_H * 5.0)).flex().flex_col().bg(color(t.tl_bg));
        let Some(seq) = self.session.active_sequence() else {
            return panel.child(div().p_2().text_color(color(t.text_faint)).child("No sequence open"));
        };
        let playhead_x = self.view.x_of(self.session.playhead());
        // video tracks are listed top (highest) to bottom, then the audio tracks
        let rows: Vec<(String, usize, &Track)> = seq
            .video_tracks
            .iter()
            .enumerate()
            .rev()
            .map(|(i, tr)| ("V".to_string(), i, tr))
            .chain(seq.audio_tracks.iter().enumerate().map(|(i, tr)| ("A".to_string(), i, tr)))
            .collect();
        let mut body = div().id("tracks").flex().flex_col().flex_1().overflow_hidden().on_scroll(cx.listener(|this, ev: &ScrollEvent, _, cx| {
            let d = ev.delta.pixel_delta(px(16.0));
            if ev.modifiers.secondary(cx) {
                let anchor = f32::from(ev.position.x - this.track_area.get().origin.x);
                this.zoom_at(if f32::from(d.y) > 0.0 { 1.1 } else { 1.0 / 1.1 }, anchor, cx);
            } else {
                this.view = this.view.scrolled_by(-f32::from(if d.x == px(0.0) { d.y } else { d.x }));
                cx.notify();
            }
        }));
        for (letter, index, track) in rows {
            body = body.child(self.render_track(&letter, index, track, cx));
        }
        let ruler = self.render_ruler(seq.settings.frame_rate, seq.settings.drop_frame, seq.settings.display, i64::from(seq.settings.sample_rate), cx);
        let playhead = div()
            .id("playhead")
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(HEADER_W + playhead_x))
            .w(px(2.0))
            .bg(color(t.playhead))
            .role(Role::Group)
            .aria_label(SharedString::from("Playhead"));
        panel.relative().child(ruler).child(body).child(if playhead_x >= 0.0 { playhead.into_any_element() } else { div().into_any_element() })
    }

    fn render_ruler(
        &self,
        rate: filmcraft_time::FrameRate,
        df: bool,
        display: filmcraft_time::TimeDisplay,
        sr: i64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.tokens;
        let view = self.view;
        let area = self.track_area.clone();
        let width = f64::from(f32::from(area.get().size.width)).max(1200.0);
        let step = ruler_step(view.pps, LABEL_GAP_PX);
        let first = view.scroll.0 / step.0;
        let last = view.tick_at(width as f32).0 / step.0 + 1;
        let mut ticks = div().absolute().size_full();
        for k in first..=last {
            let at = Tick(k * step.0);
            let x = view.x_of(at);
            ticks = ticks.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top_0()
                    .h_full()
                    .pl(px(3.0))
                    .whitespace_nowrap()
                    .border_l_1()
                    .border_color(color(t.tl_ruler_tick))
                    .text_size(px(10.0))
                    .text_color(color(t.tl_ruler_text))
                    .child(filmcraft_time::format_time(at, rate, df, display, sr)),
            );
        }
        let _ = TICKS_PER_SECOND;
        div().id("ruler-row").flex().flex_row().h(px(RULER_H)).child(div().w(px(HEADER_W)).h_full().bg(color(t.tl_header_bg))).child(
            div()
                .id("ruler")
                .role(Role::Slider)
                .aria_label("Timeline ruler: click or drag to move the playhead")
                .debug_selector(|| "ruler".into())
                .relative()
                .flex_1()
                .h_full()
                .overflow_hidden()
                .bg(color(t.tl_ruler_bg))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, ev: &MouseDownEvent, _: &mut Window, cx| {
                        this.scrubbing = true;
                        this.scrub_to(ev.position.x, cx);
                    }),
                )
                .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _: &mut Window, cx| {
                    if this.scrubbing && ev.pressed_button == Some(MouseButton::Left) {
                        this.scrub_to(ev.position.x, cx);
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _: &mut Window, _| {
                        this.scrubbing = false;
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _: &mut Window, _| {
                        this.scrubbing = false;
                    }),
                )
                .child(canvas(move |bounds, _, _| area.set(bounds), |_, _, _, _| {}).absolute().size_full())
                .child(ticks),
        )
    }

    fn render_track(&self, letter: &str, index: usize, track: &Track, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.tokens;
        let view = self.view;
        let kind = match track.kind {
            TrackKind::Video => "video",
            _ => "audio",
        };
        let fill = if track.kind == TrackKind::Video { t.track_target } else { t.render_green };
        let mut lane = div().relative().flex_1().h_full().overflow_hidden().bg(color(t.tl_track_bg));
        for (ci, clip) in track.items.iter().enumerate() {
            let selected = self.session.state.selection.contains(&clip.id);
            let x = view.x_of(clip.start);
            let w = view.width_of(clip.duration).max(2.0);
            let id = clip.id;
            let selector = format!("clip:{letter}{index}:{ci}");
            let name = SharedString::from(format!("{}, {kind} clip on {}{}", clip.name, track.name, if selected { ", selected" } else { "" }));
            lane = lane.child(
                div()
                    .id(SharedString::from(format!("clip-{}", id.0)))
                    .debug_selector(move || selector)
                    .absolute()
                    .left(px(x))
                    .top(px(2.0))
                    .w(px(w))
                    .h(px(TRACK_H - 4.0))
                    .px_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .rounded(px(t.radius_sm))
                    .border_1()
                    .border_color(color(if selected { t.clip_selected_border } else { fill }))
                    .bg(color(if clip.enabled { fill } else { t.disabled_clip_bg }))
                    .text_color(color(t.text))
                    .text_size(px(11.0))
                    .role(Role::Button)
                    .aria_label(name)
                    .focusable()
                    .tab_stop(true)
                    .focus_visible(move |s| s.border_color(color(t.focus)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _: &mut Window, cx| this.clip_pointer(id, ev.position.x, ev.modifiers.shift, cx)),
                    )
                    .child(clip.name.clone()),
            );
        }
        div()
            .id(SharedString::from(format!("track-{letter}{index}")))
            .flex()
            .flex_row()
            .h(px(TRACK_H))
            .border_b_1()
            .border_color(color(t.separator))
            .child(
                div().w(px(HEADER_W)).h_full().px_2().flex().items_center().bg(color(t.tl_header_bg)).text_color(color(t.text_dim)).child(track.name.clone()),
            )
            .child(lane)
    }
}
