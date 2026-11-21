//! Timeline geometry: the mapping between ticks and pixels, and the ruler's tick spacing. Pure
//! functions, no toolkit types.

use filmcraft_time::{TICKS_PER_SECOND, Tick};

/// Width of the track-header column.
pub const HEADER_W: f32 = 132.0;
pub const RULER_H: f32 = 24.0;
pub const TRACK_H: f32 = 44.0;
pub const MIN_PPS: f64 = 2.0;
pub const MAX_PPS: f64 = 4000.0;

const TICKS: f64 = TICKS_PER_SECOND as f64;

/// How the Timeline shows the sequence: pixels per second and the time at the left edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub pps: f64,
    pub scroll: Tick,
}

impl Default for View {
    fn default() -> Self {
        View { pps: 60.0, scroll: Tick(0) }
    }
}

impl View {
    /// The x of time `t` inside the track area (negative left of the visible start).
    pub fn x_of(&self, t: Tick) -> f32 {
        ((t.0 - self.scroll.0) as f64 / TICKS * self.pps) as f32
    }

    /// The time at x inside the track area, never before the sequence start.
    pub fn tick_at(&self, x: f32) -> Tick {
        Tick((self.scroll.0 + (f64::from(x) / self.pps * TICKS) as i64).max(0))
    }

    /// Pixel width of a span of time.
    pub fn width_of(&self, d: Tick) -> f32 {
        (d.0 as f64 / TICKS * self.pps) as f32
    }

    /// Zoom by `factor` keeping the time under `anchor_x` where it is.
    pub fn zoomed(self, factor: f64, anchor_x: f32) -> View {
        let pps = (self.pps * factor).clamp(MIN_PPS, MAX_PPS);
        let anchor = self.tick_at(anchor_x);
        let scroll = Tick((anchor.0 - (f64::from(anchor_x) / pps * TICKS) as i64).max(0));
        View { pps, scroll }
    }

    /// Scroll right by `dx` pixels (left when negative), not past the sequence start.
    pub fn scrolled_by(self, dx: f32) -> View {
        View { scroll: Tick((self.scroll.0 + (f64::from(dx) / self.pps * TICKS) as i64).max(0)), ..self }
    }
}

/// The ruler's step between labelled ticks: the smallest of 1, 2, 5, 10, 15, 30 s, 1, 2, 5, 10,
/// 30 min and 1 h that leaves at least `min_px` between two labels.
pub fn ruler_step(pps: f64, min_px: f64) -> Tick {
    const STEPS: [f64; 14] = [0.04, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 3600.0];
    let secs = STEPS.iter().copied().find(|s| s * pps >= min_px).unwrap_or(3600.0);
    Tick((secs * TICKS) as i64)
}
