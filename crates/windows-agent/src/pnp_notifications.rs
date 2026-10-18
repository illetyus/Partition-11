//! Windows P2-I2c: agent-private disk-interface PnP notification guard.
//!
//! Register before SetupAPI enumeration, invalidate the *same* atomic
//! generation observed by this diagnostic scan when Windows announces a
//! disk-interface arrival/removal. The callback performs one atomic write:
//! NO disk operations, blocking calls, allocation, logging or IPC.
//!
//! The scan also registers each successfully opened device HANDLE for
//! query-remove notifications. Idle handles are closed in the callback as
//! Windows requires. If a synchronous metadata call currently owns a handle,
//! query-remove is vetoed (ERROR_CANCELLED), the scan is invalidated, and the
//! handle is closed immediately after that call returns. This deliberately
//! leaves forced cancellation of a blocked DeviceIoControl to the next phase.
//!
//! This is NOT durable hardware identity. Tests on hosted Windows runners do
//! not simulate real USB hotplug; physical Windows 11 QA is still needed.

use crate::scan_identity::ScanGeneration;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

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
const CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE: u32 = 1;
// The callback invalidates on *all* actions, including unknown actions.
// Numeric action labels are used only in deterministic tests.
#[cfg(test)]
const CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL: u32 = 0;
#[cfg(test)]
const CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL: u32 = 1;
const CM_NOTIFY_ACTION_DEVICEQUERYREMOVE: u32 = 2;
const CM_NOTIFY_ACTION_DEVICEQUERYREMOVEFAILED: u32 = 3;
const CM_NOTIFY_ACTION_DEVICEREMOVEPENDING: u32 = 4;
const CM_NOTIFY_ACTION_DEVICEREMOVECOMPLETE: u32 = 5;
const CR_SUCCESS: u32 = 0;
const ERROR_CANCELLED: u32 = 1223;

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

fn device_handle_filter(raw_handle: usize) -> CmNotifyFilter {
    let mut filter = CmNotifyFilter {
        cb_size: size_of::<CmNotifyFilter>() as u32,
        flags: 0,
        filter_type: CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE,
        reserved: 0,
        payload: [0; 50],
    };
    // Windows x64 HANDLE is pointer-sized; the independent SDK C oracle
    // verifies that DeviceHandle.hTarget begins at the union offset (16).
    filter.payload[0] = raw_handle as u64;
    filter
}

type CloseHandleFn = fn(usize) -> bool;

fn native_close_handle(raw_handle: usize) -> bool {
    // SAFETY: Callers transfer an owned successful CreateFileW handle into
    // DeviceHandleState exactly once. The state serializes all close attempts.
    unsafe { CloseHandle(raw_handle as *mut c_void) != 0 }
}

pub(crate) struct DeviceHandleState {
    handle: Mutex<Option<usize>>,
    close_requested: AtomicBool,
    close_handle: CloseHandleFn,
}

pub(crate) struct DeviceIoLease<'a> {
    guard: MutexGuard<'a, Option<usize>>,
}

impl DeviceIoLease<'_> {
    pub(crate) fn raw_handle(&self) -> usize {
        self.guard
            .as_ref()
            .copied()
            .expect("DeviceIoLease always contains an open handle")
    }
}

impl DeviceHandleState {
    fn new(raw_handle: usize) -> Arc<Self> {
        Arc::new(Self {
            handle: Mutex::new(Some(raw_handle)),
            close_requested: AtomicBool::new(false),
            close_handle: native_close_handle,
        })
    }

    #[cfg(test)]
    fn for_test(raw_handle: usize) -> Arc<Self> {
        Arc::new(Self {
            handle: Mutex::new(Some(raw_handle)),
            close_requested: AtomicBool::new(false),
            close_handle: |_| true,
        })
    }

