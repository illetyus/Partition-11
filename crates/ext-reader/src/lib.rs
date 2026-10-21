#![deny(unsafe_code)]

//! Read-only ext4 access for image files.
//!
//! The wrapper opens one validated regular file with a read-only OS handle,
//! wraps that handle in a read-only block device, and exposes only listing and
//! streaming reads. The upstream filesystem type and its mutation methods
//! never escape this module. Windows device namespaces are rejected before open.

use fs_ext4::block_io::BlockDevice;
use fs_ext4::features::{self, FsFlavor};
use fs_ext4::superblock::EXT4_ERROR_FS;
use fs_ext4::{Error as FilesystemError, Filesystem, Superblock};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024 * 1024 * 1024;
pub const MAX_IMAGE_PATH_BYTES: usize = 4096;
pub const MAX_DIRECTORY_BYTES: u64 = 128 * 1024;
pub const MAX_DIRECTORY_ENTRIES: usize = 16_384;
pub const MAX_EXPORT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MIN_IMAGE_BYTES: u64 = 2048;
const READ_CHUNK_BYTES: usize = 64 * 1024;
const MAX_EXT_NAME_BYTES: usize = 255;

#[derive(Debug)]
pub enum ExtImageError {
    Io(io::Error),
    Filesystem(FilesystemError),
    FilesystemNeedsRecovery,
    InvalidImage,
    ImageTooSmall,
    ImageTooLarge,
    InvalidImagePath,
    UnsupportedFilesystemType,
    UnsupportedFeatureBits { incompat: u32, ro_compat: u32 },
    DirectoryTooLarge,
    TooManyDirectoryEntries,
    InvalidDirectoryEntry,
    NotDirectory,
    NotRegularFile,
    FileTooLarge { size_bytes: u64, max_bytes: u64 },
    InvalidExportLimit,
    ShortRead,
    Output(io::Error),
}

impl fmt::Display for ExtImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => f.write_str("image file could not be opened"),
            Self::Filesystem(_) => f.write_str("unsupported or corrupt ext filesystem image"),
            Self::FilesystemNeedsRecovery => {
                f.write_str("filesystem is not cleanly unmounted or needs journal recovery")
            }
            Self::InvalidImage => f.write_str("image must be a regular file"),
            Self::ImageTooSmall => f.write_str("image is too small to contain an ext superblock"),
            Self::ImageTooLarge => f.write_str("image exceeds the supported size limit"),
            Self::InvalidImagePath => f.write_str("invalid image path"),
            Self::UnsupportedFilesystemType => {
                f.write_str("only extent-based ext4 images are supported")
            }
            Self::UnsupportedFeatureBits { incompat, ro_compat } => write!(
                f,
                "filesystem feature flags exceed the read-only profile (incompat=0x{incompat:08x}, ro_compat=0x{ro_compat:08x})"
            ),
            Self::DirectoryTooLarge => f.write_str("directory exceeds the listing size limit"),
            Self::TooManyDirectoryEntries => f.write_str("directory exceeds the entry limit"),
            Self::InvalidDirectoryEntry => f.write_str("directory contains an invalid name"),
            Self::NotDirectory => f.write_str("image path does not name a directory"),
            Self::NotRegularFile => f.write_str("image path does not name a regular file"),
            Self::FileTooLarge {
                size_bytes,
                max_bytes,
            } => {
                write!(f, "file size {size_bytes} exceeds export limit {max_bytes}")
            }
            Self::InvalidExportLimit => f.write_str("export limit is outside the supported range"),
            Self::ShortRead => f.write_str("filesystem returned a short file read"),
            Self::Output(_) => f.write_str("export destination could not be written"),
        }
    }
}

