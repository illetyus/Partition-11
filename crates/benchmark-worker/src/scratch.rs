use std::ffi::{c_void, OsString};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};
use std::ptr::{null, null_mut};

const MIN_SIZE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SIZE_BYTES: u64 = 1024 * 1024 * 1024;
const SIZE_ALIGNMENT: u64 = 4096;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
#[cfg(test)]
const FILE_SHARE_DELETE: u32 = 0x0000_0004;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const DELETE: u32 = 0x0001_0000;
const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
const CREATE_NEW: u32 = 1;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
const FILE_ATTRIBUTE_DEVICE: u32 = 0x0000_0040;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
#[cfg(test)]
const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
const FILE_BEGIN: u32 = 0;
const FILE_DISPOSITION_INFO_CLASS: i32 = 4;
const FILE_STANDARD_INFO_CLASS: i32 = 1;
const FILE_ATTRIBUTE_TAG_INFO_CLASS: i32 = 9;
const FILE_ID_INFO_CLASS: i32 = 18;
const FILE_NAME_NORMALIZED: u32 = 0;
const VOLUME_NAME_GUID: u32 = 1;
const ERROR_ALREADY_EXISTS: i32 = 183;
const ERROR_FILE_EXISTS: i32 = 80;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
const MAX_FINAL_PATH_CHARS: usize = 32_767;
const SCRATCH_FILE_PREFIX: &str = ".partition11-scratch-";

type RawHandle = *mut c_void;
const INVALID_HANDLE_VALUE: RawHandle = -1isize as RawHandle;

#[link(name = "Kernel32")]
extern "system" {
    fn CloseHandle(handle: RawHandle) -> i32;
    fn CreateFileW(
        path: *const u16,
        desired_access: u32,
        share_mode: u32,
        security_attributes: *const c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: RawHandle,
    ) -> RawHandle;
    fn FlushFileBuffers(handle: RawHandle) -> i32;
    fn GetFileInformationByHandleEx(
        handle: RawHandle,
        information_class: i32,
        information: *mut c_void,
        buffer_size: u32,
    ) -> i32;
    fn GetFinalPathNameByHandleW(
        handle: RawHandle,
        path: *mut u16,
        path_length: u32,
        flags: u32,
    ) -> u32;
    fn SetEndOfFile(handle: RawHandle) -> i32;
    fn SetFileInformationByHandle(
        handle: RawHandle,
        information_class: i32,
        information: *const c_void,
        buffer_size: u32,
    ) -> i32;
    fn SetFilePointerEx(
        handle: RawHandle,
        distance_to_move: i64,
        new_file_pointer: *mut i64,
        move_method: u32,
    ) -> i32;
}

#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(
        algorithm: RawHandle,
        buffer: *mut u8,
        buffer_length: u32,
        flags: u32,
    ) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileId {
    volume_serial: u64,
    identifier: [u8; 16],
}

#[derive(Clone, Debug)]
struct ObjectIdentity {
    file_id: FileId,
    final_path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CleanupStage {
    Active,
    FileDeletePending,
    FileClosed,
    Complete,
}

/// Holds a create-new, bounded-size file at one mounted volume root.
///
/// This primitive does not start a benchmark, open a physical disk, or grant an IPC capability.
/// The returned path uses the volume-GUID form from the open file handle.
pub struct ScratchLease {
    original_root: PathBuf,
    root: Handle,
    root_identity: ObjectIdentity,
    file: Option<Handle>,
    file_identity: ObjectIdentity,
    file_path: PathBuf,
    size_bytes: u64,
    cleanup_stage: CleanupStage,
}

impl ScratchLease {
    /// Creates a new randomly named file directly at the selected volume root.
    pub fn create(root: &Path, size_bytes: u64) -> io::Result<Self> {
        Self::create_impl(root, size_bytes, true)
    }

    #[cfg(test)]
    pub(crate) fn create_for_test(root: &Path, size_bytes: u64) -> io::Result<Self> {
        Self::create_impl(root, size_bytes, false)
    }

