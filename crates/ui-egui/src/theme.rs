//! Design tokens. Every widget reads colours/sizes from [`Tokens`] so themes apply everywhere.
//!
//! The default theme follows the look of Premiere Pro's current dark UI (values measured from
//! black-box screenshots, see `plan/premiere/02-ui-ux.md`); fonts are Inter + JetBrains Mono (OFL).

use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};
pub use filmcraft_theme::ThemeKind;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub kind: ThemeKind,
    /// Space between panels / app background.
    pub app_bg: Color32,
    /// Header bar (top).
    pub header_bg: Color32,
    /// Panel body.
    pub panel_bg: Color32,
    /// Panel tab strip.
    pub tab_bg: Color32,
    pub tab_text: Color32,
    pub tab_text_active: Color32,
    /// Blue focus rectangle around the active panel.
    pub focus: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub icon: Color32,
    pub icon_active: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub field_bg: Color32,
    pub field_border: Color32,
    pub separator: Color32,
    pub row_alt: Color32,
    pub row_selected: Color32,
    /// Scrubby (hot-text) numeric value colour.
    pub hot_text: Color32,
    /// Selected pills and compact toggle backgrounds.
    pub pill_active_bg: Color32,
    /// The selected clip bar in Effect Controls.
    pub clip_bar_bg: Color32,
    /// Raised audio control surfaces, such as mixer channel strips.
    pub control_surface: Color32,
    /// Recessed audio control tracks and panner wells.
    pub control_well: Color32,
    pub control_border: Color32,
    pub control_handle_bg: Color32,
    pub control_handle: Color32,
    pub control_handle_dim: Color32,
    /// Data-plot surfaces (keyframe, EQ and dynamics graphs), distinct from media canvases.
    pub plot_bg: Color32,
    pub plot_grid: Color32,
    pub plot_axis: Color32,
    pub plot_handle: Color32,
    pub plot_handle_dim: Color32,
    /// Meter wells and the separate loudness readout surface.
    pub meter_bg: Color32,
    pub meter_readout_bg: Color32,
    pub meter_clip_off: Color32,
    pub panner_bg: Color32,
    pub panner_border: Color32,
    pub panner_speaker: Color32,
    pub fader_knob: Color32,
    pub eq_track: Color32,
    pub eq_tick: Color32,
    pub eq_knob: Color32,
    pub eq_knob_active: Color32,
    pub eq_node: Color32,
    pub eq_node_off: Color32,
    pub crossover_handle: Color32,
    pub keyframe_plot_bg: Color32,
    pub keyframe_plot_grid: Color32,
    pub keyframe_plot_axis: Color32,
    pub keyframe_handle: Color32,
    pub switch_fill: Color32,
    pub switch_knob: Color32,
    pub switch_border: Color32,
    pub slider_disabled_track: Color32,
    pub slider_disabled_knob: Color32,
    pub button_hover: Color32,
    pub button_border: Color32,
    pub caption_track_bg: Color32,
    pub scroll_thumb: Color32,
    pub scroll_thumb_hover: Color32,
    pub vertical_scroll_thumb: Color32,
    pub disabled_clip_bg: Color32,
    // timeline
    pub tl_bg: Color32,
    pub tl_track_bg: Color32,
    pub tl_track_bg_alt: Color32,
    pub tl_header_bg: Color32,
    pub tl_ruler_bg: Color32,
    pub tl_ruler_tick: Color32,
    pub tl_ruler_text: Color32,
    pub playhead: Color32,
    /// Fill of a track header's source-patch / track-targeting button while it is on.
    pub track_target: Color32,
    pub in_out_shade: Color32,
    pub clip_selected_border: Color32,
    pub render_red: Color32,
    pub render_yellow: Color32,
    pub render_green: Color32,
    pub monitor_bg: Color32,
    pub timecode: Color32,
    pub danger: Color32,
    pub radius: f32,
    pub radius_sm: f32,
    pub gap: f32,
    pub tab_h: f32,
}

