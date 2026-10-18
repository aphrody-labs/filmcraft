//! Container-level access: exact packet tables, track facts and packet reads for MP4/MOV and
//! Matroska/WebM, without touching a codec.
//!
//! [`Demuxer::open`] parses the index of a file once. After that:
//!
//! - [`Demuxer::tracks`] tells what each track is (codec, size, rate, colour, HDR);
//! - [`Demuxer::frame_index`] gives the exact decode- and presentation-order table of a track:
//!   frame count, every timestamp, which frames are keyframes;
//! - [`Demuxer::read_packet`] reads one compressed packet on demand;
//! - [`remux`](crate::remux) and [`segment`](crate::segment) are built on the same tables.

use std::sync::Arc;

use filmcraft_isobmff as iso;
use filmcraft_matroska as mkv;
use filmcraft_time::{FrameRate, Tick};

use crate::convert;
use crate::error::{Error, Result};
use crate::limits::Limits;
use crate::source::{Adapter, ByteSource, MemorySource};

/// The container format of a file, from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContainerKind {
    /// ISO base media file (MP4, M4A, 3GP, fragmented MP4).
    Mp4,
    /// QuickTime movie (`qt  ` brand or a bare `moov`).
    QuickTime,
    /// Matroska (`.mkv`, `.mka`).
    Matroska,
    /// WebM.
    WebM,
    /// AVI (RIFF).
    Avi,
    /// MXF (SMPTE ST 377).
    Mxf,
    /// MPEG-2 transport stream.
    MpegTs,
    /// MPEG program stream.
    MpegPs,
    /// Ogg.
    Ogg,
    /// RIFF/WAVE.
    Wav,
    /// Native FLAC.
    Flac,
    /// A still or animated image (PNG, JPEG, GIF, WebP).
    Image,
    /// Anything else.
    Unknown,
}

impl ContainerKind {
    /// Identify the container from the first bytes of a file (64 KiB is plenty). Never panics on
    /// short or hostile input.
    pub fn sniff(head: &[u8]) -> ContainerKind {
        let at = |o: usize, n: usize| head.get(o..o.saturating_add(n));
        if at(0, 4) == Some(&[0x1A, 0x45, 0xDF, 0xA3]) {
            // the DocType sits in the EBML header, a few bytes in
            let window = head.get(..head.len().min(64)).unwrap_or(head);
            return if window.windows(4).any(|w| w == b"webm") { ContainerKind::WebM } else { ContainerKind::Matroska };
        }
        if let Some(t) = at(4, 4) {
            if t == b"ftyp" {
                let brand = at(8, 4).unwrap_or(&[]);
                return if brand == b"qt  " { ContainerKind::QuickTime } else { ContainerKind::Mp4 };
            }
            if matches!(t, b"moov" | b"mdat" | b"wide" | b"free" | b"skip" | b"pnot") {
                return ContainerKind::QuickTime;
            }
        }
        match at(0, 4) {
            Some(b"RIFF") => {
                return match at(8, 4) {
                    Some(b"AVI ") => ContainerKind::Avi,
                    Some(b"WAVE") => ContainerKind::Wav,
                    Some(b"WEBP") => ContainerKind::Image,
                    _ => ContainerKind::Unknown,
                };
            }
            Some(b"OggS") => return ContainerKind::Ogg,
            Some(b"fLaC") => return ContainerKind::Flac,
            Some([0x06, 0x0E, 0x2B, 0x34]) => return ContainerKind::Mxf,
            Some([0x00, 0x00, 0x01, 0xBA]) => return ContainerKind::MpegPs,
            Some([0x89, b'P', b'N', b'G']) | Some([0xFF, 0xD8, 0xFF, _]) | Some([b'G', b'I', b'F', b'8']) => return ContainerKind::Image,
            _ => {}
        }
        // transport streams: sync byte 0x47 every 188 (or 192 / 204) bytes
        for (pkt, lead) in [(188usize, 0usize), (192, 4), (204, 0)] {
            let sync = |n: usize| head.get(lead + n * pkt) == Some(&0x47);
            if head.len() > lead + 3 * pkt && (0..4).all(sync) {
                return ContainerKind::MpegTs;
            }
        }
        ContainerKind::Unknown
    }

    /// Whether [`Demuxer`] can open this container.
    pub fn is_demuxable(self) -> bool {
        matches!(self, ContainerKind::Mp4 | ContainerKind::QuickTime | ContainerKind::Matroska | ContainerKind::WebM)
    }

