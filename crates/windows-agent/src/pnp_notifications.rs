//! Windows P2-I2c: agent-private disk-interface PnP notification guard.
//!
//! Register before SetupAPI enumeration, invalidate the *same* atomic
//! generation observed by this diagnostic scan when Windows announces a
//! disk-interface arrival/removal. The callback performs one atomic write:
//! NO disk operations, blocking calls, allocation, logging or IPC.
//!
//! This is NOT durable hardware identity and does not yet subscribe to
//! per-device HANDLE query-remove events. Tests on hosted Windows runners
//! do not simulate real USB hotplug; physical Windows 11 QA is still needed.

use crate::scan_identity::ScanGeneration;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

// cfgmgr32.h: CM_NOTIFY_FILTER contains a 200-WCHAR DeviceInstance union
// arm, which determines its 400-byte payload size. The independent Windows
// SDK C ABI test checks these exact x64 sizes/offsets at CI time.
#[repr(C)]
struct CmNotifyFilter {
    cb_size: u32,
    flags: u32,
    filter_type: u32,
    reserved: u32,
    payload: [u64; 50],
}

const CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE: u32 = 0;
// The callback invalidates on *all* actions, including unknown actions.
// Numeric action labels are used only in deterministic tests.
#[cfg(test)]
const CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL: u32 = 0;
#[cfg(test)]
const CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL: u32 = 1;
const CR_SUCCESS: u32 = 0;

// Ntddstor.h GUID_DEVINTERFACE_DISK:
// {53F56307-B6BF-11D0-94F2-00A0C91EFB8B}.
fn disk_filter() -> CmNotifyFilter {
    let mut filter = CmNotifyFilter {
        cb_size: size_of::<CmNotifyFilter>() as u32,
        flags: 0,
        filter_type: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
        reserved: 0,
        payload: [0; 50],
    };
    filter.payload[0] = u64::from_le_bytes([0x07, 0x63, 0xF5, 0x53, 0xBF, 0xB6, 0xD0, 0x11]);
    filter.payload[1] = u64::from_le_bytes([0x94, 0xF2, 0x00, 0xA0, 0xC9, 0x1E, 0xFB, 0x8B]);
    filter
}

type NotifyCallback = extern "system" fn(
    notification: *mut c_void,
    context: *mut c_void,
    action: u32,
    event_data: *const c_void,
    event_data_size: u32,
) -> u32;

#[link(name = "cfgmgr32")]
extern "system" {
    fn CM_Register_Notification(
        filter: *const CmNotifyFilter,
        context: *mut c_void,
        callback: Option<NotifyCallback>,
        notification: *mut *mut c_void,
    ) -> u32;
    fn CM_Unregister_Notification(notification: *mut c_void) -> u32;
}

extern "system" fn on_disk_change(
    _notification: *mut c_void,
    context: *mut c_void,
    _action: u32,
    _event_data: *const c_void,
    _event_data_size: u32,
) -> u32 {
    // SAFETY: Context points at a Box<ScanGeneration> kept alive until the
    // synchronous unregister call completes all in-flight callbacks.
    // The callback never dereferences event data or holds a device handle.
    if let Some(generation) = unsafe { (context as *const ScanGeneration).as_ref() } {
        // Even an unrecognized action invalidates the snapshot: fail closed.
        generation.invalidate();
    }
    CR_SUCCESS
}

/// Registered only within a single, non-elevated metadata scan.
/// The subscription is torn down before any aggregate result is returned.
pub(crate) struct DiskNotifications {
    notification: *mut c_void,
    context: Option<Box<ScanGeneration>>,
}

impl DiskNotifications {
    pub(crate) fn register(generation: &ScanGeneration) -> Result<Self, u32> {
        let context = Box::new(generation.clone());
        let context_ptr = (&*context as *const ScanGeneration)
            .cast_mut()
            .cast::<c_void>();
        let mut notification = ptr::null_mut();
        let filter = disk_filter();
        // SAFETY: The SDK filter size/offsets are checked independently in C;
        // context owns the Arc-backed atomic generation, and callback is an
        // extern "system" function that only increments that atomic counter.
        let status = unsafe {
            CM_Register_Notification(
                &filter,
                context_ptr,
                Some(on_disk_change),
                &mut notification,
            )
        };
        if status != CR_SUCCESS {
            return Err(status);
        }
        if notification.is_null() {
            // An impossible "success without handle" cannot be safely
            // unregistered. Leak the context rather than risk callback UAF.
            std::mem::forget(context);
            return Err(u32::MAX);
        }
        Ok(Self {
            notification,
            context: Some(context),
        })
    }

    fn close(&mut self) -> Result<(), u32> {
        if self.notification.is_null() {
            return Ok(());
        }
        // SAFETY: This handle originated from our successful registration.
        // Per Microsoft, never unregister *inside* the callback: this call
        // waits for any outstanding callbacks before the Box may be freed.
        let status = unsafe { CM_Unregister_Notification(self.notification) };
        self.notification = ptr::null_mut();
        if status != CR_SUCCESS {
            // We cannot prove callbacks are quiescent. Leak the small
            // context (one Arc) rather than free memory used by the OS.
            if let Some(context) = self.context.take() {
                std::mem::forget(context);
            }
            return Err(status);
        }
        self.context.take();
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<(), u32> {
        self.close()
    }
}

impl Drop for DiskNotifications {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(path: &str, id: &str) -> crate::disk_discovery::DiskInterface {
        crate::disk_discovery::DiskInterface {
            path: path.encode_utf16().collect(),
            instance_id: id.encode_utf16().collect(),
        }
    }

