//! Packet-level remux between MP4/MOV and Matroska/WebM: no decoding, no re-encoding, every packet
//! byte is copied.
//!
//! Which codecs travel where:
//!
//! | codec | MP4 / MOV | Matroska | WebM |
//! |---|---|---|---|
//! | H.264, HEVC, AV1, APV | yes | yes | AV1 only |
//! | VP9 | yes | yes | yes |
//! | VP8 | not in MP4 | Matroska to Matroska/WebM only | yes |
//! | ProRes, Motion-JPEG | yes | yes | no |
//! | AAC, MP3, AC-3 | yes | yes | no |
//! | Opus, FLAC | yes | yes | Opus only |
//! | PCM | yes | yes | no |
//! | Vorbis | not in MP4 | Matroska to Matroska/WebM only | yes |
//!
//! A track that cannot be copied to the chosen container is reported in
//! [`RemuxReport::skipped`] when the track set was left to the default, and is an error when it was
//! requested explicitly.

use std::io::{Read, Seek, Write};

use filmcraft_isobmff as iso;
use filmcraft_matroska as mkv;

use crate::container::{Demuxer, FrameIndex, TrackInfo, TrackKind, rescale};
use crate::control::{Control, Stage};
use crate::convert;
use crate::error::{Error, ErrorKind, Result};
use crate::limits::Limits;

/// The container a remux writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutputContainer {
    /// Progressive ISO MP4 (`moov` written last, or first with [`RemuxOptions::faststart`]).
    Mp4,
    /// Progressive QuickTime MOV.
    QuickTime,
    /// Matroska.
    Matroska,
    /// WebM (Matroska restricted to VP8/VP9/AV1 and Opus/Vorbis).
    WebM,
    /// Fragmented MP4 in one file (`moov` with `mvex`, then `moof` + `mdat` pairs).
    FragmentedMp4,
}

impl OutputContainer {
    /// The conventional file extension.
    pub fn extension(self) -> &'static str {
        match self {
            OutputContainer::Mp4 | OutputContainer::FragmentedMp4 => "mp4",
            OutputContainer::QuickTime => "mov",
            OutputContainer::Matroska => "mkv",
            OutputContainer::WebM => "webm",
        }
    }

    /// The container for a file extension (`mp4`, `m4v`, `m4a`, `mov`, `mkv`, `mka`, `webm`).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "mp4" | "m4v" | "m4a" => Some(OutputContainer::Mp4),
            "mov" => Some(OutputContainer::QuickTime),
            "mkv" | "mka" => Some(OutputContainer::Matroska),
            "webm" => Some(OutputContainer::WebM),
            _ => None,
        }
    }

    fn is_mp4_family(self) -> bool {
        matches!(self, OutputContainer::Mp4 | OutputContainer::QuickTime | OutputContainer::FragmentedMp4)
    }
}

/// Options of [`remux`]. Build with [`RemuxOptions::new`] and set the public fields.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RemuxOptions {
    /// The container to write.
    pub container: OutputContainer,
    /// Source track indices to copy, in output order; `None` copies every remuxable audio and video track.
    pub tracks: Option<Vec<usize>>,
    /// Write `moov` before `mdat` (MP4 and MOV only), so playback starts before the download ends.
    pub faststart: bool,
    /// Fragment length of [`OutputContainer::FragmentedMp4`] in milliseconds (cut at the next video keyframe).
    pub fragment_ms: u32,
    /// Size caps for the input.
    pub limits: Limits,
}

impl RemuxOptions {
    /// Copy every audio and video track into `container` with default settings.
    pub fn new(container: OutputContainer) -> Self {
        Self { container, tracks: None, faststart: false, fragment_ms: 2000, limits: Limits::default() }
    }
}

/// What a remux did.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RemuxReport {
    /// Source track indices copied, in output track order.
    pub copied: Vec<usize>,
    /// Source tracks left out, with the reason.
    pub skipped: Vec<(usize, String)>,
    /// Packets written.
    pub packets: u64,
    /// Compressed payload bytes written (container overhead excluded).
    pub payload_bytes: u64,
}