    /// A short lowercase name (`"mp4"`, `"webm"`, ...).
    pub fn name(self) -> &'static str {
        match self {
            ContainerKind::Mp4 => "mp4",
            ContainerKind::QuickTime => "mov",
            ContainerKind::Matroska => "matroska",
            ContainerKind::WebM => "webm",
            ContainerKind::Avi => "avi",
            ContainerKind::Mxf => "mxf",
            ContainerKind::MpegTs => "mpegts",
            ContainerKind::MpegPs => "mpegps",
            ContainerKind::Ogg => "ogg",
            ContainerKind::Wav => "wav",
            ContainerKind::Flac => "flac",
            ContainerKind::Image => "image",
            ContainerKind::Unknown => "unknown",
        }
    }
}

/// What a track carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TrackKind {
    /// Pictures.
    Video,
    /// Sound.
    Audio,
    /// Subtitles or captions.
    Subtitle,
    /// Timecode, metadata or anything else.
    Data,
}

/// Colour signalling of a video track, as ISO/IEC 23091-2 (H.273) code points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ColorSignal {
    /// Colour primaries (1 = BT.709, 9 = BT.2020, ...).
    pub primaries: u16,
    /// Transfer characteristics (1 = BT.709, 16 = PQ, 18 = HLG, ...).
    pub transfer: u16,
    /// Matrix coefficients (1 = BT.709, 9 = BT.2020 non-constant, ...).
    pub matrix: u16,
    /// Full-range samples (`false` = limited / studio range).
    pub full_range: bool,
}

/// HDR static metadata (SMPTE ST 2086 mastering display and CTA-861.3 light levels).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct HdrStatic {
    /// Mastering display red, green, blue primaries as CIE 1931 `(x, y)`.
    pub primaries: Option<[(f32, f32); 3]>,
    /// Mastering display white point as CIE 1931 `(x, y)`.
    pub white_point: Option<(f32, f32)>,
    /// Mastering display peak luminance in cd/m2.
    pub max_luminance: Option<f32>,
    /// Mastering display minimum luminance in cd/m2.
    pub min_luminance: Option<f32>,
    /// Maximum content light level in cd/m2.
    pub max_cll: Option<u32>,
    /// Maximum frame-average light level in cd/m2.
    pub max_fall: Option<u32>,
}

/// Facts about one track of a container.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct TrackInfo {
    /// Position in [`Demuxer::tracks`]; the handle every other call takes.
    pub index: usize,
    /// The container's own track id (MP4 `track_ID`, Matroska `TrackNumber`).
    pub id: u64,
    /// What the track carries.
    pub kind: TrackKind,
    /// Short codec name (`h264`, `hevc`, `av1`, `vp9`, `prores`, `aac`, `opus`, `flac`, ...).
    pub codec: String,
    /// The container's codec tag: the sample entry FourCC (MP4) or `CodecID` (Matroska).
    pub codec_tag: String,
    /// Ticks per second of this track's timestamps (MP4 media timescale; Matroska tracks use 10^9).
    pub timescale: u32,
    /// ISO 639 language code (`und` when unknown).
    pub language: String,
    /// Coded picture width in pixels (0 for audio).
    pub width: u32,
    /// Coded picture height in pixels (0 for audio).
    pub height: u32,
    /// Pixel aspect ratio.
    pub pixel_aspect: (u32, u32),
    /// Clockwise display rotation in degrees (0, 90, 180, 270).
    pub rotation: u16,
    /// Audio sample rate in Hz (0 for video).
    pub sample_rate: u32,
    /// Audio channel count (0 for video).
    pub channels: u32,
    /// Number of packets (samples). For a video track this is the exact frame count.
    pub packets: u64,
    /// Number of packets that are random-access points.
    pub keyframes: u64,
    /// Presentation time of the first packet.
    pub start: Tick,
    /// Time from the first presentation timestamp to the end of the last packet.
    pub duration: Tick,
    /// Average frame rate of a video track (exact when the rate is constant).
    pub frame_rate: Option<FrameRate>,
    /// Whether every video packet has the same duration.
    pub constant_frame_rate: bool,
    /// Average bitrate in bits per second, when computable.
    pub bitrate: Option<u64>,
    /// Colour signalling of a video track, when the file states it.
    pub color: Option<ColorSignal>,
    /// HDR static metadata, when the file carries it.
    pub hdr: Option<HdrStatic>,
    /// The track is enabled / default (Matroska `FlagDefault`, MP4 `tkhd` enabled).
    pub enabled: bool,
    /// Whether [`remux`](crate::remux) can copy this track (codec configuration understood).
    pub remuxable: bool,
}

/// One packet of a [`FrameIndex`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FrameEntry {
    /// Decode timestamp in the track timescale.
    pub dts: i64,
    /// Presentation timestamp in the track timescale (edit lists applied; may be negative for pre-roll).
    pub pts: i64,
    /// Presentation minus decode timestamp as stored (before edit lists): the B-frame reorder delay.
    pub composition_offset: i64,
    /// Duration in the track timescale.
    pub duration: u64,
    /// Compressed size in bytes.
    pub size: u32,
    /// Byte offset of the packet in the file.
    pub offset: u64,
    /// Random-access point.
    pub key: bool,
}

