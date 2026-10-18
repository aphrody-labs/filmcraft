//! One-call file inspection.

use std::sync::Arc;

use filmcraft_media::MediaInfo;
use filmcraft_time::Tick;

use crate::container::{ContainerKind, Demuxer};
use crate::control::Control;
use crate::decode::MediaFile;
use crate::error::Result;
use crate::limits::Limits;
use crate::source::{ByteSource, MemorySource};

/// How trustworthy a frame count is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameCount {
    /// Counted from the container's packet table: one entry per frame.
    Exact(u64),
    /// Duration times frame rate; the container has no table this crate reads yet.
    Estimated(u64),
}

impl FrameCount {
    /// The number, exact or not.
    pub fn value(self) -> u64 {
        match self {
            FrameCount::Exact(n) | FrameCount::Estimated(n) => n,
        }
    }

    /// Whether the count comes from the packet table.
    pub fn is_exact(self) -> bool {
        matches!(self, FrameCount::Exact(_))
    }
}

/// Everything known about a file without decoding a picture.
#[derive(Debug)]
#[non_exhaustive]
pub struct Probe {
    /// Decoder-level facts: size, frame rate, codec names, colour, HDR metadata, audio streams.
    pub media: MediaInfo,
    /// The sniffed container.
    pub kind: ContainerKind,
    /// Container-level facts: every track, exact frame index and packet timestamps. Present for
    /// MP4, MOV, Matroska and WebM.
    pub container: Option<Demuxer>,
}

impl Probe {
    /// The frame count of the (first) video stream.
    pub fn frame_count(&self) -> Option<FrameCount> {
        if let Some(d) = &self.container
            && let Some(i) = d.first_track(crate::container::TrackKind::Video)
            && let Some(ix) = d.frame_index(i)
        {
            return Some(FrameCount::Exact(ix.len() as u64));
        }
        let v = self.media.video.as_ref()?;
        let rate = v.frame_rate.sane();
        let n = rate.frame_at(self.media.duration - Tick(1)).saturating_add(1);
        u64::try_from(n).ok().map(FrameCount::Estimated)
    }

    /// Presentation times of every video frame in display order, from the packet table. `None`
    /// when the container has no table (see [`Probe::container`]).
    pub fn video_timestamps(&self) -> Option<Vec<Tick>> {
        let d = self.container.as_ref()?;
        let ix = d.frame_index(d.first_track(crate::container::TrackKind::Video)?)?;
        Some((0..ix.len()).filter_map(|n| ix.time_of_frame(n)).collect())
    }
}

/// Inspect a source: decoder facts plus, for MP4/MOV/Matroska/WebM, the exact packet table.
///
/// # Errors
///
/// As [`MediaFile::open`].
pub fn probe(name: &str, src: Arc<dyn ByteSource>, limits: &Limits, control: &Control) -> Result<Probe> {
    let file = MediaFile::open(name, src.clone(), limits, control)?;
    let kind = file.container_kind();
    let container = if kind.is_demuxable() { Demuxer::open(src, limits).ok() } else { None };
    Ok(Probe { media: file.info().clone(), kind, container })
}

/// [`probe`] a file in memory.
///
/// ```
/// # fn main() -> Result<(), filmcraft_lib::Error> {
/// use filmcraft_lib::{Control, Limits, probe_bytes};
/// # let bytes = filmcraft_lib::doc_fixtures::decodable_mp4();
/// let p = probe_bytes("clip.mp4", bytes, &Limits::default(), &Control::default())?;
/// let v = p.media.video.as_ref().expect("video");
/// assert_eq!((v.width, v.height), (64, 48));
/// assert!(v.codec.starts_with("H.264"));
/// assert!(p.frame_count().expect("count").is_exact());
/// # Ok(()) }
/// ```
pub fn probe_bytes(name: &str, bytes: impl Into<Arc<[u8]>>, limits: &Limits, control: &Control) -> Result<Probe> {
    probe(name, Arc::new(MemorySource::new(bytes)), limits, control)
}

/// [`probe`] a file on disk.
#[cfg(any(unix, windows))]
pub fn probe_path(path: &std::path::Path, limits: &Limits, control: &Control) -> Result<Probe> {
    let src = crate::source::FileSource::open(path)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    probe(&name, Arc::new(src), limits, control)
}
