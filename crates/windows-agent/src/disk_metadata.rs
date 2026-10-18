//! P2-I2b (first slice): agent-internal, non-elevated metadata-only probes.
//!
//! Interface paths originate ONLY from a fresh SetupAPI enumeration. The
//! renderer cannot submit a path or obtain a handle. No ReadFile, WriteFile,
//! filesystem control, elevation, mount, or disk modification exists here.
//! Even a matching temporary disk number is NOT a durable identity.

use crate::disk_discovery;
use crate::scan_identity::ScanGeneration;
use partition11_core::windows_layout::{
    parse_drive_layout, DriveLayout, LayoutError, ENTRY_BYTES, HEADER_BYTES, MAX_RESPONSE_BYTES,
};
use std::ffi::c_void;
use std::fmt;
use std::ptr;

type Handle = *mut c_void;
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;

const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const FILE_SHARE_DELETE: u32 = 4;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const TOKEN_QUERY: u32 = 8;
const TOKEN_ELEVATION_CLASS: u32 = 20;

const IOCTL_STORAGE_GET_DEVICE_NUMBER: u32 = 0x002D_1080;
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007_405C;
// winioctl.h: CTL_CODE(FILE_DEVICE_DISK, 0x14, METHOD_BUFFERED, FILE_ANY_ACCESS).
// This is GET (metadata only), never IOCTL_DISK_SET_DRIVE_LAYOUT_EX.
const IOCTL_DISK_GET_DRIVE_LAYOUT_EX: u32 = 0x0007_0050;
const ERROR_BAD_LENGTH: u32 = 24;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_MORE_DATA: u32 = 234;
const INITIAL_LAYOUT_SLOTS: usize = 4;

const DISK_DEVICE_TYPE: u32 = 7;
const STORAGE_DEVICE_PROPERTY: u32 = 0;
const STORAGE_ACCESS_ALIGNMENT_PROPERTY: u32 = 6;
const PROPERTY_STANDARD_QUERY: u32 = 0;
const MAX_DESCRIPTOR: usize = 4096;
const MAX_INTERFACE_UNITS: usize = 16 * 1024;

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        filename: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        creation: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn DeviceIoControl(
        device: Handle,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        kind: u32,
        data: *mut c_void,
        size: u32,
        returned: *mut u32,
    ) -> i32;
}

struct OwnedHandle(Handle);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: This owns a real successful CreateFileW/OpenProcessToken
        // handle, not a borrowed/pseudo-handle.
        unsafe { CloseHandle(self.0) };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataError {
    ElevatedProcess,
    AccessDenied,
    DeviceRemoved,
    Unsupported,
    MalformedResponse,
    IdentityChanged,
    LayoutTooLarge,
    Win32(u32),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElevatedProcess => write!(f, "elevated process rejected"),
            Self::AccessDenied => write!(f, "device access denied"),
            Self::DeviceRemoved => write!(f, "device unavailable or removed"),
            Self::Unsupported => write!(f, "metadata query unsupported"),
            Self::MalformedResponse => write!(f, "malformed metadata response"),
            Self::IdentityChanged => write!(f, "transient device identity changed"),
            Self::LayoutTooLarge => write!(f, "partition layout exceeds bounded reader limit"),
            Self::Win32(code) => write!(f, "Win32 error {code}"),
        }
    }
}

fn classify_error(code: u32) -> MetadataError {
    match code {
        5 => MetadataError::AccessDenied,
        2 | 3 | 21 | 55 | 1167 => MetadataError::DeviceRemoved,
        1 | 50 | 87 => MetadataError::Unsupported,
        other => MetadataError::Win32(other),
    }
}

fn last_error() -> MetadataError {
    // SAFETY: Queries the current thread's immediately preceding Win32 error.
    classify_error(unsafe { GetLastError() })
}

fn reject_elevated_process() -> Result<(), MetadataError> {
    let mut token = ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a borrowed pseudo-handle; the token
    // output is a separately owned handle closed by OwnedHandle below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(last_error());
    }
    if token.is_null() || token == INVALID_HANDLE_VALUE {
        return Err(MetadataError::MalformedResponse);
    }
    let token = OwnedHandle(token);
    let mut elevated = 0_u32;
    let mut returned = 0_u32;
    // SAFETY: TokenElevation contains precisely one 32-bit DWORD.
    if unsafe {
        GetTokenInformation(
            token.0,
            TOKEN_ELEVATION_CLASS,
            (&mut elevated as *mut u32).cast(),
            4,
            &mut returned,
        )
    } == 0
    {
        return Err(last_error());
    }
    if returned != 4 || elevated > 1 {
        return Err(MetadataError::MalformedResponse);
    }
    if elevated == 1 {
        return Err(MetadataError::ElevatedProcess);
    }
    Ok(())
}