impl Tokens {
    /// The theme's tokens in egui's colour type. The values live in `filmcraft-theme`.
    pub fn for_kind(kind: ThemeKind) -> Self {
        Self::from_neutral(&filmcraft_theme::Tokens::for_kind(kind))
    }

    fn from_neutral(n: &filmcraft_theme::Tokens) -> Self {
        let c = |c: filmcraft_theme::Rgba| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
        Tokens {
            kind: n.kind,
            app_bg: c(n.app_bg),
            header_bg: c(n.header_bg),
            panel_bg: c(n.panel_bg),
            tab_bg: c(n.tab_bg),
            tab_text: c(n.tab_text),
            tab_text_active: c(n.tab_text_active),
            focus: c(n.focus),
            accent: c(n.accent),
            accent_hover: c(n.accent_hover),
            text: c(n.text),
            text_dim: c(n.text_dim),
            text_faint: c(n.text_faint),
            icon: c(n.icon),
            icon_active: c(n.icon_active),
            hover: c(n.hover),
            pressed: c(n.pressed),
            field_bg: c(n.field_bg),
            field_border: c(n.field_border),
            separator: c(n.separator),
            row_alt: c(n.row_alt),
            row_selected: c(n.row_selected),
            hot_text: c(n.hot_text),
            pill_active_bg: c(n.pill_active_bg),
            clip_bar_bg: c(n.clip_bar_bg),
            control_surface: c(n.control_surface),
            control_well: c(n.control_well),
            control_border: c(n.control_border),
            control_handle_bg: c(n.control_handle_bg),
            control_handle: c(n.control_handle),
            control_handle_dim: c(n.control_handle_dim),
            plot_bg: c(n.plot_bg),
            plot_grid: c(n.plot_grid),
            plot_axis: c(n.plot_axis),
            plot_handle: c(n.plot_handle),
            plot_handle_dim: c(n.plot_handle_dim),
            meter_bg: c(n.meter_bg),
            meter_readout_bg: c(n.meter_readout_bg),
            meter_clip_off: c(n.meter_clip_off),
            panner_bg: c(n.panner_bg),
            panner_border: c(n.panner_border),
            panner_speaker: c(n.panner_speaker),
            fader_knob: c(n.fader_knob),
            eq_track: c(n.eq_track),
            eq_tick: c(n.eq_tick),
            eq_knob: c(n.eq_knob),
            eq_knob_active: c(n.eq_knob_active),
            eq_node: c(n.eq_node),
            eq_node_off: c(n.eq_node_off),
            crossover_handle: c(n.crossover_handle),
            keyframe_plot_bg: c(n.keyframe_plot_bg),
            keyframe_plot_grid: c(n.keyframe_plot_grid),
            keyframe_plot_axis: c(n.keyframe_plot_axis),
            keyframe_handle: c(n.keyframe_handle),
            switch_fill: c(n.switch_fill),
            switch_knob: c(n.switch_knob),
            switch_border: c(n.switch_border),
            slider_disabled_track: c(n.slider_disabled_track),
            slider_disabled_knob: c(n.slider_disabled_knob),
            button_hover: c(n.button_hover),
            button_border: c(n.button_border),
            caption_track_bg: c(n.caption_track_bg),
            scroll_thumb: c(n.scroll_thumb),
            scroll_thumb_hover: c(n.scroll_thumb_hover),
            vertical_scroll_thumb: c(n.vertical_scroll_thumb),
            disabled_clip_bg: c(n.disabled_clip_bg),
            tl_bg: c(n.tl_bg),
            tl_track_bg: c(n.tl_track_bg),
            tl_track_bg_alt: c(n.tl_track_bg_alt),
            tl_header_bg: c(n.tl_header_bg),
            tl_ruler_bg: c(n.tl_ruler_bg),
            tl_ruler_tick: c(n.tl_ruler_tick),
            tl_ruler_text: c(n.tl_ruler_text),
            playhead: c(n.playhead),
            track_target: c(n.track_target),
            in_out_shade: c(n.in_out_shade),
            clip_selected_border: c(n.clip_selected_border),
            render_red: c(n.render_red),
            render_yellow: c(n.render_yellow),
            render_green: c(n.render_green),
            monitor_bg: c(n.monitor_bg),
            timecode: c(n.timecode),
            danger: c(n.danger),
            radius: n.radius,
            radius_sm: n.radius_sm,
            gap: n.gap,
            tab_h: n.tab_h,
        }
    }

