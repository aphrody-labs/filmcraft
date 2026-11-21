//! Toolkit-neutral design tokens of the FilmCraft interface.
//!
//! Every frontend (egui, GPUI) reads its colours, sizes and spacing from [`Tokens`], so a theme
//! changes in one place. Colours are plain 8-bit sRGB ([`Rgba`]); a frontend converts them to its
//! own colour type. The default theme follows the look of Premiere Pro's current dark UI (values
//! measured from black-box screenshots); fonts are Inter and JetBrains Mono (OFL), bundled by
//! `filmcraft-text`.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use serde::{Deserialize, Serialize};

/// An sRGB colour with straight alpha, 8 bits per channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba::from_rgb(0, 0, 0);
    pub const WHITE: Rgba = Rgba::from_rgb(255, 255, 255);
    pub const TRANSPARENT: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 255 }
    }
    pub const fn from_gray(v: u8) -> Self {
        Rgba::from_rgb(v, v, v)
    }
    /// `0xRRGGBB` as an opaque colour.
    pub const fn from_hex(rgb: u32) -> Self {
        Rgba::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }
    /// The colour with its alpha replaced.
    pub const fn with_alpha(self, a: u8) -> Self {
        Rgba { a, ..self }
    }
    /// Saturation and value (in gamma space, as the HSV of the sRGB bytes) scaled by `s` and `v`.
    pub fn tinted(self, s: f32, v: f32) -> Self {
        let (r, g, b) = (f32::from(self.r) / 255.0, f32::from(self.g) / 255.0, f32::from(self.b) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta <= f32::EPSILON {
            0.0
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        let sat = (if max <= f32::EPSILON { 0.0 } else { delta / max } * s).clamp(0.0, 1.0);
        let val = (max * v).clamp(0.0, 1.0);
        let c = val * sat;
        let x = c * (1.0 - ((hue % 2.0) - 1.0).abs());
        let m = val - c;
        let (r1, g1, b1) = match hue as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let byte = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgba { r: byte(r1), g: byte(g1), b: byte(b1), a: self.a }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeKind {
    /// Premiere-style darkest (default).
    #[default]
    Dark,
    /// Slightly lighter grey panels (Premiere's brightness slider mid position).
    Medium,
    Light,
}

impl ThemeKind {
    /// Settings ▸ Appearance ▸ Color Theme value (`darkest`, `dark`, `light`).
    pub fn pref_name(self) -> &'static str {
        match self {
            ThemeKind::Dark => "darkest",
            ThemeKind::Medium => "dark",
            ThemeKind::Light => "light",
        }
    }
    pub fn from_pref(s: &str) -> ThemeKind {
        match s {
            "dark" => ThemeKind::Medium,
            "light" => ThemeKind::Light,
            _ => ThemeKind::Dark,
        }
    }
    pub fn from_name(s: &str) -> Option<ThemeKind> {
        match s.to_ascii_lowercase().as_str() {
            "dark" | "darkest" => Some(ThemeKind::Dark),
            "medium" | "grey" | "gray" => Some(ThemeKind::Medium),
            "light" => Some(ThemeKind::Light),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub kind: ThemeKind,
    /// Space between panels / app background.
    pub app_bg: Rgba,
    /// Header bar (top).
    pub header_bg: Rgba,
    /// Panel body.
    pub panel_bg: Rgba,
    /// Panel tab strip.
    pub tab_bg: Rgba,
    pub tab_text: Rgba,
    pub tab_text_active: Rgba,
    /// Blue focus rectangle around the active panel.
    pub focus: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub text: Rgba,
    pub text_dim: Rgba,
    pub text_faint: Rgba,
    pub icon: Rgba,
    pub icon_active: Rgba,
    pub hover: Rgba,
    pub pressed: Rgba,
    pub field_bg: Rgba,
    pub field_border: Rgba,
    pub separator: Rgba,
    pub row_alt: Rgba,
    pub row_selected: Rgba,
    /// Scrubby (hot-text) numeric value colour.
    pub hot_text: Rgba,
    /// Selected pills and compact toggle backgrounds.
    pub pill_active_bg: Rgba,
    /// The selected clip bar in Effect Controls.
    pub clip_bar_bg: Rgba,
    /// Raised audio control surfaces, such as mixer channel strips.
    pub control_surface: Rgba,
    /// Recessed audio control tracks and panner wells.
    pub control_well: Rgba,
    pub control_border: Rgba,
    pub control_handle_bg: Rgba,
    pub control_handle: Rgba,
    pub control_handle_dim: Rgba,
    /// Data-plot surfaces (keyframe, EQ and dynamics graphs), distinct from media canvases.
    pub plot_bg: Rgba,
    pub plot_grid: Rgba,
    pub plot_axis: Rgba,
    pub plot_handle: Rgba,
    pub plot_handle_dim: Rgba,
    /// Meter wells and the separate loudness readout surface.
    pub meter_bg: Rgba,
    pub meter_readout_bg: Rgba,
    pub meter_clip_off: Rgba,
    pub panner_bg: Rgba,
    pub panner_border: Rgba,
    pub panner_speaker: Rgba,
    pub fader_knob: Rgba,
    pub eq_track: Rgba,
    pub eq_tick: Rgba,
    pub eq_knob: Rgba,
    pub eq_knob_active: Rgba,
    pub eq_node: Rgba,
    pub eq_node_off: Rgba,
    pub crossover_handle: Rgba,
    pub keyframe_plot_bg: Rgba,
    pub keyframe_plot_grid: Rgba,
    pub keyframe_plot_axis: Rgba,
    pub keyframe_handle: Rgba,
    pub switch_fill: Rgba,
    pub switch_knob: Rgba,
    pub switch_border: Rgba,
    pub slider_disabled_track: Rgba,
    pub slider_disabled_knob: Rgba,
    pub button_hover: Rgba,
    pub button_border: Rgba,
    pub caption_track_bg: Rgba,
    pub scroll_thumb: Rgba,
    pub scroll_thumb_hover: Rgba,
    pub vertical_scroll_thumb: Rgba,
    pub disabled_clip_bg: Rgba,
    // timeline
    pub tl_bg: Rgba,
    pub tl_track_bg: Rgba,
    pub tl_track_bg_alt: Rgba,
    pub tl_header_bg: Rgba,
    pub tl_ruler_bg: Rgba,
    pub tl_ruler_tick: Rgba,
    pub tl_ruler_text: Rgba,
    pub playhead: Rgba,
    /// Fill of a track header's source-patch / track-targeting button while it is on.
    pub track_target: Rgba,
    pub in_out_shade: Rgba,
    pub clip_selected_border: Rgba,
    pub render_red: Rgba,
    pub render_yellow: Rgba,
    pub render_green: Rgba,
    pub monitor_bg: Rgba,
    pub timecode: Rgba,
    pub danger: Rgba,
    pub radius: f32,
    pub radius_sm: f32,
    pub gap: f32,
    pub tab_h: f32,
}

impl Tokens {
    pub fn for_kind(kind: ThemeKind) -> Self {
        // Measured from Premiere 26 "Darkest" (plan/premiere/02-ui-ux.md §1).
        let dark = Tokens {
            kind,
            app_bg: Rgba::from_rgb(0, 0, 0),
            header_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            panel_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tab_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tab_text: Rgba::from_rgb(0xb0, 0xb0, 0xb0),
            tab_text_active: Rgba::from_rgb(0xd1, 0xd1, 0xd1),
            focus: Rgba::from_rgb(0x57, 0x94, 0xec),
            accent: Rgba::from_rgb(0x2f, 0x6b, 0xdf),
            accent_hover: Rgba::from_rgb(0x3f, 0x7c, 0xe8),
            text: Rgba::from_rgb(0xd1, 0xd1, 0xd1),
            text_dim: Rgba::from_rgb(0xb0, 0xb0, 0xb0),
            text_faint: Rgba::from_rgb(0x6e, 0x6e, 0x6e),
            icon: Rgba::from_rgb(0xb0, 0xb0, 0xb0),
            icon_active: Rgba::from_rgb(0xd1, 0xd1, 0xd1),
            hover: Rgba::from_rgb(0x2c, 0x2c, 0x2c),
            pressed: Rgba::from_rgb(0x4b, 0x4b, 0x4b),
            field_bg: Rgba::from_rgb(0x0e, 0x0e, 0x0e),
            field_border: Rgba::from_rgb(0x30, 0x30, 0x30),
            separator: Rgba::from_rgb(0x30, 0x30, 0x30),
            row_alt: Rgba::from_rgb(0x21, 0x21, 0x21),
            row_selected: Rgba::from_rgb(0x33, 0x33, 0x33),
            hot_text: Rgba::from_rgb(0x40, 0x96, 0xf3),
            pill_active_bg: Rgba::from_rgb(0x3a, 0x3a, 0x3a),
            clip_bar_bg: Rgba::from_rgb(58, 58, 70),
            control_surface: Rgba::from_rgb(0x24, 0x24, 0x24),
            control_well: Rgba::from_rgb(0x10, 0x10, 0x10),
            control_border: Rgba::from_rgb(0x50, 0x50, 0x50),
            control_handle_bg: Rgba::from_rgb(0x2a, 0x2a, 0x2a),
            control_handle: Rgba::from_rgb(0xe0, 0xe0, 0xe0),
            control_handle_dim: Rgba::from_rgb(0xb0, 0xb0, 0xb0),
            plot_bg: Rgba::from_rgb(0x18, 0x18, 0x18),
            plot_grid: Rgba::from_rgb(0x2c, 0x2c, 0x2c),
            plot_axis: Rgba::from_rgb(0x44, 0x44, 0x44),
            plot_handle: Rgba::from_rgb(0xe0, 0xe0, 0xe0),
            plot_handle_dim: Rgba::from_rgb(0x90, 0x90, 0x90),
            meter_bg: Rgba::BLACK,
            meter_readout_bg: Rgba::from_rgb(0x14, 0x14, 0x14),
            meter_clip_off: Rgba::from_rgb(0x30, 0x30, 0x30),
            panner_bg: Rgba::from_gray(22),
            panner_border: Rgba::from_gray(96),
            panner_speaker: Rgba::from_gray(144),
            fader_knob: Rgba::from_gray(90),
            eq_track: Rgba::from_gray(48),
            eq_tick: Rgba::from_gray(96),
            eq_knob: Rgba::from_gray(192),
            eq_knob_active: Rgba::from_gray(240),
            eq_node: Rgba::from_gray(232),
            eq_node_off: Rgba::from_gray(112),
            crossover_handle: Rgba::from_gray(255),
            keyframe_plot_bg: Rgba::from_gray(25),
            keyframe_plot_grid: Rgba::from_gray(42),
            keyframe_plot_axis: Rgba::from_gray(51),
            keyframe_handle: Rgba::from_gray(208),
            switch_fill: Rgba::from_gray(212),
            switch_knob: Rgba::from_gray(29),
            switch_border: Rgba::from_gray(138),
            slider_disabled_track: Rgba::from_gray(74),
            slider_disabled_knob: Rgba::from_gray(106),
            button_hover: Rgba::from_gray(42),
            button_border: Rgba::from_gray(75),
            caption_track_bg: Rgba::from_rgb(0x23, 0x20, 0x2a),
            scroll_thumb: Rgba::from_gray(0x4b),
            scroll_thumb_hover: Rgba::from_gray(0x6a),
            vertical_scroll_thumb: Rgba::from_gray(80),
            disabled_clip_bg: Rgba::from_gray(0x2a),
            tl_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tl_track_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tl_track_bg_alt: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tl_header_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tl_ruler_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            tl_ruler_tick: Rgba::from_rgb(0x8d, 0x8d, 0x8d),
            tl_ruler_text: Rgba::from_rgb(0xb0, 0xb0, 0xb0),
            playhead: Rgba::from_rgb(0x58, 0x95, 0xec),
            track_target: Rgba::from_rgb(0x26, 0x5b, 0xc1),
            in_out_shade: Rgba::from_rgb(0x3f, 0x3f, 0x3f),
            clip_selected_border: Rgba::from_rgb(0xeb, 0xeb, 0xeb),
            render_red: Rgba::from_rgb(0xe3, 0x48, 0x50),
            render_yellow: Rgba::from_rgb(0xf0, 0xf0, 0x4f),
            render_green: Rgba::from_rgb(0x2d, 0x9d, 0x78),
            monitor_bg: Rgba::from_rgb(0x1d, 0x1d, 0x1d),
            timecode: Rgba::from_rgb(0x40, 0x96, 0xf3),
            danger: Rgba::from_rgb(0xdc, 0x51, 0x3d),
            radius: 0.0,
            radius_sm: 4.0,
            gap: 4.0,
            tab_h: 32.0,
        };
        match kind {
            ThemeKind::Dark => dark,
            ThemeKind::Medium => Tokens {
                app_bg: Rgba::from_rgb(20, 20, 20),
                header_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                panel_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                tab_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                field_bg: Rgba::from_rgb(0x22, 0x22, 0x22),
                tl_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                tl_track_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                tl_track_bg_alt: Rgba::from_rgb(0x32, 0x32, 0x32),
                tl_header_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                tl_ruler_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                monitor_bg: Rgba::from_rgb(0x32, 0x32, 0x32),
                row_alt: Rgba::from_rgb(50, 50, 50),
                hover: Rgba::from_rgb(64, 64, 64),
                ..dark
            },
            ThemeKind::Light => Tokens {
                app_bg: Rgba::from_rgb(180, 180, 180),
                header_bg: Rgba::from_rgb(214, 214, 214),
                panel_bg: Rgba::from_rgb(232, 232, 232),
                tab_bg: Rgba::from_rgb(232, 232, 232),
                tab_text: Rgba::from_rgb(90, 90, 90),
                tab_text_active: Rgba::from_rgb(20, 20, 20),
                text: Rgba::from_rgb(34, 34, 34),
                text_dim: Rgba::from_rgb(90, 90, 90),
                text_faint: Rgba::from_rgb(140, 140, 140),
                icon: Rgba::from_rgb(60, 60, 60),
                hover: Rgba::from_rgb(210, 210, 210),
                pressed: Rgba::from_rgb(196, 196, 196),
                field_bg: Rgba::from_rgb(250, 250, 250),
                field_border: Rgba::from_rgb(170, 170, 170),
                separator: Rgba::from_rgb(200, 200, 200),
                row_alt: Rgba::from_rgb(224, 224, 224),
                row_selected: Rgba::from_rgb(170, 200, 240),
                pill_active_bg: Rgba::from_rgb(196, 196, 196),
                clip_bar_bg: Rgba::from_rgb(190, 203, 224),
                control_surface: Rgba::from_rgb(218, 218, 218),
                control_well: Rgba::from_rgb(196, 196, 196),
                control_border: Rgba::from_rgb(150, 150, 150),
                control_handle_bg: Rgba::from_rgb(205, 205, 205),
                control_handle: Rgba::from_rgb(50, 50, 50),
                control_handle_dim: Rgba::from_rgb(85, 85, 85),
                plot_bg: Rgba::from_rgb(245, 245, 245),
                plot_grid: Rgba::from_rgb(210, 210, 210),
                plot_axis: Rgba::from_rgb(170, 170, 170),
                plot_handle: Rgba::from_rgb(50, 50, 50),
                plot_handle_dim: Rgba::from_rgb(100, 100, 100),
                meter_bg: Rgba::from_rgb(205, 205, 205),
                meter_readout_bg: Rgba::from_rgb(218, 218, 218),
                meter_clip_off: Rgba::from_rgb(155, 155, 155),
                panner_bg: Rgba::from_gray(196),
                panner_border: Rgba::from_gray(150),
                panner_speaker: Rgba::from_gray(100),
                fader_knob: Rgba::from_gray(140),
                eq_track: Rgba::from_gray(196),
                eq_tick: Rgba::from_gray(150),
                eq_knob: Rgba::from_gray(85),
                eq_knob_active: Rgba::from_gray(34),
                eq_node: Rgba::from_gray(50),
                eq_node_off: Rgba::from_gray(140),
                crossover_handle: Rgba::from_gray(50),
                keyframe_plot_bg: Rgba::from_gray(245),
                keyframe_plot_grid: Rgba::from_gray(210),
                keyframe_plot_axis: Rgba::from_gray(170),
                keyframe_handle: Rgba::from_gray(50),
                switch_fill: Rgba::from_gray(60),
                switch_knob: Rgba::from_gray(232),
                switch_border: Rgba::from_gray(110),
                slider_disabled_track: Rgba::from_gray(190),
                slider_disabled_knob: Rgba::from_gray(140),
                button_hover: Rgba::from_gray(210),
                button_border: Rgba::from_gray(170),
                icon_active: Rgba::from_rgb(34, 34, 34),
                caption_track_bg: Rgba::from_rgb(222, 215, 233),
                scroll_thumb: Rgba::from_gray(150),
                scroll_thumb_hover: Rgba::from_gray(130),
                vertical_scroll_thumb: Rgba::from_gray(150),
                disabled_clip_bg: Rgba::from_gray(190),
                in_out_shade: Rgba::from_gray(185),
                tl_bg: Rgba::from_rgb(210, 210, 210),
                tl_track_bg: Rgba::from_rgb(222, 222, 222),
                tl_track_bg_alt: Rgba::from_rgb(228, 228, 228),
                tl_header_bg: Rgba::from_rgb(214, 214, 214),
                tl_ruler_bg: Rgba::from_rgb(226, 226, 226),
                tl_ruler_text: Rgba::from_rgb(80, 80, 80),
                ..dark
            },
        }
    }

    /// Settings ▸ Appearance on top of a theme: the highlight colour (selections, primary buttons,
    /// the focus outline, the playhead, timecodes, drag values and track targeting) and accessible
    /// colour contrast (brighter secondary text and borders).
    pub fn with_appearance(mut self, highlight: Option<[u8; 3]>, accessible_contrast: bool) -> Self {
        if let Some([r, g, b]) = highlight {
            let c = Rgba::from_rgb(r, g, b);
            // A highlight of the user's own also colours the theme's other blues: the same hue, as
            // much more or less saturated and bright as each of them is than the theme's highlight.
            // The theme's own highlight leaves them exactly as designed.
            if c != self.accent {
                let tint = |s: f32, v: f32| c.tinted(s, v);
                self.focus = tint(0.92, 1.06);
                self.playhead = self.focus;
                self.timecode = tint(0.98, 1.09);
                self.hot_text = self.timecode;
                self.track_target = tint(1.0, 0.865);
            }
            let lift = |v: u8| v.saturating_add(16);
            self.accent = c;
            self.accent_hover = Rgba::from_rgb(lift(r), lift(g), lift(b));
        }
        if accessible_contrast {
            if self.kind == ThemeKind::Light {
                self.text_dim = Rgba::from_rgb(40, 40, 40);
                self.text_faint = Rgba::from_rgb(80, 80, 80);
                self.field_border = Rgba::from_rgb(110, 110, 110);
            } else {
                self.text_dim = self.text;
                self.text_faint = Rgba::from_rgb(0xa8, 0xa8, 0xa8);
                self.tab_text = self.tab_text_active;
                self.field_border = Rgba::from_rgb(0x6a, 0x6a, 0x6a);
                self.separator = Rgba::from_rgb(0x50, 0x50, 0x50);
            }
        }
        self
    }
}
