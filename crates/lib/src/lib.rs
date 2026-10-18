//! FilmCraft as a library.
//!
//! `filmcraft-lib` is the one crate to depend on when you want FilmCraft's pure-Rust media stack
//! without the editor: probing, exact frame indexes, seeking decode of video and audio, remuxing
//! between MP4/MOV and Matroska/WebM, CMAF/HLS segmenting and encoding. No FFmpeg, no `unsafe`, no
//! UI, GPU or speech crates in the dependency tree.
//!
//! # Where to start
//!
//! | I want to... | Use |
//! |---|---|
//! | know what is in a file (codecs, size, rate, colour, HDR, exact frame count) | [`probe_path`], [`probe_bytes`] |
//! | list every packet and timestamp of a track | [`Demuxer::frame_index`] |
//! | decode the frame at a time, or a batch | [`MediaFile::frame_at`], [`MediaFile::frames`] |
//! | decode a window of audio | [`MediaFile::audio_window`] |
//! | copy MP4/MOV to or from Matroska/WebM | [`remux`], [`remux_to_vec`], [`remux_file`] |
//! | cut fragmented MP4 for HLS or DASH | [`segment`], [`hls_playlist`] |
//! | encode H.264, ProRes, DNxHR, APV, MJPEG, AAC, FLAC, GIF, WAV | [`Encoder`], [`encode_gif`], [`encode_wav`] |
//!
//! Every long call takes a [`Control`] (cancellation, deadline, progress) and every input is
//! checked against [`Limits`]. Errors are [`Error`] values with a stable [`ErrorKind`]; no function
//! panics on hostile input.
//!
//! # Features
//!
//! | feature | default | adds |
//! |---|---|---|
//! | `container` | yes | [`Demuxer`], [`remux`], [`segment`]: MP4/MOV and Matroska/WebM packet access; depends only on the two container crates |
//! | `decode` | yes | [`MediaFile`], [`probe_path`]: every codec and container FilmCraft decodes |
//! | `encode` | yes | [`Encoder`] and friends |
//!
//! # Thread safety
//!
//! [`Demuxer`], [`MediaFile`], [`Control`], [`CancelToken`], [`Limits`] and all report types are
//! `Send + Sync`; frames returned by [`MediaFile`] are `Arc`-shared and immutable. [`Encoder`] is
//! `Send` (one owner at a time). The compile-time assertions live in the crate's tests.
//!
//! See `LIBRARY.md` in the repository for the stability policy and the roadmap.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]
#![deny(missing_docs)]

mod control;
mod error;
mod limits;

pub use control::{CancelToken, Control, Progress, Stage};
pub use error::{Error, ErrorKind, Result};
pub use limits::Limits;

#[cfg(feature = "container")]
mod codec_string;
#[cfg(feature = "container")]
mod container;
#[cfg(feature = "container")]
mod convert;
#[cfg(feature = "container")]
#[doc(hidden)]
pub mod doc_fixtures;
#[cfg(feature = "container")]
mod remux;
#[cfg(feature = "container")]
mod segment;
#[cfg(feature = "container")]
mod source;

#[cfg(feature = "container")]
pub use container::{ColorSignal, ContainerInfo, ContainerKind, Demuxer, FrameEntry, FrameIndex, HdrStatic, Packet, TrackInfo, TrackKind};
#[cfg(all(feature = "container", any(unix, windows)))]
pub use remux::remux_file;
#[cfg(feature = "container")]
pub use remux::{OutputContainer, RemuxOptions, RemuxReport, remux, remux_to_vec};
#[cfg(feature = "container")]
pub use segment::{Segment, SegmentEvent, SegmentOptions, SegmentedStream, hls_playlist, hls_variant, peak_bandwidth, segment, segment_with};
#[cfg(all(feature = "container", any(unix, windows)))]
pub use source::FileSource;
#[cfg(feature = "container")]
pub use source::{ByteSource, MemorySource};

#[cfg(feature = "decode")]
mod decode;
#[cfg(feature = "encode")]
mod encode;
#[cfg(feature = "decode")]
mod probe;

#[cfg(feature = "decode")]
pub use decode::MediaFile;
#[cfg(feature = "encode")]
pub use encode::{
    AudioCodec, AudioOptions, EncodeReport, Encoder, EncoderOptions, FirstPass, RateControl, VideoCodec, VideoOptions, analyze_h264, encode_gif, encode_png,
    encode_wav,
};
/// Set the memory decoded frames may occupy process-wide; call before opening the first file.
/// Returns `false` when a decoder pool already exists (the per-file cap still applies).
#[cfg(feature = "decode")]
pub use filmcraft_codecs::set_frame_budget;
#[cfg(feature = "decode")]
pub use filmcraft_frame::{AudioBuffer, VideoFrame};
#[cfg(feature = "decode")]
pub use filmcraft_media::{AudioStreamInfo, MediaInfo, MediaKind, VideoStreamInfo};
#[cfg(all(feature = "decode", any(unix, windows)))]
pub use probe::probe_path;
#[cfg(feature = "decode")]
pub use probe::{FrameCount, Probe, probe, probe_bytes};

/// Re-exported time types: every timestamp in this crate is a [`filmcraft_time::Tick`].
pub use filmcraft_time::{FrameRate, Tick, TimeRange};
