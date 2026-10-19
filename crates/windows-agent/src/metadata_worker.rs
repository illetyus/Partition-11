//! One-shot private worker for bounded, cancellable metadata probes.

use crate::disk_discovery::{self, DiskInterface};
use crate::disk_metadata::{DiskMetadata, MetadataError, ProbeOutcome};
use crate::metadata_worker_protocol::{
    decode_reply, decode_request, encode_reply, encode_request, WireError, WireMetadata,
    WireSectorSizes, WorkerReply, WorkerRequest, MAX_WIRE_BYTES,
};
use crate::scan_identity::ScanGeneration;
use std::io::{self, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const WORKER_DEADLINE: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

fn generation_is_current(generation: &ScanGeneration, expected_revision: u64) -> bool {
    generation.revision() == expected_revision
}

fn spawn_deadline_terminator(
    deadline: Duration,
    terminate: impl FnOnce() + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("p11-worker-deadline".into())
        .spawn(move || {
            thread::sleep(deadline);
            terminate();
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerError {
    ElevatedProcess,
    IdentityChanged,
    MalformedRequest,
    Io,
}

fn run_validated_target<T>(
    elevated: bool,
    request: &WorkerRequest,
    current: &[DiskInterface],
    query: impl FnOnce(&DiskInterface) -> T,
) -> Result<T, WorkerError> {
    if elevated {
        return Err(WorkerError::ElevatedProcess);
    }
    let target = validate_worker_target(request, current)?;
    Ok(query(&target))
}

pub(crate) fn validate_worker_target(
    request: &WorkerRequest,
    current: &[DiskInterface],
) -> Result<DiskInterface, WorkerError> {
    let mut matches = current
        .iter()
        .filter(|record| record.path == request.path && record.instance_id == request.instance_id);
    let target = matches.next().ok_or(WorkerError::IdentityChanged)?;
    if matches.next().is_some() {
        return Err(WorkerError::IdentityChanged);
    }
    Ok(target.clone())
}

fn read_bounded_request(reader: &mut impl Read) -> Result<WorkerRequest, WorkerError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_WIRE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| WorkerError::Io)?;
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(WorkerError::MalformedRequest);
    }
    decode_request(&bytes).map_err(|_| WorkerError::MalformedRequest)
}

pub(crate) fn run_stdio_worker() -> i32 {
    let _deadline = match spawn_deadline_terminator(WORKER_DEADLINE, || std::process::exit(124)) {
        Ok(deadline) => deadline,
        Err(_) => return 3,
    };
    if crate::metadata_query::reject_elevated_process().is_err() {
        return 3;
    }
    let request = match read_bounded_request(&mut io::stdin().lock()) {
        Ok(request) => request,
        Err(_) => return 3,
    };
    let current = match disk_discovery::enumerate_disk_records() {
        Ok(records) => records,
        Err(_) => return 3,
    };
    let generation = ScanGeneration::default();
    let result = run_validated_target(false, &request, &current, |record| {
        crate::metadata_query::query_worker_record(record, &generation)
    });
    let reply = match result {
        Ok(ProbeOutcome::Ready(metadata)) => WorkerReply::Ready(metadata_to_wire(metadata)),
        Ok(ProbeOutcome::Unavailable(error)) => WorkerReply::Unavailable(error_to_wire(error)),
        Err(_) => WorkerReply::Unavailable(WireError::IdentityChanged),
    };
    write_worker_reply(&reply)
}

fn write_worker_reply(reply: &WorkerReply) -> i32 {
    let Ok(bytes) = encode_reply(reply) else {
        return 3;
    };
    let mut stdout = io::stdout().lock();
    if stdout.write_all(&bytes).is_err() || stdout.flush().is_err() {
        return 3;
    }
    0
}

fn metadata_to_wire(metadata: DiskMetadata) -> WireMetadata {
    WireMetadata {
        device_number: metadata.device_number,
        capacity_bytes: metadata.capacity_bytes.map_err(error_to_wire),
        sectors: metadata
            .sectors
            .map(|sectors| WireSectorSizes {
                logical: sectors.logical,
                physical: sectors.physical,
            })
            .map_err(error_to_wire),
        bus_type: metadata.bus_type.map_err(error_to_wire),
        partition_layout: metadata.partition_layout.map_err(error_to_wire),
    }
}

fn error_to_wire(error: MetadataError) -> WireError {
    match error {
        MetadataError::ElevatedProcess | MetadataError::WorkerFailed => WireError::WorkerFailed,
        MetadataError::AccessDenied => WireError::AccessDenied,
        MetadataError::DeviceRemoved => WireError::DeviceRemoved,
        MetadataError::Unsupported => WireError::Unsupported,
        MetadataError::MalformedResponse => WireError::Malformed,
        MetadataError::IdentityChanged => WireError::IdentityChanged,
        MetadataError::LayoutTooLarge => WireError::LayoutTooLarge,
        MetadataError::TimedOut => WireError::TimedOut,
        MetadataError::Cancelled => WireError::Cancelled,
        MetadataError::Win32(code) => WireError::Win32(code),
    }
}

fn error_from_wire(error: WireError) -> MetadataError {
    match error {
        WireError::AccessDenied => MetadataError::AccessDenied,
        WireError::DeviceRemoved => MetadataError::DeviceRemoved,
        WireError::Unsupported => MetadataError::Unsupported,
        WireError::Malformed => MetadataError::MalformedResponse,
        WireError::IdentityChanged => MetadataError::IdentityChanged,
        WireError::LayoutTooLarge => MetadataError::LayoutTooLarge,
        WireError::TimedOut => MetadataError::TimedOut,
        WireError::Cancelled => MetadataError::Cancelled,
        WireError::WorkerFailed => MetadataError::WorkerFailed,
        WireError::Win32(code) => MetadataError::Win32(code),
    }
}

fn metadata_from_wire(metadata: WireMetadata) -> DiskMetadata {
    DiskMetadata {
        device_number: metadata.device_number,
        capacity_bytes: metadata.capacity_bytes.map_err(error_from_wire),
        sectors: metadata
            .sectors
            .map(|sectors| crate::metadata_query::SectorSizes {
                logical: sectors.logical,
                physical: sectors.physical,
            })
            .map_err(error_from_wire),
        bus_type: metadata.bus_type.map_err(error_from_wire),
        partition_layout: metadata.partition_layout.map_err(error_from_wire),
    }
}

fn decode_worker_result(
    successful_exit: bool,
    bytes: &[u8],
    is_current: impl Fn() -> bool,
) -> ProbeOutcome {
    if !is_current() {
        return ProbeOutcome::Unavailable(MetadataError::IdentityChanged);
    }
    if !successful_exit {
        return ProbeOutcome::Unavailable(MetadataError::WorkerFailed);
    }
    match decode_reply(bytes) {
        Ok(WorkerReply::Ready(metadata)) if is_current() => {
            ProbeOutcome::Ready(metadata_from_wire(metadata))
        }
        Ok(WorkerReply::Unavailable(error)) if is_current() => {
            ProbeOutcome::Unavailable(error_from_wire(error))
        }
        Ok(_) => ProbeOutcome::Unavailable(MetadataError::IdentityChanged),
        Err(_) => ProbeOutcome::Unavailable(MetadataError::WorkerFailed),
    }
}

fn worker_outcome_after_wait(
    wait_result: Result<ExitStatus, WatchError>,
    bytes: &[u8],
    is_current: impl Fn() -> bool,
) -> ProbeOutcome {
    if !is_current() {
        return ProbeOutcome::Unavailable(MetadataError::IdentityChanged);
    }
    match wait_result {
        Ok(status) if status.code() == Some(124) => {
            ProbeOutcome::Unavailable(MetadataError::TimedOut)
        }
        Ok(status) => decode_worker_result(status.success(), bytes, is_current),
        Err(WatchError::Invalidated) => ProbeOutcome::Unavailable(MetadataError::IdentityChanged),
        Err(WatchError::TimedOut) => ProbeOutcome::Unavailable(MetadataError::TimedOut),
        Err(WatchError::Process) => ProbeOutcome::Unavailable(MetadataError::WorkerFailed),
    }
}

trait WorkerChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
}