    fn create_impl(root: &Path, size_bytes: u64, require_volume_root: bool) -> io::Result<Self> {
        validate_size(size_bytes)?;
        validate_root_argument(root)?;

        let root_handle = open_directory(root, FILE_READ_ATTRIBUTES)?;
        let root_identity = query_identity(root_handle.raw())?;
        validate_directory(root_handle.raw(), &root_identity)?;
        verify_path_identity(root, &root_identity, true)?;
        let original_root = root_identity.final_path.clone();

        let volume_prefix = volume_guid_prefix(&root_identity.final_path)?;
        if require_volume_root
            && !windows_path_equal(&root_identity.final_path, Path::new(&volume_prefix))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "scratch root must be the mounted volume root",
            ));
        }
        verify_path_identity(&original_root, &root_identity, true)?;
        let mut created_file = None;
        for _ in 0..8 {
            let file_path = root_identity.final_path.join(random_file_name()?);
            match open_new_file(&file_path) {
                Ok(file) => {
                    created_file = Some((file_path, file));
                    break;
                }
                Err(error)
                    if matches!(
                        error.raw_os_error(),
                        Some(ERROR_ALREADY_EXISTS) | Some(ERROR_FILE_EXISTS)
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        let (file_path, file) = created_file.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not allocate a unique scratch file",
            )
        })?;

        let (file, file_identity) =
            capture_and_validate_created_file(file, &root_identity, &volume_prefix)?;

        let lease = Self {
            original_root,
            root: root_handle,
            root_identity,
            file: Some(file),
            file_identity,
            file_path,
            size_bytes,
            cleanup_stage: CleanupStage::Active,
        };

        lease.set_file_size()?;
        lease.verify()?;
        Ok(lease)
    }

    /// Returns the identity-bound volume-GUID path of this lease's file.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Rechecks the open handles and their current paths against their captured identities.
    pub fn verify(&self) -> io::Result<()> {
        if self.cleanup_stage != CleanupStage::Active {
            return Err(invalid_data("scratch lease is already being cleaned up"));
        }

        validate_directory(self.root.raw(), &self.root_identity)?;
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| invalid_data("scratch file handle is closed"))?;
        validate_regular_file(file.raw(), &self.file_identity, self.size_bytes)?;

        let volume_prefix = volume_guid_prefix(&self.root_identity.final_path)?;
        ensure_same_volume(&self.root_identity, &self.file_identity, &volume_prefix)?;

        verify_path_identity(&self.original_root, &self.root_identity, true)?;
        verify_path_identity(&self.file_path, &self.file_identity, false)?;
        Ok(())
    }

    /// Deletes only the file captured by this lease.
    ///
    /// An identity mismatch stops cleanup; no pathname, wildcard, or directory deletion is used.
    /// The method is safe to retry after a partial cleanup.
    pub fn cleanup(&mut self) -> io::Result<()> {
        loop {
            match self.cleanup_stage {
                CleanupStage::Active => {
                    self.verify_for_cleanup()?;
                    let file = self
                        .file
                        .as_ref()
                        .ok_or_else(|| invalid_data("scratch file handle is closed"))?;
                    set_delete_disposition(file.raw())?;
                    self.cleanup_stage = CleanupStage::FileDeletePending;
                }
                CleanupStage::FileDeletePending => {
                    self.file
                        .as_mut()
                        .ok_or_else(|| invalid_data("scratch file handle is closed"))?
                        .close()?;
                    self.file = None;
                    self.cleanup_stage = CleanupStage::FileClosed;
                }
                CleanupStage::FileClosed => {
                    validate_directory(self.root.raw(), &self.root_identity)?;
                    verify_path_identity(&self.original_root, &self.root_identity, true)?;
                    self.root.close()?;
                    self.cleanup_stage = CleanupStage::Complete;
                }
                CleanupStage::Complete => return Ok(()),
            }
        }
    }

    fn verify_for_cleanup(&self) -> io::Result<()> {
        if self.cleanup_stage != CleanupStage::Active {
            return Err(invalid_data("scratch lease is already being cleaned up"));
        }
        validate_directory(self.root.raw(), &self.root_identity)?;
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| invalid_data("scratch file handle is closed"))?;
        validate_owned_regular_file(file.raw(), &self.file_identity)?;
        let volume_prefix = volume_guid_prefix(&self.root_identity.final_path)?;
        ensure_same_volume(&self.root_identity, &self.file_identity, &volume_prefix)?;
        verify_path_identity(&self.original_root, &self.root_identity, true)?;
        verify_path_identity(&self.file_path, &self.file_identity, false)?;
        Ok(())
    }
    fn set_file_size(&self) -> io::Result<()> {
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| invalid_data("scratch file handle is closed"))?;
        if unsafe { SetFilePointerEx(file.raw(), self.size_bytes as i64, null_mut(), FILE_BEGIN) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe { SetEndOfFile(file.raw()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { FlushFileBuffers(file.raw()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn capture_and_validate_created_file(
    file: Handle,
    root_identity: &ObjectIdentity,
    volume_prefix: &str,
) -> io::Result<(Handle, ObjectIdentity)> {
    let file_identity = query_identity(file.raw()).and_then(|identity| {
        validate_owned_regular_file(file.raw(), &identity)?;
        ensure_same_volume(root_identity, &identity, volume_prefix)?;
        Ok(identity)
    });

    match file_identity {
        Ok(identity) => Ok((file, identity)),
        Err(error) => {
            cleanup_failed_file_creation(file);
            Err(error)
        }
    }
}

fn cleanup_failed_file_creation(mut file: Handle) {
    if set_delete_disposition(file.raw()).is_ok() {
        let _ = file.close();
    }
}

impl Drop for ScratchLease {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

struct Handle {
    raw: RawHandle,
}

impl Handle {
    fn from_raw(raw: RawHandle) -> io::Result<Self> {
        if raw == INVALID_HANDLE_VALUE || raw.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self { raw })
        }
    }

    fn raw(&self) -> RawHandle {
        self.raw
    }

    fn close(&mut self) -> io::Result<()> {
        if self.raw.is_null() {
            return Ok(());
        }
        if unsafe { CloseHandle(self.raw) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.raw = null_mut();
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[repr(C)]
struct FileIdInfo {
    volume_serial: u64,
    identifier: [u8; 16],
}

#[repr(C)]
struct FileStandardInfo {
    allocation_size: i64,
    end_of_file: i64,
    number_of_links: u32,
    delete_pending: u8,
    directory: u8,
}

#[repr(C)]
struct FileAttributeTagInfo {
    file_attributes: u32,
    reparse_tag: u32,
}

#[repr(C)]
struct FileDispositionInfo {
    delete_file: u8,
}

fn validate_size(size_bytes: u64) -> io::Result<()> {
    if !(MIN_SIZE_BYTES..=MAX_SIZE_BYTES).contains(&size_bytes) || size_bytes % SIZE_ALIGNMENT != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scratch file size must be 4 MiB through 1 GiB and 4096-byte aligned",
        ));
    }
    Ok(())
}

fn validate_root_argument(root: &Path) -> io::Result<()> {
    let has_dot_segment = root
        .to_string_lossy()
        .split(|separator| separator == '/' || separator == char::from(92))
        .any(|segment| matches!(segment, "." | ".."));
    if !root.is_absolute()
        || has_dot_segment
        || root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scratch root must be an absolute canonical directory path",
        ));
    }
    let canonical = std::fs::canonicalize(root)?;
    if !windows_path_equal(root, &canonical) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scratch root is not canonical",
        ));
    }
    Ok(())
}