/// One scheduled output sample.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sched {
    /// Decode time relative to the first sample, in `ts` units.
    pub dts: i64,
    /// Composition offset (presentation minus decode) in `ts` units.
    pub cto: i64,
    /// Duration in `ts` units.
    pub dur: u64,
    /// Presentation time in nanoseconds on the common output timeline.
    pub pts_ns: i64,
    /// Duration in nanoseconds.
    pub dur_ns: u64,
}

pub(crate) struct Planned {
    pub src: usize,
    pub kind: TrackKind,
    pub entry: iso::SampleEntry,
    pub ts: u32,
    pub language: String,
    pub samples: Vec<Sched>,
    pub edits: Vec<iso::Edit>,
    pub media_start: Option<i64>,
    pub spec: Option<mkv::TrackSpec>,
}

/// Copy the packets of `input` into a new `container`, writing to `out`.
///
/// Returns the writer (finished and flushed) and a [`RemuxReport`]. `out` must be seekable because
/// the container sizes, index and duration are patched at the end; use
/// [`remux_to_vec`] for memory or [`remux_file`] for a path.
///
/// # Errors
///
/// [`crate::ErrorKind::Unsupported`] when a requested track's codec has no mapping in the target;
/// [`crate::ErrorKind::Cancelled`] / [`crate::ErrorKind::DeadlineExceeded`] from `control`;
/// [`crate::ErrorKind::LimitExceeded`] for an oversized packet; I/O and container errors otherwise.
///
/// # Example
///
/// ```
/// # fn main() -> Result<(), filmcraft_lib::Error> {
/// use filmcraft_lib::{Control, Demuxer, Limits, OutputContainer, RemuxOptions, remux_to_vec};
/// # let mp4 = filmcraft_lib::doc_fixtures::mp4_h264_aac();
/// let input = Demuxer::open_bytes(mp4, &Limits::default())?;
/// let (mkv, report) = remux_to_vec(&input, &RemuxOptions::new(OutputContainer::Matroska), &Control::default())?;
/// assert_eq!(report.copied.len(), 2);
/// assert_eq!(&mkv[..4], &[0x1A, 0x45, 0xDF, 0xA3]);
/// # Ok(()) }
/// ```
pub fn remux<W: Read + Write + Seek>(input: &Demuxer, out: W, opts: &RemuxOptions, control: &Control) -> Result<(W, RemuxReport)> {
    let (selected, skipped) = select_tracks(input, opts)?;
    let webm = opts.container == OutputContainer::WebM;
    let plans = plan(input, &selected, opts.container, webm)?;
    let total: u64 = plans.iter().map(|p| p.samples.len() as u64).sum();
    control.check()?;
    control.report(Stage::Writing, 0, Some(total));
    let slots: Vec<usize> = plans.iter().map(|p| p.src).collect();
    let order = input.interleaved(&slots);
    let mut report = RemuxReport { copied: slots.clone(), skipped, packets: 0, payload_bytes: 0 };
    let writer = match opts.container {
        OutputContainer::Mp4 | OutputContainer::QuickTime => {
            let brand = if opts.container == OutputContainer::QuickTime { iso::Brand::Mov } else { iso::Brand::Mp4 };
            let mut wo = iso::WriterOptions::new(brand);
            wo.movie_timescale = input.movie_timescale();
            wo.metadata = input.info().metadata.iter().filter(|(k, _)| k.starts_with('\u{a9}')).cloned().collect();
            let mut w = iso::Mp4Writer::new(out, wo)?;
            for p in &plans {
                w.add_track(track_config(p))?;
            }
            copy_packets(input, &plans, &order, control, &mut report, opts, |slot, p, s, key| {
                w.write_sample(
                    slot,
                    iso::WriteSample {
                        data: &p.data,
                        duration: u32::try_from(s.dur).map_err(|_| too_long())?,
                        composition_offset: i32::try_from(s.cto).map_err(|_| too_long())?,
                        is_sync: key,
                    },
                )?;
                Ok(())
            })?;
            control.report(Stage::Finishing, total, Some(total));
            if opts.faststart { w.finish_faststart()? } else { w.finish()? }
        }
        OutputContainer::FragmentedMp4 => {
            let mut wo = iso::WriterOptions::new(iso::Brand::Mp4);
            wo.movie_timescale = input.movie_timescale();
            let mut w = iso::FragmentedWriter::new(out, wo);
            for p in &plans {
                w.add_track(track_config(p))?;
            }
            let lead = plans.iter().position(|p| p.kind == TrackKind::Video).unwrap_or(0);
            let mut cut = FragmentClock::new(plans.get(lead).map_or(1, |p| p.ts), opts.fragment_ms);
            copy_packets(input, &plans, &order, control, &mut report, opts, |slot, p, s, key| {
                if slot == lead && key && cut.due(s.dts) {
                    w.flush_fragment()?;
                }
                w.write_sample(
                    slot,
                    iso::WriteSample {
                        data: &p.data,
                        duration: u32::try_from(s.dur).map_err(|_| too_long())?,
                        composition_offset: i32::try_from(s.cto).map_err(|_| too_long())?,
                        is_sync: key,
                    },
                )?;
                Ok(())
            })?;
            control.report(Stage::Finishing, total, Some(total));
            w.finish()?
        }
        OutputContainer::Matroska | OutputContainer::WebM => {
            let specs: Vec<mkv::TrackSpec> = plans.iter().filter_map(|p| p.spec.clone()).collect();
            let mut mo = mkv::MuxOptions::default();
            if webm {
                mo.doc_type = "webm".into();
            }
            mo.title = input.info().metadata.iter().find(|(k, _)| k == "title" || k.ends_with("nam")).map(|(_, v)| v.clone());
            let mut w = mkv::MkvWriter::new(out, specs, mo)?;
            let variable = plans.iter().map(|p| matches!(p.entry.codec, iso::CodecConfig::Opus(_)) || p.kind == TrackKind::Subtitle).collect::<Vec<_>>();
            copy_packets(input, &plans, &order, control, &mut report, opts, |slot, p, s, key| {
                let dur = variable.get(slot).copied().unwrap_or(false).then_some(s.dur_ns);
                w.write_frame(slot, s.pts_ns, key, &p.data, dur)?;
                Ok(())
            })?;
            control.report(Stage::Finishing, total, Some(total));
            w.finish()?
        }
    };
    Ok((writer, report))
}

