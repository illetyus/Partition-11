//! P2-I2a: native SetupAPI-only enumeration of *present* disk interfaces.
//!
//! Uses Windows PnP metadata, not raw disk handles. We deliberately do NOT
//! CreateFile, DeviceIoControl, request UAC, or export paths to Electron.
//! Device-interface strings are transient internal locators, NOT stable
//! device identities. P2-I2b will add read-only metadata queries with
//! identity snapshots, bounded errors and independent acceptance.
//!
//! SAFETY: The only permitted unsafe operations in the Windows agent live
//! in this module. All FFI structs follow the Windows C ABI, handles are
//! released by RAII, and the variable UTF-16 result is strictly bounded.

use std::ffi::c_void;
use std::fmt;
use std::mem::size_of;
use std::ptr;

const DIGCF_PRESENT: u32 = 0x0000_0002;
const DIGCF_DEVICEINTERFACE: u32 = 0x0000_0010;
const ERROR_NO_MORE_ITEMS: u32 = 259;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const MAX_DISK_INTERFACES: u32 = 256;
const MAX_DETAIL_BYTES: usize = 32 * 1024;
const MAX_INSTANCE_UNITS: u32 = 2048;

// Ntddstor.h GUID_DEVINTERFACE_DISK: {53F56307-B6BF-11D0-94F2-00A0C91EFB8B}
#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const DISK_GUID: Guid = Guid {
    data1: 0x53F5_6307,
    data2: 0xB6BF,
    data3: 0x11D0,
    data4: [0x94, 0xF2, 0x00, 0xA0, 0xC9, 0x1E, 0xFB, 0x8B],
};

#[repr(C)]
struct DeviceInterfaceData {
    cb_size: u32,
    interface_class_guid: Guid,
    flags: u32,
    reserved: usize,
}

impl DeviceInterfaceData {
    fn new() -> Self {
        Self {
            cb_size: size_of::<Self>() as u32,
            interface_class_guid: DISK_GUID,
            flags: 0,
            reserved: 0,
        }
    }
}

/// setupapi.h SP_DEVINFO_DATA; size/alignment also checked against SDK C CI.
#[repr(C)]
struct DeviceInfoData {
    cb_size: u32,
    class_guid: Guid,
    dev_inst: u32,
    reserved: usize,
}

impl DeviceInfoData {
    fn new() -> Self {
        Self {
            cb_size: size_of::<Self>() as u32,
            class_guid: DISK_GUID,
            dev_inst: 0,
            reserved: 0,
        }
    }
}

/// An internal-only PnP path + Windows device instance-id observation.
/// The instance ID is NOT a hardware-authenticated identifier or proof that
/// an unplugged disk wasn't replaced with another disk on the same port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiskInterface {
    pub path: Vec<u16>,
    pub instance_id: Vec<u16>,
}

type Hdevinfo = *mut c_void;
const INVALID_HANDLE_VALUE: Hdevinfo = -1_isize as Hdevinfo;

#[link(name = "setupapi")]
extern "system" {
    fn SetupDiGetClassDevsW(
        class_guid: *const Guid,
        enumerator: *const u16,
        parent: *mut c_void,
        flags: u32,
    ) -> Hdevinfo;
    fn SetupDiEnumDeviceInterfaces(
        set: Hdevinfo,
        device: *mut c_void,
        class_guid: *const Guid,
        member_index: u32,
        interface_data: *mut DeviceInterfaceData,
    ) -> i32;
    fn SetupDiGetDeviceInterfaceDetailW(
        set: Hdevinfo,
        interface_data: *mut DeviceInterfaceData,
        detail_data: *mut c_void,
        detail_size: u32,
        required_size: *mut u32,
        device_info_data: *mut c_void,
    ) -> i32;
    fn SetupDiGetDeviceInstanceIdW(
        set: Hdevinfo,
        device_info_data: *mut DeviceInfoData,
        instance_id: *mut u16,
        capacity_chars: u32,
        required_chars: *mut u32,
    ) -> i32;
    fn SetupDiDestroyDeviceInfoList(set: Hdevinfo) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
}

struct DeviceInfoSet(Hdevinfo);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        // SAFETY: Non-null, non-INVALID handle came from SetupDiGetClassDevsW.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

#[derive(Debug)]
pub(crate) enum DiscoveryError {
    Win32 { step: &'static str, code: u32 },
    UnexpectedResponse,
    LimitExceeded,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Win32 { step, code } => write!(f, "{step}: Win32 error {code}"),
            Self::UnexpectedResponse => write!(f, "invalid bounded SetupAPI response"),
            Self::LimitExceeded => write!(f, "too many disk interfaces for a bounded scan"),
        }
    }
}