impl WorkerChild for Child {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Child::try_wait(self)
    }

    fn kill(&mut self) -> io::Result<()> {
        Child::kill(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchError {
    TimedOut,
    Invalidated,
    Process,
}

fn wait_for_child(
    child: &mut impl WorkerChild,
    deadline: Duration,
    mut elapsed: impl FnMut() -> Duration,
    mut sleep: impl FnMut(Duration),
    mut is_current: impl FnMut() -> bool,
) -> Result<ExitStatus, WatchError> {
    loop {
        if !is_current() {
            let _ = child.kill();
            return Err(WatchError::Invalidated);
        }
        let now = elapsed();
        if now >= deadline {
            let _ = child.kill();
            return Err(WatchError::TimedOut);
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(_) => {
                let _ = child.kill();
                return Err(WatchError::Process);
            }
        };
        if let Some(status) = status {
            return Ok(status);
        }
        sleep(POLL_INTERVAL.min(deadline - now));
    }
}

struct QuarantinedWorker {
    child: Option<Child>,
    threads: Vec<JoinHandle<()>>,
}

static WORKER_SERIAL: Mutex<()> = Mutex::new(());
static WORKER_QUARANTINE: OnceLock<Mutex<Option<QuarantinedWorker>>> = OnceLock::new();

fn quarantine_slot() -> &'static Mutex<Option<QuarantinedWorker>> {
    WORKER_QUARANTINE.get_or_init(|| Mutex::new(None))
}

fn worker_serial_guard() -> MutexGuard<'static, ()> {
    WORKER_SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn can_launch_worker() -> bool {
    let mut slot = quarantine_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(quarantined) = slot.as_mut() else {
        return true;
    };
    if let Some(child) = quarantined.child.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => quarantined.child = None,
            Ok(None) | Err(_) => return false,
        }
    }
    if quarantined.threads.iter().all(JoinHandle::is_finished) {
        if let Some(finished) = slot.take() {
            for thread in finished.threads {
                let _ = thread.join();
            }
        }
        true
    } else {
        false
    }
}