fn too_long() -> Error {
    Error::new(ErrorKind::Unsupported, "a sample duration or composition offset does not fit 32 bits")
}

/// Cuts a fragmented file roughly every `ms` milliseconds.
struct FragmentClock {
    pub ts: u32,
    every: i64,
    next: i64,
}

impl FragmentClock {
    fn new(ts: u32, ms: u32) -> Self {
        let every = rescale(i64::from(ms.max(100)), 1000, ts.max(1));
        Self { ts, every: every.max(1), next: every.max(1) }
    }
    fn due(&mut self, dts: i64) -> bool {
        let _ = self.ts;
        if dts >= self.next {
            self.next = dts.saturating_add(self.every);
            true
        } else {
            false
        }
    }
}

/// The shared packet loop: read each scheduled packet in interleave order and hand it to `put`.
fn copy_packets(
    input: &Demuxer,
    plans: &[Planned],
    order: &[(usize, usize)],
    control: &Control,
    report: &mut RemuxReport,
    opts: &RemuxOptions,
    mut put: impl FnMut(usize, &crate::container::Packet, &Sched, bool) -> Result<()>,
) -> Result<()> {
    let total = order.len() as u64;
    if total > opts.limits.max_packets {
        return Err(Error::limit("packet count", opts.limits.max_packets, total));
    }
    for (n, &(src, idx)) in order.iter().enumerate() {
        control.check()?;
        let Some(slot) = plans.iter().position(|p| p.src == src) else { continue };
        let plan = &plans[slot];
        let Some(sched) = plan.samples.get(idx) else { continue };
        let pkt = input.read_packet(src, idx)?;
        let key = pkt.key || plan.kind != TrackKind::Video;
        put(slot, &pkt, sched, key)?;
        report.packets += 1;
        report.payload_bytes += pkt.data.len() as u64;
        if n % 128 == 0 {
            control.report(Stage::Writing, n as u64, Some(total));
        }
    }
    control.report(Stage::Writing, total, Some(total));
    Ok(())
}