/// The exact packet table of one track: decode order as stored, with presentation order derived.
///
/// For a video track, `len()` is the exact frame count and [`presented`](Self::presented) walks the
/// frames in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameIndex {
    timescale: u32,
    entries: Vec<FrameEntry>,
    /// Entry indices sorted by presentation time (ties by decode order).
    order: Vec<u32>,
}

impl FrameIndex {
    pub(crate) fn new(timescale: u32, entries: Vec<FrameEntry>) -> Self {
        let mut order: Vec<u32> = (0..entries.len() as u32).collect();
        order.sort_by_key(|&i| (entries.get(i as usize).map_or(0, |e| e.pts), i));
        Self { timescale, entries, order }
    }

    /// Ticks per second of the timestamps in this table.
    pub fn timescale(&self) -> u32 {
        self.timescale
    }

    /// Number of packets (frames for video).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the track has no packets.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The packets in decode (storage) order.
    pub fn entries(&self) -> &[FrameEntry] {
        &self.entries
    }

    /// Packet `i` in decode order.
    pub fn get(&self, i: usize) -> Option<&FrameEntry> {
        self.entries.get(i)
    }

    /// The decode-order index of the `n`th frame in display order.
    pub fn display_to_decode(&self, n: usize) -> Option<usize> {
        self.order.get(n).map(|&i| i as usize)
    }

    /// The `n`th frame in display order.
    pub fn presented(&self, n: usize) -> Option<&FrameEntry> {
        self.entries.get(self.display_to_decode(n)?)
    }

    /// Presentation time of the `n`th frame in display order.
    pub fn time_of_frame(&self, n: usize) -> Option<Tick> {
        Some(self.to_tick(self.presented(n)?.pts))
    }

    /// Convert a timestamp of this table to [`Tick`].
    pub fn to_tick(&self, ts: i64) -> Tick {
        Tick::from_rational(ts, 1, i64::from(self.timescale.max(1)))
    }

    /// Display-order number of the frame shown at `t`: the last frame whose presentation time is at
    /// or before `t`, or the first frame when `t` precedes them all. `None` for an empty track.
    pub fn frame_at(&self, t: Tick) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let ts = t.to_rational_round(1, i64::from(self.timescale.max(1)));
        let k = self.order.partition_point(|&i| self.entries.get(i as usize).is_some_and(|e| e.pts <= ts));
        Some(k.saturating_sub(1))
    }

    /// Decode-order index of the closest keyframe at or before decode index `i` (0 when none).
    pub fn keyframe_before(&self, i: usize) -> usize {
        let end = i.saturating_add(1).min(self.entries.len());
        self.entries.get(..end).and_then(|s| s.iter().rposition(|e| e.key)).unwrap_or(0)
    }

    /// Decode-order indices of the keyframes.
    pub fn keyframes(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().enumerate().filter(|(_, e)| e.key).map(|(i, _)| i)
    }

    /// First presentation timestamp.
    pub fn first_pts(&self) -> Option<i64> {
        self.entries.iter().map(|e| e.pts).min()
    }

    /// Latest end of a packet on the presentation timeline.
    pub fn end_pts(&self) -> Option<i64> {
        self.entries.iter().map(|e| e.pts.saturating_add(i64::try_from(e.duration).unwrap_or(i64::MAX))).max()
    }

    /// Whether every packet has the same duration.
    pub fn is_constant_duration(&self) -> bool {
        match self.entries.split_first() {
            Some((first, rest)) => rest.iter().all(|e| e.duration == first.duration),
            None => true,
        }
    }
}

/// One compressed packet read from a container.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Packet {
    /// Track index.
    pub track: usize,
    /// Decode-order index within the track.
    pub index: usize,
    /// Decode timestamp in the track timescale.
    pub dts: i64,
    /// Presentation timestamp in the track timescale.
    pub pts: i64,
    /// Duration in the track timescale.
    pub duration: u64,
    /// Random-access point.
    pub key: bool,
    /// The compressed bytes, in the container's native framing (length-prefixed NAL units for
    /// H.264 / HEVC, raw OBUs for AV1, raw AAC access units, ...).
    pub data: Vec<u8>,
}

/// Container-wide facts.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ContainerInfo {
    /// Which container.
    pub kind: ContainerKind,
    /// MP4 major brand or Matroska DocType.
    pub brand: String,
    /// Fragmented MP4 (`moof` boxes) or a live Matroska stream.
    pub fragmented: bool,
    /// Longest presentation span over all tracks.
    pub duration: Tick,
    /// Title and other text metadata.
    pub metadata: Vec<(String, String)>,
}