    #[test]
    fn callback_invalidates_existing_snapshot_without_touching_event_data() {
        let generation = ScanGeneration::default();
        let records = [disk("path", "instance")];
        let snapshot = generation.capture(&records).unwrap();
        assert_eq!(snapshot.validate(&records, &[1]), Ok(()));
        let context = Box::new(generation.clone());
        let pointer = (&*context as *const ScanGeneration)
            .cast_mut()
            .cast::<c_void>();
        assert_eq!(
            on_disk_change(
                ptr::null_mut(),
                pointer,
                CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL,
                ptr::null(),
                0,
            ),
            CR_SUCCESS
        );
        assert!(matches!(
            snapshot.validate(&records, &[1]),
            Err(crate::scan_identity::ScanIdentityError::Cancelled)
        ));
        let newer = generation.capture(&records).unwrap();
        assert_eq!(
            on_disk_change(
                ptr::null_mut(),
                pointer,
                CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL,
                ptr::null(),
                0,
            ),
            CR_SUCCESS
        );
        assert!(matches!(
            newer.validate(&records, &[1]),
            Err(crate::scan_identity::ScanIdentityError::Cancelled)
        ));
    }

    #[test]
    fn notification_filter_is_bounded_and_identifies_disk_class() {
        let filter = disk_filter();
        assert_eq!(size_of::<CmNotifyFilter>(), 416);
        assert_eq!(filter.cb_size, 416);
        assert_eq!(filter.filter_type, 0);
        assert_eq!(filter.flags, 0);
        assert_eq!(
            filter.payload[0].to_le_bytes()[..4],
            [0x07, 0x63, 0xF5, 0x53]
        );
        assert_eq!(filter.payload[1].to_le_bytes()[..2], [0x94, 0xF2]);
    }

    #[test]
    fn device_handle_filter_targets_exact_open_handle() {
        let raw = 0x1234_usize;
        let filter = device_handle_filter(raw);
        assert_eq!(filter.cb_size, size_of::<CmNotifyFilter>() as u32);
        assert_eq!(filter.filter_type, CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE);
        assert_eq!(filter.flags, 0);
        assert_eq!(filter.payload[0], raw as u64);
    }

    #[test]
    fn query_remove_closes_idle_handle_and_invalidates_scan() {
        let generation = ScanGeneration::default();
        let state = DeviceHandleState::for_test(0x1234);
        let before = generation.revision();
        let context = Box::new(DeviceHandleContext::for_test(
            generation.clone(),
            state.clone(),
        ));
        let pointer = (&*context as *const DeviceHandleContext)
            .cast_mut()
            .cast::<c_void>();

        assert_eq!(
            on_device_handle_change(
                ptr::null_mut(),
                pointer,
                CM_NOTIFY_ACTION_DEVICEQUERYREMOVE,
                ptr::null(),
                0,
            ),
            CR_SUCCESS
        );
        assert_eq!(generation.revision(), before + 1);
        assert!(!state.has_open_handle());
    }

    #[test]
    fn busy_query_remove_is_vetoed_then_handle_closes_after_io_releases() {
        let generation = ScanGeneration::default();
        let state = DeviceHandleState::for_test(0x1234);
        let lease = state.begin_io().expect("test handle lease");
        let context = Box::new(DeviceHandleContext::for_test(
            generation.clone(),
            state.clone(),
        ));
        let pointer = (&*context as *const DeviceHandleContext)
            .cast_mut()
            .cast::<c_void>();

        assert_eq!(
            on_device_handle_change(
                ptr::null_mut(),
                pointer,
                CM_NOTIFY_ACTION_DEVICEQUERYREMOVE,
                ptr::null(),
                0,
            ),
            ERROR_CANCELLED
        );
        assert!(state.has_open_handle());
        drop(lease);
        state.finish_io();
        assert!(!state.has_open_handle());
    }

    #[test]
    fn query_remove_failed_never_resurrects_stale_handle_in_same_scan() {
        let generation = ScanGeneration::default();
        let state = DeviceHandleState::for_test(0x1234);
        state.request_close();
        state.finish_io();
        assert!(!state.has_open_handle());

        let context = Box::new(DeviceHandleContext::for_test(
            generation.clone(),
            state.clone(),
        ));
        let pointer = (&*context as *const DeviceHandleContext)
            .cast_mut()
            .cast::<c_void>();
        assert_eq!(
            on_device_handle_change(
                ptr::null_mut(),
                pointer,
                CM_NOTIFY_ACTION_DEVICEQUERYREMOVEFAILED,
                ptr::null(),
                0,
            ),
            CR_SUCCESS
        );
        assert!(!state.has_open_handle());
    }

    #[test]
    fn can_subscribe_and_unsubscribe_on_hosted_windows_without_disk_io() {
        // Hosted CI Windows Server is not a real Windows 11 USB hotplug test.
        let generation = ScanGeneration::default();
        let subscription = DiskNotifications::register(&generation)
            .expect("Windows PnP disk notification subscription");
        subscription.finish().expect("Windows PnP unregister");
    }
}