pub(crate) fn track_config(p: &Planned) -> iso::TrackConfig {
    let mut c = iso::TrackConfig::new(p.entry.clone(), p.ts);
    c.language = p.language.clone();
    c.edits = p.edits.clone();
    c.media_start = if p.edits.is_empty() { p.media_start } else { None };
    c
}

pub(crate) fn select_tracks(input: &Demuxer, opts: &RemuxOptions) -> Result<(Vec<usize>, Vec<(usize, String)>)> {
    let tracks = input.tracks();
    let webm = opts.container == OutputContainer::WebM;
    let mp4 = opts.container.is_mp4_family();
    let can = |t: &TrackInfo| -> std::result::Result<(), String> {
        if !matches!(t.kind, TrackKind::Video | TrackKind::Audio) {
            return Err(format!("{:?} tracks are not copied", t.kind));
        }
        if !t.remuxable && !(input.mkv_track(t.index).is_some() && !mp4) {
            return Err(format!("the {} codec configuration is not understood", t.codec));
        }
        if webm && !convert::webm_allows(&t.codec, t.kind) {
            return Err(format!("{} cannot go into WebM", t.codec));
        }
        if mp4 && matches!(t.codec.as_str(), "vp8" | "vorbis") {
            return Err(format!("{} has no MP4 mapping", t.codec));
        }
        Ok(())
    };
    match &opts.tracks {
        Some(list) => {
            let mut seen = std::collections::HashSet::new();
            for &i in list {
                let t = tracks.get(i).ok_or_else(|| Error::not_found(format!("track {i}")))?;
                if !seen.insert(i) {
                    return Err(Error::invalid(format!("track {i} listed twice")));
                }
                can(t).map_err(|why| Error::unsupported(format!("track {i}: {why}")))?;
            }
            if list.is_empty() {
                return Err(Error::invalid("no tracks selected"));
            }
            Ok((list.clone(), Vec::new()))
        }
        None => {
            let mut keep = Vec::new();
            let mut skipped = Vec::new();
            for t in tracks {
                match can(t) {
                    Ok(()) => keep.push(t.index),
                    Err(why) => skipped.push((t.index, why)),
                }
            }
            if keep.is_empty() {
                return Err(Error::unsupported(format!("no track can be copied into {}", opts.container.extension())));
            }
            Ok((keep, skipped))
        }
    }
}

/// Decide, for each selected track, the output timeline: timescale, per-sample schedule, edit list.
pub(crate) fn plan(input: &Demuxer, selected: &[usize], target: OutputContainer, webm: bool) -> Result<Vec<Planned>> {
    // Matroska targets have no negative timestamps: find the shift that lifts every track to its
    // allowance (an Opus track may start at minus its codec delay). MP4 targets keep the source
    // timeline and describe a late start or pre-roll in the edit list.
    let mut first_ns: Vec<i128> = Vec::new();
    for &t in selected {
        let ix = input.frame_index(t).ok_or_else(|| Error::not_found(format!("track {t}")))?;
        let first = ix.first_pts().unwrap_or(0);
        first_ns.push(i128::from(first) * 1_000_000_000 / i128::from(ix.timescale().max(1)));
    }
    let mut out = Vec::new();
    let mp4_target = target.is_mp4_family();
    // nanosecond shift applied to every track so that none starts before its allowance
    let mut shift_ns: i128 = 0;
    if !mp4_target {
        for (n, &t) in selected.iter().enumerate() {
            let allowed = match input.plan(t).map(|p| &p.entry.codec) {
                Some(iso::CodecConfig::Opus(o)) => i128::from(o.pre_skip) * 1_000_000_000 / 48_000,
                _ => input.mkv_track(t).map_or(0, |m| i128::from(m.codec_delay_ns)),
            };
            let f = first_ns.get(n).copied().unwrap_or(0);
            shift_ns = shift_ns.max(-f - allowed);
        }
    }
    for &t in selected {
        let info = input.tracks().get(t).ok_or_else(|| Error::not_found(format!("track {t}")))?;
        let ix = input.frame_index(t).ok_or_else(|| Error::not_found(format!("track {t}")))?;
        let p = input.plan(t);
        let src_is_mkv = input.mkv_track(t).is_some();
        let spec = if mp4_target {
            None
        } else if let Some(m) = input.mkv_track(t) {
            Some(spec_from_mkv(m, info, webm)?)
        } else {
            let entry = p.map(|p| &p.entry).ok_or_else(|| Error::unsupported(format!("track {t} has no codec configuration")))?;
            Some(convert::spec_from_entry(entry, info, webm)?)
        };
        let entry = match p {
            Some(p) => p.entry.clone(),
            None if !mp4_target => placeholder_entry(info),
            None => return Err(Error::unsupported(format!("track {t} ({}) has no MP4 mapping", info.codec))),
        };
        let language = p.map_or_else(|| info.language.clone(), |p| p.language.clone());
        let (ts, samples, edits, media_start) = if mp4_target {
            if src_is_mkv { schedule_from_mkv(input, info, ix, &entry)? } else { schedule_mp4_passthrough(input, ix, p) }
        } else {
            (ix.timescale(), mkv_schedule(ix, shift_ns, src_is_mkv), Vec::new(), None)
        };
        out.push(Planned { src: t, kind: info.kind, entry, ts, language, samples, edits, media_start, spec });
    }
    Ok(out)
}