/// Decode a WCHAR path from an 8-byte-aligned SetupAPI detail result.
/// The string starts at byte offset 4, regardless of struct padding.
/// The returned units omit NUL; never publish them through the UI.
fn copy_detail_path(words: &[u64], actual_bytes: usize) -> Result<Vec<u16>, DiscoveryError> {
    if !(6..=MAX_DETAIL_BYTES).contains(&actual_bytes) {
        return Err(DiscoveryError::UnexpectedResponse);
    }
    let capacity_bytes = words
        .len()
        .checked_mul(8)
        .ok_or(DiscoveryError::UnexpectedResponse)?;
    if actual_bytes > capacity_bytes {
        return Err(DiscoveryError::UnexpectedResponse);
    }
    let count = (actual_bytes - 4) / 2;
    // SAFETY: Vec<u64> is 8-byte aligned; +4 remains WCHAR-aligned;
    // count is derived from and constrained by the initialized allocation.
    let units = unsafe {
        std::slice::from_raw_parts((words.as_ptr() as *const u8).add(4) as *const u16, count)
    };
    let end = units
        .iter()
        .position(|&u| u == 0)
        .ok_or(DiscoveryError::UnexpectedResponse)?;
    if end == 0 {
        return Err(DiscoveryError::UnexpectedResponse);
    }
    Ok(units[..end].to_vec())
}

/// A two-pass, bounded native Windows SDK instance ID query.
fn get_device_instance_id(
    set: Hdevinfo,
    info: &mut DeviceInfoData,
) -> Result<Vec<u16>, DiscoveryError> {
    let mut needed = 0_u32;
    // SAFETY: DeviceInfoData is initialized to the matching C struct
    // layout and belongs to the live SetupAPI information set.
    let ok = unsafe { SetupDiGetDeviceInstanceIdW(set, info, ptr::null_mut(), 0, &mut needed) };
    let status = if ok == 0 {
        // SAFETY: GetLastError is sampled immediately after the failing API.
        unsafe { GetLastError() }
    } else {
        0
    };
    if ok != 0 || status != ERROR_INSUFFICIENT_BUFFER {
        return Err(DiscoveryError::Win32 {
            step: "SetupDiGetDeviceInstanceIdW/size",
            code: status,
        });
    }
    if !(2..=MAX_INSTANCE_UNITS).contains(&needed) {
        return Err(DiscoveryError::UnexpectedResponse);
    }
    let mut units = vec![0_u16; needed as usize];
    let mut returned = 0_u32;
    // SAFETY: The buffer is allocated for 'needed' WCHARs, and Win32
    // receives the same character count, not a byte count.
    let ok = unsafe {
        SetupDiGetDeviceInstanceIdW(set, info, units.as_mut_ptr(), needed, &mut returned)
    };
    if ok == 0 {
        // SAFETY: Error sampled immediately after failed Win32 call.
        return Err(DiscoveryError::Win32 {
            step: "SetupDiGetDeviceInstanceIdW/data",
            code: unsafe { GetLastError() },
        });
    }
    parse_instance_id(&units, returned as usize)
}

/// Verify exactly one nonempty, NUL-terminated UTF-16 instance ID.
fn parse_instance_id(units: &[u16], returned: usize) -> Result<Vec<u16>, DiscoveryError> {
    if !(2..=MAX_INSTANCE_UNITS as usize).contains(&returned)
        || returned > units.len()
        || units[returned - 1] != 0
        || units[..returned - 1].contains(&0)
    {
        return Err(DiscoveryError::UnexpectedResponse);
    }
    Ok(units[..returned - 1].to_vec())
}

