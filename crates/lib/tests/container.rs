//! Container-level behaviour on generated files: probe, exact index, remux both ways, CMAF.

use filmcraft_lib::doc_fixtures::{mkv_h264_aac, mp4_h264_aac, mp4_h264_bframes};
use filmcraft_lib::{
    CancelToken, Control, Demuxer, ErrorKind, FrameRate, Limits, OutputContainer, RemuxOptions, SegmentOptions, Tick, TrackKind, remux_to_vec, segment,
};

fn open(bytes: Vec<u8>) -> Demuxer {
    Demuxer::open_bytes(bytes, &Limits::default()).expect("fixture opens")
}

fn all_packets(d: &Demuxer, track: usize) -> Vec<Vec<u8>> {
    let n = d.frame_index(track).map_or(0, |i| i.len());
    (0..n).map(|i| d.read_packet(track, i).expect("packet").data).collect()
}

#[test]
fn mp4_probe_reports_exact_counts_and_rates() {
    let d = open(mp4_h264_aac());
    let t = d.tracks();
    assert_eq!(t.len(), 2);
    let v = &t[0];
    assert_eq!((v.kind, v.codec.as_str(), v.width, v.height), (TrackKind::Video, "h264", 64, 48));
    assert_eq!(v.packets, 30);
    assert_eq!(v.keyframes, 3);
    assert_eq!(v.frame_rate, Some(FrameRate::new(30000, 1001)));
    assert!(v.constant_frame_rate);
    assert_eq!(v.duration, Tick::from_rational(30 * 1001, 1, 30_000));
    let a = &t[1];
    assert_eq!((a.kind, a.codec.as_str(), a.sample_rate, a.channels), (TrackKind::Audio, "aac", 44_100, 2));
    assert_eq!(a.packets, 44);
    assert_eq!(d.codec_string(0).as_deref(), Some("avc1.64001F"));
    assert_eq!(d.codec_string(1).as_deref(), Some("mp4a.40.2"));
}

#[test]
fn frame_index_maps_time_to_frame_exactly() {
    let d = open(mp4_h264_aac());
    let ix = d.frame_index(0).expect("index");
    assert_eq!(ix.len(), 30);
    for n in [0usize, 1, 17, 29] {
        let t = ix.time_of_frame(n).expect("time");
        assert_eq!(ix.frame_at(t), Some(n), "frame {n}");
    }
    assert_eq!(ix.keyframe_before(17), 10);
    assert_eq!(ix.keyframes().collect::<Vec<_>>(), vec![0, 10, 20]);
}

#[test]
fn mp4_to_matroska_copies_every_packet_byte() {
    let src = open(mp4_h264_aac());
    let (mkv, report) = remux_to_vec(&src, &RemuxOptions::new(OutputContainer::Matroska), &Control::default()).expect("remux");
    assert_eq!(report.copied, vec![0, 1]);
    assert_eq!(report.packets, 74);
    let out = open(mkv);
    assert_eq!(out.info().brand, "matroska");
    for t in 0..2 {
        assert_eq!(all_packets(&out, t), all_packets(&src, t), "track {t}");
    }
    let (v, a) = (&out.tracks()[0], &out.tracks()[1]);
    assert_eq!((v.codec.as_str(), v.width, v.height, v.packets, v.keyframes), ("h264", 64, 48, 30, 3));
    assert_eq!((a.codec.as_str(), a.sample_rate, a.channels, a.packets), ("aac", 44_100, 2, 44));
    // presentation times survive to the millisecond
    let (si, oi) = (src.frame_index(0).expect("ix"), out.frame_index(0).expect("ix"));
    for n in 0..30 {
        let a = si.time_of_frame(n).expect("t").seconds();
        let b = oi.time_of_frame(n).expect("t").seconds();
        assert!((a - b).abs() < 0.0006, "frame {n}: {a} vs {b}");
    }
}