    fn to_neutral(self) -> filmcraft_theme::Tokens {
        let c = |c: Color32| filmcraft_theme::Rgba { r: c.r(), g: c.g(), b: c.b(), a: c.a() };
        filmcraft_theme::Tokens {
            kind: self.kind,
            app_bg: c(self.app_bg),
            header_bg: c(self.header_bg),
            panel_bg: c(self.panel_bg),
            tab_bg: c(self.tab_bg),
            tab_text: c(self.tab_text),
            tab_text_active: c(self.tab_text_active),
            focus: c(self.focus),
            accent: c(self.accent),
            accent_hover: c(self.accent_hover),
            text: c(self.text),
            text_dim: c(self.text_dim),
            text_faint: c(self.text_faint),
            icon: c(self.icon),
            icon_active: c(self.icon_active),
            hover: c(self.hover),
            pressed: c(self.pressed),
            field_bg: c(self.field_bg),
            field_border: c(self.field_border),
            separator: c(self.separator),
            row_alt: c(self.row_alt),
            row_selected: c(self.row_selected),
            hot_text: c(self.hot_text),
            pill_active_bg: c(self.pill_active_bg),
            clip_bar_bg: c(self.clip_bar_bg),
            control_surface: c(self.control_surface),
            control_well: c(self.control_well),
            control_border: c(self.control_border),
            control_handle_bg: c(self.control_handle_bg),
            control_handle: c(self.control_handle),
            control_handle_dim: c(self.control_handle_dim),
            plot_bg: c(self.plot_bg),
            plot_grid: c(self.plot_grid),
            plot_axis: c(self.plot_axis),
            plot_handle: c(self.plot_handle),
            plot_handle_dim: c(self.plot_handle_dim),
            meter_bg: c(self.meter_bg),
            meter_readout_bg: c(self.meter_readout_bg),
            meter_clip_off: c(self.meter_clip_off),
            panner_bg: c(self.panner_bg),
            panner_border: c(self.panner_border),
            panner_speaker: c(self.panner_speaker),
            fader_knob: c(self.fader_knob),
            eq_track: c(self.eq_track),
            eq_tick: c(self.eq_tick),
            eq_knob: c(self.eq_knob),
            eq_knob_active: c(self.eq_knob_active),
            eq_node: c(self.eq_node),
            eq_node_off: c(self.eq_node_off),
            crossover_handle: c(self.crossover_handle),
            keyframe_plot_bg: c(self.keyframe_plot_bg),
            keyframe_plot_grid: c(self.keyframe_plot_grid),
            keyframe_plot_axis: c(self.keyframe_plot_axis),
            keyframe_handle: c(self.keyframe_handle),
            switch_fill: c(self.switch_fill),
            switch_knob: c(self.switch_knob),
            switch_border: c(self.switch_border),
            slider_disabled_track: c(self.slider_disabled_track),
            slider_disabled_knob: c(self.slider_disabled_knob),
            button_hover: c(self.button_hover),
            button_border: c(self.button_border),
            caption_track_bg: c(self.caption_track_bg),
            scroll_thumb: c(self.scroll_thumb),
            scroll_thumb_hover: c(self.scroll_thumb_hover),
            vertical_scroll_thumb: c(self.vertical_scroll_thumb),
            disabled_clip_bg: c(self.disabled_clip_bg),
            tl_bg: c(self.tl_bg),
            tl_track_bg: c(self.tl_track_bg),
            tl_track_bg_alt: c(self.tl_track_bg_alt),
            tl_header_bg: c(self.tl_header_bg),
            tl_ruler_bg: c(self.tl_ruler_bg),
            tl_ruler_tick: c(self.tl_ruler_tick),
            tl_ruler_text: c(self.tl_ruler_text),
            playhead: c(self.playhead),
            track_target: c(self.track_target),
            in_out_shade: c(self.in_out_shade),
            clip_selected_border: c(self.clip_selected_border),
            render_red: c(self.render_red),
            render_yellow: c(self.render_yellow),
            render_green: c(self.render_green),
            monitor_bg: c(self.monitor_bg),
            timecode: c(self.timecode),
            danger: c(self.danger),
            radius: self.radius,
            radius_sm: self.radius_sm,
            gap: self.gap,
            tab_h: self.tab_h,
        }
    }