fn open_agent_discovered_path(path: &[u16]) -> Result<OwnedHandle, MetadataError> {
    if path.is_empty() || path.len() > MAX_INTERFACE_UNITS || path.contains(&0) {
        return Err(MetadataError::MalformedResponse);
    }
    let mut wide = path.to_vec();
    wide.push(0);
    // SAFETY: The only call site uses a freshly SetupAPI-enumerated path.
    // GENERIC_READ grants no write access; OPEN_EXISTING never creates files.
    // Sharing all access avoids exclusive locks on mounted/system devices.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    Ok(OwnedHandle(handle))
}

/// Only fixed, hardcoded metadata IOCTLs are used. There is no arbitrary
/// IOCTL code, user-provided path or sector-read API.
fn ioctl(
    handle: &OwnedHandle,
    code: u32,
    input: &[u8],
    max_output: usize,
) -> Result<Vec<u8>, MetadataError> {
    if max_output == 0 || max_output > MAX_DESCRIPTOR {
        return Err(MetadataError::MalformedResponse);
    }
    let mut bytes = vec![0_u8; max_output];
    let mut returned = 0_u32;
    let input_ptr = if input.is_empty() {
        ptr::null()
    } else {
        input.as_ptr().cast()
    };
    // SAFETY: Inputs are immutable, outputs own initialized writable bytes;
    // buffers and DWORD lengths are bounded, and calls are synchronous.
    let ok = unsafe {
        DeviceIoControl(
            handle.0,
            code,
            input_ptr,
            input.len() as u32,
            bytes.as_mut_ptr().cast(),
            bytes.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(last_error());
    }
    if returned == 0 || returned as usize > bytes.len() {
        return Err(MetadataError::MalformedResponse);
    }
    bytes.truncate(returned as usize);
    Ok(bytes)
}

fn dword(bytes: &[u8], offset: usize) -> Result<u32, MetadataError> {
    let part = bytes
        .get(offset..offset + 4)
        .ok_or(MetadataError::MalformedResponse)?;
    Ok(u32::from_le_bytes(
        part.try_into()
            .map_err(|_| MetadataError::MalformedResponse)?,
    ))
}

fn parse_device_number(bytes: &[u8]) -> Result<u32, MetadataError> {
    if bytes.len() < 12 || dword(bytes, 0)? != DISK_DEVICE_TYPE {
        return Err(MetadataError::MalformedResponse);
    }
    let number = dword(bytes, 4)?;
    if number == u32::MAX {
        return Err(MetadataError::MalformedResponse);
    }
    // PartitionNumber is temporary metadata, not proof of device identity.
    let _partition_number = dword(bytes, 8)?;
    Ok(number)
}

fn parse_disk_length(bytes: &[u8]) -> Result<u64, MetadataError> {
    let raw: [u8; 8] = bytes
        .get(..8)
        .ok_or(MetadataError::MalformedResponse)?
        .try_into()
        .map_err(|_| MetadataError::MalformedResponse)?;
    let length = i64::from_le_bytes(raw);
    if length <= 0 {
        return Err(MetadataError::MalformedResponse);
    }
    Ok(length as u64)
}

fn property_input(property: u32) -> [u8; 12] {
    // STORAGE_PROPERTY_QUERY includes AdditionalParameters[1], plus C padding.
    let mut input = [0_u8; 12];
    input[..4].copy_from_slice(&property.to_le_bytes());
    input[4..8].copy_from_slice(&PROPERTY_STANDARD_QUERY.to_le_bytes());
    input
}

fn descriptor_header(bytes: &[u8], minimum: usize) -> Result<(), MetadataError> {
    let version = dword(bytes, 0)? as usize;
    let declared = dword(bytes, 4)? as usize;
    if bytes.len() < minimum
        || version < minimum
        || declared < minimum
        || declared > bytes.len()
        || version > declared
    {
        return Err(MetadataError::MalformedResponse);
    }
    Ok(())
}

fn sector_size(raw: u32) -> Result<Option<u32>, MetadataError> {
    if raw == 0 {
        return Ok(None);
    }
    if !(512..=65536).contains(&raw) || !raw.is_power_of_two() {
        return Err(MetadataError::MalformedResponse);
    }
    Ok(Some(raw))
}

#[derive(Debug)]
pub(crate) struct SectorSizes {
    pub logical: Option<u32>,
    pub physical: Option<u32>,
}

fn parse_sector_sizes(bytes: &[u8]) -> Result<SectorSizes, MetadataError> {
    descriptor_header(bytes, 28)?;
    let logical = sector_size(dword(bytes, 16)?)?;
    let physical = sector_size(dword(bytes, 20)?)?;
    if let (Some(logical), Some(physical)) = (logical, physical) {
        if physical < logical || physical % logical != 0 {
            return Err(MetadataError::MalformedResponse);
        }
    }
    Ok(SectorSizes { logical, physical })
}

fn parse_bus_type(bytes: &[u8]) -> Result<Option<u32>, MetadataError> {
    descriptor_header(bytes, 36)?;
    let bus = dword(bytes, 28)?;
    // A zero BusTypeUnknown is not a real classification.
    Ok((bus != 0).then_some(bus))
}

#[derive(Debug)]
pub(crate) struct DiskMetadata {
    pub device_number: u32, // OS-local, NOT stable across restart/removal
    pub capacity_bytes: Result<u64, MetadataError>,
    pub sectors: Result<SectorSizes, MetadataError>,
    pub bus_type: Result<Option<u32>, MetadataError>,
    pub partition_layout: Result<DriveLayout, MetadataError>,
}

#[derive(Debug)]
pub(crate) enum ProbeOutcome {
    Ready(DiskMetadata),
    Unavailable(MetadataError),
}

fn next_layout_buffer_size(current: usize) -> Result<usize, MetadataError> {
    if !(HEADER_BYTES + ENTRY_BYTES..MAX_RESPONSE_BYTES).contains(&current) {
        return Err(MetadataError::LayoutTooLarge);
    }
    // Doubling is mandated by the Windows driver contract, but cap the
    // allocation. A disk with >128 entries is explicitly unsupported.
    Ok(current.saturating_mul(2).min(MAX_RESPONSE_BYTES))
}

fn map_layout_error(error: LayoutError) -> MetadataError {
    match error {
        LayoutError::ExcessiveCount => MetadataError::LayoutTooLarge,
        _ => MetadataError::MalformedResponse,
    }
}

/// Query only the agent-owned, non-elevated GENERIC_READ handle.
/// No sector I/O, partition changes, user-provided IOCTL, or renderer API.
fn query_drive_layout(
    handle: &OwnedHandle,
    capacity: u64,
    logical_sector_size: u32,
) -> Result<DriveLayout, MetadataError> {
    let mut limit = HEADER_BYTES + ENTRY_BYTES * INITIAL_LAYOUT_SLOTS;
    loop {
        let mut bytes = vec![0_u8; limit];
        let mut returned = 0_u32;
        // SAFETY: Fixed metadata-only GET IOCTL and a private, initialized,
        // bounded output allocation. Input/overlapped are null; synchronous.
        let ok = unsafe {
            DeviceIoControl(
                handle.0,
                IOCTL_DISK_GET_DRIVE_LAYOUT_EX,
                ptr::null(),
                0,
                bytes.as_mut_ptr().cast(),
                limit as u32,
                &mut returned,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            // SAFETY: Immediately after failed Win32 call.
            let status = unsafe { GetLastError() };
            if matches!(
                status,
                ERROR_BAD_LENGTH | ERROR_INSUFFICIENT_BUFFER | ERROR_MORE_DATA
            ) {
                limit = next_layout_buffer_size(limit)?;
                continue;
            }
            return Err(classify_error(status));
        }
        if returned == 0 || returned as usize > limit {
            return Err(MetadataError::MalformedResponse);
        }
        bytes.truncate(returned as usize);
        return parse_drive_layout(&bytes, capacity, Some(logical_sector_size))
            .map_err(map_layout_error);
    }
}

// The offsets are intentionally mirrored from Windows' C ABI. This checks
// Rust alignment on Windows but is NOT a substitute for a native SDK oracle.
#[cfg(test)]
mod layout_abi {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[repr(C)]
    struct NativePartitionEntry {
        style: u32,
        start: i64,
        length: i64,
        number: u32,
        rewrite: u8,
        service: u8,
        reserved: [u8; 2],
        data: [u64; 14],
    }

    #[repr(C)]
    struct NativeDriveLayout {
        style: u32,
        count: u32,
        header_union: [u64; 5],
        entry: NativePartitionEntry,
    }

    #[test]
    fn expected_layout_and_ioctl_code_match_abi_model() {
        assert_eq!(size_of::<NativePartitionEntry>(), ENTRY_BYTES);
        assert_eq!(offset_of!(NativeDriveLayout, entry), HEADER_BYTES);
        assert_eq!(size_of::<NativeDriveLayout>(), HEADER_BYTES + ENTRY_BYTES);
        assert_eq!(IOCTL_DISK_GET_DRIVE_LAYOUT_EX, 0x0007_0050);
    }

    #[test]
    fn growth_is_bounded_and_never_wraps() {
        let mut size = HEADER_BYTES + ENTRY_BYTES * INITIAL_LAYOUT_SLOTS;
        for _ in 0..10 {
            if size == MAX_RESPONSE_BYTES {
                break;
            }
            let next = next_layout_buffer_size(size).unwrap();
            assert!(next > size && next <= MAX_RESPONSE_BYTES);
            size = next;
        }
        assert_eq!(size, MAX_RESPONSE_BYTES);
        assert_eq!(
            next_layout_buffer_size(size),
            Err(MetadataError::LayoutTooLarge)
        );
        assert_eq!(
            next_layout_buffer_size(0),
            Err(MetadataError::LayoutTooLarge)
        );
        assert_eq!(
            map_layout_error(LayoutError::ExcessiveCount),
            MetadataError::LayoutTooLarge
        );
    }
}

fn query_device_number(handle: &OwnedHandle) -> Result<u32, MetadataError> {
    parse_device_number(&ioctl(handle, IOCTL_STORAGE_GET_DEVICE_NUMBER, &[], 12)?)
}

fn query_one(path: &[u16]) -> ProbeOutcome {
    let result = (|| {
        let handle = open_agent_discovered_path(path)?;
        let before = query_device_number(&handle)?;
        let capacity_bytes =
            ioctl(&handle, IOCTL_DISK_GET_LENGTH_INFO, &[], 8).and_then(|b| parse_disk_length(&b));
        let sectors = ioctl(
            &handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &property_input(STORAGE_ACCESS_ALIGNMENT_PROPERTY),
            28,
        )
        .and_then(|b| parse_sector_sizes(&b));
        let bus_type = ioctl(
            &handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &property_input(STORAGE_DEVICE_PROPERTY),
            MAX_DESCRIPTOR,
        )
        .and_then(|b| parse_bus_type(&b));
        // Fail closed if capacity or logical-sector geometry is unavailable;
        // otherwise ask Windows for bounded partition metadata only.
        let partition_layout = match (&capacity_bytes, &sectors) {
            (Ok(capacity), Ok(sizes)) => match sizes.logical {
                Some(logical) => query_drive_layout(&handle, *capacity, logical),
                None => Err(MetadataError::Unsupported),
            },
            _ => Err(MetadataError::Unsupported),
        };
        // Detect reuse/removal on the same handle, but do NOT treat equality
        // as durable or strong device identity. P2-I2c must solve that.
        let after = query_device_number(&handle)?;
        validate_number_stable(before, after)?;
        Ok(DiskMetadata {
            device_number: before,
            capacity_bytes,
            sectors,
            bus_type,
            partition_layout,
        })
    })();
    match result {
        Ok(metadata) => ProbeOutcome::Ready(metadata),
        Err(err) => ProbeOutcome::Unavailable(err),
    }
}

fn validate_number_stable(before: u32, after: u32) -> Result<(), MetadataError> {
    if before == after {
        Ok(())
    } else {
        Err(MetadataError::IdentityChanged)
    }
}

#[derive(Debug)]
pub(crate) enum ScanError {
    Discovery(disk_discovery::DiscoveryError),
    Security(MetadataError),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(e) => write!(f, "Windows enumeration: {e}"),
            Self::Security(e) => write!(f, "Windows metadata policy: {e}"),
        }
    }
}