impl std::error::Error for ExtImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) | Self::Output(error) => Some(error),
            Self::Filesystem(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ExtImageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FilesystemError> for ExtImageError {
    fn from(error: FilesystemError) -> Self {
        Self::Filesystem(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtEntryType {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtDirectoryEntry {
    /// Raw on-disk name bytes. Names are not assumed to be UTF-8.
    pub name: Vec<u8>,
    pub kind: ExtEntryType,
    pub size_bytes: u64,
}

/// A mounted, read-only view of one regular image file.
pub struct ExtImageReader {
    filesystem: Filesystem,
}

impl ExtImageReader {
    /// Opens and mounts a regular image file through one read-only OS handle.
    pub fn open(path: &Path) -> Result<Self, ExtImageError> {
        validate_local_file_path(path)?;
        let link_metadata = std::fs::symlink_metadata(path)?;
        if !link_metadata.is_file() || link_metadata.file_type().is_symlink() {
            return Err(ExtImageError::InvalidImage);
        }

        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            const FILE_SHARE_READ: u32 = 0x0000_0001;
            const SECURITY_IDENTIFICATION: u32 = 0x0000_0001;
            options
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .security_qos_flags(SECURITY_IDENTIFICATION);
        }
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ExtImageError::InvalidImage);
        }
        let image_size = metadata.len();
        if image_size < MIN_IMAGE_BYTES {
            return Err(ExtImageError::ImageTooSmall);
        }
        if image_size > MAX_IMAGE_BYTES {
            return Err(ExtImageError::ImageTooLarge);
        }

        let device = ImageFileDevice {
            file: Mutex::new(file),
            size_bytes: image_size,
        };
        let superblock = Superblock::read(&device)?;
        validate_read_profile(
            superblock.feature_compat,
            superblock.feature_incompat,
            superblock.feature_ro_compat,
        )?;
        if !superblock.is_clean()
            || superblock.state & EXT4_ERROR_FS != 0
            || superblock.feature_incompat & features::Incompat::RECOVER.bits() != 0
        {
            return Err(ExtImageError::FilesystemNeedsRecovery);
        }
        let filesystem = Filesystem::mount(Arc::new(device))?;
        Ok(Self { filesystem })
    }

    /// Lists one directory, bounded by its declared byte size and entry count.
    pub fn list_directory(
        &self,
        image_path: &[u8],
    ) -> Result<Vec<ExtDirectoryEntry>, ExtImageError> {
        validate_image_path(image_path)?;
        let inode_number = self.filesystem.lookup_path_bytes(image_path)?;
        let inode = self.filesystem.stat_ino(inode_number)?;
        if !inode.is_dir() {
            return Err(ExtImageError::NotDirectory);
        }
        if inode.size > MAX_DIRECTORY_BYTES {
            return Err(ExtImageError::DirectoryTooLarge);
        }

        let raw_entries = self.filesystem.read_dir_ino(inode_number)?;
        if raw_entries.len() > MAX_DIRECTORY_ENTRIES {
            return Err(ExtImageError::TooManyDirectoryEntries);
        }
        let mut entries = Vec::with_capacity(raw_entries.len());
        for raw in raw_entries {
            if raw.name == b"." || raw.name == b".." {
                continue;
            }
            if raw.name.is_empty()
                || raw.name.len() > MAX_EXT_NAME_BYTES
                || raw.name.contains(&0)
                || raw.name.contains(&b'/')
            {
                return Err(ExtImageError::InvalidDirectoryEntry);
            }
            let inode = self.filesystem.stat_ino(raw.inode)?;
            let kind = if inode.is_file() {
                ExtEntryType::File
            } else if inode.is_dir() {
                ExtEntryType::Directory
            } else if inode.is_symlink() {
                ExtEntryType::Symlink
            } else {
                ExtEntryType::Other
            };
            entries.push(ExtDirectoryEntry {
                name: raw.name,
                kind,
                size_bytes: inode.size,
            });
        }
        Ok(entries)
    }

    /// Streams one regular file into a caller-owned writer without buffering it all.
    ///
    /// The caller controls the destination and remains responsible for cleaning
    /// up partial output if the writer fails. The destination is never derived
    /// from an on-disk ext filename.
    pub fn copy_file_to<W: Write>(
        &self,
        image_path: &[u8],
        max_bytes: u64,
        output: &mut W,
    ) -> Result<u64, ExtImageError> {
        validate_image_path(image_path)?;
        if max_bytes > MAX_EXPORT_BYTES {
            return Err(ExtImageError::InvalidExportLimit);
        }
        let inode_number = self.filesystem.lookup_path_bytes(image_path)?;
        let inode = self.filesystem.stat_ino(inode_number)?;
        if !inode.is_file() {
            return Err(ExtImageError::NotRegularFile);
        }
        if inode.size > max_bytes {
            return Err(ExtImageError::FileTooLarge {
                size_bytes: inode.size,
                max_bytes,
            });
        }

        let mut buffer = [0_u8; READ_CHUNK_BYTES];
        let mut offset = 0_u64;
        while offset < inode.size {
            let remaining = inode.size - offset;
            let requested = usize::try_from(remaining.min(READ_CHUNK_BYTES as u64))
                .expect("bounded by fixed read chunk");
            let read = self
                .filesystem
                .read_ino(inode_number, offset, &mut buffer[..requested])?;
            if read == 0 || read > requested {
                return Err(ExtImageError::ShortRead);
            }
            output
                .write_all(&buffer[..read])
                .map_err(ExtImageError::Output)?;
            offset = offset
                .checked_add(read as u64)
                .ok_or(ExtImageError::ShortRead)?;
        }
        Ok(offset)
    }
}

struct ImageFileDevice {
    file: Mutex<File>,
    size_bytes: u64,
}

impl BlockDevice for ImageFileDevice {
    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), FilesystemError> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| FilesystemError::Corrupt("image file lock was poisoned"))?;
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;
        Ok(())
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

fn validate_read_profile(
    feature_compat: u32,
    feature_incompat: u32,
    feature_ro_compat: u32,
) -> Result<(), ExtImageError> {
    let flavor = FsFlavor::detect(feature_compat, feature_incompat);
    if !flavor.uses_extents() {
        return Err(ExtImageError::UnsupportedFilesystemType);
    }

    let unsupported_incompat = feature_incompat & !features::SUPPORTED_INCOMPAT;
    let unsupported_ro_compat = feature_ro_compat & !features::SUPPORTED_RO_COMPAT;
    if unsupported_incompat != 0 || unsupported_ro_compat != 0 {
        return Err(ExtImageError::UnsupportedFeatureBits {
            incompat: unsupported_incompat,
            ro_compat: unsupported_ro_compat,
        });
    }

    features::check_mountable(feature_incompat, feature_ro_compat)?;
    Ok(())
}
/// Rejects non-absolute paths and Windows device/network namespaces before opening a file.
pub fn validate_local_file_path(path: &Path) -> Result<(), ExtImageError> {
    if !path.is_absolute() {
        return Err(ExtImageError::InvalidImagePath);
    }

    #[cfg(windows)]
    {
        let normalized = path.to_string_lossy().replace('/', "\\");
        let lower = normalized.to_ascii_lowercase();
        let device_namespace = lower.starts_with("\\\\.\\")
            || lower.starts_with("\\\\?\\")
            || lower.starts_with("\\??\\")
            || lower.starts_with("\\device\\");
        let bytes = lower.as_bytes();
        let drive_path = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\';
        if device_namespace || !drive_path || has_windows_device_component(&lower) {
            return Err(ExtImageError::InvalidImagePath);
        }
    }

    Ok(())
}

#[cfg(windows)]
fn has_windows_device_component(path: &str) -> bool {
    path.split('\\').any(|component| {
        let base = component
            .split(':')
            .next()
            .unwrap_or_default()
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches([' ', '.']);
        matches!(base, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$")
            || (base.len() == 4
                && (base.starts_with("com") || base.starts_with("lpt"))
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
    })
}
fn validate_image_path(path: &[u8]) -> Result<(), ExtImageError> {
    if path.is_empty() || path.len() > MAX_IMAGE_PATH_BYTES || path[0] != b'/' || path.contains(&0)
    {
        return Err(ExtImageError::InvalidImagePath);
    }
    Ok(())
}

#[cfg(all(test, windows))]
mod windows_device_path_tests {
    use super::{validate_local_file_path, ExtImageError};
    use std::path::Path;

    #[test]
    fn rejects_windows_device_namespaces_before_any_file_open() {
        for path in [
            r"\\.\PhysicalDrive0",
            r"\\?\GLOBALROOT\Device\Harddisk0\Partition0",
            r"\Device\Harddisk0\Partition0",
            r"\??\PhysicalDrive0",
        ] {
            assert!(matches!(
                validate_local_file_path(Path::new(path)),
                Err(ExtImageError::InvalidImagePath)
            ));
        }
    }

    #[test]
    fn accepts_only_standard_absolute_windows_file_path_forms() {
        assert!(validate_local_file_path(Path::new(r"C:\images\disk.img")).is_ok());
        assert!(validate_local_file_path(Path::new("c:/images/disk.img")).is_ok());
        for path in [
            r"\\server\share\disk.img",
            r"C:relative.img",
            r"C:\images\NUL.txt",
            r"C:\images\COM9.img",
        ] {
            assert!(matches!(
                validate_local_file_path(Path::new(path)),
                Err(ExtImageError::InvalidImagePath)
            ));
        }
    }
}

#[cfg(test)]
mod read_profile_tests {
    use super::{validate_read_profile, ExtImageError};
    use fs_ext4::features::{Incompat, SUPPORTED_RO_COMPAT};

    #[test]
    fn accepts_ext4_extents_with_known_read_features() {
        assert!(validate_read_profile(
            0,
            Incompat::FILETYPE.bits() | Incompat::EXTENTS.bits(),
            SUPPORTED_RO_COMPAT,
        )
        .is_ok());
    }

    #[test]
    fn rejects_ext2_or_ext3_before_directory_browsing() {
        assert!(matches!(
            validate_read_profile(0, Incompat::FILETYPE.bits(), 0),
            Err(ExtImageError::UnsupportedFilesystemType)
        ));
    }

    #[test]
    fn rejects_unknown_incompatible_and_read_only_compatible_features() {
        let unknown = 1 << 31;
        assert!(matches!(
            validate_read_profile(0, Incompat::EXTENTS.bits() | unknown, 0),
            Err(ExtImageError::UnsupportedFeatureBits { incompat, ro_compat: 0 }) if incompat == unknown
        ));
        assert!(matches!(
            validate_read_profile(0, Incompat::EXTENTS.bits(), unknown),
            Err(ExtImageError::UnsupportedFeatureBits { incompat: 0, ro_compat }) if ro_compat == unknown
        ));
    }
}