    /// Settings ▸ Appearance on top of a theme: the highlight colour (selections, primary buttons,
    /// the focus outline, the playhead, timecodes, drag values and track targeting) and accessible
    /// colour contrast (brighter secondary text and borders).
    pub fn with_appearance(self, highlight: Option<[u8; 3]>, accessible_contrast: bool) -> Self {
        Self::from_neutral(&self.to_neutral().with_appearance(highlight, accessible_contrast))
    }

    /// Mono font for timecode.
    pub fn mono(size: f32) -> FontId {
        FontId::new(size, FontFamily::Monospace)
    }
    pub fn ui(size: f32) -> FontId {
        FontId::new(size, FontFamily::Proportional)
    }
    pub fn semibold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name("semibold".into()))
    }
    /// The big blue timecode over the timeline and under the monitors: regular weight, as in
    /// Premiere (Premiere's reads 10 px tall and 76 px wide for 00:00:16:05 at 1x).
    pub fn timecode() -> FontId {
        FontId::new(16.0, FontFamily::Proportional)
    }
}

/// Menu text: Premiere's menus are the system's, larger and brighter than panel text, one item
/// every 24 px.
pub const MENU_TEXT_SIZE: f32 = 13.0;
pub const MENU_TEXT: Color32 = Color32::from_rgb(0xde, 0xde, 0xde);

/// Style of the menu bar's menus and their submenus (on top of egui's own menu style).
pub fn menu_style(s: &mut egui::Style) {
    egui::containers::menu::menu_style(s);
    s.text_styles.insert(TextStyle::Button, FontId::new(MENU_TEXT_SIZE, FontFamily::Proportional));
    s.text_styles.insert(TextStyle::Body, FontId::new(MENU_TEXT_SIZE, FontFamily::Proportional));
    s.spacing.item_spacing.y = 0.0;
    if s.visuals.dark_mode {
        s.visuals.override_text_color = Some(MENU_TEXT);
    }
}

/// Every font family the theme defines (fallback fonts such as the Japanese fonts are added to each
/// of them).
pub fn font_families() -> Vec<FontFamily> {
    vec![FontFamily::Proportional, FontFamily::Monospace, FontFamily::Name("semibold".into()), FontFamily::Name("medium".into())]
}

