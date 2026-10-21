//! Bounded, metadata-only Windows volume inventory.
//!
//! Volume GUID paths remain inside the agent protocol boundary. The renderer
//! receives only a sanitized drive root and this scan's eligibility result.

use crate::disk_metadata::{MetadataError, ProbeOutcome};
use crate::scan_identity::ScanGeneration;
use crate::volume_eligibility::{self, Assessment, DriveType, VolumeFacts};
use std::time::Instant;

pub(crate) const MAX_VOLUMES: usize = 256;
const MAX_VOLUME_NAME_UNITS: usize = 128;
const MAX_PATH_UNITS: usize = 32_768;
const MAX_MOUNT_ROOTS: usize = 64;
const ERROR_NO_MORE_FILES: u32 = 18;
const ERROR_MORE_DATA: u32 = 234;
const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VolumeRecord {
    pub mount_point: Option<String>,
    pub volume_guid_path: Option<String>,
    pub drive_type: DriveType,
    pub file_system: Option<String>,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub read_only: bool,
    pub assessment: Assessment,
}

#[cfg(windows)]
type Handle = *mut std::ffi::c_void;

#[cfg(windows)]
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn FindFirstVolumeW(name: *mut u16, length: u32) -> Handle;
    fn FindNextVolumeW(search: Handle, name: *mut u16, length: u32) -> i32;
    fn FindVolumeClose(search: Handle) -> i32;
    fn GetVolumePathNamesForVolumeNameW(
        volume_name: *const u16,
        paths: *mut u16,
        length: u32,
        returned: *mut u32,
    ) -> i32;
    fn GetVolumeInformationW(
        root: *const u16,
        volume_name: *mut u16,
        volume_name_size: u32,
        serial: *mut u32,
        max_component: *mut u32,
        flags: *mut u32,
        file_system_name: *mut u16,
        file_system_name_size: u32,
    ) -> i32;
    fn GetDiskFreeSpaceExW(
        path: *const u16,
        available: *mut u64,
        total: *mut u64,
        free: *mut u64,
    ) -> i32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetSystemWindowsDirectoryW(path: *mut u16, size: u32) -> u32;
    fn GetVolumePathNameW(path: *const u16, volume_path: *mut u16, size: u32) -> i32;
    fn GetVolumeNameForVolumeMountPointW(path: *const u16, name: *mut u16, size: u32) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(windows)]
struct SearchHandle(Handle);

#[cfg(windows)]
impl Drop for SearchHandle {
    fn drop(&mut self) {
        unsafe { FindVolumeClose(self.0) };
    }
}

#[cfg(windows)]
fn current(generation: &ScanGeneration, revision: u64) -> Result<(), MetadataError> {
    if generation.revision() == revision {
        Ok(())
    } else {
        Err(MetadataError::IdentityChanged)
    }
}

#[cfg(windows)]
fn wide_string(units: &[u16]) -> Option<String> {
    let end = units.iter().position(|unit| *unit == 0)?;
    if end == 0 {
        return None;
    }
    String::from_utf16(&units[..end]).ok()
}

#[cfg(windows)]
pub(crate) fn valid_volume_guid_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if !path.starts_with("\\\\?\\Volume{") || !path.ends_with("}\\") || bytes.len() != 49 {
        return false;
    }
    let guid = &bytes[11..47];
    guid.len() == 36
        && guid.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(windows)]
fn valid_mount_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\'
        && bytes.last() == Some(&b'\\')
}

#[cfg(windows)]
fn enumerate_volume_names() -> Result<Vec<String>, MetadataError> {
    let mut name = vec![0_u16; MAX_VOLUME_NAME_UNITS];
    let search = unsafe { FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32) };
    if search.is_null() || search == INVALID_HANDLE_VALUE {
        return Err(MetadataError::Win32(unsafe { GetLastError() }));
    }
    let _search = SearchHandle(search);
    let mut names = Vec::new();
    loop {
        let value = wide_string(&name).ok_or(MetadataError::MalformedResponse)?;
        if !valid_volume_guid_path(&value) || names.iter().any(|seen| seen == &value) {
            return Err(MetadataError::MalformedResponse);
        }
        if names.len() == MAX_VOLUMES {
            return Err(MetadataError::LayoutTooLarge);
        }
        names.push(value);
        name.fill(0);
        if unsafe { FindNextVolumeW(search, name.as_mut_ptr(), name.len() as u32) } == 0 {
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(MetadataError::Win32(error));
        }
    }
    Ok(names)
}

