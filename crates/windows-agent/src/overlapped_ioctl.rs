//! Cancellable, bounded overlapped metadata IOCTL state machine.

#[cfg(windows)]
use std::time::Duration;

pub(crate) const IOCTL_TIMEOUT_MS: u32 = 5_000;
pub(crate) const CANCEL_GRACE_MS: u32 = 1_000;
pub(crate) const WAIT_SLICE_MS: u32 = 50;

const ERROR_OPERATION_ABORTED: u32 = 995;
#[cfg(windows)]
const ERROR_IO_PENDING: u32 = 997;
#[cfg(windows)]
const ERROR_IO_INCOMPLETE: u32 = 996;
#[cfg(windows)]
const ERROR_NOT_FOUND: u32 = 1_168;
#[cfg(windows)]
const WAIT_TIMEOUT: u32 = 258;
#[cfg(windows)]
const MAX_IOCTL_INPUT_BYTES: usize = 12;
#[cfg(windows)]
const MAX_IOCTL_OUTPUT_BYTES: usize = partition11_core::windows_layout::MAX_RESPONSE_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlappedIoError {
    #[cfg(windows)]
    TimedOut,
    #[cfg(windows)]
    Cancelled,
    MalformedResponse,
    #[cfg(windows)]
    Win32(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendResult {
    Completed(usize),
    Pending,
    Failed(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BackendCancel {
    #[default]
    Requested,
    NotFound,
    #[cfg(windows)]
    Failed(u32),
}

trait IoctlBackend {
    fn now_ms(&self) -> u64;
    fn issue(&mut self) -> BackendResult;
    fn wait(&mut self, timeout_ms: u32) -> BackendResult;
    fn cancel_exact(&mut self) -> BackendCancel;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IoDecision {
    Completed(usize),
    TimedOut,
    Cancelled,
    Failed(u32),
    StuckAfterCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelReason {
    Timeout,
    Pnp,
}

fn decision_after_cancel(reason: CancelReason) -> IoDecision {
    match reason {
        CancelReason::Timeout => IoDecision::TimedOut,
        CancelReason::Pnp => IoDecision::Cancelled,
    }
}

fn coordinate_ioctl(
    backend: &mut impl IoctlBackend,
    cancellation_requested: impl Fn() -> bool,
) -> IoDecision {
    let deadline = backend.now_ms().saturating_add(u64::from(IOCTL_TIMEOUT_MS));
    if cancellation_requested() {
        return IoDecision::Cancelled;
    }
    match backend.issue() {
        BackendResult::Completed(bytes) => {
            if cancellation_requested() {
                return IoDecision::Cancelled;
            }
            return if backend.now_ms() >= deadline {
                IoDecision::TimedOut
            } else {
                IoDecision::Completed(bytes)
            };
        }
        BackendResult::Failed(ERROR_OPERATION_ABORTED) => return IoDecision::Cancelled,
        BackendResult::Failed(code) => {
            if cancellation_requested() {
                return IoDecision::Cancelled;
            }
            return if backend.now_ms() >= deadline {
                IoDecision::TimedOut
            } else {
                IoDecision::Failed(code)
            };
        }
        BackendResult::Pending => {}
    }

    let mut cancel_reason = None;
    let mut cancel_deadline = None;
    loop {
        let now = backend.now_ms();
        if let Some(reason) = cancel_reason {
            let deadline = cancel_deadline.expect("cancellation always has a grace deadline");
            if now >= deadline {
                return IoDecision::StuckAfterCancel;
            }
            let wait_ms = (deadline - now).min(u64::from(WAIT_SLICE_MS)) as u32;
            match backend.wait(wait_ms) {
                BackendResult::Completed(_) | BackendResult::Failed(ERROR_OPERATION_ABORTED) => {
                    return decision_after_cancel(reason)
                }
                BackendResult::Failed(code) => return IoDecision::Failed(code),
                BackendResult::Pending => {}
            }
            continue;
        }

        let reason = if cancellation_requested() {
            Some(CancelReason::Pnp)
        } else if now >= deadline {
            Some(CancelReason::Timeout)
        } else {
            None
        };
        if let Some(reason) = reason {
            cancel_reason = Some(reason);
            match backend.cancel_exact() {
                BackendCancel::NotFound => match backend.wait(0) {
                    BackendResult::Completed(bytes) if reason == CancelReason::Pnp => {
                        return IoDecision::Completed(bytes)
                    }
                    BackendResult::Completed(_)
                    | BackendResult::Failed(ERROR_OPERATION_ABORTED) => {
                        return decision_after_cancel(reason)
                    }
                    BackendResult::Failed(code) => return IoDecision::Failed(code),
                    BackendResult::Pending => {}
                },
                BackendCancel::Requested => {}
                #[cfg(windows)]
                BackendCancel::Failed(_) => {}
            }
            cancel_deadline = Some(backend.now_ms().saturating_add(u64::from(CANCEL_GRACE_MS)));
            continue;
        }

        let wait_ms = (deadline - now).min(u64::from(WAIT_SLICE_MS)) as u32;
        match backend.wait(wait_ms) {
            BackendResult::Completed(bytes) => return IoDecision::Completed(bytes),
            BackendResult::Failed(ERROR_OPERATION_ABORTED) => return IoDecision::Cancelled,
            BackendResult::Failed(code) => return IoDecision::Failed(code),
            BackendResult::Pending => {}
        }
    }
}

fn finish_output(mut output: Vec<u8>, returned: usize) -> Result<Vec<u8>, OverlappedIoError> {
    if returned > output.len() {
        return Err(OverlappedIoError::MalformedResponse);
    }
    output.truncate(returned);
    Ok(output)
}

#[cfg(windows)]
type Handle = *mut std::ffi::c_void;

#[cfg(windows)]
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;

#[cfg(windows)]
#[repr(C)]
struct NativeOverlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}

#[cfg(windows)]
struct ActiveRequest {
    event: Handle,
    overlapped: Box<NativeOverlapped>,
    input: Vec<u8>,
    output: Vec<u8>,
}

#[cfg(windows)]
impl ActiveRequest {
    fn new(input: &[u8], output_bytes: usize) -> Result<Self, OverlappedIoError> {
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if event.is_null() || event == INVALID_HANDLE_VALUE {
            return Err(OverlappedIoError::Win32(unsafe { GetLastError() }));
        }
        Ok(Self {
            event,
            overlapped: Box::new(NativeOverlapped {
                internal: 0,
                internal_high: 0,
                offset: 0,
                offset_high: 0,
                event,
            }),
            input: input.to_vec(),
            output: vec![0; output_bytes],
        })
    }
}

#[cfg(windows)]
impl Drop for ActiveRequest {
    fn drop(&mut self) {
        if !self.event.is_null() && self.event != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.event) };
            self.event = std::ptr::null_mut();
        }
    }
}

#[cfg(windows)]
struct NativeBackend<'a> {
    handle: Handle,
    code: u32,
    request: &'a mut ActiveRequest,
    started: std::time::Instant,
}

#[cfg(windows)]
impl IoctlBackend for NativeBackend<'_> {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }

    fn issue(&mut self) -> BackendResult {
        let mut returned = 0_u32;
        let input_ptr = if self.request.input.is_empty() {
            std::ptr::null()
        } else {
            self.request.input.as_ptr().cast()
        };
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                self.code,
                input_ptr,
                self.request.input.len() as u32,
                self.request.output.as_mut_ptr().cast(),
                self.request.output.len() as u32,
                &mut returned,
                (&mut *self.request.overlapped as *mut NativeOverlapped).cast(),
            )
        };
        if ok != 0 {
            BackendResult::Completed(returned as usize)
        } else {
            let error = unsafe { GetLastError() };
            if error == ERROR_IO_PENDING {
                BackendResult::Pending
            } else {
                BackendResult::Failed(error)
            }
        }
    }

    fn wait(&mut self, timeout_ms: u32) -> BackendResult {
        let mut returned = 0_u32;
        let ok = unsafe {
            GetOverlappedResultEx(
                self.handle,
                (&mut *self.request.overlapped) as *mut NativeOverlapped,
                &mut returned,
                timeout_ms,
                0,
            )
        };
        if ok != 0 {
            BackendResult::Completed(returned as usize)
        } else {
            let error = unsafe { GetLastError() };
            if error == ERROR_IO_INCOMPLETE || error == WAIT_TIMEOUT {
                BackendResult::Pending
            } else {
                BackendResult::Failed(error)
            }
        }
    }

    fn cancel_exact(&mut self) -> BackendCancel {
        let ok = unsafe {
            CancelIoEx(
                self.handle,
                (&mut *self.request.overlapped) as *mut NativeOverlapped,
            )
        };
        if ok != 0 {
            BackendCancel::Requested
        } else {
            let error = unsafe { GetLastError() };
            if error == ERROR_NOT_FOUND {
                BackendCancel::NotFound
            } else {
                BackendCancel::Failed(error)
            }
        }
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateEventW(
        attributes: *mut std::ffi::c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn DeviceIoControl(
        device: Handle,
        code: u32,
        input: *const std::ffi::c_void,
        input_len: u32,
        output: *mut std::ffi::c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn GetOverlappedResultEx(
        device: Handle,
        overlapped: *mut NativeOverlapped,
        transferred: *mut u32,
        milliseconds: u32,
        alertable: i32,
    ) -> i32;
    fn CancelIoEx(device: Handle, overlapped: *mut NativeOverlapped) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(windows)]
fn hold_pending_request_until_process_exit(request: ActiveRequest) -> ! {
    // Forgetting the owner deliberately retains the event and every buffer
    // still reachable by the kernel. This worker never reports or reuses it.
    std::mem::forget(request);
    loop {
        std::thread::park_timeout(Duration::from_secs(60));
    }
}

#[cfg(windows)]
pub(crate) fn run_worker_ioctl(
    raw_handle: usize,
    code: u32,
    input: &[u8],
    max_output: usize,
    cancellation_requested: impl Fn() -> bool,
) -> Result<Vec<u8>, OverlappedIoError> {
    // The worker is not a general IOCTL proxy. Keep both input and output
    // storage within the fixed metadata-only request envelope.
    const IOCTL_STORAGE_GET_DEVICE_NUMBER: u32 = 0x002D_1080;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007_405C;
    const IOCTL_DISK_GET_DRIVE_LAYOUT_EX: u32 = 0x0007_0050;
    if ![
        IOCTL_STORAGE_GET_DEVICE_NUMBER,
        IOCTL_STORAGE_QUERY_PROPERTY,
        IOCTL_DISK_GET_LENGTH_INFO,
        IOCTL_DISK_GET_DRIVE_LAYOUT_EX,
    ]
    .contains(&code)
        || input.len() > MAX_IOCTL_INPUT_BYTES
        || max_output == 0
        || max_output > MAX_IOCTL_OUTPUT_BYTES
    {
        return Err(OverlappedIoError::MalformedResponse);
    }

    let raw_handle = raw_handle as Handle;
    if raw_handle.is_null() || raw_handle == INVALID_HANDLE_VALUE {
        return Err(OverlappedIoError::MalformedResponse);
    }
    let mut request = ActiveRequest::new(input, max_output)?;
    let decision = {
        let mut backend = NativeBackend {
            handle: raw_handle,
            code,
            request: &mut request,
            started: std::time::Instant::now(),
        };
        coordinate_ioctl(&mut backend, cancellation_requested)
    };

    match decision {
        IoDecision::Completed(returned) => {
            finish_output(std::mem::take(&mut request.output), returned)
        }
        IoDecision::TimedOut => Err(OverlappedIoError::TimedOut),
        IoDecision::Cancelled => Err(OverlappedIoError::Cancelled),
        IoDecision::Failed(ERROR_OPERATION_ABORTED) => Err(OverlappedIoError::Cancelled),
        IoDecision::Failed(code) => Err(OverlappedIoError::Win32(code)),
        IoDecision::StuckAfterCancel => hold_pending_request_until_process_exit(request),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeBackend {
        now_ms: u64,
        issue_elapsed_ms: u64,
        immediate: Option<BackendResult>,
        completion_at_ms: Option<(u64, BackendResult)>,
        final_after_not_found: Option<BackendResult>,
        cancel_result: BackendCancel,
        cancel_at_ms: Option<u64>,
        issue_count: usize,
        wait_requests: Vec<u32>,
        cancel_flag_on_first_wait: Option<Arc<AtomicBool>>,
    }

    impl IoctlBackend for FakeBackend {
        fn now_ms(&self) -> u64 {
            self.now_ms
        }

        fn issue(&mut self) -> BackendResult {
            self.issue_count += 1;
            self.now_ms = self.now_ms.saturating_add(self.issue_elapsed_ms);
            self.immediate.take().unwrap_or(BackendResult::Pending)
        }

        fn wait(&mut self, timeout_ms: u32) -> BackendResult {
            self.wait_requests.push(timeout_ms);
            if let Some(flag) = self.cancel_flag_on_first_wait.take() {
                flag.store(true, Ordering::Release);
            }
            if self.cancel_at_ms.is_some()
                && self.cancel_result == BackendCancel::NotFound
                && self.final_after_not_found.is_some()
            {
                return self.final_after_not_found.take().unwrap();
            }
            let end = self.now_ms.saturating_add(u64::from(timeout_ms));
            if let Some((completion_at, result)) = self.completion_at_ms {
                if completion_at <= end {
                    self.now_ms = completion_at;
                    self.completion_at_ms = None;
                    return result;
                }
            }
            self.now_ms = end;
            BackendResult::Pending
        }

        fn cancel_exact(&mut self) -> BackendCancel {
            self.cancel_at_ms = Some(self.now_ms);
            self.cancel_result
        }
    }

    fn pnp_signal(backend: &mut FakeBackend) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        backend.cancel_flag_on_first_wait = Some(Arc::clone(&flag));
        flag
    }

    #[test]
    fn immediate_completion_returns_bounded_bytes() {
        let mut backend = FakeBackend {
            immediate: Some(BackendResult::Completed(3)),
            ..FakeBackend::default()
        };
        let decision = coordinate_ioctl(&mut backend, || false);
        assert_eq!(decision, IoDecision::Completed(3));
        assert_eq!(backend.issue_count, 1);
        assert_eq!(finish_output(vec![1, 2, 3, 4], 3), Ok(vec![1, 2, 3]));
    }

    #[test]
    fn late_immediate_completion_expires_the_request_budget() {
        let mut backend = FakeBackend {
            issue_elapsed_ms: u64::from(IOCTL_TIMEOUT_MS) + 1,
            immediate: Some(BackendResult::Completed(3)),
            ..FakeBackend::default()
        };
        assert_eq!(
            coordinate_ioctl(&mut backend, || false),
            IoDecision::TimedOut
        );
        assert_eq!(backend.issue_count, 1);
        assert_eq!(backend.cancel_at_ms, None);
    }

    #[test]
    fn pending_issue_that_uses_budget_is_cancelled_immediately() {
        let mut backend = FakeBackend {
            issue_elapsed_ms: u64::from(IOCTL_TIMEOUT_MS),
            immediate: Some(BackendResult::Pending),
            completion_at_ms: Some((
                u64::from(IOCTL_TIMEOUT_MS) + 100,
                BackendResult::Failed(ERROR_OPERATION_ABORTED),
            )),
            ..FakeBackend::default()
        };
        assert_eq!(
            coordinate_ioctl(&mut backend, || false),
            IoDecision::TimedOut
        );
        assert_eq!(backend.cancel_at_ms, Some(u64::from(IOCTL_TIMEOUT_MS)));
        assert_eq!(backend.issue_count, 1);
    }

    #[test]
    fn pending_completion_finishes_before_deadline() {
        let mut backend = FakeBackend {
            completion_at_ms: Some((200, BackendResult::Completed(8))),
            ..FakeBackend::default()
        };
        assert_eq!(
            coordinate_ioctl(&mut backend, || false),
            IoDecision::Completed(8)
        );
        assert_eq!(backend.now_ms, 200);
        assert_eq!(backend.cancel_at_ms, None);
        assert!(backend
            .wait_requests
            .iter()
            .all(|wait| *wait <= WAIT_SLICE_MS));
    }

    #[test]
    fn five_second_deadline_requests_exact_cancel() {
        let mut backend = FakeBackend {
            completion_at_ms: Some((5_100, BackendResult::Failed(ERROR_OPERATION_ABORTED))),
            ..FakeBackend::default()
        };
        assert_eq!(
            coordinate_ioctl(&mut backend, || false),
            IoDecision::TimedOut
        );
        assert_eq!(backend.cancel_at_ms, Some(u64::from(IOCTL_TIMEOUT_MS)));
        assert_eq!(backend.issue_count, 1);
        assert!(backend
            .wait_requests
            .iter()
            .all(|wait| *wait <= WAIT_SLICE_MS));
    }

    #[test]
    fn pnp_cancel_request_is_seen_within_one_wait_slice() {
        let mut backend = FakeBackend {
            completion_at_ms: Some((100, BackendResult::Failed(ERROR_OPERATION_ABORTED))),
            ..FakeBackend::default()
        };
        let flag = pnp_signal(&mut backend);
        assert_eq!(
            coordinate_ioctl(&mut backend, || flag.load(Ordering::Acquire)),
            IoDecision::Cancelled
        );
        assert_eq!(backend.cancel_at_ms, Some(u64::from(WAIT_SLICE_MS)));
    }

    #[test]
    fn cancelled_request_reports_operation_aborted() {
        let mut backend = FakeBackend {
            completion_at_ms: Some((100, BackendResult::Failed(ERROR_OPERATION_ABORTED))),
            ..FakeBackend::default()
        };
        let flag = pnp_signal(&mut backend);
        assert_eq!(
            coordinate_ioctl(&mut backend, || flag.load(Ordering::Acquire)),
            IoDecision::Cancelled
        );
    }

    #[test]
    fn completion_can_win_cancel_race() {
        let mut backend = FakeBackend {
            cancel_result: BackendCancel::NotFound,
            final_after_not_found: Some(BackendResult::Completed(12)),
            ..FakeBackend::default()
        };
        let flag = pnp_signal(&mut backend);
        assert_eq!(
            coordinate_ioctl(&mut backend, || flag.load(Ordering::Acquire)),
            IoDecision::Completed(12)
        );
    }

    #[test]
    fn cancel_not_found_resolves_final_completion_state() {
        let mut backend = FakeBackend {
            cancel_result: BackendCancel::NotFound,
            final_after_not_found: Some(BackendResult::Pending),
            completion_at_ms: Some((100, BackendResult::Failed(ERROR_OPERATION_ABORTED))),
            ..FakeBackend::default()
        };
        let flag = pnp_signal(&mut backend);
        assert_eq!(
            coordinate_ioctl(&mut backend, || flag.load(Ordering::Acquire)),
            IoDecision::Cancelled
        );
    }

    #[test]
    fn cancel_grace_expiry_is_never_reported_as_success() {
        let mut backend = FakeBackend::default();
        let flag = pnp_signal(&mut backend);
        assert_eq!(
            coordinate_ioctl(&mut backend, || flag.load(Ordering::Acquire)),
            IoDecision::StuckAfterCancel
        );
        assert_eq!(backend.now_ms, u64::from(WAIT_SLICE_MS + CANCEL_GRACE_MS));
        assert_eq!(backend.issue_count, 1);
    }

    #[test]
    fn output_larger_than_bound_is_rejected() {
        assert_eq!(
            finish_output(vec![0; 8], 9),
            Err(OverlappedIoError::MalformedResponse)
        );
    }
}