/// Everything remux and segmentation need about a track beyond [`TrackInfo`].
#[derive(Clone, Debug)]
pub(crate) struct TrackPlan {
    pub entry: iso::SampleEntry,
    pub edits: Vec<iso::Edit>,
    pub language: String,
}

enum Inner {
    Mp4(Box<iso::Mp4File>),
    Mkv(Box<mkv::MkvFile>),
}

/// A parsed MP4/MOV or Matroska/WebM file: the index in memory, packets read on demand.
///
/// `Demuxer` is `Send + Sync`: share it between threads and read packets concurrently.
pub struct Demuxer {
    src: Arc<dyn ByteSource>,
    inner: Inner,
    info: ContainerInfo,
    tracks: Vec<TrackInfo>,
    indexes: Vec<FrameIndex>,
    plans: Vec<Option<TrackPlan>>,
    /// Container-level movie timescale (MP4), used to express edit lists.
    movie_timescale: u32,
    max_packet_bytes: u64,
}

impl std::fmt::Debug for Demuxer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Demuxer").field("info", &self.info).field("tracks", &self.tracks.len()).finish()
    }
}

impl Demuxer {
    /// Open from any [`ByteSource`]. Only the index is read now.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::LimitExceeded`] when the source is larger than `limits.max_input_bytes` or has
    /// more packets than `limits.max_packets`; [`crate::ErrorKind::Unsupported`] for other containers;
    /// [`crate::ErrorKind::Invalid`] for damaged files.
    pub fn open(src: Arc<dyn ByteSource>, limits: &Limits) -> Result<Self> {
        if src.len() > limits.max_input_bytes {
            return Err(Error::limit("input size", limits.max_input_bytes, src.len()));
        }
        let mut head = vec![0u8; src.len().min(64 * 1024) as usize];
        src.read_at(0, &mut head)?;
        let kind = ContainerKind::sniff(&head);
        let adapter = Adapter(src.clone());
        let inner = match kind {
            ContainerKind::Mp4 | ContainerKind::QuickTime => Inner::Mp4(Box::new(iso::open(adapter)?)),
            ContainerKind::Matroska | ContainerKind::WebM => Inner::Mkv(Box::new(mkv::open(adapter)?)),
            other => {
                return Err(Error::unsupported(format!("the container layer reads MP4/MOV and Matroska/WebM, found {}", other.name())));
            }
        };
        let mut d = Self {
            src,
            inner,
            info: ContainerInfo { kind, brand: String::new(), fragmented: false, duration: Tick::ZERO, metadata: Vec::new() },
            tracks: Vec::new(),
            indexes: Vec::new(),
            plans: Vec::new(),
            movie_timescale: 1000,
            max_packet_bytes: limits.max_packet_bytes,
        };
        d.build(limits)?;
        Ok(d)
    }

    /// Open a file held in memory.
    pub fn open_bytes(bytes: impl Into<Arc<[u8]>>, limits: &Limits) -> Result<Self> {
        Self::open(Arc::new(MemorySource::new(bytes)), limits)
    }

    /// Open a file on disk (positional reads; the file is not loaded into memory).
    #[cfg(any(unix, windows))]
    pub fn open_path(path: &std::path::Path, limits: &Limits) -> Result<Self> {
        let src = crate::source::FileSource::open(path)?;
        Self::open(Arc::new(src), limits)
    }

    /// Container-wide facts.
    pub fn info(&self) -> &ContainerInfo {
        &self.info
    }

    /// Every track, in file order.
    pub fn tracks(&self) -> &[TrackInfo] {
        &self.tracks
    }

    /// The exact packet table of `track`.
    pub fn frame_index(&self, track: usize) -> Option<&FrameIndex> {
        self.indexes.get(track)
    }

    /// Index of the first track of `kind`.
    pub fn first_track(&self, kind: TrackKind) -> Option<usize> {
        self.tracks.iter().position(|t| t.kind == kind)
    }

    /// The RFC 6381 `codecs` string of `track` (`avc1.64001F`, `mp4a.40.2`, ...), when its codec
    /// configuration is understood.
    pub fn codec_string(&self, track: usize) -> Option<String> {
        crate::codec_string::codec_string(&self.plan(track)?.entry)
    }

    /// The byte source this demuxer reads from.
    pub fn source(&self) -> &Arc<dyn ByteSource> {
        &self.src
    }

    pub(crate) fn plan(&self, track: usize) -> Option<&TrackPlan> {
        self.plans.get(track)?.as_ref()
    }

    /// The Matroska track behind `track`, when this is a Matroska file.
    pub(crate) fn mkv_track(&self, track: usize) -> Option<&mkv::Track> {
        match &self.inner {
            Inner::Mkv(f) => f.tracks.get(track),
            Inner::Mp4(_) => None,
        }
    }

