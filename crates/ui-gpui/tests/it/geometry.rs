use filmcraft_time::Tick;
use filmcraft_ui_gpui::geometry::{View, ruler_step};

#[test]
fn x_and_tick_round_trip_within_a_pixel() {
    let v = View { pps: 80.0, scroll: Tick(3 * filmcraft_time::TICKS_PER_SECOND) };
    let t = Tick(10 * filmcraft_time::TICKS_PER_SECOND);
    let back = v.tick_at(v.x_of(t));
    assert!((v.x_of(back) - v.x_of(t)).abs() < 1.0);
}

#[test]
fn zooming_keeps_the_time_under_the_pointer() {
    let v = View { pps: 60.0, scroll: Tick(2 * filmcraft_time::TICKS_PER_SECOND) };
    let anchor = 300.0;
    let before = v.tick_at(anchor);
    let z = v.zoomed(2.0, anchor);
    assert!(z.pps > v.pps);
    let drift = (z.x_of(before) - anchor).abs();
    assert!(drift < 1.0, "the anchored time moved by {drift} px");
}

#[test]
fn zoom_and_scroll_stay_in_range() {
    let v = View::default().zoomed(1.0e9, 0.0).zoomed(1.0e-9, 0.0);
    assert!(v.pps >= filmcraft_ui_gpui::geometry::MIN_PPS);
    assert!(View::default().scrolled_by(-1.0e9).scroll >= Tick(0));
}

#[test]
fn ruler_labels_are_never_closer_than_asked() {
    for pps in [3.0, 20.0, 60.0, 400.0, 3000.0] {
        let step = ruler_step(pps, 84.0);
        let px = step.0 as f64 / filmcraft_time::TICKS_PER_SECOND as f64 * pps;
        assert!(px >= 84.0 || step.0 >= 3600 * filmcraft_time::TICKS_PER_SECOND, "pps {pps}: {px}");
    }
}