#[cfg(windows)]
fn mounted_paths(volume_name: &str) -> Vec<String> {
    let mut volume = volume_name.encode_utf16().collect::<Vec<_>>();
    volume.push(0);
    let mut buffer = vec![0_u16; 1024];
    let mut returned = 0_u32;
    let mut ok = unsafe {
        GetVolumePathNamesForVolumeNameW(
            volume.as_ptr(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            &mut returned,
        )
    };
    if ok == 0 && unsafe { GetLastError() } == ERROR_MORE_DATA {
        if returned == 0 || returned as usize > MAX_PATH_UNITS {
            return Vec::new();
        }
        buffer = vec![0_u16; returned as usize];
        ok = unsafe {
            GetVolumePathNamesForVolumeNameW(
                volume.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut returned,
            )
        };
    }
    if ok == 0 || returned as usize > buffer.len() || returned as usize > MAX_PATH_UNITS {
        return Vec::new();
    }
    let units = &buffer[..returned as usize];
    let mut paths: Vec<String> = Vec::new();
    let mut offset = 0;
    while offset < units.len() {
        if units[offset] == 0 {
            paths.sort_unstable_by_key(|path| path.to_ascii_lowercase());
            return paths;
        }
        let Some(relative_end) = units[offset..].iter().position(|unit| *unit == 0) else {
            return Vec::new();
        };
        let end = offset + relative_end;
        let Ok(path) = String::from_utf16(&units[offset..end]) else {
            return Vec::new();
        };
        if !valid_mount_path(&path)
            || paths
                .iter()
                .any(|seen: &String| seen.eq_ignore_ascii_case(&path))
        {
            return Vec::new();
        }
        paths.push(path);
        if paths.len() > MAX_MOUNT_ROOTS {
            return Vec::new();
        }
        offset = end + 1;
    }
    Vec::new()
}

#[cfg(windows)]
fn system_volume_identity() -> Option<(String, String)> {
    let mut windows_path = vec![0_u16; MAX_PATH_UNITS];
    let length =
        unsafe { GetSystemWindowsDirectoryW(windows_path.as_mut_ptr(), windows_path.len() as u32) }
            as usize;
    if length == 0 || length >= windows_path.len() {
        return None;
    }
    windows_path.truncate(length);
    windows_path.push(0);
    let mut mount_path = vec![0_u16; MAX_PATH_UNITS];
    if unsafe {
        GetVolumePathNameW(
            windows_path.as_ptr(),
            mount_path.as_mut_ptr(),
            mount_path.len() as u32,
        )
    } == 0
    {
        return None;
    }
    let system_root = wide_string(&mount_path)?;
    if !valid_mount_path(&system_root) {
        return None;
    }
    let mut volume_name = vec![0_u16; MAX_VOLUME_NAME_UNITS];
    if unsafe {
        GetVolumeNameForVolumeMountPointW(
            mount_path.as_ptr(),
            volume_name.as_mut_ptr(),
            volume_name.len() as u32,
        )
    } == 0
    {
        return None;
    }
    let value = wide_string(&volume_name)?;
    valid_volume_guid_path(&value).then_some((value, system_root))
}

#[cfg(windows)]
fn drive_type(root: Option<&str>) -> DriveType {
    let Some(root) = root else {
        return DriveType::Unknown;
    };
    let mut wide = root.encode_utf16().collect::<Vec<_>>();
    wide.push(0);
    match unsafe { GetDriveTypeW(wide.as_ptr()) } {
        DRIVE_REMOVABLE => DriveType::Removable,
        DRIVE_FIXED => DriveType::Fixed,
        DRIVE_REMOTE => DriveType::Remote,
        DRIVE_CDROM => DriveType::Cdrom,
        DRIVE_RAMDISK => DriveType::Ramdisk,
        DRIVE_NO_ROOT_DIR => DriveType::NoRoot,
        _ => DriveType::Unknown,
    }
}

#[cfg(windows)]
fn volume_information(volume_name: &str) -> (Option<String>, bool) {
    let mut volume = volume_name.encode_utf16().collect::<Vec<_>>();
    volume.push(0);
    let mut file_system = vec![0_u16; 261];
    let mut flags = 0_u32;
    let ok = unsafe {
        GetVolumeInformationW(
            volume.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut flags,
            file_system.as_mut_ptr(),
            file_system.len() as u32,
        )
    };
    if ok == 0 {
        return (None, false);
    }
    let name = wide_string(&file_system);
    (name, flags & FILE_READ_ONLY_VOLUME != 0)
}

#[cfg(windows)]
fn volume_space(volume_name: &str) -> (Option<u64>, Option<u64>) {
    let mut volume = volume_name.encode_utf16().collect::<Vec<_>>();
    volume.push(0);
    let mut available = 0_u64;
    let mut total = 0_u64;
    let mut free = 0_u64;
    let ok = unsafe { GetDiskFreeSpaceExW(volume.as_ptr(), &mut available, &mut total, &mut free) };
    if ok == 0 {
        (None, None)
    } else {
        (Some(total), Some(available))
    }
}

#[cfg(windows)]
pub(crate) fn inspect(
    disks: &[ProbeOutcome],
    generation: &ScanGeneration,
    revision: u64,
    deadline: Instant,
) -> Result<Vec<VolumeRecord>, MetadataError> {
    let system_volume = system_volume_identity();
    let mappings = match crate::volume_mapping::query(generation, revision, deadline) {
        Ok(mappings) => Some(mappings),
        Err(crate::volume_mapping::MappingError::Unavailable) => None,
        Err(crate::volume_mapping::MappingError::Malformed) => {
            return Err(MetadataError::MalformedResponse);
        }
        Err(crate::volume_mapping::MappingError::TimedOut) => {
            return Err(MetadataError::TimedOut);
        }
        Err(crate::volume_mapping::MappingError::Cancelled) => {
            return Err(MetadataError::IdentityChanged);
        }
    };
    let system_disk_number = mappings.as_deref().and_then(|mappings| {
        let root = &system_volume.as_ref()?.1;
        let mut system_mappings = mappings
            .iter()
            .filter(|mapping| mapping.drive_root.eq_ignore_ascii_case(root));
        let mapping = system_mappings.next()?;
        if system_mappings.next().is_some()
            || !volume_eligibility::mapping_matches_disk_layout(mapping.extent, disks)
        {
            return None;
        }
        Some(mapping.extent.disk_number)
    });
    let names = enumerate_volume_names()?;
    let mut initial_mounts = Vec::new();
    let mut volumes = Vec::with_capacity(names.len());
    for volume_guid_path in &names {
        if Instant::now() >= deadline {
            return Err(MetadataError::TimedOut);
        }
        current(generation, revision)?;
        let mount_roots = mounted_paths(volume_guid_path);
        if mount_roots.is_empty() {
            continue;
        }
        initial_mounts.push((volume_guid_path.clone(), mount_roots.clone()));
        let mut drive_roots = mount_roots
            .iter()
            .filter(|root| volume_eligibility::is_drive_letter_root(root));
        let first_drive_root = drive_roots.next().cloned();
        let mount_point = if drive_roots.next().is_none() {
            first_drive_root
        } else {
            None
        };
        let observed_drive_type = drive_type(mount_point.as_deref());
        let (file_system, read_only) = volume_information(volume_guid_path);
        let (total_bytes, available_bytes) = volume_space(volume_guid_path);
        let extents = mappings
            .as_deref()
            .zip(mount_point.as_deref())
            .map(|(mappings, root)| {
                mappings
                    .iter()
                    .filter(|mapping| mapping.drive_root.eq_ignore_ascii_case(root))
                    .map(|mapping| mapping.extent)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let extents_known = mappings.is_some() && !extents.is_empty();
        current(generation, revision)?;
        let facts = VolumeFacts {
            mount_roots,
            drive_type: observed_drive_type,
            file_system: file_system.clone(),
            read_only,
            total_bytes,
            available_bytes,
            is_system_volume: system_volume
                .as_ref()
                .is_some_and(|(guid, _)| guid == volume_guid_path),
            system_volume_identity_known: system_disk_number.is_some(),
            system_disk_number,
            extents_known,
            extents,
        };
        let assessment = volume_eligibility::assess(&facts, disks);
        volumes.push(VolumeRecord {
            mount_point,
            volume_guid_path: Some(volume_guid_path.clone()),
            drive_type: observed_drive_type,
            file_system,
            total_bytes,
            available_bytes,
            read_only,
            assessment,
        });
    }
    current(generation, revision)?;
    let mut after_names = enumerate_volume_names()?;
    let mut before_names = names;
    before_names.sort_unstable();
    after_names.sort_unstable();
    if before_names != after_names || system_volume_identity() != system_volume {
        return Err(MetadataError::IdentityChanged);
    }
    for (name, roots) in initial_mounts {
        current(generation, revision)?;
        if mounted_paths(&name) != roots {
            return Err(MetadataError::IdentityChanged);
        }
    }
    if mappings.is_some() {
        current(generation, revision)?;
        let after_mappings =
            crate::volume_mapping::query(generation, revision, deadline).map_err(|error| {
                match error {
                    crate::volume_mapping::MappingError::TimedOut => MetadataError::TimedOut,
                    crate::volume_mapping::MappingError::Cancelled => {
                        MetadataError::IdentityChanged
                    }
                    crate::volume_mapping::MappingError::Malformed => {
                        MetadataError::MalformedResponse
                    }
                    crate::volume_mapping::MappingError::Unavailable => {
                        MetadataError::IdentityChanged
                    }
                }
            })?;
        if mappings.as_ref() != Some(&after_mappings) {
            return Err(MetadataError::IdentityChanged);
        }
    }
    current(generation, revision)?;
    Ok(volumes)
}
