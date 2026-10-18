//! Fragmented MP4 (CMAF) segmenting and HLS playlists.
//!
//! [`segment`] cuts the tracks of a [`Demuxer`] into an initialisation segment (`ftyp` + `moov`
//! with `mvex`) and media segments (`moof` + `mdat`) that start at video keyframes, ready to serve
//! as `init.mp4` / `seg-N.m4s` for HLS or DASH. Packets are copied, never re-encoded; Matroska and
//! WebM inputs are converted on the way (see [`remux`](crate::remux) for which codecs can be).

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use filmcraft_isobmff as iso;

use crate::container::{Demuxer, TrackKind};
use crate::control::{Control, Stage};
use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::remux::{OutputContainer, plan, select_tracks, track_config};
use filmcraft_time::Tick;

/// Options of [`segment`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SegmentOptions {
    /// Target segment length in milliseconds; a segment ends at the first video keyframe at or
    /// after this length (at the first audio packet without video).
    pub target_ms: u32,
    /// Source track indices to include; `None` takes every remuxable audio and video track.
    pub tracks: Option<Vec<usize>>,
    /// Size caps for the input.
    pub limits: Limits,
}

impl SegmentOptions {
    /// Six-second segments of every audio and video track.
    pub fn new() -> Self {
        Self { target_ms: 6000, tracks: None, limits: Limits::default() }
    }
}

impl Default for SegmentOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// One media segment.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Segment {
    /// Zero-based position.
    pub index: usize,
    /// Presentation start on the lead track's timeline (decode time of its first packet).
    pub start: Tick,
    /// Length until the next segment starts (the packet durations for the last one).
    pub duration: Tick,
    /// The `moof` + `mdat` bytes.
    pub data: Vec<u8>,
}

/// The initialisation segment and every media segment of a stream.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SegmentedStream {
    /// `ftyp` + `moov`: serve once, referenced by `EXT-X-MAP` or `initialization`.
    pub init: Vec<u8>,
    /// Media segments in order.
    pub segments: Vec<Segment>,
    /// RFC 6381 codec strings of the included tracks, in track order (for manifests).
    pub codecs: Vec<String>,
}

/// A piece of a stream handed to the callback of [`segment_with`].
#[derive(Debug)]
#[non_exhaustive]
pub enum SegmentEvent {
    /// The initialisation segment; always first.
    Init(Vec<u8>),
    /// A finished media segment.
    Segment(Segment),
}

/// A `Write` that appends to a shared buffer so the fragmented writer's output can be drained
/// after every fragment.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Shared {
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Cut `input` into CMAF segments and return them all in memory.
///
/// Use [`segment_with`] to stream segments out as they are produced.
///
/// # Errors
///
/// As [`remux`](crate::remux): unsupported codecs, cancellation, limits, damaged input.
///
/// # Example
///
/// ```
/// # fn main() -> Result<(), filmcraft_lib::Error> {
/// use filmcraft_lib::{Control, Demuxer, Limits, SegmentOptions, hls_playlist, segment};
/// # let mp4 = filmcraft_lib::doc_fixtures::mp4_h264_aac();
/// let input = Demuxer::open_bytes(mp4, &Limits::default())?;
/// let mut opts = SegmentOptions::new();
/// opts.target_ms = 500;
/// let stream = segment(&input, &opts, &Control::default())?;
/// assert_eq!(&stream.init[4..8], b"ftyp");
/// let m3u8 = hls_playlist(&stream, "init.mp4", |i| format!("seg-{i}.m4s"));
/// assert!(m3u8.starts_with("#EXTM3U"));
/// # Ok(()) }
/// ```
pub fn segment(input: &Demuxer, opts: &SegmentOptions, control: &Control) -> Result<SegmentedStream> {
    let mut init = Vec::new();
    let mut segments = Vec::new();
    let codecs = segment_with(input, opts, control, |ev| {
        match ev {
            SegmentEvent::Init(b) => init = b,
            SegmentEvent::Segment(s) => segments.push(s),
        }
        Ok(())
    })?;
    Ok(SegmentedStream { init, segments, codecs })
}