/// Enumerate paired PnP locators and device instance IDs while the same
/// SetupAPI device set is alive. Does not open physical disks.
pub(crate) fn enumerate_disk_records() -> Result<Vec<DiskInterface>, DiscoveryError> {
    // SAFETY: The GUID has the Windows layout, optional pointers are NULL,
    // flags request only present device-interface metadata.
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &DISK_GUID,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        // SAFETY: Called immediately after failed Windows API, no intervening FFI.
        return Err(DiscoveryError::Win32 {
            step: "SetupDiGetClassDevsW",
            code: unsafe { GetLastError() },
        });
    }
    let set = DeviceInfoSet(raw);
    let mut interfaces = Vec::new();

    // Include a sentinel iteration so that >MAX_DISK_INTERFACES fails closed.
    for index in 0..=MAX_DISK_INTERFACES {
        let mut data = DeviceInterfaceData::new();
        // SAFETY: Valid RAII-held set, C ABI struct and output pointer.
        let found = unsafe {
            SetupDiEnumDeviceInterfaces(set.0, ptr::null_mut(), &DISK_GUID, index, &mut data)
        };
        if found == 0 {
            // SAFETY: Called immediately after failed enumeration.
            let code = unsafe { GetLastError() };
            if code == ERROR_NO_MORE_ITEMS {
                return Ok(interfaces);
            }
            return Err(DiscoveryError::Win32 {
                step: "SetupDiEnumDeviceInterfaces",
                code,
            });
        }
        if index == MAX_DISK_INTERFACES {
            return Err(DiscoveryError::LimitExceeded);
        }
        let mut needed = 0_u32;
        // SAFETY: Standard two-pass SetupAPI call; NULL output and zero bytes.
        let first = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &mut data,
                ptr::null_mut(),
                0,
                &mut needed,
                ptr::null_mut(),
            )
        };
        // SAFETY: Error code sampled immediately after the call.
        let status = if first == 0 {
            unsafe { GetLastError() }
        } else {
            0
        };
        if first != 0 || status != ERROR_INSUFFICIENT_BUFFER {
            return Err(DiscoveryError::Win32 {
                step: "SetupDiGetDeviceInterfaceDetailW/size",
                code: status,
            });
        }
        if !(6..=MAX_DETAIL_BYTES).contains(&(needed as usize)) {
            return Err(DiscoveryError::UnexpectedResponse);
        }
        // Vec<u64> guarantees at least 8-byte alignment for the variable
        // detail buffer. DWORD cbSize is 8 for x64, 6 for x86 (Unicode).
        let words_len = (needed as usize).div_ceil(8);
        let mut words = vec![0_u64; words_len];
        let cb_size = if size_of::<usize>() == 8 {
            8_u32
        } else {
            6_u32
        };
        // SAFETY: Allocated 8-byte aligned, >=6 bytes; first DWORD writable.
        unsafe { (words.as_mut_ptr() as *mut u32).write(cb_size) };
        let mut returned = 0_u32;
        let mut info = DeviceInfoData::new();
        // SAFETY: Win32 receives a byte-buffer pointer to an initialized,
        // properly aligned writable allocation of size >= needed.
        let ok = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &mut data,
                words.as_mut_ptr().cast(),
                needed,
                &mut returned,
                (&mut info as *mut DeviceInfoData).cast(),
            )
        };
        if ok == 0 {
            // SAFETY: Error code sampled immediately after failure.
            return Err(DiscoveryError::Win32 {
                step: "SetupDiGetDeviceInterfaceDetailW/data",
                code: unsafe { GetLastError() },
            });
        }
        if !(6..=needed).contains(&returned) {
            return Err(DiscoveryError::UnexpectedResponse);
        }
        let path = copy_detail_path(&words, returned as usize)?;
        let instance_id = get_device_instance_id(set.0, &mut info)?;
        interfaces.push(DiskInterface { path, instance_id });
    }

    Err(DiscoveryError::LimitExceeded)
}

/// Preserve count-only legacy diagnostic, but require the PnP instance
/// retrieval to succeed before acknowledging a discovered device.
pub(crate) fn enumerate_disk_interfaces() -> Result<Vec<Vec<u16>>, DiscoveryError> {
    Ok(enumerate_disk_records()?
        .into_iter()
        .map(|record| record.path)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_detail_buffer_rejects_invalid_lengths_and_missing_nul() {
        let words = [0_u64; 2];
        assert!(copy_detail_path(&words, 0).is_err());
        assert!(copy_detail_path(&words, 100).is_err());
        assert!(copy_detail_path(&words, MAX_DETAIL_BYTES + 1).is_err());

        let mut missing_nul = [u64::MAX; 2];
        // Valid cbSize, remaining WCHAR data has no NUL.
        missing_nul[0] = (missing_nul[0] & !0xffff_ffff_u64) | 8;
        assert!(copy_detail_path(&missing_nul, 12).is_err());
    }

    #[test]
    fn instance_id_parser_rejects_missing_or_embedded_terminator() {
        assert_eq!(parse_instance_id(&[65, 66, 0], 3).unwrap(), vec![65, 66]);
        assert!(parse_instance_id(&[0], 1).is_err());
        assert!(parse_instance_id(&[65, 66], 2).is_err());
        assert!(parse_instance_id(&[65, 0, 66, 0], 4).is_err());
        assert!(parse_instance_id(&[65, 0], 3).is_err());
    }

    #[test]
    fn windows_setupapi_enumeration_is_bounded_and_non_mutating() {
        // CI Windows Server VM check, NOT physical Windows 11 acceptance.
        // No disk handles are opened and no raw data is accessed.
        let devices = enumerate_disk_records().expect("SetupAPI disk interface enumeration");
        assert!(devices.len() <= MAX_DISK_INTERFACES as usize);
        assert!(devices
            .iter()
            .all(|d| !d.path.is_empty() && !d.instance_id.is_empty()));
        let unique: std::collections::HashSet<_> = devices.iter().map(|d| &d.path).collect();
        assert_eq!(unique.len(), devices.len());
    }
}