fn open_directory(path: &Path, desired_access: u32) -> io::Result<Handle> {
    let path = wide_path(path)?;
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            desired_access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    Handle::from_raw(raw)
}

fn open_new_file(path: &Path) -> io::Result<Handle> {
    let path = wide_path(path)?;
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | DELETE | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    Handle::from_raw(raw)
}

#[cfg(test)]
fn open_delete_on_close_probe_file(path: &Path) -> io::Result<Handle> {
    let path = wide_path(path)?;
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | DELETE | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_DELETE_ON_CLOSE,
            null_mut(),
        )
    };
    Handle::from_raw(raw)
}

#[cfg(test)]
fn set_file_size(handle: RawHandle, size_bytes: u64) -> io::Result<()> {
    if unsafe { SetFilePointerEx(handle, size_bytes as i64, null_mut(), FILE_BEGIN) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { SetEndOfFile(handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { FlushFileBuffers(handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.is_empty() || value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is empty or contains a nul character",
        ));
    }
    Ok(value.into_iter().chain(std::iter::once(0)).collect())
}

fn random_file_name() -> io::Result<String> {
    let mut random = [0u8; 16];
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            random.as_mut_ptr(),
            random.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        return Err(io::Error::from_raw_os_error(status));
    }
    let mut name = String::with_capacity(SCRATCH_FILE_PREFIX.len() + random.len() * 2 + 4);
    name.push_str(SCRATCH_FILE_PREFIX);
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut name, "{byte:02x}").expect("writing to String cannot fail");
    }
    name.push_str(".bin");
    Ok(name)
}