fn quarantine_worker(child: Option<Child>, threads: Vec<JoinHandle<()>>) {
    let mut slot = quarantine_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    debug_assert!(slot.is_none());
    *slot = Some(QuarantinedWorker { child, threads });
}

type SpawnedWorker = (
    Child,
    Receiver<io::Result<()>>,
    Receiver<io::Result<Vec<u8>>>,
    Vec<JoinHandle<()>>,
);

fn spawn_worker(frame: Vec<u8>) -> Result<SpawnedWorker, MetadataError> {
    let executable = std::env::current_exe().map_err(|_| MetadataError::WorkerFailed)?;
    let mut child = Command::new(executable)
        .arg("__metadata-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| MetadataError::WorkerFailed)?;
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill();
        quarantine_worker(Some(child), Vec::new());
        return Err(MetadataError::WorkerFailed);
    };
    let Some(stdout) = child.stdout.take() else {
        drop(stdin);
        let _ = child.kill();
        quarantine_worker(Some(child), Vec::new());
        return Err(MetadataError::WorkerFailed);
    };

    let (writer_tx, writer_rx) = mpsc::sync_channel(1);
    let writer = match thread::Builder::new()
        .name("p11-worker-input".into())
        .spawn(move || {
            let mut pipe = stdin;
            let result = pipe.write_all(&frame);
            drop(pipe);
            let _ = writer_tx.send(result);
        }) {
        Ok(thread) => thread,
        Err(_) => {
            drop(stdout);
            let _ = child.kill();
            quarantine_worker(Some(child), Vec::new());
            return Err(MetadataError::WorkerFailed);
        }
    };

    let (reader_tx, reader_rx) = mpsc::sync_channel(1);
    let reader = match thread::Builder::new()
        .name("p11-worker-output".into())
        .spawn(move || {
            let mut pipe = stdout;
            let mut bytes = Vec::new();
            let result = pipe
                .by_ref()
                .take((MAX_WIRE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .and_then(|_| {
                    if bytes.len() > MAX_WIRE_BYTES {
                        Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "worker reply exceeds bound",
                        ))
                    } else {
                        Ok(bytes)
                    }
                });
            drop(pipe);
            let _ = reader_tx.send(result);
        }) {
        Ok(thread) => thread,
        Err(_) => {
            let _ = child.kill();
            quarantine_worker(Some(child), vec![writer]);
            return Err(MetadataError::WorkerFailed);
        }
    };
    Ok((child, writer_rx, reader_rx, vec![writer, reader]))
}

