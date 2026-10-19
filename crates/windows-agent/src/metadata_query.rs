//! Fixed metadata-only disk query engine owned by the disposable worker.
//!
//! Device paths are accepted only after the worker freshly matches the exact
//! SetupAPI path and instance ID. This module exposes no raw read/write API.

use crate::disk_discovery::DiskInterface;
use crate::disk_metadata::{DiskMetadata, MetadataError, ProbeOutcome};
use crate::overlapped_ioctl::{run_worker_ioctl, OverlappedIoError};
use crate::scan_identity::ScanGeneration;
use partition11_core::windows_layout::{
    parse_drive_layout, DriveLayout, LayoutError, ENTRY_BYTES, HEADER_BYTES, MAX_RESPONSE_BYTES,
};
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

type Handle = *mut c_void;
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;
const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const FILE_SHARE_DELETE: u32 = 4;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const TOKEN_QUERY: u32 = 8;
const TOKEN_ELEVATION_CLASS: u32 = 20;
const IOCTL_STORAGE_GET_DEVICE_NUMBER: u32 = 0x002D_1080;
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007_405C;
// Fixed GET-only metadata IOCTL; never IOCTL_DISK_SET_DRIVE_LAYOUT_EX.
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
        // SAFETY: This type is now used only for OpenProcessToken. Device
        // handles use RegisteredDeviceHandle below so PnP can close them.
        unsafe { CloseHandle(self.0) };
    }
}

struct RegisteredDeviceHandle {
    state: Arc<crate::pnp_notifications::DeviceHandleState>,
    notifications: Option<crate::pnp_notifications::DeviceHandleNotifications>,
}

impl RegisteredDeviceHandle {
    fn from_raw(raw_handle: Handle, generation: &ScanGeneration) -> Result<Self, MetadataError> {
        let (state, notifications) = crate::pnp_notifications::DeviceHandleNotifications::register(
            raw_handle as usize,
            generation,
        )
        .map_err(|_| MetadataError::IdentityChanged)?;
        Ok(Self {
            state,
            notifications: Some(notifications),
        })
    }

    fn with_raw<T>(
        &self,
        operation: impl FnOnce(Handle) -> Result<T, MetadataError>,
    ) -> Result<T, MetadataError> {
        let lease = self
            .state
            .begin_io()
            .map_err(|_| MetadataError::DeviceRemoved)?;
        let raw_handle = lease.raw_handle() as Handle;
        let result = operation(raw_handle);
        drop(lease);
        // If a callback raced this synchronous operation, it only requested
        // close while the lease existed. Close immediately after release.
        self.state.finish_io();
        result
    }

    fn finish(mut self) -> Result<(), MetadataError> {
        let unregister = if let Some(notifications) = self.notifications.take() {
            notifications.finish()
        } else {
            Ok(())
        };
        self.state.request_close();
        self.state.finish_io();
        unregister.map_err(|_| MetadataError::IdentityChanged)
    }
}

