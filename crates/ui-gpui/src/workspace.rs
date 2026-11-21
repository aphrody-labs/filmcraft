//! The window's root view: engine session, tool, timeline view and the panels' layout.

use std::cell::Cell;
use std::rc::Rc;

use filmcraft_engine::shortcuts::{CommandInfo, Platform};
use filmcraft_engine::{Event, Session};
use filmcraft_project::ClipId;
use filmcraft_theme::{Rgba, ThemeKind, Tokens};
use gpui::{
    Bounds, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Pixels, Render, SharedString, Styled as _,
    Task, Window, div, px,
};
use serde_json::{Value, json};

use crate::geometry::View;
use crate::keys::chord_of;
use crate::viewer::Viewer;

/// The editing tool. Tools belong to the frontend (the engine has no notion of a current tool);
/// they are registered with the shortcut map as `tool.*` commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Selection,
    Razor,
}

pub struct Workspace {
    pub session: Session,
    pub tool: Tool,
    pub view: View,
    pub tokens: Tokens,
    /// The last command's outcome, shown in the status bar.
    pub status: SharedString,
    pub(crate) viewer: Viewer,
    pub(crate) render_task: Option<Task<()>>,
    pub(crate) focus: FocusHandle,
    pub(crate) scrubbing: bool,
    /// Where the Timeline's track area was drawn last: the origin that pointer x's are relative to.
    pub(crate) track_area: Rc<Cell<Bounds<Pixels>>>,
}

/// An 8-bit theme colour as a GPUI colour.
pub fn color(c: Rgba) -> gpui::Rgba {
    gpui::rgba(u32::from_be_bytes([c.r, c.g, c.b, c.a]))
}

impl Workspace {
    pub fn new(mut session: Session, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        session.shortcuts.register_external(vec![
            CommandInfo::new("tool.selection", "Selection Tool", &[], Some("V")),
            CommandInfo::new("tool.razor", "Razor Tool", &[], Some("C")),
        ]);
        let mut this = Workspace {
            session,
            tool: Tool::Selection,
            view: View::default(),
            tokens: Tokens::for_kind(ThemeKind::Dark),
            status: "Ready".into(),
            viewer: Viewer::default(),
            render_task: None,
            focus: cx.focus_handle(),
            scrubbing: false,
            track_area: Rc::new(Cell::new(Bounds::default())),
        };
        this.request_frame(cx);
        this
    }

    /// Run a registered command by id. Every change to the project goes through here.
    pub fn dispatch(&mut self, id: &str, params: Value, cx: &mut Context<Self>) -> Result<Value, String> {
        let result = self.session.execute(id, params).map_err(|e| e.to_string());
        self.status = match &result {
            Ok(_) => filmcraft_engine::find_command(id).map_or(id, |c| c.label).to_string().into(),
            Err(e) => format!("{id}: {e}").into(),
        };
        for event in std::mem::take(&mut self.session.events) {
            if let Event::Toast { message, .. } = event {
                self.status = message.into();
            }
        }
        self.request_frame(cx);
        cx.notify();
        result
    }

    /// File ▸ Open: the project file at `path`.
    pub fn open_project(&mut self, path: &str, cx: &mut Context<Self>) -> Result<Value, String> {
        self.dispatch("file.open", json!({"path": path}), cx)
    }

    /// Move the playhead to the time at window x `x` (a click or drag on the ruler).
    pub fn scrub_to(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let t = self.view.tick_at(f32::from(x - self.track_area.get().origin.x));
        let _ = self.dispatch("playhead.set", json!({"time": t.0}), cx);
    }

    /// A press on a clip: the Razor cuts it at the pointer's time, the Selection tool selects it.
    pub fn clip_pointer(&mut self, clip: ClipId, x: Pixels, add: bool, cx: &mut Context<Self>) {
        let t = self.view.tick_at(f32::from(x - self.track_area.get().origin.x));
        let _ = match self.tool {
            Tool::Razor => self.dispatch("timeline.razor", json!({"time": t.0, "clip": clip.0}), cx),
            Tool::Selection => self.dispatch("timeline.select", json!({"clips": [clip.0], "add": add}), cx),
        };
    }

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tool = tool;
        cx.notify();
    }

    /// Zoom the Timeline around the playhead.
    pub fn zoom(&mut self, factor: f64, cx: &mut Context<Self>) {
        let anchor = self.view.x_of(self.session.playhead());
        self.zoom_at(factor, anchor, cx);
    }

    /// Zoom the Timeline keeping the time under track-area x `anchor_x` where it is.
    pub fn zoom_at(&mut self, factor: f64, anchor_x: f32, cx: &mut Context<Self>) {
        self.view = self.view.zoomed(factor, anchor_x);
        cx.notify();
    }

    /// A key press: the engine's shortcut map decides which command it runs, with the Timeline's
    /// own bindings first. The tools are the frontend's.
    pub(crate) fn key_down(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let platform = Platform::current();
        let Some(chord) = chord_of(&ev.keystroke, platform) else { return };
        let Some(id) = self.session.shortcuts.resolve(&chord, Some("Timeline"), platform).map(|b| b.command.clone()) else { return };
        match id.as_str() {
            "tool.selection" => self.set_tool(Tool::Selection, cx),
            "tool.razor" => self.set_tool(Tool::Razor, cx),
            _ if filmcraft_engine::find_command(&id).is_some() => {
                let _ = self.dispatch(&id, json!({}), cx);
            }
            _ => self.status = format!("{id}: not available in this window yet").into(),
        }
    }

    /// The window's focus handle, to focus it in tests.
    pub fn focus_handle_for_tests(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// The frame the Program monitor shows, if one has been rendered.
    pub fn shown_frame(&self) -> Option<crate::Shown> {
        self.viewer.shown
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.tokens;
        div()
            .id("filmcraft")
            .size_full()
            .flex()
            .flex_col()
            .gap(px(t.gap))
            .bg(color(t.app_bg))
            .text_color(color(t.text))
            .text_size(px(12.0))
            .font_family(crate::FONT_FAMILY)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| this.key_down(ev, cx)))
            .child(self.render_toolbar(cx))
            .child(self.render_viewer(cx))
            .child(self.render_timeline(cx))
            .child(self.render_status())
    }
}
