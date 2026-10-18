//! Explicit size and memory caps.

/// Caps applied to untrusted input. Every field is enforced before the allocation or decode it
/// guards, and an excess is an error of kind
/// [`ErrorKind::LimitExceeded`](crate::ErrorKind::LimitExceeded), never a truncation.
///
/// The defaults suit a server decoding user uploads; use [`Limits::unlimited`] for trusted local
/// files and tighten fields for hostile ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// Largest input in bytes (file size or buffer length).
    pub max_input_bytes: u64,
    /// Largest input in bytes for formats that cannot be read in place (stills, FLAC/MP3 files,
    /// anything decoded in one go): the file is read whole, so this is a memory cap.
    pub max_whole_file_bytes: u64,
    /// Largest picture, width times height, a decode may produce.
    pub max_frame_pixels: u64,
    /// Most audio samples (frames times channels) one audio request may return.
    pub max_audio_samples_per_request: u64,
    /// Most frames one batch decode may return.
    pub max_batch_frames: u64,
    /// Largest single packet (compressed sample) accepted when remuxing or segmenting.
    pub max_packet_bytes: u64,
    /// Most packets a remux or segmentation may handle.
    pub max_packets: u64,
    /// Memory the process-wide decoded-frame caches may hold, in bytes (see
    /// [`set_frame_budget`](crate::set_frame_budget)); `None` keeps the built-in 1 GiB.
    pub frame_cache_bytes: Option<usize>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1 << 40,
            max_whole_file_bytes: 256 << 20,
            max_frame_pixels: 8192 * 8192,
            max_audio_samples_per_request: 48_000 * 2 * 600,
            max_batch_frames: 4096,
            max_packet_bytes: 256 << 20,
            max_packets: 50_000_000,
            frame_cache_bytes: None,
        }
    }
}

impl Limits {
    /// No caps at all: for trusted files.
    pub fn unlimited() -> Self {
        Self {
            max_input_bytes: u64::MAX,
            max_whole_file_bytes: u64::MAX,
            max_frame_pixels: u64::MAX,
            max_audio_samples_per_request: u64::MAX,
            max_batch_frames: u64::MAX,
            max_packet_bytes: u64::MAX,
            max_packets: u64::MAX,
            frame_cache_bytes: None,
        }
    }

    /// Tight caps for hostile input: 64 MiB whole-file reads, 4096x4096 pictures, 32 MiB packets.
    pub fn strict() -> Self {
        Self {
            max_input_bytes: 8 << 30,
            max_whole_file_bytes: 64 << 20,
            max_frame_pixels: 4096 * 4096,
            max_audio_samples_per_request: 48_000 * 2 * 60,
            max_batch_frames: 256,
            max_packet_bytes: 32 << 20,
            max_packets: 5_000_000,
            frame_cache_bytes: Some(256 << 20),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_is_tighter_than_default() {
        let (d, s) = (Limits::default(), Limits::strict());
        assert!(s.max_input_bytes < d.max_input_bytes);
        assert!(s.max_frame_pixels < d.max_frame_pixels);
        assert!(s.max_whole_file_bytes < d.max_whole_file_bytes);
        assert!(Limits::unlimited().max_packets > d.max_packets);
    }
}
