//! The toolbar, the status bar and the button every panel uses.

use gpui::{
    ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use serde_json::json;

use crate::workspace::{Tool, Workspace, color};

impl Workspace {
    /// A button: a visible label, a role and an accessible name, a tab stop, and a click handler.
    pub(crate) fn button(
        &self,
        id: &'static str,
        label: &'static str,
        name: &'static str,
        active: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let t = self.tokens;
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded(px(t.radius_sm))
            .border_1()
            .border_color(color(if active { t.focus } else { t.button_border }))
            .bg(color(if active { t.pill_active_bg } else { t.field_bg }))
            .text_color(color(t.text))
            .cursor_pointer()
            .hover(move |s| s.bg(color(t.hover)))
            .focus_visible(move |s| s.border_color(color(t.focus)))
            .role(Role::Button)
            .aria_label(SharedString::from(name))
            .focusable()
            .tab_stop(true)
            .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| on_click(this, cx)))
            .child(label)
    }

    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.tokens;
        let tool = self.tool;
        div()
            .id("toolbar")
            .role(Role::Toolbar)
            .aria_label("Tools")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(t.gap))
            .p(px(t.gap))
            .h(px(t.tab_h))
            .bg(color(t.header_bg))
            .child(self.button("open", "Open…", "Open project", false, cx, |this, cx| this.open_dialog(cx)))
            .child(self.button("demo", "Demo", "New demo project", false, cx, |this, cx| {
                let _ = this.dispatch("file.openDemoProject", json!({}), cx);
            }))
            .child(self.button("save", "Save", "Save project", false, cx, |this, cx| {
                let _ = this.dispatch("file.save", json!({}), cx);
            }))
            .child(self.button("undo", "Undo", "Undo", false, cx, |this, cx| {
                let _ = this.dispatch("edit.undo", json!({}), cx);
            }))
            .child(self.button("redo", "Redo", "Redo", false, cx, |this, cx| {
                let _ = this.dispatch("edit.redo", json!({}), cx);
            }))
            .child(div().w(px(t.gap * 3.0)))
            .child(self.button("tool-selection", "Select", "Selection tool", tool == Tool::Selection, cx, |this, cx| this.set_tool(Tool::Selection, cx)))
            .child(self.button("tool-razor", "Razor", "Razor tool", tool == Tool::Razor, cx, |this, cx| this.set_tool(Tool::Razor, cx)))
            .child(div().w(px(t.gap * 3.0)))
            .child(self.button("zoom-out", "−", "Zoom timeline out", false, cx, |this, cx| this.zoom(0.8, cx)))
            .child(self.button("zoom-in", "+", "Zoom timeline in", false, cx, |this, cx| this.zoom(1.25, cx)))
    }

    pub(crate) fn render_status(&self) -> impl IntoElement {
        let t = self.tokens;
        let dirty = self.session.is_dirty();
        div()
            .id("status")
            .role(Role::Status)
            .aria_label(self.status.clone())
            .h(px(22.0))
            .px_2()
            .flex()
            .items_center()
            .bg(color(t.header_bg))
            .text_color(color(t.text_dim))
            .child(format!("{}{}", self.status, if dirty { "  ·  unsaved changes" } else { "" }))
    }

    /// File ▸ Open…: ask for a project file, then open it with `file.open`.
    pub(crate) fn open_dialog(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.first() else { return };
            let path = path.to_string_lossy().into_owned();
            this.update(cx, |this, cx| {
                let _ = this.open_project(&path, cx);
            })
            .ok();
        })
        .detach();
    }
}
