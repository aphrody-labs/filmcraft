//! One error type for the whole facade.

use std::fmt;

/// Result alias used by every fallible function of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Broad class of an [`Error`], stable across releases so callers can branch without matching
/// message text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The file uses a codec, container feature or option FilmCraft does not implement.
    Unsupported,
    /// The input is damaged, truncated or hostile (inconsistent sizes, impossible values).
    Invalid,
    /// An explicit [`Limits`](crate::Limits) value was exceeded.
    LimitExceeded,
    /// The [`CancelToken`](crate::CancelToken) was triggered.
    Cancelled,
    /// The [`Control`](crate::Control) deadline passed.
    DeadlineExceeded,
    /// Reading or writing bytes failed.
    Io,
    /// The requested stream, track or frame does not exist.
    NotFound,
    /// A decoder rejected the stream.
    Decode,
    /// An encoder or muxer failed.
    Encode,
}

/// What went wrong. Match on [`Error::kind`] rather than on the message.
#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    /// An error of `kind` with a human-readable message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }

    /// The stable class of the error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The human-readable message (not stable).
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether the operation was stopped by the caller (cancel token or deadline) rather than by a fault.
    pub fn is_interrupted(&self) -> bool {
        matches!(self.kind, ErrorKind::Cancelled | ErrorKind::DeadlineExceeded)
    }

    #[cfg_attr(not(any(feature = "container", feature = "decode", feature = "encode")), allow(dead_code))]
    pub(crate) fn unsupported(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unsupported, m)
    }
    #[cfg_attr(not(any(feature = "container", feature = "decode", feature = "encode")), allow(dead_code))]
    pub(crate) fn invalid(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid, m)
    }
    #[cfg_attr(not(any(feature = "container", feature = "decode", feature = "encode")), allow(dead_code))]
    pub(crate) fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, m)
    }
    #[cfg(any(feature = "container", feature = "decode", feature = "encode"))]
    pub(crate) fn limit(what: &str, limit: u64, requested: u64) -> Self {
        Self::new(ErrorKind::LimitExceeded, format!("{what}: {requested} exceeds the limit of {limit}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, e.to_string())
    }
}

#[cfg(feature = "container")]
impl From<filmcraft_isobmff::Error> for Error {
    fn from(e: filmcraft_isobmff::Error) -> Self {
        use filmcraft_isobmff::Error as E;
        let kind = match &e {
            E::Io(_) => ErrorKind::Io,
            E::Unsupported(_) => ErrorKind::Unsupported,
            E::Mux(_) => ErrorKind::Encode,
            E::NoSuchTrack(_) | E::NoSuchSample(_) => ErrorKind::NotFound,
            _ => ErrorKind::Invalid,
        };
        Self::new(kind, format!("mp4: {e}"))
    }
}

#[cfg(feature = "container")]
impl From<filmcraft_matroska::Error> for Error {
    fn from(e: filmcraft_matroska::Error) -> Self {
        use filmcraft_matroska::Error as E;
        let kind = match &e {
            E::Io(_) => ErrorKind::Io,
            E::Unsupported(_) => ErrorKind::Unsupported,
            E::NoSuchTrack(_) | E::NoSuchSample(_) => ErrorKind::NotFound,
            _ => ErrorKind::Invalid,
        };
        Self::new(kind, format!("matroska: {e}"))
    }
}

#[cfg(feature = "decode")]
impl From<filmcraft_media::MediaError> for Error {
    fn from(e: filmcraft_media::MediaError) -> Self {
        use filmcraft_media::MediaError as E;
        let kind = match &e {
            E::Unsupported(_) => ErrorKind::Unsupported,
            E::Io(_) | E::Offline(_) => ErrorKind::Io,
            E::Decode(_) => ErrorKind::Decode,
            E::NoStream(_) => ErrorKind::NotFound,
            E::Cancelled => ErrorKind::Cancelled,
        };
        Self::new(kind, e.to_string())
    }
}

#[cfg(feature = "decode")]
impl From<filmcraft_codecs::CodecError> for Error {
    fn from(e: filmcraft_codecs::CodecError) -> Self {
        filmcraft_media::MediaError::from(e).into()
    }
}

#[cfg(feature = "encode")]
impl From<filmcraft_export::ExportError> for Error {
    fn from(e: filmcraft_export::ExportError) -> Self {
        use filmcraft_export::ExportError as E;
        let kind = match &e {
            E::Unsupported(_) | E::NoSequence => ErrorKind::Unsupported,
            E::Io(_) => ErrorKind::Io,
            E::Encode(_) => ErrorKind::Encode,
            E::Cancelled => ErrorKind::Cancelled,
        };
        Self::new(kind, e.to_string())
    }
}
