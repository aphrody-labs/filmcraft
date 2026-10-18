//! Seeking decode of video frames and audio windows from any container FilmCraft reads.
//!
//! [`MediaFile`] wraps FilmCraft's generic `MediaSource`: MP4/MOV, Matroska/WebM, AVI, MXF,
//! MPEG-TS/PS, Ogg, Y4M, APV, WAV and standalone audio files (MP3, FLAC, AIFF...), stills, animated
//! GIF/APNG. Video codecs: H.264, HEVC, VP8, VP9, AV1, APV, ProRes, DNxHD/DNxHR, Motion-JPEG,
//! MPEG-1/2. Audio: AAC, Opus, Vorbis, MP3/MP2/MP1, FLAC, ALAC, AC-3, PCM.

use std::sync::Arc;

use filmcraft_frame::{AudioBuffer, VideoFrame};
use filmcraft_media::{ByteReader, FrameRequest, MediaInfo, SharedReader, SharedSource};
use filmcraft_time::{FrameRate, Tick};

use crate::container::ContainerKind;
use crate::control::{Control, Stage};
use crate::error::{Error, ErrorKind, Result};
use crate::limits::Limits;
use crate::source::ByteSource;

/// A [`ByteSource`] seen as the media crate's reader.
struct Reader(Arc<dyn ByteSource>);

impl ByteReader for Reader {
    fn len(&self) -> u64 {
        self.0.len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        self.0.read_at(offset, buf)
    }
}

/// Give `name` an extension when it has none FilmCraft recognises but the first bytes identify an
/// audio format: standalone MP3 and FLAC files are recognised by extension.
fn hinted_name(name: &str, head: &[u8]) -> String {
    if filmcraft_media::is_importable(std::path::Path::new(name)) {
        return name.to_string();
    }
    let ext = match ContainerKind::sniff(head) {
        ContainerKind::Flac => Some("flac"),
        ContainerKind::Wav => Some("wav"),
        ContainerKind::Ogg => Some("ogg"),
        _ if head.starts_with(b"ID3") || matches!(head, [0xFF, b, ..] if b & 0xE0 == 0xE0 && b & 0x06 != 0) => Some("mp3"),
        _ => None,
    };
    match ext {
        Some(e) => format!("{name}.{e}"),
        None => name.to_string(),
    }
}

/// An open media file: probe facts plus seeking decode.
///
/// `MediaFile` is `Send + Sync`; frame requests from several threads are safe and share the
/// decoder caches. Frames are `Arc<VideoFrame>`: immutable and cheap to clone.
pub struct MediaFile {
    source: SharedSource,
    limits: Limits,
    kind: ContainerKind,
}

impl std::fmt::Debug for MediaFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaFile").field("kind", &self.kind).field("info", self.source.info()).finish()
    }
}

impl MediaFile {
    /// Open from a [`ByteSource`]. `name` is only used for messages and as a format hint (its
    /// extension) when the bytes alone do not identify the format.
    ///
    /// Containers with an index (MP4, Matroska, AVI, MXF, MPEG-TS, Ogg, WAV) are read in place;
    /// formats decoded in one go (stills, MP3/FLAC files) are read whole, up to
    /// `limits.max_whole_file_bytes`.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::LimitExceeded`] for oversized input; [`crate::ErrorKind::Unsupported`] for formats or
    /// codecs FilmCraft does not read; [`crate::ErrorKind::Invalid`] / [`crate::ErrorKind::Decode`] for damage.
    pub fn open(name: &str, src: Arc<dyn ByteSource>, limits: &Limits, control: &Control) -> Result<Self> {
        if src.len() > limits.max_input_bytes {
            return Err(Error::limit("input size", limits.max_input_bytes, src.len()));
        }
        if let Some(b) = limits.frame_cache_bytes {
            filmcraft_codecs::set_frame_budget(b);
        }
        let mut head = vec![0u8; src.len().min(64 * 1024) as usize];
        src.read_at(0, &mut head)?;
        let kind = ContainerKind::sniff(&head);
        let reader: SharedReader = Arc::new(Reader(src));
        let name = hinted_name(name, &head);
        let source = control.guard(|| {
            filmcraft_media::reader::open_reader_within(
                &name,
                reader,
                &filmcraft_codecs::reader_openers(),
                &filmcraft_codecs::openers(),
                limits.max_whole_file_bytes,
            )
            .map_err(Error::from)
        })?;
        Ok(Self { source, limits: *limits, kind })
    }

