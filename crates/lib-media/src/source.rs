//! Źródła bajtów dla parserów: pamięć ([`SliceSource`]) i plik czytany fragmentami
//! ([`FileSource`] nad [`RangeRead`]: dysk — [`StdFiles`], port plików — [`PortFiles`]).

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use platform_contract::{FsPort, PlatformError};

use crate::MediaError;

/// Źródło bajtów z dostępem swobodnym.
pub trait ByteSource {
    /// Rozmiar (B).
    fn size(&self) -> u64;
    /// Do `len` bajtów od `offset` (mniej na końcu źródła).
    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, MediaError>;
}

/// Bajty w pamięci.
#[derive(Debug, Clone, Copy)]
pub struct SliceSource<'a>(&'a [u8]);

impl<'a> SliceSource<'a> {
    /// Źródło nad buforem.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

impl ByteSource for SliceSource<'_> {
    fn size(&self) -> u64 {
        self.0.len() as u64
    }

    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, MediaError> {
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(self.0.len());
        let end = start.saturating_add(len).min(self.0.len());
        Ok(self
            .0
            .get(start..end)
            .map(<[u8]>::to_vec)
            .unwrap_or_default())
    }
}

/// Odczyt fragmentów plików (nagłówki bez wczytywania całości).
pub trait RangeRead: Send + Sync {
    /// Rozmiar zwykłego pliku (B); katalog albo urządzenie — błąd.
    fn size(&self, path: &Path) -> Result<u64, PlatformError>;
    /// Do `len` bajtów od `offset` (mniej na końcu pliku).
    fn read_at(&self, path: &Path, offset: u64, len: usize) -> Result<Vec<u8>, PlatformError>;
    /// Cały plik (do `max` B; większy — błąd).
    fn read_all(&self, path: &Path, max: u64) -> Result<Vec<u8>, PlatformError> {
        let size = self.size(path)?;
        if size > max {
            return Err(PlatformError::Unsupported(format!(
                "plik ma {size} B — więcej niż limit {max} B"
            )));
        }
        let len =
            usize::try_from(size).map_err(|_| PlatformError::Unsupported("plik za duży".into()))?;
        self.read_at(path, 0, len)
    }
}

fn io_error(path: &Path, e: &std::io::Error) -> PlatformError {
    match e.kind() {
        std::io::ErrorKind::NotFound => PlatformError::NotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => {
            PlatformError::PermissionDenied(path.display().to_string())
        }
        _ => PlatformError::Io(format!("{}: {e}", path.display())),
    }
}

/// Pliki na dysku (`std::fs`): tylko zwykłe pliki, odczyt od pozycji.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdFiles;

impl RangeRead for StdFiles {
    fn size(&self, path: &Path) -> Result<u64, PlatformError> {
        let meta = std::fs::metadata(path).map_err(|e| io_error(path, &e))?;
        if !meta.is_file() {
            return Err(PlatformError::InvalidPath(path.to_path_buf()));
        }
        Ok(meta.len())
    }

    fn read_at(&self, path: &Path, offset: u64, len: usize) -> Result<Vec<u8>, PlatformError> {
        let mut file = std::fs::File::open(path).map_err(|e| io_error(path, &e))?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| io_error(path, &e))?;
        let mut buf = Vec::with_capacity(len.min(1 << 20));
        file.take(len as u64)
            .read_to_end(&mut buf)
            .map_err(|e| io_error(path, &e))?;
        Ok(buf)
    }
}

/// Pliki przez `FsPort` (atrapy, testy): czyta cały plik do limitu i wycina fragment.
#[derive(Clone)]
pub struct PortFiles {
    fs: Arc<dyn FsPort>,
}

impl std::fmt::Debug for PortFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortFiles").finish_non_exhaustive()
    }
}

impl PortFiles {
    /// Adapter nad portem plików.
    pub fn new(fs: Arc<dyn FsPort>) -> Self {
        Self { fs }
    }
}

impl RangeRead for PortFiles {
    fn size(&self, path: &Path) -> Result<u64, PlatformError> {
        Ok(self.fs.read(path)?.len() as u64)
    }

    fn read_at(&self, path: &Path, offset: u64, len: usize) -> Result<Vec<u8>, PlatformError> {
        let all = self.fs.read(path)?;
        let start = usize::try_from(offset).unwrap_or(usize::MAX).min(all.len());
        let end = start.saturating_add(len).min(all.len());
        Ok(all.get(start..end).map(<[u8]>::to_vec).unwrap_or_default())
    }
}

/// Plik jako źródło bajtów (rozmiar ustalony przy otwarciu).
pub struct FileSource<'a> {
    files: &'a dyn RangeRead,
    path: PathBuf,
    size: u64,
}

impl<'a> FileSource<'a> {
    /// Otwiera plik (sprawdza rozmiar).
    pub fn open(files: &'a dyn RangeRead, path: &Path) -> Result<Self, PlatformError> {
        let size = files.size(path)?;
        Ok(Self {
            files,
            path: path.to_path_buf(),
            size,
        })
    }
}

impl ByteSource for FileSource<'_> {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, MediaError> {
        self.files
            .read_at(&self.path, offset, len)
            .map_err(|e| MediaError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_source_clamps_ranges() {
        let data = [1u8, 2, 3, 4];
        let mut s = SliceSource::new(&data);
        assert_eq!(s.size(), 4);
        assert_eq!(s.read_at(2, 10).unwrap(), vec![3, 4]);
        assert!(s.read_at(10, 2).unwrap().is_empty());
        assert!(s.read_at(u64::MAX, usize::MAX).unwrap().is_empty());
    }

    #[test]
    fn std_files_read_ranges_and_reject_dirs() {
        let dir = std::env::temp_dir().join(format!("lib-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.bin");
        std::fs::write(&path, b"0123456789").unwrap();
        let files = StdFiles;
        assert_eq!(files.size(&path).unwrap(), 10);
        assert_eq!(files.read_at(&path, 3, 4).unwrap(), b"3456");
        assert_eq!(files.read_at(&path, 8, 100).unwrap(), b"89");
        assert_eq!(files.read_all(&path, 100).unwrap().len(), 10);
        assert!(files.read_all(&path, 5).is_err());
        assert!(matches!(
            files.size(&dir),
            Err(PlatformError::InvalidPath(_))
        ));
        assert!(matches!(
            files.size(&dir.join("brak")),
            Err(PlatformError::NotFound(_))
        ));
        let mut src = FileSource::open(&files, &path).unwrap();
        assert_eq!(src.size(), 10);
        assert_eq!(src.read_at(0, 2).unwrap(), b"01");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