#[test]
fn matroska_to_mp4_restores_the_frame_grid() {
    let mkv = open(mkv_h264_aac());
    let (mp4, report) = remux_to_vec(&mkv, &RemuxOptions::new(OutputContainer::Mp4), &Control::default()).expect("remux");
    assert_eq!(report.copied.len(), 2);
    let out = open(mp4);
    let v = &out.tracks()[0];
    assert_eq!(v.timescale, 30_000, "29.97 fps snaps to its own timescale");
    assert!(v.constant_frame_rate);
    assert_eq!(v.frame_rate, Some(FrameRate::new(30000, 1001)));
    assert_eq!(v.packets, 30);
    let a = &out.tracks()[1];
    assert_eq!(a.timescale, 44_100);
    assert!(out.frame_index(1).expect("ix").is_constant_duration() || a.packets == 44);
    for t in 0..2 {
        assert_eq!(all_packets(&out, t), all_packets(&mkv, t), "track {t}");
    }
}

#[test]
fn reordered_video_keeps_its_presentation_order_through_both_containers() {
    let src = open(mp4_h264_bframes());
    let want: Vec<i64> = (0..30).map(|n| src.frame_index(0).and_then(|i| i.presented(n)).map(|e| e.pts).expect("pts")).collect();
    let (mkv, _) = remux_to_vec(&src, &RemuxOptions::new(OutputContainer::Matroska), &Control::default()).expect("to mkv");
    let mkv = open(mkv);
    let (mp4, _) = remux_to_vec(&mkv, &RemuxOptions::new(OutputContainer::Mp4), &Control::default()).expect("back to mp4");
    let back = open(mp4);
    let got: Vec<i64> = (0..30).map(|n| back.frame_index(0).and_then(|i| i.presented(n)).map(|e| e.pts).expect("pts")).collect();
    let base_w = want[0];
    let base_g = got[0];
    let ts_w = src.tracks()[0].timescale as i64;
    let ts_g = back.tracks()[0].timescale as i64;
    for (a, b) in want.iter().zip(&got) {
        // compare as seconds from the first frame
        let sa = (*a - base_w) as f64 / ts_w as f64;
        let sb = (*b - base_g) as f64 / ts_g as f64;
        assert!((sa - sb).abs() < 0.0006, "{sa} vs {sb}");
    }
    assert_eq!(all_packets(&back, 0), all_packets(&src, 0));
    // decode order is untouched: the same packet is still a keyframe at the same index
    let keys = |d: &Demuxer| d.frame_index(0).expect("ix").keyframes().collect::<Vec<_>>();
    assert_eq!(keys(&back), keys(&src));
}

#[test]
fn webm_refuses_h264_and_explicit_selection_errors_name_the_track() {
    let src = open(mp4_h264_aac());
    let err = remux_to_vec(&src, &RemuxOptions::new(OutputContainer::WebM), &Control::default()).expect_err("h264 is not WebM");
    assert_eq!(err.kind(), ErrorKind::Unsupported);
    let mut o = RemuxOptions::new(OutputContainer::WebM);
    o.tracks = Some(vec![0]);
    let err = remux_to_vec(&src, &o, &Control::default()).expect_err("explicit h264 -> webm");
    assert!(err.message().contains("track 0"), "{err}");
}

#[test]
fn track_selection_copies_only_the_chosen_track() {
    let src = open(mp4_h264_aac());
    let mut o = RemuxOptions::new(OutputContainer::Matroska);
    o.tracks = Some(vec![1]);
    let (mkv, rep) = remux_to_vec(&src, &o, &Control::default()).expect("audio only");
    assert_eq!(rep.copied, vec![1]);
    let out = open(mkv);
    assert_eq!(out.tracks().len(), 1);
    assert_eq!(out.tracks()[0].kind, TrackKind::Audio);
}