/// Install the Latin UI fonts, Chinese fallback (craft-fonts or an installed face), Japanese
/// craft-fonts, Arabic and Hebrew fallbacks (craft-fonts or installed faces, `crate::rtl`) and
/// egui visuals. Document names keep their glyphs in every interface language.
pub fn install(ctx: &egui::Context, t: &Tokens) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter".into(), Arc::new(FontData::from_static(filmcraft_text::fonts::INTER_REGULAR)));
    fonts.font_data.insert("inter-medium".into(), Arc::new(FontData::from_static(filmcraft_text::fonts::INTER_MEDIUM)));
    fonts.font_data.insert("inter-semibold".into(), Arc::new(FontData::from_static(filmcraft_text::fonts::INTER_SEMIBOLD)));
    fonts.font_data.insert("jbmono".into(), Arc::new(FontData::from_static(filmcraft_text::fonts::JETBRAINS_MONO_REGULAR)));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "jbmono".into());
    fonts.families.insert(FontFamily::Name("semibold".into()), vec!["inter-semibold".into(), "inter".into()]);
    fonts.families.insert(FontFamily::Name("medium".into()), vec!["inter-medium".into(), "inter".into()]);
    // Japanese before Chinese: the two share code points, and Japanese text must keep Japanese
    // glyph forms; Chinese faces still cover the hanzi Japanese fonts lack
    add_craft_fonts(&mut fonts);
    crate::cjk::install(&mut fonts);
    crate::rtl::install(&mut fonts);
    ctx.set_fonts(fonts);
    apply_visuals(ctx, t);
}

/// Name of the egui font for a craft-fonts entry.
fn craft_font_name(f: &filmcraft_text::fonts::CraftFont) -> String {
    format!("craft:{} {}", f.family, f.style)
}

/// Append the Japanese craft-fonts (empty unless built with `CRAFT_FONTS_DIR`) as the last fallbacks
/// of every font family, after the app's own fonts: BIZ UDPGothic first (bold before regular in
/// the "semibold" and "medium" families), then the other Japanese faces.
fn add_craft_fonts(fonts: &mut FontDefinitions) {
    let jpan: Vec<_> = filmcraft_text::fonts::craft_japanese().collect();
    for f in &jpan {
        fonts.font_data.insert(craft_font_name(f), Arc::new(FontData::from_static(f.bytes)));
    }
    for (family, stack) in fonts.families.iter_mut() {
        let heavy = matches!(family, FontFamily::Name(n) if matches!(n.as_ref(), "semibold" | "medium"));
        let mut order = jpan.clone();
        order.sort_by_key(|f| (!f.family.contains("Gothic"), (f.style == "Bold") != heavy));
        stack.extend(order.iter().map(|f| craft_font_name(f)));
    }
}

pub fn apply_visuals(ctx: &egui::Context, t: &Tokens) {
    let light = t.kind == ThemeKind::Light;
    // FilmCraft decides between light and dark itself (Settings ▸ Appearance ▸ Appearance Mode), so
    // egui must not switch to its other built-in style when the system appearance changes.
    ctx.set_theme(if light { egui::Theme::Light } else { egui::Theme::Dark });
    let mut v = if light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = t.panel_bg;
    v.window_fill = t.panel_bg;
    v.extreme_bg_color = t.field_bg;
    v.faint_bg_color = t.row_alt;
    v.override_text_color = Some(t.text);
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = t.accent;
    v.window_stroke = Stroke::new(1.0, t.field_border);
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.menu_corner_radius = egui::CornerRadius::same(6);
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(140) };
    v.window_shadow = v.popup_shadow;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(t.radius_sm as u8);
    }
    v.widgets.noninteractive.bg_fill = t.panel_bg;
    v.widgets.noninteractive.weak_bg_fill = t.panel_bg;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.separator);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.inactive.bg_fill = t.field_bg;
    v.widgets.inactive.weak_bg_fill = t.field_bg;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.field_border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.field_border);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, t.tab_text_active);
    v.widgets.active.bg_fill = t.pressed;
    v.widgets.active.weak_bg_fill = t.pressed;
    v.widgets.active.fg_stroke = Stroke::new(1.0, t.tab_text_active);
    v.widgets.open.bg_fill = t.hover;
    v.widgets.open.weak_bg_fill = t.hover;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.menu_margin = egui::Margin::same(4);
        s.text_styles.insert(TextStyle::Body, FontId::new(12.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(12.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(15.0, FontFamily::Name("semibold".into())));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace));
        s.animation_time = ANIMATION_TIME;
    });
}