    pub(crate) fn begin_io(&self) -> Result<DeviceIoLease<'_>, ()> {
        let guard = self.handle.lock().map_err(|_| ())?;
        if guard.is_none() {
            return Err(());
        }
        Ok(DeviceIoLease { guard })
    }

    pub(crate) fn request_close(&self) {
        self.close_requested.store(true, Ordering::Release);
    }

    fn close_locked(&self, slot: &mut Option<usize>) -> bool {
        let Some(raw_handle) = slot.take() else {
            self.close_requested.store(false, Ordering::Release);
            return true;
        };
        if (self.close_handle)(raw_handle) {
            self.close_requested.store(false, Ordering::Release);
            true
        } else {
            *slot = Some(raw_handle);
            self.close_requested.store(true, Ordering::Release);
            false
        }
    }

    fn try_close_now(&self) -> bool {
        match self.handle.try_lock() {
            Ok(mut slot) => self.close_locked(&mut slot),
            Err(TryLockError::WouldBlock) | Err(TryLockError::Poisoned(_)) => {
                self.request_close();
                false
            }
        }
    }

    pub(crate) fn finish_io(&self) {
        if !self.close_requested.load(Ordering::Acquire) {
            return;
        }
        match self.handle.lock() {
            Ok(mut slot) => {
                let _ = self.close_locked(&mut slot);
            }
            Err(poisoned) => {
                let mut slot = poisoned.into_inner();
                let _ = self.close_locked(&mut slot);
            }
        }
    }

    #[cfg(test)]
    fn has_open_handle(&self) -> bool {
        match self.handle.lock() {
            Ok(slot) => slot.is_some(),
            Err(poisoned) => poisoned.into_inner().is_some(),
        }
    }
}

impl Drop for DeviceHandleState {
    fn drop(&mut self) {
        let slot = match self.handle.get_mut() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(raw_handle) = slot.take() {
            let _ = (self.close_handle)(raw_handle);
        }
    }
}

struct DeviceHandleContext {
    generation: ScanGeneration,
    state: Arc<DeviceHandleState>,
    registration_ready: AtomicBool,
}

impl DeviceHandleContext {
    fn new(generation: &ScanGeneration, state: Arc<DeviceHandleState>) -> Self {
        Self {
            generation: generation.clone(),
            state,
            registration_ready: AtomicBool::new(false),
        }
    }

    fn mark_ready(&self) {
        self.registration_ready.store(true, Ordering::Release);
    }

    #[cfg(test)]
    fn for_test(generation: ScanGeneration, state: Arc<DeviceHandleState>) -> Self {
        Self {
            generation,
            state,
            registration_ready: AtomicBool::new(true),
        }
    }

    #[cfg(test)]
    fn for_test_not_ready(generation: ScanGeneration, state: Arc<DeviceHandleState>) -> Self {
        Self {
            generation,
            state,
            registration_ready: AtomicBool::new(false),
        }
    }
}

type NotifyCallback = extern "system" fn(
    notification: *mut c_void,
    context: *mut c_void,
    action: u32,
    event_data: *const c_void,
    event_data_size: u32,
) -> u32;

#[link(name = "kernel32")]
extern "system" {
    fn CloseHandle(handle: *mut c_void) -> i32;
}

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