/// An MKV-only track copied to Matroska never needs an MP4 sample entry; this fills the slot.
fn placeholder_entry(info: &TrackInfo) -> iso::SampleEntry {
    iso::SampleEntry::audio(
        iso::FourCc(*b"unkn"),
        iso::CodecConfig::Unknown { fourcc: iso::FourCc(*b"unkn"), raw: Vec::new() },
        info.channels,
        f64::from(info.sample_rate),
        16,
    )
}

fn spec_from_mkv(m: &mkv::Track, info: &TrackInfo, webm: bool) -> Result<mkv::TrackSpec> {
    if webm && !convert::webm_allows(&info.codec, info.kind) {
        return Err(Error::unsupported(format!("{} cannot go into WebM (it carries VP8, VP9, AV1, Opus and Vorbis)", info.codec)));
    }
    let mut s = mkv::TrackSpec::new(m.kind, m.codec_id.clone());
    s.codec_private = m.codec_private.clone();
    s.default_duration_ns = m.default_duration_ns;
    s.language = Some(m.language.clone()).filter(|l| !l.is_empty());
    s.name = Some(m.name.clone()).filter(|n| !n.is_empty());
    s.codec_delay_ns = m.codec_delay_ns;
    s.seek_pre_roll_ns = m.seek_pre_roll_ns;
    if let Some(v) = &m.video {
        s.video_size = Some((v.pixel_width, v.pixel_height));
        if let (Some(w), Some(h)) = (v.display_width, v.display_height)
            && v.display_unit == 0
        {
            s.display_size = Some((w, h));
        }
        s.colour = v.colour.clone();
    }
    if let Some(a) = &m.audio {
        s.audio = Some((a.output_sampling_frequency.unwrap_or(a.sampling_frequency), a.channels, a.bit_depth));
    }
    Ok(s)
}

/// Samples for a Matroska target: presentation times in ns with the common shift applied.
fn mkv_schedule(ix: &FrameIndex, shift_ns: i128, src_is_mkv: bool) -> Vec<Sched> {
    let ts = i128::from(ix.timescale().max(1));
    ix.entries()
        .iter()
        .map(|e| {
            let (pts_ns, dur_ns) = if src_is_mkv {
                (i128::from(e.pts), i128::from(e.duration))
            } else {
                (i128::from(e.pts) * 1_000_000_000 / ts, i128::from(e.duration) * 1_000_000_000 / ts)
            };
            Sched {
                dts: e.dts,
                cto: e.composition_offset,
                dur: e.duration,
                pts_ns: i64::try_from(pts_ns + shift_ns).unwrap_or(i64::MAX),
                dur_ns: u64::try_from(dur_ns).unwrap_or(0),
            }
        })
        .collect()
}

type Timeline = (u32, Vec<Sched>, Vec<iso::Edit>, Option<i64>);