/// How long egui's fades and slides take (menus, collapsing sections, tooltips); 0 when
/// Settings ▸ Appearance ▸ "Animate zoom, scrolling and menus" is off (#2325).
pub const ANIMATION_TIME: f32 = 0.12;

#[cfg(test)]
mod tests {
    use super::*;

    const JAPANESE: &str = "日本語の文字";

    fn installed() -> egui::Context {
        let ctx = egui::Context::default();
        install(&ctx, &Tokens::for_kind(ThemeKind::default()));
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        ctx
    }

    #[test]
    fn light_theme_has_light_chrome_and_plot_surfaces() {
        let dark = Tokens::for_kind(ThemeKind::Dark);
        let light = Tokens::for_kind(ThemeKind::Light);
        let luminance = |c: Color32| u16::from(c.r()) + u16::from(c.g()) + u16::from(c.b());

        for (name, dark_surface, light_surface) in [
            ("active pill", dark.pill_active_bg, light.pill_active_bg),
            ("clip bar", dark.clip_bar_bg, light.clip_bar_bg),
            ("control surface", dark.control_surface, light.control_surface),
            ("control well", dark.control_well, light.control_well),
            ("plot", dark.plot_bg, light.plot_bg),
            ("keyframe plot", dark.keyframe_plot_bg, light.keyframe_plot_bg),
            ("panner", dark.panner_bg, light.panner_bg),
            ("caption lane", dark.caption_track_bg, light.caption_track_bg),
            ("active toggle", dark.pressed, light.pressed),
            ("meter", dark.meter_bg, light.meter_bg),
            ("meter readout", dark.meter_readout_bg, light.meter_readout_bg),
        ] {
            assert!(luminance(light_surface) > luminance(dark_surface), "{name} did not adapt to Light");
        }
        assert!(luminance(light.control_handle) < luminance(light.control_handle_bg));
        assert!(luminance(light.plot_handle) < luminance(light.plot_bg));
        assert!(luminance(light.icon_active) < luminance(light.pressed));
        assert_eq!(Tokens::for_kind(ThemeKind::Medium).plot_bg, dark.plot_bg, "Medium keeps the established dark plot treatment");
    }

    #[test]
    fn dark_and_medium_keep_existing_audio_and_effect_palettes() {
        for kind in [ThemeKind::Dark, ThemeKind::Medium] {
            let t = Tokens::for_kind(kind);
            // Existing painted colors, recorded independently of the Light palette.
            assert_eq!(t.pill_active_bg, Color32::from_gray(0x3a));
            assert_eq!(t.clip_bar_bg, Color32::from_rgb(58, 58, 70));
            assert_eq!(t.control_surface, Color32::from_gray(0x24));
            assert_eq!(t.control_well, Color32::from_gray(0x10));
            assert_eq!(t.control_handle, Color32::from_gray(0xe0));
            assert_eq!(t.meter_bg, Color32::BLACK);
            assert_eq!(t.plot_bg, Color32::from_gray(0x18));
            assert_eq!(t.plot_grid, Color32::from_gray(0x2c));
            assert_eq!(t.keyframe_plot_bg, Color32::from_gray(0x19));
            assert_eq!(t.keyframe_plot_grid, Color32::from_gray(0x2a));
            assert_eq!(t.keyframe_plot_axis, Color32::from_gray(0x33));
            assert_eq!(t.keyframe_handle, Color32::from_gray(0xd0));
        }
    }

    /// The colours the highlight drives besides the accent (#707).
    fn highlight_blues(t: &Tokens) -> [Color32; 5] {
        [t.focus, t.playhead, t.timecode, t.hot_text, t.track_target]
    }

    #[test]
    fn the_default_highlight_leaves_the_theme_blues_as_designed() {
        let default = filmcraft_engine::settings::parse_hex(filmcraft_engine::settings::DEFAULT_HIGHLIGHT);
        for k in [ThemeKind::Dark, ThemeKind::Medium, ThemeKind::Light] {
            assert_eq!(highlight_blues(&Tokens::for_kind(k).with_appearance(default, false)), highlight_blues(&Tokens::for_kind(k)), "{k:?}");
        }
    }