    pub(crate) fn movie_timescale(&self) -> u32 {
        self.movie_timescale
    }

    /// Read packet `index` (decode order) of `track`.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::NotFound`] for an unknown track or index; [`crate::ErrorKind::LimitExceeded`] when the
    /// packet is larger than `limits.max_packet_bytes`; [`crate::ErrorKind::Invalid`] when the file ends
    /// before the packet does.
    pub fn read_packet(&self, track: usize, index: usize) -> Result<Packet> {
        let ix = self.indexes.get(track).ok_or_else(|| Error::not_found(format!("track {track}")))?;
        let e = *ix.get(index).ok_or_else(|| Error::not_found(format!("packet {index} of track {track}")))?;
        if u64::from(e.size) > self.max_packet_bytes {
            return Err(Error::limit("packet size", self.max_packet_bytes, u64::from(e.size)));
        }
        let src = Adapter(self.src.clone());
        let data = match &self.inner {
            Inner::Mp4(f) => f.read_sample(&src, track, index)?,
            Inner::Mkv(f) => f.read_sample(&src, track, index)?,
        };
        Ok(Packet { track, index, dts: e.dts, pts: e.pts, duration: e.duration, key: e.key, data })
    }

    /// All packets of the selected `tracks` in decode order, interleaved by decode time.
    pub fn interleaved(&self, tracks: &[usize]) -> Vec<(usize, usize)> {
        let mut heads: Vec<(usize, usize)> = tracks.iter().copied().filter(|t| self.indexes.get(*t).is_some_and(|i| !i.is_empty())).map(|t| (t, 0)).collect();
        let total: usize = heads.iter().map(|(t, _)| self.indexes.get(*t).map_or(0, FrameIndex::len)).sum();
        let mut out = Vec::with_capacity(total);
        while !heads.is_empty() {
            let mut best: Option<(usize, i128)> = None;
            for (slot, (t, i)) in heads.iter().enumerate() {
                let Some(ix) = self.indexes.get(*t) else { continue };
                let Some(e) = ix.get(*i) else { continue };
                // decode time in nanoseconds, exact
                let ns = i128::from(e.dts) * 1_000_000_000 / i128::from(ix.timescale().max(1));
                if best.is_none_or(|(_, b)| ns < b) {
                    best = Some((slot, ns));
                }
            }
            let Some((slot, _)) = best else { break };
            let Some(head) = heads.get_mut(slot) else { break };
            out.push(*head);
            head.1 += 1;
            if self.indexes.get(head.0).is_none_or(|ix| head.1 >= ix.len()) {
                heads.swap_remove(slot);
            }
        }
        out
    }

    fn build(&mut self, limits: &Limits) -> Result<()> {
        match &self.inner {
            Inner::Mp4(_) => self.build_mp4(limits),
            Inner::Mkv(_) => self.build_mkv(limits),
        }
    }
}

pub(crate) fn rescale(v: i64, from: u32, to: u32) -> i64 {
    if from == 0 {
        return 0;
    }
    let n = i128::from(v) * i128::from(to);
    let d = i128::from(from);
    let q = (n + if n >= 0 { d / 2 } else { -(d / 2) }) / d;
    i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX })
}