#[test]
fn fragmented_output_reopens_with_all_packets() {
    let src = open(mp4_h264_aac());
    let mut o = RemuxOptions::new(OutputContainer::FragmentedMp4);
    o.fragment_ms = 300;
    let (frag, _) = remux_to_vec(&src, &o, &Control::default()).expect("fragmented");
    let out = open(frag);
    assert!(out.info().fragmented);
    assert_eq!(out.tracks()[0].packets, 30);
    assert_eq!(out.tracks()[1].packets, 44);
    assert_eq!(all_packets(&out, 0), all_packets(&src, 0));
}

#[test]
fn cmaf_segments_start_at_keyframes_and_concatenate_to_a_valid_file() {
    let src = open(mp4_h264_aac());
    let mut o = SegmentOptions::new();
    o.target_ms = 300;
    let stream = segment(&src, &o, &Control::default()).expect("segments");
    assert_eq!(&stream.init[4..8], b"ftyp");
    assert_eq!(stream.segments.len(), 3, "keyframes at 0, 10, 20 and a 300 ms target");
    assert_eq!(stream.codecs, vec!["avc1.64001F".to_string(), "mp4a.40.2".to_string()]);
    let total: f64 = stream.segments.iter().map(|s| s.duration.seconds()).sum();
    assert!((total - 30.0 * 1001.0 / 30_000.0).abs() < 0.001, "{total}");
    for (i, s) in stream.segments.iter().enumerate() {
        assert_eq!(s.index, i);
        assert_eq!(&s.data[4..8], b"moof");
    }
    let mut whole = stream.init.clone();
    for s in &stream.segments {
        whole.extend_from_slice(&s.data);
    }
    let out = open(whole);
    assert_eq!(out.tracks()[0].packets, 30);
    assert_eq!(out.tracks()[0].keyframes, 3);
    assert_eq!(out.tracks()[1].packets, 44);
    assert_eq!(all_packets(&out, 0), all_packets(&src, 0));
}

#[test]
fn matroska_input_segments_too() {
    let mkv = open(mkv_h264_aac());
    let mut o = SegmentOptions::new();
    o.target_ms = 300;
    let stream = segment(&mkv, &o, &Control::default()).expect("segments");
    assert_eq!(stream.segments.len(), 3);
}

#[test]
fn cancellation_and_limits_are_reported_with_stable_kinds() {
    let src = open(mp4_h264_aac());
    let token = CancelToken::new();
    token.cancel();
    let c = Control::new().with_cancel(token);
    let err = remux_to_vec(&src, &RemuxOptions::new(OutputContainer::Matroska), &c).expect_err("cancelled");
    assert_eq!(err.kind(), ErrorKind::Cancelled);

    let mut l = Limits::default();
    l.max_packets = 10;
    let err = Demuxer::open_bytes(mp4_h264_aac(), &l).expect_err("too many packets");
    assert_eq!(err.kind(), ErrorKind::LimitExceeded);
    let mut l = Limits::default();
    l.max_input_bytes = 100;
    assert_eq!(Demuxer::open_bytes(mp4_h264_aac(), &l).expect_err("too big").kind(), ErrorKind::LimitExceeded);
    let mut l = Limits::default();
    l.max_packet_bytes = 10;
    let d = Demuxer::open_bytes(mp4_h264_aac(), &l).expect("opens");
    assert_eq!(d.read_packet(0, 0).expect_err("packet too big").kind(), ErrorKind::LimitExceeded);
}

#[test]
fn progress_reaches_the_total() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    let src = open(mp4_h264_aac());
    let last = Arc::new(AtomicU64::new(0));
    let l2 = last.clone();
    let c = Control::new().with_progress(move |p| {
        if p.total == Some(74) {
            l2.fetch_max(p.done, Ordering::SeqCst);
        }
    });
    remux_to_vec(&src, &RemuxOptions::new(OutputContainer::Matroska), &c).expect("remux");
    assert_eq!(last.load(Ordering::SeqCst), 74);
}