    #[test]
    fn a_highlight_of_ones_own_colours_the_playhead_timecodes_focus_and_targeting() {
        let pink = Color32::from_rgb(0xe0, 0x45, 0x7b);
        let t = Tokens::for_kind(ThemeKind::Dark).with_appearance(Some([0xe0, 0x45, 0x7b]), false);
        let hue = |c: Color32| egui::ecolor::HsvaGamma::from(c).h;
        for (c, was) in highlight_blues(&t).into_iter().zip(highlight_blues(&Tokens::for_kind(ThemeKind::Dark))) {
            assert_ne!(c, was);
            assert!((hue(c) - hue(pink)).abs() < 0.01, "{c:?} has the highlight's hue");
        }
        let v = |c: Color32| egui::ecolor::HsvaGamma::from(c).v;
        assert!(v(t.playhead) > v(pink) && v(t.timecode) > v(pink) && v(t.track_target) < v(pink), "lighter and darker as the default blues are");
    }

    /// Chinese media and track names must render even when the interface stays in English.
    /// Compare the rasterized glyphs with the missing-glyph box, including custom weight families.
    #[test]
    fn chinese_names_render_in_every_theme_and_font_family() {
        const CHINESE: &str = "中文旁白音乐声轨测试简体汉语繁體漢語车";
        filmcraft_text::fonts::scan_system();
        let available = filmcraft_text::fonts::all_faces()
            .iter()
            .any(|f| matches!(f.info.origin, "system" | filmcraft_text::fonts::CRAFT_ORIGIN) && f.covers_text(CHINESE));
        if !available {
            eprintln!("SKIPPED: no craft-fonts or installed Chinese font");
            return;
        }
        let ctx = egui::Context::default();
        for kind in [ThemeKind::Dark, ThemeKind::Medium, ThemeKind::Light] {
            install(&ctx, &Tokens::for_kind(kind));
            ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
            ctx.fonts_mut(|fonts| {
                for family in font_families() {
                    let font = FontId::new(13.0, family);
                    let missing = fonts.layout_no_wrap("\u{fffd}".into(), font.clone(), Color32::WHITE);
                    let missing_uv = missing.rows[0].glyphs[0].uv_rect;
                    let names = fonts.layout_no_wrap(CHINESE.into(), font.clone(), Color32::WHITE);
                    assert_eq!(names.rows.iter().map(|r| r.glyphs.len()).sum::<usize>(), CHINESE.chars().count());
                    for glyph in names.rows.iter().flat_map(|r| &r.glyphs) {
                        assert!(!glyph.uv_rect.is_nothing(), "empty {} in {font:?}", glyph.chr);
                        assert_ne!(glyph.uv_rect, missing_uv, "missing {} in {kind:?} / {font:?}", glyph.chr);
                    }
                }
            });
        }
        // the layouts above put glyphs in the atlas: take that update so it isn't dropped unapplied
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    }

    /// Typed, pasted or IME-composed Hangul draws real glyphs, not the missing-glyph box (artcraft#2093).
    #[test]
    fn korean_text_renders_with_an_installed_korean_font() {
        const KOREAN: &str = "한국어 입력 테스트";
        filmcraft_text::fonts::scan_system();
        // `covers_text`, not `has_char`: probing keeps no font file loaded (see `cjk::find_system_font`)
        let available = filmcraft_text::fonts::all_faces().iter().any(|f| f.info.origin == "system" && f.covers_text(KOREAN));
        if !available {
            eprintln!("SKIPPED: no installed Korean font");
            return;
        }
        let ctx = egui::Context::default();
        install(&ctx, &Tokens::for_kind(ThemeKind::Dark));
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        ctx.fonts_mut(|fonts| {
            for family in font_families() {
                let font = FontId::new(13.0, family);
                let missing = fonts.layout_no_wrap("\u{fffd}".into(), font.clone(), Color32::WHITE);
                let missing_uv = missing.rows[0].glyphs[0].uv_rect;
                let text = fonts.layout_no_wrap(KOREAN.into(), font.clone(), Color32::WHITE);
                for glyph in text.rows.iter().flat_map(|r| &r.glyphs).filter(|g| g.chr != ' ') {
                    assert_ne!(glyph.uv_rect, missing_uv, "missing {} in {font:?}", glyph.chr);
                }
            }
        });
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    }

