//! Random-access byte sources for the container layer.

use std::io;
use std::sync::Arc;

/// Random-access, read-only bytes: a file, a buffer, a ranged HTTP resource.
///
/// Implementations must be cheap to call concurrently (`Send + Sync`) and positional: a read never
/// depends on an earlier one.
pub trait ByteSource: Send + Sync {
    /// Total length in bytes.
    fn len(&self) -> u64;

    /// Fill `buf` entirely from `offset`; `UnexpectedEof` when the range ends past the source.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;

    /// Whether the source has no bytes.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: ByteSource + ?Sized> ByteSource for Arc<T> {
    fn len(&self) -> u64 {
        (**self).len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_at(offset, buf)
    }
}

/// A source over bytes held in memory.
#[derive(Clone, Debug)]
pub struct MemorySource(Arc<[u8]>);

impl MemorySource {
    /// Wrap `bytes` (no copy for an `Arc<[u8]>`, one copy for a `Vec<u8>` or slice).
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self(bytes.into())
    }

    /// The wrapped bytes.
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.0
    }
}

impl ByteSource for MemorySource {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let eof = || io::Error::new(io::ErrorKind::UnexpectedEof, "read past end of source");
        let start = usize::try_from(offset).map_err(|_| eof())?;
        let end = start.checked_add(buf.len()).ok_or_else(eof)?;
        buf.copy_from_slice(self.0.get(start..end).ok_or_else(eof)?);
        Ok(())
    }
}

/// A source over a file on disk, read with positional reads (no shared cursor, safe to share
/// between threads). Not available on `wasm32`.
#[cfg(any(unix, windows))]
#[derive(Debug)]
pub struct FileSource {
    file: std::fs::File,
    len: u64,
}

#[cfg(any(unix, windows))]
impl FileSource {
    /// Open `path` for reading. A directory or an unreadable file is an error.
    pub fn open(path: &std::path::Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
        }
        Ok(Self { file, len: meta.len() })
    }
}

#[cfg(any(unix, windows))]
impl ByteSource for FileSource {
    fn len(&self) -> u64 {
        self.len
    }

    #[cfg(unix)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        use std::os::unix::fs::FileExt;
        self.file.read_exact_at(buf, offset)
    }

    #[cfg(windows)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        use std::os::windows::fs::FileExt;
        let mut done = 0usize;
        while done < buf.len() {
            let at = offset.checked_add(done as u64).ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "offset overflow"))?;
            let rest = buf.get_mut(done..).ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short read"))?;
            match self.file.seek_read(rest, at) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "read past end of file")),
                Ok(n) => done += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// The container crates' own `ByteSource` traits, over ours.
#[cfg(feature = "container")]
#[derive(Clone)]
pub(crate) struct Adapter(pub Arc<dyn ByteSource>);

#[cfg(feature = "container")]
impl filmcraft_isobmff::ByteSource for Adapter {
    fn len(&self) -> u64 {
        self.0.len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_at(offset, buf)
    }
}

#[cfg(feature = "container")]
impl filmcraft_matroska::ByteSource for Adapter {
    fn len(&self) -> u64 {
        self.0.len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_at(offset, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_source_reads_and_reports_eof() {
        let s = MemorySource::new(&b"0123456789"[..]);
        let mut b = [0u8; 3];
        s.read_at(4, &mut b).unwrap();
        assert_eq!(&b, b"456");
        assert_eq!(s.read_at(8, &mut b).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(s.read_at(u64::MAX, &mut b).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert!(!s.is_empty());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn file_source_reads_positionally_and_rejects_directories() {
        let dir = std::env::temp_dir().join(format!("filmcraft-lib-src-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("b.bin");
        std::fs::write(&p, b"abcdefghij").unwrap();
        let s = FileSource::open(&p).unwrap();
        let mut b = [0u8; 4];
        s.read_at(6, &mut b).unwrap();
        assert_eq!(&b, b"ghij");
        s.read_at(0, &mut b).unwrap();
        assert_eq!(&b, b"abcd");
        assert!(s.read_at(8, &mut b).is_err());
        assert!(FileSource::open(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