#[test]
fn hostile_input_never_panics() {
    for base in [mp4_h264_aac(), mkv_h264_aac()] {
        // truncation at many points
        for cut in (0..base.len()).step_by(base.len() / 97 + 1) {
            let r = std::panic::catch_unwind(|| {
                if let Ok(d) = Demuxer::open_bytes(base[..cut].to_vec(), &Limits::default()) {
                    for t in 0..d.tracks().len() {
                        for i in 0..d.frame_index(t).map_or(0, |x| x.len()) {
                            let _ = d.read_packet(t, i);
                        }
                    }
                    let _ = remux_to_vec(&d, &RemuxOptions::new(OutputContainer::Mp4), &Control::default());
                }
            });
            assert!(r.is_ok(), "panic at truncation {cut}");
        }
        // bit flips
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..300 {
            let mut b = base.clone();
            for _ in 0..4 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let at = (seed >> 33) as usize % b.len();
                b[at] ^= 1 << ((seed >> 20) & 7);
            }
            let r = std::panic::catch_unwind(|| {
                if let Ok(d) = Demuxer::open_bytes(b, &Limits::default()) {
                    let mut o = SegmentOptions::new();
                    o.target_ms = 200;
                    let _ = segment(&d, &o, &Control::default());
                    let _ = remux_to_vec(&d, &RemuxOptions::new(OutputContainer::Matroska), &Control::default());
                }
            });
            assert!(r.is_ok(), "panic on a bit-flipped file");
        }
    }
}

#[test]
fn colour_and_hdr_metadata_survive_mp4_to_matroska_and_back() {
    use filmcraft_isobmff as iso;
    let avc = iso::AvcConfig::new(vec![vec![0x67, 0x64, 0x00, 0x1F, 0xAC]], vec![vec![0x68, 0xEE, 0x3C, 0x80]], 4);
    let mut entry = iso::SampleEntry::avc(avc, 64, 48);
    if let Some(v) = entry.video.as_mut() {
        v.color = Some(iso::ColorInfo::Nclx { primaries: 9, transfer: 16, matrix: 9, full_range: false });
        v.mastering_display = Some(iso::MasteringDisplay::bt2020(1000.0, 0.005));
        v.content_light = Some((1000, 400));
        v.pixel_aspect = Some((4, 3));
    }
    let mut w = iso::Mp4Writer::new(std::io::Cursor::new(Vec::new()), iso::WriterOptions::new(iso::Brand::Mp4)).expect("writer");
    let t = w.add_track(iso::TrackConfig::new(entry, 25)).expect("track");
    for i in 0..5u32 {
        let data = [0, 0, 0, 3, if i == 0 { 5 } else { 1 }, 1, 2];
        w.write_sample(t, iso::WriteSample { data: &data, duration: 1, composition_offset: 0, is_sync: i == 0 }).expect("sample");
    }
    let src = open(w.finish().expect("finish").into_inner());
    let want = src.tracks()[0].clone();
    assert_eq!(want.color.map(|c| (c.primaries, c.transfer, c.matrix)), Some((9, 16, 9)));
    let (mkv, _) = remux_to_vec(&src, &RemuxOptions::new(OutputContainer::Matroska), &Control::default()).expect("to mkv");
    let mkv = open(mkv);
    let m = &mkv.tracks()[0];
    assert_eq!(m.color, want.color);
    assert_eq!(m.pixel_aspect, (4, 3));
    let (h, wh) = (m.hdr.expect("hdr in mkv"), want.hdr.expect("hdr in mp4"));
    assert_eq!((h.max_cll, h.max_fall), (Some(1000), Some(400)));
    assert!((h.max_luminance.expect("max") - 1000.0).abs() < 0.01 && (wh.max_luminance.expect("max") - 1000.0).abs() < 0.01);
    let (mp4, _) = remux_to_vec(&mkv, &RemuxOptions::new(OutputContainer::Mp4), &Control::default()).expect("to mp4");
    let back = open(mp4);
    assert_eq!(back.tracks()[0].color, want.color);
    assert_eq!(back.tracks()[0].hdr.map(|x| (x.max_cll, x.max_fall)), Some((Some(1000), Some(400))));
}