impl Demuxer {
    fn build_mp4(&mut self, limits: &Limits) -> Result<()> {
        let Inner::Mp4(file) = &self.inner else { return Ok(()) };
        let file: &iso::Mp4File = file;
        self.info.brand = String::from_utf8_lossy(&file.major_brand.0).trim().to_string();
        self.info.fragmented = file.fragmented;
        self.info.metadata = file.metadata.entries.clone();
        self.movie_timescale = file.timescale.max(1);
        let mut total: u64 = 0;
        let mut tracks = Vec::new();
        let mut indexes = Vec::new();
        let mut plans = Vec::new();
        let mut longest = Tick::ZERO;
        for (n, t) in file.tracks.iter().enumerate() {
            total = total.saturating_add(t.samples.len() as u64);
            if total > limits.max_packets {
                return Err(Error::limit("packet count", limits.max_packets, total));
            }
            let timescale = t.timescale.max(1);
            let entries: Vec<FrameEntry> = t
                .samples
                .iter()
                .enumerate()
                .map(|(i, s)| FrameEntry {
                    dts: s.dts,
                    pts: t.presentation_pts(i).unwrap_or(s.pts),
                    composition_offset: s.pts.saturating_sub(s.dts),
                    duration: u64::from(s.duration),
                    size: s.size,
                    offset: s.offset,
                    key: s.is_sync,
                })
                .collect();
            let index = FrameIndex::new(timescale, entries);
            let entry = t.entries.first();
            let kind = match t.kind {
                iso::TrackKind::Video => TrackKind::Video,
                iso::TrackKind::Audio => TrackKind::Audio,
                iso::TrackKind::Subtitle => TrackKind::Subtitle,
                _ => TrackKind::Data,
            };
            let mut info = base_info(n, u64::from(t.id), kind, timescale, &t.language, &index);
            info.enabled = t.enabled;
            if let Some(e) = entry {
                info.codec = e.codec.name().to_string();
                info.codec_tag = String::from_utf8_lossy(&e.format.0).to_string();
                if let Some(v) = &e.video {
                    info.width = if v.width > 0 { u32::from(v.width) } else { t.width };
                    info.height = if v.height > 0 { u32::from(v.height) } else { t.height };
                    info.pixel_aspect = v.pixel_aspect.filter(|(a, b)| *a > 0 && *b > 0).unwrap_or((1, 1));
                    info.color = v.color.as_ref().and_then(|c| match c {
                        iso::ColorInfo::Nclx { primaries, transfer, matrix, full_range } => {
                            Some(ColorSignal { primaries: *primaries, transfer: *transfer, matrix: *matrix, full_range: *full_range })
                        }
                        iso::ColorInfo::Nclc { primaries, transfer, matrix } => {
                            Some(ColorSignal { primaries: *primaries, transfer: *transfer, matrix: *matrix, full_range: false })
                        }
                        iso::ColorInfo::Icc { .. } => None,
                    });
                    info.hdr = hdr_from_mp4(v.mastering_display.as_ref(), v.content_light);
                }
                if let Some(a) = &e.audio {
                    info.sample_rate = if let iso::CodecConfig::Aac(c) = &e.codec { c.sample_rate.max(a.sample_rate as u32) } else { a.sample_rate as u32 };
                    info.channels = a.channels;
                }
                if let iso::CodecConfig::Opus(o) = &e.codec {
                    info.sample_rate = 48_000;
                    info.channels = u32::from(o.output_channels);
                }
            }
            info.rotation = t.display_rotation().map_or(0, |q| u16::from(q) * 90);
            info.remuxable = entry.is_some_and(|e| !matches!(e.codec, iso::CodecConfig::Unknown { .. } | iso::CodecConfig::Timecode(_)))
                && matches!(kind, TrackKind::Video | TrackKind::Audio);
            longest = longest.max(info.start + info.duration);
            let media_edit = if t.edits.is_empty() { None } else { Some(t.edits.clone()) };
            plans.push(entry.map(|e| TrackPlan { entry: e.clone(), edits: media_edit.unwrap_or_default(), language: t.language.clone() }));
            tracks.push(info);
            indexes.push(index);
        }
        let movie = Tick::from_rational(i64::try_from(file.duration).unwrap_or(i64::MAX), 1, i64::from(file.timescale.max(1)));
        self.info.duration = longest.max(movie.min(longest.max(Tick::ZERO)));
        if self.info.duration == Tick::ZERO {
            self.info.duration = movie;
        }
        self.tracks = tracks;
        self.indexes = indexes;
        self.plans = plans;
        Ok(())
    }

