//! Render the main window headlessly (GPUI's offscreen renderer, no display needed) and save it as
//! a PNG: `cargo run --example screenshot -- out.png [project.fcproj]`. Without a project it shows
//! the demo project, with the playhead and a selected clip.

use std::sync::Arc;

use filmcraft_engine::Session;
use filmcraft_ui_gpui::{Workspace, load_fonts};
use gpui::{AppContext as _, HeadlessAppContext, PlatformHeadlessRenderer, px, size};
use serde_json::json;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = args.next().ok_or("usage: screenshot <out.png> [project.fcproj]")?;
    let mut session = Session::default();
    let opened = match args.next() {
        Some(path) => session.execute("file.open", json!({"path": path})),
        None => session.execute("file.openDemoProject", json!({})),
    };
    opened.map_err(|e| e.to_string())?;

    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new("Inter"));
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), || {
        let renderer: Box<dyn PlatformHeadlessRenderer> = Box::new(gpui_wgpu::WgpuHeadlessRenderer::new()?);
        Ok(Some(renderer))
    });
    cx.update(|app| load_fonts(app));
    cx.allow_parking();
    let window = cx.open_window(size(px(1600.0), px(980.0)), |window, app| app.new(|cx| Workspace::new(session, window, cx))).map_err(|e| e.to_string())?;
    // the first frame renders on a background thread; let it land, then draw again
    for _ in 0..50 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        let ready = cx.update_window(window.into(), |view, _, app| view.downcast::<Workspace>().ok().is_some_and(|w| w.read(app).shown_frame().is_some()));
        if ready.unwrap_or(false) {
            break;
        }
    }
    cx.update_window(window.into(), |_, window, _| window.refresh()).map_err(|e| e.to_string())?;
    cx.run_until_parked();
    let image = cx.capture_screenshot(window.into()).map_err(|e| e.to_string())?;
    image.save(&out).map_err(|e| e.to_string())?;
    println!("wrote {out} ({}x{})", image.width(), image.height());
    Ok(())
}