fn query_identity(handle: RawHandle) -> io::Result<ObjectIdentity> {
    let mut file_id: FileIdInfo = unsafe { zeroed() };
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FILE_ID_INFO_CLASS,
            &mut file_id as *mut FileIdInfo as *mut c_void,
            size_of::<FileIdInfo>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    let path = final_path_by_handle(handle)?;
    volume_guid_prefix(&path)?;
    Ok(ObjectIdentity {
        file_id: FileId {
            volume_serial: file_id.volume_serial,
            identifier: file_id.identifier,
        },
        final_path: path,
    })
}

fn validate_directory(handle: RawHandle, expected: &ObjectIdentity) -> io::Result<()> {
    let (attributes, standard) = query_attributes_and_standard(handle)?;
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || attributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || attributes & FILE_ATTRIBUTE_DEVICE != 0
        || standard.directory == 0
        || standard.delete_pending != 0
    {
        return Err(invalid_data(
            "scratch directory is not a stable ordinary directory",
        ));
    }
    verify_same_object(handle, expected)
}

fn validate_owned_file_metadata(attributes: u32, standard: &FileStandardInfo) -> io::Result<()> {
    if attributes
        & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_DEVICE)
        != 0
        || standard.directory != 0
        || standard.delete_pending != 0
        || standard.number_of_links != 1
    {
        return Err(invalid_data(
            "scratch target is reparse-linked, non-regular, multiply-linked, or pending deletion",
        ));
    }
    Ok(())
}

fn validate_owned_regular_file(handle: RawHandle, expected: &ObjectIdentity) -> io::Result<()> {
    let (attributes, standard) = query_attributes_and_standard(handle)?;
    validate_owned_file_metadata(attributes, &standard)?;
    verify_same_object(handle, expected)
}
fn validate_regular_file(
    handle: RawHandle,
    expected: &ObjectIdentity,
    expected_size: u64,
) -> io::Result<()> {
    let (_, standard) = query_attributes_and_standard(handle)?;
    validate_owned_regular_file(handle, expected)?;
    if standard.end_of_file < 0 || standard.end_of_file as u64 != expected_size {
        return Err(invalid_data("scratch target has an unexpected size"));
    }
    Ok(())
}

fn query_attributes_and_standard(handle: RawHandle) -> io::Result<(u32, FileStandardInfo)> {
    let mut attributes: FileAttributeTagInfo = unsafe { zeroed() };
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FILE_ATTRIBUTE_TAG_INFO_CLASS,
            &mut attributes as *mut FileAttributeTagInfo as *mut c_void,
            size_of::<FileAttributeTagInfo>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut standard: FileStandardInfo = unsafe { zeroed() };
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FILE_STANDARD_INFO_CLASS,
            &mut standard as *mut FileStandardInfo as *mut c_void,
            size_of::<FileStandardInfo>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((attributes.file_attributes, standard))
}

fn verify_same_object(handle: RawHandle, expected: &ObjectIdentity) -> io::Result<()> {
    let actual = query_identity(handle)?;
    if actual.file_id != expected.file_id
        || !windows_path_equal(&actual.final_path, &expected.final_path)
    {
        return Err(invalid_data(
            "scratch path no longer names its captured file identity",
        ));
    }
    Ok(())
}