/// MP4 to MP4/MOV: copy the timeline as stored, keeping the edit list.
fn schedule_mp4_passthrough(input: &Demuxer, ix: &FrameIndex, plan: Option<&crate::container::TrackPlan>) -> Timeline {
    let d0 = ix.get(0).map_or(0, |e| e.dts);
    let samples = ix.entries().iter().map(|e| Sched { dts: e.dts - d0, cto: e.composition_offset, dur: e.duration, pts_ns: 0, dur_ns: 0 }).collect();
    let edits = plan
        .map(|p| p.edits.iter().map(|ed| iso::Edit { media_time: if ed.media_time >= 0 { ed.media_time - d0 } else { ed.media_time }, ..*ed }).collect())
        .unwrap_or_default();
    let _ = input;
    (ix.timescale(), samples, edits, None)
}

/// Matroska to MP4/MOV: rebuild decode times and pick a timescale that keeps frame durations whole.
fn schedule_from_mkv(input: &Demuxer, info: &TrackInfo, ix: &FrameIndex, entry: &iso::SampleEntry) -> Result<Timeline> {
    let e = ix.entries();
    let first_pts = ix.first_pts().unwrap_or(0);
    // units are counted from the track's own first presentation time; where the track starts on
    // the file timeline (delay or pre-roll) is carried by the edit list, in exact nanoseconds
    let origin_ns = i128::from(first_pts);
    let (ts, pts_u) = match info.kind {
        TrackKind::Video => video_units(e, first_pts, origin_ns, input.mkv_track(info.index).and_then(|t| t.default_duration_ns)),
        _ => audio_units(e, first_pts, origin_ns, info, entry),
    };
    // decode order = file order; presentation order sorted
    let mut sorted = pts_u.clone();
    sorted.sort_unstable();
    let shift = pts_u.iter().zip(sorted.iter()).map(|(p, s)| s - p).max().unwrap_or(0).max(0);
    let s0 = sorted.first().copied().unwrap_or(0);
    let n = e.len();
    let regular = info.kind == TrackKind::Video || matches!(entry.codec, iso::CodecConfig::Aac(_) | iso::CodecConfig::Mp3 | iso::CodecConfig::Ac3 { .. });
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let dts = sorted[i] - s0;
        let next = sorted.get(i + 1).map(|v| v - s0);
        let dur = match next {
            Some(nx) => (nx - dts).max(0) as u64,
            // the last packet lasts as long as the one before it when the rate is regular
            None if regular && i > 0 => samples.last().map_or(0, |p: &Sched| p.dur),
            None => u64::try_from(rescale(i64::try_from(e[i].duration).unwrap_or(0), 1_000_000_000, ts)).unwrap_or(0),
        };
        samples.push(Sched { dts, cto: pts_u[i] - sorted[i] + shift, dur, pts_ns: 0, dur_ns: 0 });
    }
    // edit list: a late start is an empty edit, a start before zero (codec pre-roll) is skipped
    let comp_end = samples.iter().map(|s| s.dts + s.cto + s.dur as i64).max().unwrap_or(0);
    let (delay_ms, skip) =
        if first_pts > 0 { (rescale(first_pts, 1_000_000_000, 1000), 0) } else { (0, rescale(first_pts.saturating_neg(), 1_000_000_000, ts)) };
    let mut edits = Vec::new();
    let mut media_start = None;
    if delay_ms > 0 {
        let rest = (comp_end - shift).max(0);
        edits.push(iso::Edit { segment_duration: u64::try_from(delay_ms).unwrap_or(0), media_time: -1, media_rate: 0x10000 });
        edits.push(iso::Edit { segment_duration: u64::try_from(rescale(rest, ts, 1000)).unwrap_or(0), media_time: shift, media_rate: 0x10000 });
    } else if shift + skip > 0 {
        media_start = Some(shift + skip);
    }
    Ok((ts, samples, edits, media_start))
}

