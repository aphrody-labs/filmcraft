//! Starting the app: fonts, window, event loop.

use std::borrow::Cow;

use filmcraft_engine::Session;
use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};

use crate::workspace::Workspace;

/// The proportional family every panel uses.
pub const FONT_FAMILY: &str = "Inter";

/// Register the bundled Inter and JetBrains Mono faces (`filmcraft-text`).
pub fn load_fonts(cx: &App) {
    use filmcraft_text::fonts::{INTER_MEDIUM, INTER_REGULAR, INTER_SEMIBOLD, JETBRAINS_MONO_REGULAR};
    let faces = vec![Cow::Borrowed(INTER_REGULAR), Cow::Borrowed(INTER_MEDIUM), Cow::Borrowed(INTER_SEMIBOLD), Cow::Borrowed(JETBRAINS_MONO_REGULAR)];
    if let Err(e) = cx.text_system().add_fonts(faces) {
        log::warn!("bundled fonts not loaded: {e:#}");
    }
}

/// Open the main window on `session` and run until it is closed.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn run(session: Session) -> Result<(), String> {
    let app = gpui::Application::with_platform(gpui_linux::current_platform(false));
    app.run(move |cx: &mut App| {
        load_fonts(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1600.0), px(980.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions { title: Some("FilmCraft".into()), ..Default::default() }),
                ..Default::default()
            },
            |window, cx| {
                let workspace = cx.new(|cx| Workspace::new(session, window, cx));
                gpui::Focusable::focus_handle(&workspace, cx).focus(window, cx);
                workspace
            },
        );
        if let Err(e) = opened {
            log::error!("cannot open the window: {e:#}");
            cx.quit();
        }
        cx.activate(true);
    });
    Ok(())
}

/// GPUI has no platform here yet (see the workspace manifest for why macOS is missing).
#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
pub fn run(_session: Session) -> Result<(), String> {
    Err("the GPUI frontend is not available on this operating system yet".into())
}