fn verify_path_identity(path: &Path, expected: &ObjectIdentity, directory: bool) -> io::Result<()> {
    let handle = if directory {
        open_directory(path, FILE_READ_ATTRIBUTES)?
    } else {
        let wide = wide_path(path)?;
        let raw = unsafe {
            CreateFileW(
                wide.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        Handle::from_raw(raw)?
    };
    if directory {
        validate_directory(handle.raw(), expected)
    } else {
        let (attributes, standard) = query_attributes_and_standard(handle.raw())?;
        validate_owned_file_metadata(attributes, &standard)?;
        verify_same_object(handle.raw(), expected)
    }
}
fn ensure_same_volume(
    left: &ObjectIdentity,
    right: &ObjectIdentity,
    expected_volume_prefix: &str,
) -> io::Result<()> {
    if left.file_id.volume_serial != right.file_id.volume_serial
        || !volume_guid_prefix(&left.final_path)?.eq_ignore_ascii_case(expected_volume_prefix)
        || !volume_guid_prefix(&right.final_path)?.eq_ignore_ascii_case(expected_volume_prefix)
    {
        return Err(invalid_data(
            "scratch objects are not on the same captured volume",
        ));
    }
    Ok(())
}

fn final_path_by_handle(handle: RawHandle) -> io::Result<PathBuf> {
    let mut capacity = 512usize;
    loop {
        if capacity > MAX_FINAL_PATH_CHARS {
            return Err(invalid_data(
                "final scratch path exceeds the Windows path limit",
            ));
        }
        let mut buffer = vec![0u16; capacity];
        let length = unsafe {
            GetFinalPathNameByHandleW(
                handle,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_GUID,
            )
        } as usize;
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        capacity = length.saturating_add(1);
    }
}

fn volume_guid_prefix(path: &Path) -> io::Result<String> {
    let value = path.to_string_lossy();
    let prefix = "\\\\?\\volume{";
    if !value.to_ascii_lowercase().starts_with(prefix) {
        return Err(invalid_data("scratch path has no volume-GUID identity"));
    }
    let end = value
        .find("}\\")
        .map(|index| index + 2)
        .ok_or_else(|| invalid_data("scratch path has a malformed volume-GUID prefix"))?;
    Ok(value[..end].to_string())
}

fn windows_path_equal(left: &Path, right: &Path) -> bool {
    fn key(path: &Path) -> String {
        let mut value = path.to_string_lossy().replace('/', "\\");
        if value.to_ascii_lowercase().starts_with("\\\\?\\unc\\") {
            value = format!("\\\\{}", &value[8..]);
        } else if value.starts_with("\\\\?\\") {
            value = value[4..].to_string();
        }
        value.to_ascii_lowercase()
    }
    key(left) == key(right)
}

fn set_delete_disposition(handle: RawHandle) -> io::Result<()> {
    let disposition = FileDispositionInfo { delete_file: 1 };
    if unsafe {
        SetFileInformationByHandle(
            handle,
            FILE_DISPOSITION_INFO_CLASS,
            &disposition as *const FileDispositionInfo as *const c_void,
            size_of::<FileDispositionInfo>() as u32,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::ScratchLease;
    use std::ffi::OsString;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

    fn test_root() -> PathBuf {
        let ticks = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "p11-bench-test-{}-{ticks}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::canonicalize(root).unwrap()
    }

    #[test]
    fn delete_on_close_probe_allows_delete_sharing_and_removes_file_after_last_handle(
    ) -> io::Result<()> {
        use super::{open_delete_on_close_probe_file, random_file_name, set_file_size};
        use std::os::windows::fs::OpenOptionsExt;

        let root = test_root();
        let path = root.join(random_file_name()?);
        let file = open_delete_on_close_probe_file(&path)?;
        set_file_size(file.raw(), 4 * 1024 * 1024)?;
        assert!(
            path.exists(),
            "probe file must remain present while handles are open"
        );

        let second_handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(super::FILE_SHARE_READ | super::FILE_SHARE_WRITE | super::FILE_SHARE_DELETE)
            .open(&path)?;
        drop(second_handle);
        drop(file);
        assert!(
            !path.exists(),
            "delete-on-close must remove only the probe file"
        );
        fs::remove_dir(root)
    }

    #[test]
    #[ignore = "CI-only: validates the pinned DiskSpd executable against a temporary delete-on-close file"]
    fn pinned_diskspd_reads_delete_on_close_file_under_contained_job() -> io::Result<()> {
        use super::{open_delete_on_close_probe_file, random_file_name, set_file_size};
        use crate::job_process::{run_process, ProcessCommand, ProcessLimits};

        if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "pinned DiskSpd compatibility probe is restricted to GitHub Actions",
            ));
        }
        let executable =
            PathBuf::from(std::env::var_os("P11_DISKSPD_TEST_BINARY").ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "CI DiskSpd path is required")
            })?);
        let verified = crate::VerifiedDiskSpd::verify(&executable)?;
        let root = test_root();
        let path = root.join(random_file_name()?);
        let file = open_delete_on_close_probe_file(&path)?;
        set_file_size(file.raw(), 64 * 1024 * 1024)?;

        let args = vec![
            OsString::from("-b1M"),
            OsString::from("-o1"),
            OsString::from("-t1"),
            OsString::from("-w0"),
            OsString::from("-W0"),
            OsString::from("-d2"),
            OsString::from("-Su"),
            OsString::from("-L"),
            OsString::from("-Rxml"),
            path.as_os_str().to_os_string(),
        ];
        let command = ProcessCommand::test_fixture(verified.path().to_path_buf(), args)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let limits = ProcessLimits::from_workload(0, 2)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let output = run_process(command, limits, &AtomicBool::new(false))
            .map_err(|error| io::Error::other(error.to_string()))?;
        if output.exit_code != 0 || output.stdout.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pinned DiskSpd did not complete the bounded temporary-file read",
            ));
        }
        drop(verified);
        drop(file);
        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "delete-on-close probe file remained after the contained process drained",
            ));
        }
        fs::remove_dir(root)
    }
    #[test]
    fn rejects_reparse_file_attribute_without_filesystem_privileges() {
        let standard = super::FileStandardInfo {
            allocation_size: 4 * 1024 * 1024,
            end_of_file: 4 * 1024 * 1024,
            number_of_links: 1,
            delete_pending: 0,
            directory: 0,
        };
        let result =
            super::validate_owned_file_metadata(super::FILE_ATTRIBUTE_REPARSE_POINT, &standard);
        assert!(matches!(
            result,
            Err(error) if error.kind() == io::ErrorKind::InvalidData
        ));
    }
    #[test]
    fn rejects_junction_roots_before_creating_payloads() -> io::Result<()> {
        use std::process::Command;

        let root = test_root();
        let target = root.join("target");
        let junction = root.join("junction");
        fs::create_dir(&target)?;
        let output = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()?;
        if !output.status.success() {
            let details = String::from_utf8_lossy(&output.stderr).into_owned();
            fs::remove_dir(&target)?;
            fs::remove_dir(&root)?;
            return Err(io::Error::other(details));
        }

        let result = ScratchLease::create_for_test(&junction, 4 * 1024 * 1024);
        let payload_count = fs::read_dir(&target)?.count();
        fs::remove_dir(&junction)?;
        fs::remove_dir(&target)?;
        fs::remove_dir(&root)?;
        assert!(
            matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidInput),
            "a junction must be rejected as a scratch root"
        );
        assert_eq!(
            payload_count, 0,
            "root rejection must precede payload creation"
        );
        Ok(())
    }
    #[test]
    fn rejects_a_replacement_at_a_captured_file_path() -> io::Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;

        let root = test_root();
        let path = root.join("captured.txt");
        let moved = root.join("original.txt");
        let original = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(super::FILE_SHARE_READ | super::FILE_SHARE_WRITE | super::FILE_SHARE_DELETE)
            .create_new(true)
            .open(&path)?;
        let identity = super::query_identity(original.as_raw_handle())?;
        fs::rename(&path, &moved)?;
        fs::write(&path, b"replacement file")?;

        let rejected = super::verify_path_identity(&path, &identity, false).is_err();
        drop(original);
        fs::remove_file(&path)?;
        fs::remove_file(&moved)?;
        fs::remove_dir(&root)?;
        assert!(
            rejected,
            "a replacement file must not match the captured file identity"
        );
        Ok(())
    }

    #[test]
    fn abrupt_exit_child_process() -> io::Result<()> {
        let Some(root) = std::env::var_os("P11_BENCHMARK_SCRATCH_CHILD_ROOT") else {
            return Ok(());
        };
        let lease = ScratchLease::create_for_test(std::path::Path::new(&root), 4 * 1024 * 1024)?;
        println!("P11_SCRATCH_PATH={}", lease.file_path().display());
        std::io::Write::flush(&mut std::io::stdout())?;
        std::process::exit(86);
    }

    #[test]
    #[ignore = "red acceptance probe: enable only after the CI DiskSpd delete-on-close gate passes"]
    fn lease_file_is_removed_after_owner_exits_without_drop() -> io::Result<()> {
        use std::process::Command;

        let root = test_root();
        let output = Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg("scratch::tests::abrupt_exit_child_process")
            .arg("--nocapture")
            .env("P11_BENCHMARK_SCRATCH_CHILD_ROOT", &root)
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let scratch_path = stdout
            .lines()
            .find_map(|line| line.strip_prefix("P11_SCRATCH_PATH="))
            .map(PathBuf::from);
        let survived_process_exit = scratch_path.as_ref().is_some_and(|path| path.exists());
        let exit_code = output.status.code();
        fs::remove_dir_all(&root)?;

        assert_eq!(
            exit_code,
            Some(86),
            "child helper did not terminate at the crash point: {stdout}"
        );
        assert!(
            scratch_path.is_some(),
            "child helper did not report its owned scratch file: {stdout}"
        );
        assert!(
            !survived_process_exit,
            "the lease file must be removed when the owning process exits without running Drop"
        );
        Ok(())
    }
    #[test]
    fn creates_exclusive_file_binds_identity_and_cleans_only_its_file() -> io::Result<()> {
        let root = test_root();
        let mut lease = ScratchLease::create_for_test(&root, 4 * 1024 * 1024)?;
        let path = lease.file_path().to_path_buf();

        assert_eq!(fs::metadata(&path)?.len(), 4 * 1024 * 1024);
        let other_handle = fs::OpenOptions::new().write(true).open(&path)?;
        other_handle.set_len(4 * 1024 * 1024 + 4096)?;
        assert!(
            lease.verify().is_err(),
            "lease must reject an unexpected file size"
        );
        other_handle.set_len(4 * 1024 * 1024)?;
        drop(other_handle);
        lease.verify()?;
        assert!(
            fs::rename(&path, path.with_extension("renamed")).is_err(),
            "open lease must prevent target rename"
        );
        let moved_root = root.with_extension("moved");
        assert!(
            fs::rename(&root, &moved_root).is_err(),
            "open lease must keep its volume root path stable"
        );

        let foreign_file = root.join("foreign.txt");
        fs::write(&foreign_file, b"must remain untouched")?;
        lease.cleanup()?;
        assert!(
            !path.exists(),
            "the owned scratch file should be removed by its handle"
        );
        assert!(
            foreign_file.exists(),
            "cleanup must preserve unrelated files"
        );

        fs::remove_file(&foreign_file)?;
        lease.cleanup()?;
        lease.cleanup()?;
        assert!(lease.verify().is_err());
        assert!(!path.exists());
        fs::remove_dir(root)
    }

    #[test]
    fn production_entry_rejects_non_volume_roots_before_creating_payloads() -> io::Result<()> {
        let root = test_root();
        let result = ScratchLease::create(&root, 4 * 1024 * 1024);
        assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidInput));
        assert_eq!(fs::read_dir(&root)?.count(), 0);
        fs::remove_dir(root)
    }

    #[test]
    fn failed_post_create_validation_deletes_the_exact_file() -> io::Result<()> {
        use super::{
            capture_and_validate_created_file, open_directory, open_new_file, query_identity,
            random_file_name, FILE_READ_ATTRIBUTES,
        };

        let root = test_root();
        let root_handle = open_directory(&root, FILE_READ_ATTRIBUTES)?;
        let root_identity = query_identity(root_handle.raw())?;
        let file_path = root_identity.final_path.join(random_file_name()?);
        let file = open_new_file(&file_path)?;

        let result = capture_and_validate_created_file(file, &root_identity, "");
        assert!(
            result.is_err(),
            "the deliberately invalid volume must fail closed"
        );
        assert!(
            !file_path.exists(),
            "the owned file should be deleted by handle"
        );

        drop(root_handle);
        fs::remove_dir(root)
    }

    #[test]
    fn rejects_unsafe_sizes_and_noncanonical_roots_without_creating_files() -> io::Result<()> {
        let root = test_root();
        for bytes in [
            0,
            1,
            4095,
            4 * 1024 * 1024 - 1,
            4 * 1024 * 1024 + 1,
            1024 * 1024 * 1024 + 1,
        ] {
            assert!(
                ScratchLease::create(&root, bytes).is_err(),
                "accepted unsafe size {bytes}"
            );
        }
        let noncanonical = PathBuf::from(format!("{}\\..", root.display()));
        assert!(ScratchLease::create(&noncanonical, 4 * 1024 * 1024).is_err());
        assert_eq!(fs::read_dir(&root)?.count(), 0);
        fs::remove_dir(root)
    }
}