fn receive_worker_io(
    writer_rx: &Receiver<io::Result<()>>,
    reader_rx: &Receiver<io::Result<Vec<u8>>>,
    deadline: Duration,
    start: Instant,
    mut is_current: impl FnMut() -> bool,
) -> Result<((), Vec<u8>), WatchError> {
    let mut writer_result = None;
    let mut reader_result = None;
    loop {
        if !is_current() {
            return Err(WatchError::Invalidated);
        }
        let elapsed = start.elapsed();
        if elapsed >= deadline {
            return Err(WatchError::TimedOut);
        }
        if writer_result.is_none() {
            match writer_rx.try_recv() {
                Ok(result) => writer_result = Some(result),
                Err(TryRecvError::Disconnected) => return Err(WatchError::Process),
                Err(TryRecvError::Empty) => {}
            }
        }
        if reader_result.is_none() {
            match reader_rx.try_recv() {
                Ok(result) => reader_result = Some(result),
                Err(TryRecvError::Disconnected) => return Err(WatchError::Process),
                Err(TryRecvError::Empty) => {}
            }
        }
        if let (Some(writer), Some(reader)) = (&writer_result, &reader_result) {
            writer.as_ref().map_err(|_| WatchError::Process)?;
            let bytes = reader.as_ref().map_err(|_| WatchError::Process)?.clone();
            return Ok(((), bytes));
        }
        thread::sleep(POLL_INTERVAL.min(deadline - elapsed));
    }
}

