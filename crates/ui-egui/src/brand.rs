//! Original typographic identity for the Aphrody Video fork; no embedded trademark artwork.

/// Draw the identity with the editor's existing font and semantic foreground color.
/// Keep the optional rect contract used by the import and About panels.
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, _dark: bool) -> Option<egui::Rect> {
    if !height.is_finite() || height <= 0.0 || !left_center.x.is_finite() || !left_center.y.is_finite() {
        return None;
    }
    let font = crate::theme::Tokens::semibold(height.min(320.0));
    Some(ui.painter().text(left_center, egui::Align2::LEFT_CENTER, "Aphrody Video", font, ui.visuals().text_color()))
}