extern "system" fn on_device_handle_change(
    _notification: *mut c_void,
    context: *mut c_void,
    action: u32,
    _event_data: *const c_void,
    _event_data_size: u32,
) -> u32 {
    // SAFETY: DeviceHandleNotifications keeps this Box alive until
    // CM_Unregister_Notification has quiesced all callbacks. The callback
    // performs no allocation, logging, disk I/O or blocking lock acquisition.
    let Some(context) = (unsafe { (context as *const DeviceHandleContext).as_ref() }) else {
        return CR_SUCCESS;
    };

    // Any handle-level PnP event makes this short-lived diagnostic stale.
    context.generation.invalidate();

    match action {
        CM_NOTIFY_ACTION_DEVICEQUERYREMOVE => {
            // The filter handle must stay valid until CM_Register_Notification
            // itself returns. A callback racing registration therefore vetoes
            // this removal attempt, then the owner closes after registration.
            if !context.registration_ready.load(Ordering::Acquire) {
                context.state.request_close();
                return ERROR_CANCELLED;
            }
            // Never close underneath a synchronous DeviceIoControl. If the
            // handle is currently leased, veto this query-remove and close as
            // soon as the owner releases the lease.
            if context.state.try_close_now() {
                CR_SUCCESS
            } else {
                ERROR_CANCELLED
            }
        }
        CM_NOTIFY_ACTION_DEVICEQUERYREMOVEFAILED => {
            // Do not resurrect this scan. Microsoft permits reopening to keep
            // using the interface; Partition 11 instead invalidates the scan.
            // A later fresh scan re-enumerates and opens a fresh handle.
            CR_SUCCESS
        }
        CM_NOTIFY_ACTION_DEVICEREMOVEPENDING | CM_NOTIFY_ACTION_DEVICEREMOVECOMPLETE => {
            context.state.request_close();
            if context.registration_ready.load(Ordering::Acquire) {
                let _ = context.state.try_close_now();
            }
            CR_SUCCESS
        }
        _ => CR_SUCCESS,
    }
}

pub(crate) struct DeviceHandleNotifications {
    notification: *mut c_void,
    context: Option<Box<DeviceHandleContext>>,
}

impl DeviceHandleNotifications {
    pub(crate) fn register(
        raw_handle: usize,
        generation: &ScanGeneration,
    ) -> Result<(Arc<DeviceHandleState>, Self), u32> {
        let state = DeviceHandleState::new(raw_handle);
        let context = Box::new(DeviceHandleContext::new(generation, Arc::clone(&state)));
        let context_ptr = (&*context as *const DeviceHandleContext)
            .cast_mut()
            .cast::<c_void>();
        let filter = device_handle_filter(raw_handle);
        let mut notification = ptr::null_mut();

        // SAFETY: The raw handle remains inside DeviceHandleState and cannot
        // be closed while registration_ready is false. The SDK ABI oracle
        // verifies the x64 filter layout and enum/action values independently.
        let status = unsafe {
            CM_Register_Notification(
                &filter,
                context_ptr,
                Some(on_device_handle_change),
                &mut notification,
            )
        };
        if status != CR_SUCCESS {
            return Err(status);
        }
        if notification.is_null() {
            // If Windows ever reports success without a registration handle,
            // callbacks cannot be quiesced. Keep their context alive but
            // detach and close the raw device handle first so we do not pin
            // removable media indefinitely.
            context.mark_ready();
            state.request_close();
            state.finish_io();
            std::mem::forget(context);
            return Err(u32::MAX);
        }

        context.mark_ready();
        // A query-remove may have raced the registration call and requested
        // deferred close while the filter handle had to remain valid.
        state.finish_io();

        Ok((
            state,
            Self {
                notification,
                context: Some(context),
            },
        ))
    }

    fn close(&mut self) -> Result<(), u32> {
        if self.notification.is_null() {
            return Ok(());
        }
        // Never called from the callback. Windows waits here until any
        // outstanding callback returns before its context may be released.
        let status = unsafe { CM_Unregister_Notification(self.notification) };
        self.notification = ptr::null_mut();
        if status != CR_SUCCESS {
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

impl Drop for DeviceHandleNotifications {
    fn drop(&mut self) {
        let _ = self.close();
    }
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
    fn query_remove_during_registration_vetoes_without_closing_filter_handle() {
        let generation = ScanGeneration::default();
        let state = DeviceHandleState::for_test(0x1234);
        let context = Box::new(DeviceHandleContext::for_test_not_ready(
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
        context.mark_ready();
        state.finish_io();
        assert!(!state.has_open_handle());
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