    /// Built with craft-fonts: every font family ends with the Japanese faces (BIZ UDPGothic
    /// first), and Japanese text gets real glyphs, not the replacement box.
    #[test]
    fn japanese_renders_with_craft_fonts() {
        if filmcraft_text::fonts::craft_japanese().next().is_none() {
            eprintln!("SKIPPED: built without craft-fonts (set CRAFT_FONTS_DIR to run)");
            return;
        }
        let ctx = installed();
        ctx.fonts_mut(|fonts| {
            let defs = fonts.definitions().clone();
            for (family, stack) in &defs.families {
                // installed faces come after the craft run: right-to-left ones (Hebrew is not in
                // craft-fonts) and the Hangul face (none is in craft-fonts)
                let stack: Vec<&String> = stack
                    .iter()
                    .filter(|n| !crate::rtl::is_system_face(n) && n.as_str() != crate::cjk::KOREAN_FACE && n.as_str() != crate::cjk::THAI_FACE)
                    .collect();
                let first = stack.iter().position(|n| n.starts_with("craft:")).unwrap_or(stack.len());
                assert!(first > 0 && stack[first..].iter().all(|n| n.starts_with("craft:")), "{family:?}: {stack:?}");
                let first_japanese = stack.iter().find(|n| n.starts_with("craft:BIZ") || n.starts_with("craft:Shippori"));
                assert!(first_japanese.is_some_and(|n| n.starts_with("craft:BIZ UDPGothic")), "{family:?}: {stack:?}");
            }
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                let font = FontId::new(13.0, family);
                for ch in JAPANESE.chars() {
                    assert!(fonts.has_glyph(&font, ch), "missing {ch} in {font:?}");
                }
            }
        });
        // laid out and drawn: the replacement box (U+FFFD) would show as one glyph per character
        // from the fallback face; a real face gives each character its own width
        let galley = ctx.fonts_mut(|f| f.layout_no_wrap(JAPANESE.into(), FontId::new(13.0, FontFamily::Proportional), Color32::WHITE));
        assert_eq!(galley.rows.iter().map(|r| r.glyphs.len()).sum::<usize>(), JAPANESE.chars().count());
        assert!(galley.size().x > 13.0 * 4.0, "{:?}", galley.size());
    }

    /// Latin text keeps its UI faces, with or without craft-fonts and system Chinese fonts.
    #[test]
    fn works_without_craft_fonts() {
        let ctx = installed();
        let n = filmcraft_text::fonts::craft_japanese().count()
            + crate::cjk::craft_fonts().count()
            + crate::rtl::craft_fonts().count()
            + crate::cjk::thai_craft_fonts().count();
        ctx.fonts_mut(|fonts| {
            let defs = fonts.definitions().clone();
            assert_eq!(defs.font_data.keys().filter(|k| k.starts_with("craft:")).count(), n);
            assert_eq!(defs.families[&FontFamily::Proportional].first().map(String::as_str), Some("inter"));
            assert_eq!(defs.families[&FontFamily::Monospace].first().map(String::as_str), Some("jbmono"));
            assert!(fonts.has_glyph(&FontId::new(13.0, FontFamily::Proportional), 'A'));
            if n == 0 {
                assert!(defs.families.values().flatten().all(|name| !name.starts_with("craft:")));
            }
        });
    }
}
