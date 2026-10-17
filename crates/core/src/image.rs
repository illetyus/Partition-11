//! P2-I1: bounded, regular-file-only image reader.
//!
//! Internal core primitive ONLY. Not exposed through the Windows agent or
//! Electron IPC. It never enumerates disks, elevates privileges or opens a
//! device intentionally. Passing a path is not device authorization.
//!
//! This is not a coherent disk snapshot: concurrent mutation can change bytes
//! without changing file length. The later agent must own an immutable image
//! lease and independently authorize paths before the UI uses this adapter.
#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const MAX_SINGLE_READ: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum ImageReadError {
    UnsafeTarget,
    EmptyImage,
    InvalidReadLength,
    InvalidSectorSize,
    InvalidGeometry,
    OutOfBounds,
    ImageSizeChanged,
    Io(std::io::Error),
}

impl From<std::io::Error> for ImageReadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Stores a private read-only file handle; there is deliberately no write API.
#[derive(Debug)]
pub struct ReadOnlyImage {
    file: File,
    len: u64,
}

impl ReadOnlyImage {
    /// The caller MUST supply a separately trusted, local, ordinary image
    /// pathname; neither renderer paths nor raw devices are authorized here.
    pub fn open(path: &Path) -> Result<Self, ImageReadError> {
        if !path.is_absolute() {
            return Err(ImageReadError::UnsafeTarget);
        }
        // Fail closed on device namespaces and UNC sources on Windows.
        // Final-component symlinks are also rejected on all platforms.
        #[cfg(windows)]
        {
            let text = path.to_string_lossy();
            if text.starts_with(r"\\.\")
                || text.starts_with(r"\\?\")
                || text.starts_with(r"\Device\")
                || text.starts_with(r"\\")
            {
                return Err(ImageReadError::UnsafeTarget);
            }
        }
        let entry = fs::symlink_metadata(path)?;
        if !entry.file_type().is_file() || entry.file_type().is_symlink() {
            return Err(ImageReadError::UnsafeTarget);
        }
        // No create, truncate, append or write flags are ever set.
        let file = OpenOptions::new().read(true).open(path)?;
        let opened = file.metadata()?;
        if !opened.is_file() {
            return Err(ImageReadError::UnsafeTarget);
        }
        if opened.len() != entry.len() {
            return Err(ImageReadError::ImageSizeChanged);
        }
        if opened.len() == 0 {
            return Err(ImageReadError::EmptyImage);
        }
        Ok(Self { len: opened.len(), file })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    /// Bounded, exact byte read. Rejects short reads and file length changes.
    pub fn read_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, ImageReadError> {
        if length == 0 || length > MAX_SINGLE_READ {
            return Err(ImageReadError::InvalidReadLength);
        }
        let end = offset.checked_add(length as u64).ok_or(ImageReadError::OutOfBounds)?;
        if end > self.len {
            return Err(ImageReadError::OutOfBounds);
        }
        if self.file.metadata()?.len() != self.len {
            return Err(ImageReadError::ImageSizeChanged);
        }
        self.file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0_u8; length];
        self.file.read_exact(&mut bytes)?;
        if self.file.metadata()?.len() != self.len {
            return Err(ImageReadError::ImageSizeChanged);
        }
        Ok(bytes)
    }

    pub fn read_sector(&mut self, lba: u64, logical_sector_size: usize) -> Result<Vec<u8>, ImageReadError> {
        if !matches!(logical_sector_size, 512 | 4096) {
            return Err(ImageReadError::InvalidSectorSize);
        }
        if self.len % logical_sector_size as u64 != 0 {
            return Err(ImageReadError::InvalidGeometry);
        }
        let offset = lba.checked_mul(logical_sector_size as u64)
            .ok_or(ImageReadError::OutOfBounds)?;
        self.read_at(offset, logical_sector_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);
    impl Fixture {
        fn create(contents: &[u8]) -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("p11-image-{}-{stamp}.img", std::process::id()));
            let mut file = OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
            file.write_all(contents).unwrap();
            file.sync_all().unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
    }

    #[test]
    fn reads_exact_512_and_4kn_sectors() {
        let mut contents = vec![0_u8; 8192];
        contents[510] = 0x55;
        contents[511] = 0xAA;
        contents[4096] = 0x7E;
        let fixture = Fixture::create(&contents);
        let mut img = ReadOnlyImage::open(&fixture.0).unwrap();
        assert_eq!(img.len(), 8192);
        assert_eq!(&img.read_sector(0, 512).unwrap()[510..512], &[0x55, 0xAA]);
        assert_eq!(img.read_sector(1, 4096).unwrap()[0], 0x7E);
        assert_eq!(img.read_at(4095, 2).unwrap(), vec![0, 0x7E]);
    }

    #[test]
    fn rejects_invalid_lengths_offsets_and_sectors() {
        let fixture = Fixture::create(&vec![0_u8; 4096]);
        let mut img = ReadOnlyImage::open(&fixture.0).unwrap();
        assert!(matches!(img.read_at(0, 0), Err(ImageReadError::InvalidReadLength)));
        assert!(matches!(img.read_at(0, MAX_SINGLE_READ + 1), Err(ImageReadError::InvalidReadLength)));
        assert!(matches!(img.read_at(u64::MAX, 512), Err(ImageReadError::OutOfBounds)));
        assert!(matches!(img.read_sector(8, 512), Err(ImageReadError::OutOfBounds)));
        assert!(matches!(img.read_sector(0, 1024), Err(ImageReadError::InvalidSectorSize)));
    }

    #[test]
    fn refuses_directories_empty_images_and_changed_sizes() {
        let empty = Fixture::create(&[]);
        assert!(matches!(ReadOnlyImage::open(&empty.0), Err(ImageReadError::EmptyImage)));
        assert!(matches!(ReadOnlyImage::open(&std::env::temp_dir()), Err(ImageReadError::UnsafeTarget)));
        let fixture = Fixture::create(&vec![5_u8; 4096]);
        let mut img = ReadOnlyImage::open(&fixture.0).unwrap();
        // A separate handle truncates the disposable fixture, not a user file.
        OpenOptions::new().write(true).open(&fixture.0).unwrap().set_len(512).unwrap();
        assert!(matches!(img.read_at(0, 512), Err(ImageReadError::ImageSizeChanged)));
    }

    #[test]
    fn refuses_relative_and_unaligned_images() {
        assert!(matches!(ReadOnlyImage::open(Path::new("relative.img")), Err(ImageReadError::UnsafeTarget)));
        let fixture = Fixture::create(&vec![0_u8; 513]);
        let mut img = ReadOnlyImage::open(&fixture.0).unwrap();
        assert!(matches!(img.read_sector(0, 512), Err(ImageReadError::InvalidGeometry)));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_final_component_symlinks() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::create(&vec![0_u8; 512]);
        let link = fixture.0.with_extension("link");
        symlink(&fixture.0, &link).unwrap();
        assert!(matches!(ReadOnlyImage::open(&link), Err(ImageReadError::UnsafeTarget)));
        fs::remove_file(link).unwrap();
    }
}
