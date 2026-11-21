//! The Program monitor: the frame at the playhead, rendered off the UI thread.

use std::sync::Arc;

use filmcraft_engine::{MediaPool, Services};
use filmcraft_project::{ItemId, Project};
use filmcraft_time::Tick;
use gpui::{
    Context, ImageSource, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, RenderImage, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, div, img, px,
};
use image::{Frame, RgbaImage};
use serde_json::json;

use crate::workspace::{Workspace, color};

/// Widest frame the monitor asks the renderer for.
const MAX_FRAME_W: f32 = 960.0;

/// Which frame an image shows: the sequence, the time and the project revision it was rendered at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub seq: ItemId,
    pub time: Tick,
    pub revision: u64,
}

#[derive(Default)]
pub struct Viewer {
    pub image: Option<Arc<RenderImage>>,
    /// The frame `image` shows.
    pub shown: Option<Shown>,
    /// The frame being rendered.
    pub requested: Option<Shown>,
}

struct Job {
    project: Arc<Project>,
    pool: Arc<MediaPool>,
    services: Arc<dyn Services>,
    seq: ItemId,
    time: Tick,
    scale: f32,
}

/// Render the frame as RGBA8 over black.
fn render(job: Job) -> (usize, usize, Vec<u8>) {
    let provider = job.pool.provider(job.project.clone(), job.services);
    let opts = filmcraft_render::RenderOptions { scale: job.scale, captions: true, ..Default::default() };
    filmcraft_render::render_sequence_rgba8(&job.project, job.seq, job.time, opts, &provider)
}

/// GPUI draws images as BGRA; the frame is opaque, so alpha is forced to 255.
fn to_image(w: usize, h: usize, mut px: Vec<u8>) -> Option<Arc<RenderImage>> {
    for p in px.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        p[3] = 255;
    }
    let img = RgbaImage::from_raw(u32::try_from(w).ok()?, u32::try_from(h).ok()?, px)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(img)])))
}

impl Workspace {
    /// Start rendering the frame at the playhead unless it is shown or already being rendered. A
    /// newer request replaces an older one still in flight.
    pub(crate) fn request_frame(&mut self, cx: &mut Context<Self>) {
        let s = &self.session;
        let Some(seq) = s.state.active_sequence else {
            self.viewer = Default::default();
            self.render_task = None;
            return;
        };
        let want = Shown { seq, time: s.playhead(), revision: s.revision };
        if self.viewer.shown == Some(want) || self.viewer.requested == Some(want) {
            return;
        }
        let width = s.project.sequence(seq).map_or(1920, |q| q.settings.width.max(1));
        let scale = (MAX_FRAME_W / width as f32).min(1.0);
        let job = Job { project: s.project.clone(), pool: s.media.clone(), services: s.services.clone(), seq, time: want.time, scale };
        self.viewer.requested = Some(want);
        self.render_task = Some(cx.spawn(async move |this, cx| {
            let (w, h, px) = cx.background_executor().spawn(async move { render(job) }).await;
            this.update(cx, |this, cx| {
                if this.viewer.requested != Some(want) {
                    return;
                }
                this.viewer.requested = None;
                if let Some(image) = to_image(w, h, px) {
                    this.viewer.image = Some(image);
                    this.viewer.shown = Some(want);
                }
                cx.notify();
            })
            .ok();
        }));
    }
}

impl Workspace {
    pub(crate) fn render_viewer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.tokens;
        let (code, rate_ok) = match self.session.active_sequence() {
            Some(q) => {
                let s = &q.settings;
                (filmcraft_time::format_time(self.session.playhead(), s.frame_rate, s.drop_frame, s.display, i64::from(s.sample_rate)), true)
            }
            None => ("--:--:--:--".to_string(), false),
        };
        let picture = match &self.viewer.image {
            Some(image) => img(ImageSource::Render(image.clone())).object_fit(ObjectFit::Contain).size_full().into_any_element(),
            None => div().text_color(color(t.text_faint)).child(if rate_ok { "Rendering…" } else { "No sequence open" }).into_any_element(),
        };
        div()
            .id("program-monitor")
            .role(Role::Group)
            .aria_label("Program monitor")
            .flex_1()
            .min_h(px(120.0))
            .flex()
            .flex_col()
            .bg(color(t.monitor_bg))
            .child(
                div()
                    .id("viewer-frame")
                    .debug_selector(|| "viewer-frame".into())
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .child(picture),
            )
            .child(
                div()
                    .id("transport")
                    .role(Role::Toolbar)
                    .aria_label("Transport")
                    .h(px(t.tab_h))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(t.gap))
                    .child(self.button("go-start", "|◀", "Go to sequence start", false, cx, |this, cx| {
                        let _ = this.dispatch("playhead.start", json!({}), cx);
                    }))
                    .child(self.button("step-back", "◀", "Step back one frame", false, cx, |this, cx| {
                        let _ = this.dispatch("playhead.stepBack", json!({}), cx);
                    }))
                    .child(
                        div()
                            .id("timecode")
                            .role(Role::Timer)
                            .aria_label(SharedString::from(format!("Playhead timecode {code}")))
                            .px_2()
                            .text_size(px(16.0))
                            .font_family("JetBrains Mono")
                            .text_color(color(t.timecode))
                            .child(code),
                    )
                    .child(self.button("step-forward", "▶", "Step forward one frame", false, cx, |this, cx| {
                        let _ = this.dispatch("playhead.stepForward", json!({}), cx);
                    }))
                    .child(self.button("go-end", "▶|", "Go to sequence end", false, cx, |this, cx| {
                        let _ = this.dispatch("playhead.end", json!({}), cx);
                    })),
            )
    }
}