pub(crate) fn probe_via_worker_with_timeout(
    record: &DiskInterface,
    generation: &ScanGeneration,
    expected_revision: u64,
    timeout: Duration,
) -> ProbeOutcome {
    let _serial = worker_serial_guard();
    let unavailable = |error| ProbeOutcome::Unavailable(error);
    if !generation_is_current(generation, expected_revision) {
        return unavailable(MetadataError::IdentityChanged);
    }
    if !can_launch_worker() {
        return unavailable(MetadataError::WorkerFailed);
    }
    let request = WorkerRequest {
        path: record.path.clone(),
        instance_id: record.instance_id.clone(),
    };
    let frame = match encode_request(&request) {
        Ok(frame) => frame,
        Err(_) => return unavailable(MetadataError::MalformedResponse),
    };
    let started = Instant::now();
    let (mut child, writer_rx, reader_rx, threads) = match spawn_worker(frame) {
        Ok(process) => process,
        Err(error) => return unavailable(error),
    };
    let current = || generation_is_current(generation, expected_revision);
    let status = wait_for_child(
        &mut child,
        WORKER_DEADLINE.min(timeout),
        || started.elapsed(),
        thread::sleep,
        current,
    );
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            quarantine_worker(Some(child), threads);
            return worker_outcome_after_wait(Err(error), &[], current);
        }
    };
    let io_result = receive_worker_io(
        &writer_rx,
        &reader_rx,
        WORKER_DEADLINE.min(timeout),
        started,
        current,
    );
    let (_, bytes) = match io_result {
        Ok(result) => result,
        Err(error) => {
            quarantine_worker(None, threads);
            return worker_outcome_after_wait(Err(error), &[], current);
        }
    };
    drop(threads);
    worker_outcome_after_wait(Ok(status), &bytes, current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_worker_protocol::{encode_reply, WireError, WireMetadata, WireSectorSizes};
    use std::cell::Cell;
    use std::os::windows::process::ExitStatusExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeChild {
        polls: usize,
        exit_after: Option<usize>,
        status: Option<ExitStatus>,
        killed: bool,
    }

    impl WorkerChild for FakeChild {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            self.polls += 1;
            if self.exit_after.is_some_and(|limit| self.polls >= limit) {
                Ok(self.status)
            } else {
                Ok(None)
            }
        }

        fn kill(&mut self) -> io::Result<()> {
            self.killed = true;
            Ok(())
        }
    }

    fn status(success: bool) -> ExitStatus {
        ExitStatus::from_raw(if success { 0 } else { 1 })
    }

    fn status_with_code(code: u32) -> ExitStatus {
        ExitStatus::from_raw(code)
    }

    fn test_request() -> WorkerRequest {
        WorkerRequest {
            path: vec![65, 66],
            instance_id: vec![67, 68],
        }
    }

    fn test_record() -> DiskInterface {
        DiskInterface {
            path: vec![65, 66],
            instance_id: vec![67, 68],
        }
    }

    #[test]
    fn worker_accepts_exactly_one_fresh_path_instance_pair() {
        let expected = test_record();
        let other = DiskInterface {
            path: vec![70],
            instance_id: vec![71],
        };
        assert_eq!(
            validate_worker_target(&test_request(), &[other, expected.clone()]),
            Ok(expected)
        );
    }

    #[test]
    fn worker_rejects_unknown_path_mismatched_instance_and_duplicates() {
        let expected = test_record();
        assert!(validate_worker_target(
            &WorkerRequest {
                path: vec![1],
                instance_id: vec![2]
            },
            std::slice::from_ref(&expected),
        )
        .is_err());
        assert!(validate_worker_target(
            &WorkerRequest {
                path: expected.path.clone(),
                instance_id: vec![1]
            },
            std::slice::from_ref(&expected),
        )
        .is_err());
        assert!(validate_worker_target(&test_request(), &[expected.clone(), expected],).is_err());
    }

    #[test]
    fn worker_rejects_elevated_execution_before_device_open() {
        let mut opened = false;
        let result = run_validated_target(true, &test_request(), &[test_record()], |_| {
            opened = true;
        });
        assert_eq!(result, Err(WorkerError::ElevatedProcess));
        assert!(!opened);
    }

    #[test]
    fn worker_reads_exactly_one_bounded_request() {
        let request = test_request();
        let bytes = encode_request(&request).expect("valid request");
        assert_eq!(
            read_bounded_request(&mut io::Cursor::new(bytes)).unwrap(),
            request
        );
    }

    #[test]
    fn worker_rejects_malformed_or_extra_request_bytes() {
        let mut bytes = encode_request(&test_request()).expect("valid request");
        bytes.push(0);
        assert_eq!(
            read_bounded_request(&mut io::Cursor::new(bytes)),
            Err(WorkerError::MalformedRequest)
        );
        assert_eq!(
            read_bounded_request(&mut io::Cursor::new(vec![0; MAX_WIRE_BYTES + 1])),
            Err(WorkerError::MalformedRequest)
        );
    }

    #[test]
    fn worker_never_accepts_an_ioctl_code_from_protocol() {
        let request = test_request();
        let encoded = encode_request(&request).expect("request contains only identity fields");
        assert_eq!(decode_request(&encoded).unwrap(), request);
    }

    #[test]
    fn worker_success_before_deadline_decodes_reply() {
        let reply = WorkerReply::Unavailable(WireError::Unsupported);
        let bytes = encode_reply(&reply).unwrap();
        let mut child = FakeChild {
            exit_after: Some(2),
            status: Some(status(true)),
            ..FakeChild::default()
        };
        let now = Cell::new(Duration::ZERO);
        let exit = wait_for_child(
            &mut child,
            Duration::from_secs(1),
            || now.get(),
            |duration| now.set(now.get() + duration),
            || true,
        )
        .unwrap();
        assert!(exit.success());
        assert_eq!(decode_reply(&bytes).unwrap(), reply);
        assert!(!child.killed);
    }

    #[test]
    fn worker_abnormal_exit_is_rejected() {
        let mut child = FakeChild {
            exit_after: Some(1),
            status: Some(status(false)),
            ..FakeChild::default()
        };
        let result = wait_for_child(
            &mut child,
            Duration::from_secs(1),
            || Duration::ZERO,
            |_| {},
            || true,
        );
        assert!(!result.unwrap().success());
    }

    #[test]
    fn worker_malformed_reply_is_rejected() {
        assert!(matches!(
            decode_worker_result(true, &[0, 1, 2, 3], || true),
            ProbeOutcome::Unavailable(MetadataError::WorkerFailed)
        ));
    }

    #[test]
    fn timed_out_worker_ignores_even_a_valid_captured_reply() {
        let reply = WorkerReply::Ready(WireMetadata {
            device_number: 1,
            capacity_bytes: Ok(4096),
            sectors: Ok(WireSectorSizes {
                logical: Some(512),
                physical: Some(512),
            }),
            bus_type: Ok(None),
            partition_layout: Ok(partition11_core::windows_layout::DriveLayout {
                style: partition11_core::windows_layout::LayoutStyle::Raw,
                occupied: Vec::new(),
                returned_slots: 0,
            }),
        });
        let bytes = encode_reply(&reply).unwrap();
        assert!(matches!(
            worker_outcome_after_wait(Err(WatchError::TimedOut), &bytes, || true),
            ProbeOutcome::Unavailable(MetadataError::TimedOut)
        ));
    }

    #[test]
    fn worker_self_deadline_exit_is_reported_as_timeout() {
        assert!(matches!(
            worker_outcome_after_wait(Ok(status_with_code(124)), &[], || true),
            ProbeOutcome::Unavailable(MetadataError::TimedOut)
        ));
    }

    #[test]
    fn worker_self_deadline_runs_its_terminator() {
        let terminated = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&terminated);
        spawn_deadline_terminator(Duration::from_millis(1), move || {
            flag.store(true, Ordering::Release);
        })
        .unwrap()
        .join()
        .unwrap();
        assert!(terminated.load(Ordering::Acquire));
    }

    #[test]
    fn worker_reply_after_pnp_invalidation_never_becomes_ready() {
        let reply = WorkerReply::Ready(WireMetadata {
            device_number: 1,
            capacity_bytes: Ok(4096),
            sectors: Ok(WireSectorSizes {
                logical: Some(512),
                physical: Some(512),
            }),
            bus_type: Ok(None),
            partition_layout: Ok(partition11_core::windows_layout::DriveLayout {
                style: partition11_core::windows_layout::LayoutStyle::Raw,
                occupied: Vec::new(),
                returned_slots: 0,
            }),
        });
        let bytes = encode_reply(&reply).unwrap();
        assert!(matches!(
            decode_worker_result(true, &bytes, || false),
            ProbeOutcome::Unavailable(MetadataError::IdentityChanged)
        ));
    }

    #[test]
    fn worker_deadline_kills_and_discards_valid_output() {
        let mut child = FakeChild::default();
        let now = Cell::new(Duration::ZERO);
        let polls = Cell::new(0);
        let result = wait_for_child(
            &mut child,
            Duration::from_millis(25),
            || now.get(),
            |duration| {
                polls.set(polls.get() + 1);
                now.set(now.get() + duration);
            },
            || true,
        );
        assert_eq!(result, Err(WatchError::TimedOut));
        assert!(child.killed);
        assert!(polls.get() >= 2);
        let reply = WorkerReply::Ready(WireMetadata {
            device_number: 1,
            capacity_bytes: Ok(4096),
            sectors: Ok(WireSectorSizes {
                logical: Some(512),
                physical: Some(512),
            }),
            bus_type: Ok(None),
            partition_layout: Ok(partition11_core::windows_layout::DriveLayout {
                style: partition11_core::windows_layout::LayoutStyle::Raw,
                occupied: Vec::new(),
                returned_slots: 0,
            }),
        });
        let captured_output = encode_reply(&reply).unwrap();
        assert!(matches!(
            worker_outcome_after_wait(result, &captured_output, || true),
            ProbeOutcome::Unavailable(MetadataError::TimedOut)
        ));
    }

    #[test]
    fn quarantined_pipe_thread_blocks_the_next_worker_launch() {
        let _serial = worker_serial_guard();
        assert!(can_launch_worker());
        let (release_tx, release_rx) = mpsc::channel();
        let pipe_thread = thread::spawn(move || {
            let _ = release_rx.recv();
        });
        quarantine_worker(None, vec![pipe_thread]);
        assert!(!can_launch_worker());
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !can_launch_worker() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(can_launch_worker());
    }

    #[test]
    fn generation_change_while_worker_runs_discards_valid_reply() {
        let mut child = FakeChild::default();
        let now = Cell::new(Duration::ZERO);
        let current = Cell::new(true);
        let result = wait_for_child(
            &mut child,
            Duration::from_secs(1),
            || now.get(),
            |_| current.set(false),
            || current.get(),
        );
        assert_eq!(result, Err(WatchError::Invalidated));
        assert!(child.killed);
    }

    #[test]
    fn worker_result_is_rejected_when_post_scan_inventory_changes() {
        let generation = ScanGeneration::default();
        let first = [DiskInterface {
            path: vec![65],
            instance_id: vec![66],
        }];
        let snapshot = generation.capture(&first).unwrap();
        let after = [DiskInterface {
            path: vec![65],
            instance_id: vec![67],
        }];
        assert!(snapshot.validate(&after, &[1]).is_err());
    }
}

#[cfg(test)]
mod protocol_error_assertions {
    use super::*;

    #[test]
    fn wire_metadata_error_mapping_preserves_timeout_and_cancel() {
        assert_eq!(
            error_from_wire(WireError::TimedOut),
            MetadataError::TimedOut
        );
        assert_eq!(
            error_from_wire(WireError::Cancelled),
            MetadataError::Cancelled
        );
    }
}