impl Drop for RegisteredDeviceHandle {
    fn drop(&mut self) {
        if let Some(notifications) = self.notifications.take() {
            let _ = notifications.finish();
        }
        self.state.request_close();
        self.state.finish_io();
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

pub(crate) fn reject_elevated_process() -> Result<(), MetadataError> {
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

fn open_agent_discovered_path(
    path: &[u16],
    generation: &ScanGeneration,
) -> Result<RegisteredDeviceHandle, MetadataError> {
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
            metadata_open_flags(),
            ptr::null_mut(),
        )
    };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    // The raw handle is transferred to the per-HANDLE PnP state. If
    // registration fails, that state closes it before this function returns.
    RegisteredDeviceHandle::from_raw(handle, generation)
}

/// Only fixed metadata IOCTLs can enter the disposable worker.
fn ioctl(
    handle: &RegisteredDeviceHandle,
    code: u32,
    input: &[u8],
    max_output: usize,
) -> Result<Vec<u8>, MetadataError> {
    if !is_fixed_metadata_ioctl(code)
        || input.len() > 12
        || max_output == 0
        || max_output > MAX_RESPONSE_BYTES
    {
        return Err(MetadataError::MalformedResponse);
    }
    let bytes = handle.with_raw(|raw_handle| {
        run_worker_ioctl(raw_handle as usize, code, input, max_output, || {
            handle.state.io_cancel_requested()
        })
        .map_err(map_overlapped_error)
    })?;
    if bytes.is_empty() || bytes.len() > max_output {
        return Err(MetadataError::MalformedResponse);
    }
    Ok(bytes)
}

fn is_fixed_metadata_ioctl(code: u32) -> bool {
    matches!(
        code,
        IOCTL_STORAGE_GET_DEVICE_NUMBER
            | IOCTL_DISK_GET_LENGTH_INFO
            | IOCTL_STORAGE_QUERY_PROPERTY
            | IOCTL_DISK_GET_DRIVE_LAYOUT_EX
    )
}

fn metadata_open_flags() -> u32 {
    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED
}

fn map_overlapped_error(error: OverlappedIoError) -> MetadataError {
    match error {
        OverlappedIoError::TimedOut => MetadataError::TimedOut,
        OverlappedIoError::Cancelled => MetadataError::Cancelled,
        OverlappedIoError::MalformedResponse => MetadataError::MalformedResponse,
        OverlappedIoError::Win32(code) => classify_error(code),
    }
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

/// Bounded retry helper: only the three documented size statuses grow the buffer.
fn query_drive_layout_with(
    capacity: u64,
    logical_sector_size: u32,
    mut request: impl FnMut(usize) -> Result<Vec<u8>, MetadataError>,
) -> Result<DriveLayout, MetadataError> {
    let mut limit = HEADER_BYTES + ENTRY_BYTES * INITIAL_LAYOUT_SLOTS;
    loop {
        match request(limit) {
            Ok(bytes) => {
                if bytes.is_empty() || bytes.len() > limit {
                    return Err(MetadataError::MalformedResponse);
                }
                return parse_drive_layout(&bytes, capacity, Some(logical_sector_size))
                    .map_err(map_layout_error);
            }
            Err(MetadataError::Win32(
                ERROR_BAD_LENGTH | ERROR_INSUFFICIENT_BUFFER | ERROR_MORE_DATA,
            )) => {
                limit = next_layout_buffer_size(limit)?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn query_drive_layout(
    handle: &RegisteredDeviceHandle,
    capacity: u64,
    logical_sector_size: u32,
    epoch: &ProbeEpoch<'_>,
) -> Result<DriveLayout, MetadataError> {
    query_drive_layout_with(capacity, logical_sector_size, |limit| {
        epoch.ensure_current()?;
        let bytes = ioctl(handle, IOCTL_DISK_GET_DRIVE_LAYOUT_EX, &[], limit)?;
        epoch.ensure_current()?;
        Ok(bytes)
    })
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

struct ProbeEpoch<'a> {
    generation: &'a ScanGeneration,
    revision: u64,
}

impl<'a> ProbeEpoch<'a> {
    fn capture(generation: &'a ScanGeneration) -> Self {
        Self {
            generation,
            revision: generation.revision(),
        }
    }

    fn ensure_current(&self) -> Result<(), MetadataError> {
        if self.generation.revision() != self.revision {
            Err(MetadataError::IdentityChanged)
        } else {
            Ok(())
        }
    }

    fn run<T>(
        &self,
        operation: impl FnOnce() -> Result<T, MetadataError>,
    ) -> Result<T, MetadataError> {
        self.ensure_current()?;
        let result = operation();
        // A notification during an I/O call overrides both its success and
        // an optional "unsupported" result: we must not use stale metadata.
        self.ensure_current()?;
        result
    }
}

fn query_device_number(handle: &RegisteredDeviceHandle) -> Result<u32, MetadataError> {
    parse_device_number(&ioctl(handle, IOCTL_STORAGE_GET_DEVICE_NUMBER, &[], 12)?)
}

fn retain_optional_query<T>(
    result: Result<T, MetadataError>,
) -> Result<Result<T, MetadataError>, MetadataError> {
    match result {
        Err(
            error @ (MetadataError::TimedOut
            | MetadataError::Cancelled
            | MetadataError::IdentityChanged),
        ) => Err(error),
        other => Ok(other),
    }
}

fn query_one(path: &[u16], epoch: &ProbeEpoch<'_>) -> ProbeOutcome {
    let result = (|| {
        // Check both before and after CreateFileW: an event racing with open
        // must invalidate the new handle before any metadata IOCTL begins.
        let handle = epoch.run(|| open_agent_discovered_path(path, epoch.generation))?;
        let before = epoch.run(|| query_device_number(&handle))?;
        let capacity_bytes = retain_optional_query(epoch.run(|| {
            ioctl(&handle, IOCTL_DISK_GET_LENGTH_INFO, &[], 8).and_then(|b| parse_disk_length(&b))
        }))?;
        epoch.ensure_current()?;
        let sectors = retain_optional_query(epoch.run(|| {
            ioctl(
                &handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &property_input(STORAGE_ACCESS_ALIGNMENT_PROPERTY),
                28,
            )
            .and_then(|b| parse_sector_sizes(&b))
        }))?;
        epoch.ensure_current()?;
        let bus_type = retain_optional_query(epoch.run(|| {
            ioctl(
                &handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &property_input(STORAGE_DEVICE_PROPERTY),
                MAX_DESCRIPTOR,
            )
            .and_then(|b| parse_bus_type(&b))
        }))?;
        epoch.ensure_current()?;
        // Fail closed if capacity or logical-sector geometry is unavailable;
        // otherwise ask Windows for bounded partition metadata only.
        let partition_layout = retain_optional_query(match (&capacity_bytes, &sectors) {
            (Ok(capacity), Ok(sizes)) => match sizes.logical {
                Some(logical) => {
                    epoch.run(|| query_drive_layout(&handle, *capacity, logical, epoch))
                }
                None => Err(MetadataError::Unsupported),
            },
            _ => Err(MetadataError::Unsupported),
        })?;
        epoch.ensure_current()?;
        // Same-handle number matching cannot prove hardware identity.
        let after = epoch.run(|| query_device_number(&handle))?;
        validate_number_stable(before, after)?;
        epoch.ensure_current()?;
        let metadata = DiskMetadata {
            device_number: before,
            capacity_bytes,
            sectors,
            bus_type,
            partition_layout,
        };
        // Unregister handle notifications off-callback, then close the handle.
        // A callback racing unregister invalidates the epoch and is observed
        // by the final check below.
        handle.finish()?;
        epoch.ensure_current()?;
        Ok(metadata)
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

pub(crate) fn query_worker_record(
    record: &DiskInterface,
    generation: &ScanGeneration,
) -> ProbeOutcome {
    let epoch = ProbeEpoch::capture(generation);
    if let Err(error) = epoch.ensure_current() {
        return ProbeOutcome::Unavailable(error);
    }
    query_one(&record.path, &epoch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlapped_ioctl::OverlappedIoError;
    use std::cell::Cell;

    #[test]
    fn cancelled_epoch_never_starts_a_later_device_query() {
        let generation = ScanGeneration::default();
        let epoch = ProbeEpoch::capture(&generation);
        generation.invalidate();
        let ran = Cell::new(false);
        assert_eq!(
            epoch.run(|| {
                ran.set(true);
                Ok::<u32, MetadataError>(7)
            }),
            Err(MetadataError::IdentityChanged)
        );
        assert!(!ran.get(), "cancelled scan must not issue more Win32 I/O");
    }

    #[test]
    fn event_during_query_discards_even_successful_or_optional_results() {
        let generation = ScanGeneration::default();
        let first = ProbeEpoch::capture(&generation);
        assert_eq!(
            first.run(|| {
                generation.invalidate();
                Ok::<u32, MetadataError>(123)
            }),
            Err(MetadataError::IdentityChanged)
        );

        let second = ProbeEpoch::capture(&generation);
        assert_eq!(
            second.run(|| {
                generation.invalidate();
                Err::<u32, MetadataError>(MetadataError::Unsupported)
            }),
            Err(MetadataError::IdentityChanged)
        );
        let fresh = ProbeEpoch::capture(&generation);
        assert_eq!(fresh.run(|| Ok::<u32, MetadataError>(9)), Ok(9));
    }

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

    #[test]
    fn worker_open_uses_overlapped_flag_constant() {
        assert_ne!(metadata_open_flags() & FILE_FLAG_OVERLAPPED, 0);
        assert_eq!(
            metadata_open_flags(),
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED
        );
    }

    #[test]
    fn only_fixed_metadata_ioctl_codes_can_enter_query_function() {
        assert!(is_fixed_metadata_ioctl(IOCTL_STORAGE_GET_DEVICE_NUMBER));
        assert!(is_fixed_metadata_ioctl(IOCTL_DISK_GET_LENGTH_INFO));
        assert!(is_fixed_metadata_ioctl(IOCTL_STORAGE_QUERY_PROPERTY));
        assert!(is_fixed_metadata_ioctl(IOCTL_DISK_GET_DRIVE_LAYOUT_EX));
        assert!(!is_fixed_metadata_ioctl(0));
        assert!(!is_fixed_metadata_ioctl(0x0007_0004));
    }

    #[test]
    fn pnp_cancellation_maps_to_metadata_cancelled() {
        assert_eq!(
            map_overlapped_error(OverlappedIoError::Cancelled),
            MetadataError::Cancelled
        );
    }

    #[test]
    fn ioctl_timeout_maps_to_metadata_timed_out() {
        assert_eq!(
            map_overlapped_error(OverlappedIoError::TimedOut),
            MetadataError::TimedOut
        );
    }

    #[test]
    fn bounded_layout_retry_stops_after_cancellation() {
        let mut calls = 0;
        let result = query_drive_layout_with(1_048_576, 512, |_| {
            calls += 1;
            match calls {
                1 => Err(MetadataError::Win32(ERROR_INSUFFICIENT_BUFFER)),
                2 => Err(MetadataError::Cancelled),
                _ => panic!("layout retry issued after cancellation"),
            }
        });
        assert_eq!(result, Err(MetadataError::Cancelled));
        assert_eq!(calls, 2);
    }

    #[test]
    fn timeout_stops_remaining_device_queries() {
        let mut later_query_started = false;
        let result = retain_optional_query::<u64>(Err(MetadataError::TimedOut)).map(|_| {
            later_query_started = true;
        });
        assert_eq!(result, Err(MetadataError::TimedOut));
        assert!(!later_query_started);
    }
}