    fn build_mkv(&mut self, limits: &Limits) -> Result<()> {
        let Inner::Mkv(file) = &self.inner else { return Ok(()) };
        let file: &mkv::MkvFile = file;
        self.info.brand = file.doc_type.clone();
        self.info.fragmented = file.live;
        if let Some(title) = &file.info.title {
            self.info.metadata.push(("title".into(), title.clone()));
        }
        let mut total: u64 = 0;
        let mut tracks = Vec::new();
        let mut indexes = Vec::new();
        let mut plans = Vec::new();
        let mut longest = Tick::ZERO;
        for (n, t) in file.tracks.iter().enumerate() {
            total = total.saturating_add(t.samples.len() as u64);
            if total > limits.max_packets {
                return Err(Error::limit("packet count", limits.max_packets, total));
            }
            let kind = match t.kind {
                mkv::TrackKind::Video => TrackKind::Video,
                mkv::TrackKind::Audio => TrackKind::Audio,
                mkv::TrackKind::Subtitle => TrackKind::Subtitle,
                _ => TrackKind::Data,
            };
            let entries = mkv_entries(t, kind == TrackKind::Video);
            let index = FrameIndex::new(1_000_000_000, entries);
            let mut info = base_info(n, t.number, kind, 1_000_000_000, &t.language, &index);
            info.codec = t.codec.name();
            info.codec_tag = t.codec_id.clone();
            info.enabled = t.enabled;
            if let Some(v) = &t.video {
                info.width = v.pixel_width;
                info.height = v.pixel_height;
                info.pixel_aspect = v.pixel_aspect();
                info.rotation = v.display_rotation().map_or(0, |q| u16::from(q) * 90);
                if let Some(c) = &v.colour {
                    info.color = Some(ColorSignal {
                        primaries: c.primaries.map_or(2, |x| x.min(255) as u16),
                        transfer: c.transfer_characteristics.map_or(2, |x| x.min(255) as u16),
                        matrix: c.matrix_coefficients.map_or(2, |x| x.min(255) as u16),
                        full_range: c.full_range(),
                    });
                    if c.mastering.is_some() || c.max_cll.is_some() || c.max_fall.is_some() {
                        info.hdr = Some(HdrStatic {
                            primaries: c.mastering.as_ref().map(|m| m.primaries.map(|(x, y)| (x as f32, y as f32))),
                            white_point: c.mastering.as_ref().map(|m| (m.white_point.0 as f32, m.white_point.1 as f32)),
                            max_luminance: c.mastering.as_ref().map(|m| m.luminance_max as f32),
                            min_luminance: c.mastering.as_ref().map(|m| m.luminance_min as f32),
                            max_cll: c.max_cll.map(|x| x.min(u64::from(u32::MAX)) as u32),
                            max_fall: c.max_fall.map(|x| x.min(u64::from(u32::MAX)) as u32),
                        });
                    }
                }
            }
            if let Some(a) = &t.audio {
                info.sample_rate = a.output_sampling_frequency.unwrap_or(a.sampling_frequency).clamp(0.0, 4_000_000.0).round() as u32;
                info.channels = a.channels.min(u64::from(u16::MAX)) as u32;
            }
            let plan = convert::entry_from_matroska(t, &info, file, &self.src, &index);
            info.remuxable = plan.is_ok() && matches!(kind, TrackKind::Video | TrackKind::Audio);
            longest = longest.max(info.start + info.duration);
            plans.push(plan.ok().map(|entry| TrackPlan { entry, edits: Vec::new(), language: t.language.clone() }));
            tracks.push(info);
            indexes.push(index);
        }
        self.info.duration =
            longest.max(file.duration_ns().map_or(Tick::ZERO, |ns| Tick::from_rational(i64::try_from(ns).unwrap_or(i64::MAX), 1, 1_000_000_000)));
        self.tracks = tracks;
        self.indexes = indexes;
        self.plans = plans;
        Ok(())
    }
}

/// The part of `TrackInfo` every container fills the same way, from the packet table.
fn base_info(index_no: usize, id: u64, kind: TrackKind, timescale: u32, language: &str, ix: &FrameIndex) -> TrackInfo {
    let packets = ix.len() as u64;
    let keyframes = ix.entries().iter().filter(|e| e.key).count() as u64;
    let first = ix.first_pts().unwrap_or(0);
    let end = ix.end_pts().unwrap_or(first);
    let span = end.saturating_sub(first).max(0);
    let bytes: u64 = ix.entries().iter().map(|e| u64::from(e.size)).sum();
    let duration = ix.to_tick(span);
    let bitrate = if span > 0 { u64::try_from(i128::from(bytes) * 8 * i128::from(timescale) / i128::from(span)).ok() } else { None };
    let (frame_rate, constant) = if kind == TrackKind::Video && packets > 0 && span > 0 {
        let constant = ix.is_constant_duration();
        let rate = match ix.get(0) {
            Some(e) if constant && e.duration > 0 => FrameRate::new(i64::from(timescale), i64::try_from(e.duration).unwrap_or(i64::MAX)),
            _ => {
                let num = i128::from(packets) * i128::from(timescale);
                match (i64::try_from(num), span) {
                    (Ok(n), s) => FrameRate::new(n, s),
                    _ => FrameRate::default(),
                }
            }
        };
        (Some(rate), constant)
    } else {
        (None, true)
    };
    TrackInfo {
        index: index_no,
        id,
        kind,
        codec: String::new(),
        codec_tag: String::new(),
        timescale,
        language: if language.is_empty() { "und".into() } else { language.to_string() },
        width: 0,
        height: 0,
        pixel_aspect: (1, 1),
        rotation: 0,
        sample_rate: 0,
        channels: 0,
        packets,
        keyframes,
        start: ix.to_tick(first),
        duration,
        frame_rate,
        constant_frame_rate: constant,
        bitrate,
        color: None,
        hdr: None,
        enabled: true,
        remuxable: false,
    }
}

fn hdr_from_mp4(md: Option<&iso::MasteringDisplay>, cll: Option<(u16, u16)>) -> Option<HdrStatic> {
    if md.is_none() && cll.is_none() {
        return None;
    }
    let chroma = |v: u16| f32::from(v) * 0.00002;
    Some(HdrStatic {
        primaries: md.map(|m| m.primaries.map(|(x, y)| (chroma(x), chroma(y)))),
        white_point: md.map(|m| (chroma(m.white_point.0), chroma(m.white_point.1))),
        max_luminance: md.map(|m| m.max_luminance as f32 / 10_000.0),
        min_luminance: md.map(|m| m.min_luminance as f32 / 10_000.0),
        max_cll: cll.map(|c| u32::from(c.0)),
        max_fall: cll.map(|c| u32::from(c.1)),
    })
}

