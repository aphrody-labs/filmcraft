//! Small generated files for the documentation examples and tests. Not part of the supported API.
//!
//! The pictures are not decodable: they are length-prefixed placeholder NAL units, enough for every
//! container-level operation. Decodable fixtures come from the encoder (see the `encode` module).

// Fixture builders run on literal inputs: a failure here is a bug in the fixture, not hostile input.
#![allow(clippy::expect_used)]

use filmcraft_isobmff as iso;

use crate::{Control, Demuxer, Limits, OutputContainer, RemuxOptions, SegmentOptions, SegmentedStream, remux_to_vec, segment};

fn nal(kind: u8, len: usize, seed: u8) -> Vec<u8> {
    let mut v = ((len + 1) as u32).to_be_bytes().to_vec();
    v.push(kind);
    v.extend((0..len).map(|i| seed.wrapping_add(i as u8)));
    v
}

/// A 1 second, 30 fps, 64x48 "H.264" track (IDR every 10 frames) plus a 44.1 kHz stereo "AAC"
/// track, as an MP4 file.
pub fn mp4_h264_aac() -> Vec<u8> {
    build(false)
}

/// Like [`mp4_h264_aac`] with B-frame style reordering (decode order I P B P B ... ).
pub fn mp4_h264_bframes() -> Vec<u8> {
    build(true)
}

fn build(reorder: bool) -> Vec<u8> {
    let avc = iso::AvcConfig::new(vec![vec![0x67, 0x64, 0x00, 0x1F, 0xAC]], vec![vec![0x68, 0xEE, 0x3C, 0x80]], 4);
    let mut w = iso::Mp4Writer::new(std::io::Cursor::new(Vec::new()), iso::WriterOptions::new(iso::Brand::Mp4)).expect("in-memory writer");
    let mut v = iso::TrackConfig::new(iso::SampleEntry::avc(avc, 64, 48), 30_000);
    if reorder {
        v.media_start = Some(1001);
    }
    let vt = w.add_track(v).expect("video track");
    let at = w.add_track(iso::TrackConfig::new(iso::SampleEntry::aac(vec![0x12, 0x10], 2, 44_100), 44_100)).expect("audio track");
    for i in 0..30u32 {
        let key = i % 10 == 0;
        let cto = if reorder { if i % 2 == 1 { 2002 } else { 1001 } } else { 0 };
        let data = [nal(if key { 5 } else { 1 }, 200 + i as usize, i as u8)].concat();
        w.write_sample(vt, iso::WriteSample { data: &data, duration: 1001, composition_offset: cto, is_sync: key }).expect("video sample");
    }
    for i in 0..44u32 {
        let data: Vec<u8> = (0..180).map(|k| (i as u8).wrapping_mul(7).wrapping_add(k as u8)).collect();
        w.write_sample(at, iso::WriteSample { data: &data, duration: 1024, composition_offset: 0, is_sync: true }).expect("audio sample");
    }
    w.finish().expect("finish").into_inner()
}

/// [`mp4_h264_aac`] remuxed to Matroska.
pub fn mkv_h264_aac() -> Vec<u8> {
    let input = Demuxer::open_bytes(mp4_h264_aac(), &Limits::default()).expect("fixture parses");
    remux_to_vec(&input, &RemuxOptions::new(OutputContainer::Matroska), &Control::default()).expect("fixture remuxes").0
}

/// [`mp4_h264_aac`] cut into 300 ms CMAF segments.
pub fn segmented() -> SegmentedStream {
    let input = Demuxer::open_bytes(mp4_h264_aac(), &Limits::default()).expect("fixture parses");
    let mut o = SegmentOptions::new();
    o.target_ms = 300;
    segment(&input, &o, &Control::default()).expect("fixture segments")
}

/// A decodable 64x48, 10 frame, 30 fps H.264 + stereo AAC MP4 made by the encoder.
#[cfg(all(feature = "encode", feature = "decode"))]
pub fn decodable_mp4() -> Vec<u8> {
    use crate::{AudioOptions, Encoder, EncoderOptions, FrameRate, VideoCodec, VideoOptions};
    let video = VideoOptions::new(VideoCodec::H264, 64, 48, FrameRate::FPS_30);
    let opts = EncoderOptions::new(Some(video), Some(AudioOptions::aac(48_000, 2)));
    let mut enc = Encoder::new(std::io::Cursor::new(Vec::new()), opts).expect("encoder");
    for i in 0..10u32 {
        let mut px = vec![0u8; 64 * 48 * 4];
        for (n, p) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = ((n % 64) as u32, (n / 64) as u32);
            p.copy_from_slice(&[(x * 4 + i * 8) as u8, (y * 5) as u8, 128, 255]);
        }
        enc.push_rgba(&px).expect("frame");
    }
    let tone: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.05).sin() * 0.4).collect();
    enc.push_audio(&[tone.clone(), tone]).expect("audio");
    enc.finish(&Control::default()).expect("finish").0.into_inner()
}