/// Return provisional metadata only to the local agent, never to Electron.
/// A later, independent SetupAPI pass rejects paths that disappeared.
pub(crate) fn inspect_discovered_devices() -> Result<Vec<ProbeOutcome>, ScanError> {
    reject_elevated_process().map_err(ScanError::Security)?;
    let before = disk_discovery::enumerate_disk_records().map_err(ScanError::Discovery)?;
    // A scan is short-lived and private to this invocation; never mint
    // persistent disk IDs from PnP paths or temporary disk numbers.
    // Per-invocation guard only. Future PnP notifications must share this
    // generation across concurrent scans; no event subscription exists yet.
    let generation = ScanGeneration::default();
    let snapshot = generation.capture(&before)
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    let results: Vec<_> = before
        .iter()
        .map(|record| query_one(&record.path))
        .collect();
    let after = disk_discovery::enumerate_disk_records().map_err(ScanError::Discovery)?;
    let numbers: Vec<_> = results
        .iter()
        .filter_map(|result| match result {
            ProbeOutcome::Ready(metadata) => Some(metadata.device_number),
            ProbeOutcome::Unavailable(_) => None,
        })
        .collect();
    // Fail the *entire* result on any PnP inventory churn, duplicate path,
    // duplicate temporary device number, or expired scan. Reporting only the
    // surviving entries could present a stale device as a safe selection.
    snapshot
        .validate(&after, &numbers)
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_layout_decoder_rejects_bad_buffer_and_geometry() {
        let size = HEADER_BYTES + ENTRY_BYTES * 4;
        let mut bytes = vec![0_u8; size];
        bytes[..4].copy_from_slice(&0_u32.to_le_bytes()); // MBR
        bytes[4..8].copy_from_slice(&4_u32.to_le_bytes());
        // Four unoccupied slots: valid empty MBR metadata.
        assert!(parse_drive_layout(&bytes, 1024 * 1024, Some(512)).is_ok());
        bytes[4..8].copy_from_slice(&129_u32.to_le_bytes());
        assert_eq!(
            map_layout_error(parse_drive_layout(&bytes, 1024 * 1024, Some(512)).unwrap_err()),
            MetadataError::LayoutTooLarge
        );
    }

    #[test]
    fn metadata_probe_stays_local_and_non_elevated() {
        // Read-only metadata IOCTLs on GitHub Windows VM; no raw sectors.
        match inspect_discovered_devices() {
            Ok(entries) => assert!(entries.len() <= 256),
            Err(ScanError::Security(MetadataError::ElevatedProcess)) => (),
            Err(e) => panic!("unexpected native metadata error: {e}"),
        }
    }

    #[test]
    fn win32_denied_removed_unsupported_are_distinct() {
        assert_eq!(classify_error(5), MetadataError::AccessDenied);
        assert_eq!(classify_error(1167), MetadataError::DeviceRemoved);
        assert_eq!(classify_error(50), MetadataError::Unsupported);
        assert_eq!(classify_error(123), MetadataError::Win32(123));
        assert_eq!(
            validate_number_stable(1, 2),
            Err(MetadataError::IdentityChanged)
        );
        assert!(validate_number_stable(1, 1).is_ok());
    }

    #[test]
    fn device_number_and_length_reject_truncation() {
        assert!(parse_device_number(&[0; 11]).is_err());
        let mut number = [0_u8; 12];
        number[..4].copy_from_slice(&DISK_DEVICE_TYPE.to_le_bytes());
        number[4..8].copy_from_slice(&3_u32.to_le_bytes());
        number[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_device_number(&number).unwrap(), 3);
        number[..4].copy_from_slice(&1_u32.to_le_bytes());
        assert!(parse_device_number(&number).is_err());
        assert!(parse_disk_length(&[0_u8; 7]).is_err());
        assert!(parse_disk_length(&0_i64.to_le_bytes()).is_err());
        assert_eq!(parse_disk_length(&4096_i64.to_le_bytes()).unwrap(), 4096);
    }

    #[test]
    fn sector_descriptors_reject_bad_lengths_and_impossible_geometry() {
        let mut bytes = [0_u8; 28];
        bytes[..4].copy_from_slice(&28_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&28_u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&512_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&4096_u32.to_le_bytes());
        let sectors = parse_sector_sizes(&bytes).unwrap();
        assert_eq!(sectors.logical, Some(512));
        assert_eq!(sectors.physical, Some(4096));
        assert!(parse_sector_sizes(&bytes[..27]).is_err());
        bytes[4..8].copy_from_slice(&1024_u32.to_le_bytes());
        assert!(parse_sector_sizes(&bytes).is_err());
        bytes[4..8].copy_from_slice(&28_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&513_u32.to_le_bytes());
        assert!(parse_sector_sizes(&bytes).is_err());
        bytes[20..24].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(parse_sector_sizes(&bytes).unwrap().physical, None);
    }

    #[test]
    fn bus_descriptor_rejects_short_and_fake_declarations() {
        let mut bytes = [0_u8; 36];
        bytes[..4].copy_from_slice(&36_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&36_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&17_u32.to_le_bytes());
        assert_eq!(parse_bus_type(&bytes).unwrap(), Some(17));
        assert!(parse_bus_type(&bytes[..35]).is_err());
        bytes[4..8].copy_from_slice(&4096_u32.to_le_bytes());
        assert!(parse_bus_type(&bytes).is_err());
    }
}