/// The packet table of a Matroska track. Matroska stores presentation times only, so decode times
/// are rebuilt for video: the sorted presentation times, shifted back far enough that no packet is
/// presented before it is decoded. Audio and other tracks decode when they are presented.
fn mkv_entries(t: &mkv::Track, video: bool) -> Vec<FrameEntry> {
    let ns = |ticks: i64| t.ticks_to_ns(ticks);
    let pts: Vec<i64> = t.samples.iter().map(|s| ns(s.pts)).collect();
    let mut sorted = pts.clone();
    sorted.sort_unstable();
    // duration of each packet: its own, else up to the next one in presentation order
    let dur_of = |i: usize| -> u64 {
        let own = t.samples.get(i).map_or(0, |s| ns(i64::try_from(s.duration).unwrap_or(i64::MAX))).max(0) as u64;
        if own > 0 {
            return own;
        }
        let me = pts.get(i).copied().unwrap_or(0);
        let k = sorted.partition_point(|&p| p <= me);
        match sorted.get(k) {
            Some(next) => (next - me).max(0) as u64,
            None => t.default_duration_ns.unwrap_or(0),
        }
    };
    let shift = if video { pts.iter().zip(sorted.iter()).map(|(p, s)| s - p).max().unwrap_or(0).max(0) } else { 0 };
    t.samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let p = pts.get(i).copied().unwrap_or(0);
            let dts = if video { sorted.get(i).copied().unwrap_or(p) - shift } else { p };
            FrameEntry {
                dts,
                pts: p,
                composition_offset: p - dts,
                duration: dur_of(i),
                size: u32::try_from(t.sample_size(i).unwrap_or(0)).unwrap_or(u32::MAX),
                offset: s.offset,
                key: s.keyframe || !video,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_identifies_containers_and_survives_garbage() {
        assert_eq!(ContainerKind::sniff(b"\0\0\0\x18ftypisom\0\0\0\0isom"), ContainerKind::Mp4);
        assert_eq!(ContainerKind::sniff(b"\0\0\0\x14ftypqt  \0\0\0\0qt  "), ContainerKind::QuickTime);
        assert_eq!(ContainerKind::sniff(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 1, 0x42, 0x82, 0x84, b'w', b'e', b'b', b'm']), ContainerKind::WebM);
        assert_eq!(ContainerKind::sniff(&[0x1A, 0x45, 0xDF, 0xA3, 0x84, 0x42, 0x82, 0x88]), ContainerKind::Matroska);
        assert_eq!(ContainerKind::sniff(b"RIFF\0\0\0\0AVI LIST"), ContainerKind::Avi);
        assert_eq!(ContainerKind::sniff(b"OggS\0\x02"), ContainerKind::Ogg);
        assert_eq!(ContainerKind::sniff(b""), ContainerKind::Unknown);
        assert_eq!(ContainerKind::sniff(&[0xFF; 3]), ContainerKind::Unknown);
        let mut ts = vec![0u8; 188 * 5];
        for i in 0..5 {
            ts[i * 188] = 0x47;
        }
        assert_eq!(ContainerKind::sniff(&ts), ContainerKind::MpegTs);
    }

    #[test]
    fn frame_index_orders_by_presentation_and_finds_frames() {
        // decode order I P B: presentation I B P
        let e = |dts, pts, key| FrameEntry { dts, pts, composition_offset: pts - dts, duration: 10, size: 1, offset: 0, key };
        let ix = FrameIndex::new(100, vec![e(0, 10, true), e(10, 30, false), e(20, 20, false)]);
        assert_eq!(ix.len(), 3);
        assert_eq!(ix.display_to_decode(1), Some(2));
        assert_eq!(ix.frame_at(Tick::from_rational(25, 1, 100)), Some(1));
        assert_eq!(ix.frame_at(Tick::from_rational(-5, 1, 100)), Some(0));
        assert_eq!(ix.keyframe_before(2), 0);
        assert!(ix.is_constant_duration());
        assert_eq!(ix.first_pts(), Some(10));
        assert_eq!(ix.end_pts(), Some(40));
    }

    #[test]
    fn rescale_rounds_and_never_overflows() {
        assert_eq!(rescale(1001, 1000, 90_000), 90_090);
        assert_eq!(rescale(i64::MAX, 1, u32::MAX), i64::MAX);
        assert_eq!(rescale(5, 0, 10), 0);
    }
}