    /// Open a file held in memory.
    pub fn open_bytes(name: &str, bytes: impl Into<Arc<[u8]>>, limits: &Limits, control: &Control) -> Result<Self> {
        Self::open(name, Arc::new(crate::source::MemorySource::new(bytes)), limits, control)
    }

    /// Open a file on disk. Open files are pooled, so thousands of `MediaFile`s do not exhaust
    /// descriptors.
    #[cfg(any(unix, windows))]
    pub fn open_path(path: &std::path::Path, limits: &Limits, control: &Control) -> Result<Self> {
        let len = std::fs::metadata(path)?.len();
        if len > limits.max_input_bytes {
            return Err(Error::limit("input size", limits.max_input_bytes, len));
        }
        if let Some(b) = limits.frame_cache_bytes {
            filmcraft_codecs::set_frame_budget(b);
        }
        let reader: SharedReader = Arc::new(filmcraft_media::reader::FileReader::open(path)?);
        let mut head = vec![0u8; len.min(64 * 1024) as usize];
        reader.read_at(0, &mut head)?;
        let kind = ContainerKind::sniff(&head);
        let name = hinted_name(&path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), &head);
        let source = control.guard(|| {
            filmcraft_media::reader::open_reader_within(
                &name,
                reader,
                &filmcraft_codecs::reader_openers(),
                &filmcraft_codecs::openers(),
                limits.max_whole_file_bytes,
            )
            .map_err(Error::from)
        })?;
        Ok(Self { source, limits: *limits, kind })
    }

    /// Stream facts: size, frame rate, codec names, colour, HDR, audio streams, duration.
    pub fn info(&self) -> &MediaInfo {
        self.source.info()
    }

    /// The container sniffed from the first bytes.
    pub fn container_kind(&self) -> ContainerKind {
        self.kind
    }

    /// The video frame rate, when the file has video.
    pub fn frame_rate(&self) -> Option<FrameRate> {
        self.info().video.as_ref().map(|v| v.frame_rate)
    }

    /// Estimated frame count: duration times frame rate, rounded. Exact only for constant-rate
    /// files; use [`probe_bytes`](crate::probe_bytes) for the exact count of MP4 and Matroska.
    pub fn estimated_frames(&self) -> Option<u64> {
        let rate = self.frame_rate()?.sane();
        let f = rate.frame_at(self.info().duration - Tick(1));
        u64::try_from(f.saturating_add(1)).ok()
    }

    fn check_picture(&self) -> Result<()> {
        let v = self.info().video.as_ref().ok_or_else(|| Error::not_found("video stream"))?;
        let px = u64::from(v.width).saturating_mul(u64::from(v.height));
        if px > self.limits.max_frame_pixels {
            return Err(Error::limit("picture size in pixels", self.limits.max_frame_pixels, px));
        }
        Ok(())
    }

    /// The frame shown at media time `t` (0 is the first frame). `scale` below 1.0 may return a
    /// smaller picture; sources are free to return more than asked.
    ///
    /// Seeking decodes from the preceding keyframe and caches the group of pictures, so stepping
    /// through nearby times is cheap.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::NotFound`] without a video stream; [`crate::ErrorKind::LimitExceeded`] when the picture
    /// is larger than `limits.max_frame_pixels`; [`crate::ErrorKind::Decode`] on a damaged stream;
    /// [`crate::ErrorKind::Cancelled`] / [`crate::ErrorKind::DeadlineExceeded`] from `control`.
    pub fn frame_at_scaled(&self, t: Tick, scale: f32, control: &Control) -> Result<Arc<VideoFrame>> {
        self.check_picture()?;
        let scale = if scale.is_finite() && scale > 0.0 { scale.min(1.0) } else { 1.0 };
        let req = FrameRequest { time: t.max(Tick::ZERO), scale };
        control.guard(|| self.source.video_frame(req).map_err(Error::from))
    }

    /// [`frame_at_scaled`](Self::frame_at_scaled) at full size.
    ///
    /// ```
    /// # fn main() -> Result<(), filmcraft_lib::Error> {
    /// use filmcraft_lib::{Control, MediaFile, Limits, Tick};
    /// # let bytes = filmcraft_lib::doc_fixtures::decodable_mp4();
    /// let file = MediaFile::open_bytes("clip.mp4", bytes, &Limits::default(), &Control::default())?;
    /// let frame = file.frame_at(Tick::from_seconds_f64(0.1), &Control::default())?;
    /// assert_eq!((frame.width, frame.height), (64, 48));
    /// let rgba = frame.to_rgba8();
    /// assert_eq!(rgba.len(), 64 * 48 * 4);
    /// # Ok(()) }
    /// ```
    pub fn frame_at(&self, t: Tick, control: &Control) -> Result<Arc<VideoFrame>> {
        self.frame_at_scaled(t, 1.0, control)
    }

    /// Frame number `n` (0-based) on the file's frame grid.
    pub fn frame_number(&self, n: u64, control: &Control) -> Result<Arc<VideoFrame>> {
        let rate = self.frame_rate().ok_or_else(|| Error::not_found("video stream"))?.sane();
        let n = i64::try_from(n).map_err(|_| Error::invalid("frame number out of range"))?;
        self.frame_at(rate.tick_of(n), control)
    }

    /// `count` consecutive frames starting at frame number `first`, stopping early at the end of the
    /// media. `count` is capped by `limits.max_batch_frames`.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::LimitExceeded`] when `count` exceeds the cap; any error of
    /// [`frame_at`](Self::frame_at) for the first frame that fails.
    pub fn frames(&self, first: u64, count: u64, control: &Control) -> Result<Vec<Arc<VideoFrame>>> {
        if count > self.limits.max_batch_frames {
            return Err(Error::limit("batch frames", self.limits.max_batch_frames, count));
        }
        let total = self.estimated_frames().unwrap_or(u64::MAX);
        let mut out = Vec::new();
        for k in 0..count {
            let n = first.saturating_add(k);
            if n >= total {
                break;
            }
            out.push(self.frame_number(n, control)?);
            control.report(Stage::Decoding, k + 1, Some(count));
        }
        Ok(out)
    }

    /// `frames` audio frames (samples per channel) of audio stream `stream`, starting at sample
    /// `start` of the media timeline, resampled to `sample_rate`. Reads past the end are silence.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::NotFound`] for an unknown stream; [`crate::ErrorKind::LimitExceeded`] when
    /// `frames * channels` exceeds `limits.max_audio_samples_per_request`.
    ///
    /// ```
    /// # fn main() -> Result<(), filmcraft_lib::Error> {
    /// use filmcraft_lib::{Control, MediaFile, Limits};
    /// # let bytes = filmcraft_lib::doc_fixtures::decodable_mp4();
    /// let file = MediaFile::open_bytes("clip.mp4", bytes, &Limits::default(), &Control::default())?;
    /// let pcm = file.audio_window(0, 0, 4800, 48_000, &Control::default())?;
    /// assert_eq!(pcm.frames(), 4800);
    /// assert_eq!(pcm.channel_count(), 2);
    /// # Ok(()) }
    /// ```
    pub fn audio_window(&self, stream: usize, start: i64, frames: usize, sample_rate: u32, control: &Control) -> Result<AudioBuffer> {
        let info = self.info();
        let ch = info.audio_streams.get(stream).map(|a| u64::from(a.channels)).ok_or_else(|| Error::not_found(format!("audio stream {stream}")))?;
        if sample_rate == 0 {
            return Err(Error::new(ErrorKind::Invalid, "sample rate must be positive"));
        }
        let samples = (frames as u64).saturating_mul(ch.max(1));
        if samples > self.limits.max_audio_samples_per_request {
            return Err(Error::limit("audio samples per request", self.limits.max_audio_samples_per_request, samples));
        }
        control.guard(|| self.source.audio_stream(stream, start, frames, sample_rate).map_err(Error::from))
    }

    /// Audio of `stream` for `[start, start + duration)` at the stream's native sample rate.
    pub fn audio_range(&self, stream: usize, start: Tick, duration: Tick, control: &Control) -> Result<AudioBuffer> {
        let rate = self.info().audio_streams.get(stream).map(|a| a.sample_rate).ok_or_else(|| Error::not_found(format!("audio stream {stream}")))?;
        let first = start.to_units_floor(i64::from(rate));
        let n = duration.max(Tick::ZERO).to_units_floor(i64::from(rate));
        let n = usize::try_from(n).map_err(|_| Error::invalid("audio range too long"))?;
        self.audio_window(stream, first, n, rate, control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_file_is_send_sync() {
        fn is<T: Send + Sync>() {}
        is::<MediaFile>();
    }
}