/// Cut `input` into CMAF segments, calling `sink` with the init segment and then each media
/// segment as soon as it is complete. Returns the codec strings of the included tracks.
///
/// # Errors
///
/// Anything `sink` returns stops the run and is returned unchanged.
pub fn segment_with(input: &Demuxer, opts: &SegmentOptions, control: &Control, mut sink: impl FnMut(SegmentEvent) -> Result<()>) -> Result<Vec<String>> {
    let mut ro = crate::remux::RemuxOptions::new(OutputContainer::FragmentedMp4);
    ro.tracks = opts.tracks.clone();
    ro.limits = opts.limits;
    let (selected, _skipped) = select_tracks(input, &ro)?;
    let plans = plan(input, &selected, OutputContainer::FragmentedMp4, false)?;
    let codecs: Vec<String> = plans.iter().filter_map(|p| crate::codec_string::codec_string(&p.entry)).collect();
    let total: u64 = plans.iter().map(|p| p.samples.len() as u64).sum();
    if total > opts.limits.max_packets {
        return Err(Error::limit("packet count", opts.limits.max_packets, total));
    }
    let out = Shared::default();
    let mut wo = iso::WriterOptions::new(iso::Brand::Mp4);
    wo.movie_timescale = input.movie_timescale();
    let mut w = iso::FragmentedWriter::new(out.clone(), wo);
    for p in &plans {
        w.add_track(track_config(p))?;
    }
    // an empty flush writes ftyp + moov
    w.flush_fragment()?;
    sink(SegmentEvent::Init(out.take()))?;
    let lead = plans.iter().position(|p| p.kind == TrackKind::Video).unwrap_or(0);
    let lead_ts = plans.get(lead).map_or(1, |p| p.ts.max(1));
    let target = crate::container::rescale(i64::from(opts.target_ms.max(100)), 1000, lead_ts);
    let slots: Vec<usize> = plans.iter().map(|p| p.src).collect();
    let order = input.interleaved(&slots);
    let mut seg_start: Option<i64> = None;
    let mut index = 0usize;
    let flush =
        |w: &mut iso::FragmentedWriter<Shared>, start: i64, end: i64, index: &mut usize, sink: &mut dyn FnMut(SegmentEvent) -> Result<()>| -> Result<()> {
            w.flush_fragment()?;
            let data = out.take();
            if data.is_empty() {
                return Ok(());
            }
            let ts = i64::from(lead_ts);
            sink(SegmentEvent::Segment(Segment {
                index: *index,
                start: Tick::from_rational(start, 1, ts),
                duration: Tick::from_rational((end - start).max(0), 1, ts),
                data,
            }))?;
            *index += 1;
            Ok(())
        };
    let mut last_end = 0i64;
    for (n, &(src, idx)) in order.iter().enumerate() {
        control.check()?;
        let Some(slot) = plans.iter().position(|p| p.src == src) else { continue };
        let plan = &plans[slot];
        let Some(s) = plan.samples.get(idx) else { continue };
        let pkt = input.read_packet(src, idx)?;
        let key = pkt.key || plan.kind != TrackKind::Video;
        if slot == lead {
            if key && seg_start.is_some_and(|st| s.dts - st >= target) {
                let start = seg_start.unwrap_or(0);
                flush(&mut w, start, s.dts, &mut index, &mut sink)?;
                seg_start = Some(s.dts);
            }
            seg_start.get_or_insert(s.dts);
            last_end = s.dts + i64::try_from(s.dur).unwrap_or(0);
        }
        let dur = u32::try_from(s.dur).map_err(|_| Error::unsupported("sample duration does not fit 32 bits"))?;
        let cto = i32::try_from(s.cto).map_err(|_| Error::unsupported("composition offset does not fit 32 bits"))?;
        w.write_sample(slot, iso::WriteSample { data: &pkt.data, duration: dur, composition_offset: cto, is_sync: key })?;
        if n % 128 == 0 {
            control.report(Stage::Writing, n as u64, Some(total));
        }
    }
    if let Some(start) = seg_start {
        flush(&mut w, start, last_end, &mut index, &mut sink)?;
    }
    control.report(Stage::Finishing, total, Some(total));
    Ok(codecs)
}

/// An HLS media playlist (version 7, VOD) for a [`SegmentedStream`] served as `init_uri` and
/// `segment_uri(i)`.
///
/// ```
/// # let stream = filmcraft_lib::doc_fixtures::segmented();
/// let m3u8 = filmcraft_lib::hls_playlist(&stream, "init.mp4", |i| format!("seg-{i}.m4s"));
/// assert!(m3u8.contains("#EXT-X-MAP:URI=\"init.mp4\""));
/// assert!(m3u8.trim_end().ends_with("#EXT-X-ENDLIST"));
/// ```
pub fn hls_playlist(stream: &SegmentedStream, init_uri: &str, segment_uri: impl Fn(usize) -> String) -> String {
    let secs = |t: Tick| t.seconds();
    let target = stream.segments.iter().map(|s| secs(s.duration).ceil() as u64).max().unwrap_or(1).max(1);
    let mut s = String::from("#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-INDEPENDENT-SEGMENTS\n");
    s.push_str(&format!("#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n"));
    s.push_str(&format!("#EXT-X-MAP:URI=\"{}\"\n", init_uri.replace('"', "%22")));
    for seg in &stream.segments {
        s.push_str(&format!("#EXTINF:{:.6},\n{}\n", secs(seg.duration), segment_uri(seg.index)));
    }
    s.push_str("#EXT-X-ENDLIST\n");
    s
}

/// A master playlist entry for a stream: `CODECS` and `BANDWIDTH` for one variant at `uri`.
///
/// `bandwidth` is the peak bits per second (compute it from the segment sizes and durations).
pub fn hls_variant(stream: &SegmentedStream, bandwidth: u64, uri: &str) -> String {
    format!("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH={bandwidth},CODECS=\"{}\"\n{uri}\n", stream.codecs.join(","))
}

/// Peak bits per second over the segments of a stream (for [`hls_variant`]).
pub fn peak_bandwidth(stream: &SegmentedStream) -> u64 {
    stream
        .segments
        .iter()
        .filter_map(|s| {
            let secs = s.duration.seconds();
            (secs > 0.0).then(|| (s.data.len() as f64 * 8.0 / secs).ceil() as u64)
        })
        .max()
        .unwrap_or(0)
}