/// Presentation times of a Matroska video track in output units: on the exact frame grid when the
/// rate is constant (Matroska stores whole milliseconds, which would otherwise make every frame
/// duration wobble), else in a 90 kHz timescale.
fn video_units(e: &[crate::container::FrameEntry], first_pts: i64, origin_ns: i128, default_ns: Option<u64>) -> (u32, Vec<i64>) {
    let mut durs: Vec<u64> = e.iter().map(|x| x.duration).filter(|d| *d > 0).collect();
    durs.sort_unstable();
    let median = default_ns.filter(|d| *d > 0).or_else(|| durs.get(durs.len() / 2).copied());
    if let Some(d) = median {
        let fps = 1e9 / d as f64;
        let rate = filmcraft_time::FrameRate::from_f64(fps);
        if let (Ok(num), den) = (u32::try_from(rate.num), rate.den)
            && num > 0
            && den > 0
            && (rate.as_f64() - fps).abs() / fps < 0.01
        {
            let grid_ns = |k: i128| k * i128::from(den) * 1_000_000_000 / i128::from(rate.num);
            let k_of = |p: i64| -> i128 {
                let rel = i128::from(p - first_pts);
                (rel * i128::from(rate.num) + i128::from(den) * 500_000_000) / (i128::from(den) * 1_000_000_000)
            };
            let tol = 2_000_000i128.max(i128::from(d) / 20);
            if e.iter().all(|x| (i128::from(x.pts - first_pts) - grid_ns(k_of(x.pts))).abs() <= tol) {
                let off = rescale(i64::try_from(i128::from(first_pts) - origin_ns).unwrap_or(0), 1_000_000_000, num);
                return (num, e.iter().map(|x| off + (k_of(x.pts) as i64) * den).collect());
            }
        }
    }
    (90_000, e.iter().map(|x| rescale(i64::try_from(i128::from(x.pts) - origin_ns).unwrap_or(0), 1_000_000_000, 90_000)).collect())
}

/// Presentation times of an audio track in output units (the sample rate), snapped to the codec's
/// fixed frame grid when the stored millisecond timestamps agree with it.
fn audio_units(e: &[crate::container::FrameEntry], first_pts: i64, origin_ns: i128, info: &TrackInfo, entry: &iso::SampleEntry) -> (u32, Vec<i64>) {
    let rate = if info.sample_rate > 0 { info.sample_rate } else { 48_000 };
    let frame = match entry.codec {
        iso::CodecConfig::Aac(_) => 1024i128,
        iso::CodecConfig::Mp3 => 1152,
        iso::CodecConfig::Ac3 { .. } => 1536,
        _ => 0,
    };
    let plain = |x: &crate::container::FrameEntry| rescale(i64::try_from(i128::from(x.pts) - origin_ns).unwrap_or(0), 1_000_000_000, rate);
    if frame > 0 {
        let grid_ns = frame * 1_000_000_000 / i128::from(rate);
        let k_of = |p: i64| (i128::from(p - first_pts) + grid_ns / 2) / grid_ns.max(1);
        let fits = e.iter().all(|x| (i128::from(x.pts - first_pts) - k_of(x.pts) * grid_ns).abs() <= 2_000_000);
        let mut ks: Vec<i128> = e.iter().map(|x| k_of(x.pts)).collect();
        let strictly = ks.windows(2).all(|w| w[0] < w[1]);
        ks.truncate(e.len());
        if fits && strictly {
            let off = rescale(i64::try_from(i128::from(first_pts) - origin_ns).unwrap_or(0), 1_000_000_000, rate);
            return (rate, ks.iter().map(|k| off + (*k as i64) * frame as i64).collect());
        }
    }
    (rate, e.iter().map(plain).collect())
}

/// [`remux`] into memory.
pub fn remux_to_vec(input: &Demuxer, opts: &RemuxOptions, control: &Control) -> Result<(Vec<u8>, RemuxReport)> {
    let (cursor, report) = remux(input, std::io::Cursor::new(Vec::new()), opts, control)?;
    Ok((cursor.into_inner(), report))
}

/// [`remux`] into the file `path`. The output is written next to the destination under a temporary
/// name and renamed into place when complete, so a failed or cancelled run never leaves a partial
/// file at `path`.
#[cfg(any(unix, windows))]
pub fn remux_file(input: &Demuxer, path: &std::path::Path, opts: &RemuxOptions, control: &Control) -> Result<RemuxReport> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".part{}", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    let result = (|| -> Result<RemuxReport> {
        let file = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&tmp)?;
        let (file, report) = remux(input, file, opts, control)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        Ok(report)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
