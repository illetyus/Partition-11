//! One-run, consent-gated agent entry point for file-backed benchmarking.

use crate::benchmark_protocol::{
    self, BenchmarkRequest, FinalError, RequestError, TargetValidationError, ValidatedTarget,
};
use crate::disk_metadata::{self, ReadOnlyInventory, ScanError, ScanGuard};
use crate::scan_identity::ScanGeneration;
use crate::storage_identity::StorageIdentity;
use partition11_benchmark_worker::{
    run_file_benchmark, BenchmarkExecutionError, DiskSpdDirection, DiskSpdProfile, VerifiedDiskSpd,
};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const SCAN_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_HARD_DEADLINE: Duration = Duration::from_secs(120);
const MAX_CONTROL_BYTES: usize = 6;
const GENERATION_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Reads one authorized request and keeps stdin available for a single cancel
/// frame until the agent exits. Malformed requests without a valid request ID
/// never receive an echoed response.
pub(crate) fn run<R: BufRead + Send + 'static>(mut reader: R, writer: &mut impl Write) -> i32 {
    let _watchdog = super::spawn_hard_deadline(AGENT_HARD_DEADLINE, || {
        std::process::exit(124);
    })
    .unwrap_or_else(|_| std::process::exit(3));

    let request = match benchmark_protocol::read_request_frame(&mut reader) {
        Ok(request) => request,
        Err(_) => return 2,
    };
    if !request.authorization_is_valid() {
        return write_error(writer, &request.request_id, FinalError::InvalidRequest);
    }
    if crate::metadata_query::reject_elevated_process().is_err() {
        return write_error(writer, &request.request_id, FinalError::InvalidRequest);
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let control_cancel = Arc::clone(&cancel);
    if thread::Builder::new()
        .name("p11-benchmark-control".into())
        .spawn(move || observe_control(reader, &control_cancel))
        .is_err()
    {
        return write_error(writer, &request.request_id, FinalError::WorkerFailure);
    }

    let (inventory, guard) =
        match disk_metadata::inspect_discovered_devices_with_guard(SCAN_TIMEOUT) {
            Ok(scan) => scan,
            Err(error) => {
                let final_error = match error {
                    ScanError::Security(disk_metadata::MetadataError::IdentityChanged) => {
                        FinalError::StaleSelection
                    }
                    _ if cancel.load(Ordering::Acquire) => {
                        return write_cancelled(writer, &request.request_id);
                    }
                    _ => FinalError::WorkerFailure,
                };
                return write_error(writer, &request.request_id, final_error);
            }
        };

    let (generation_stop, generation_thread) =
        spawn_generation_observer(guard.generation(), guard.revision(), Arc::clone(&cancel));
    let operation = run_after_scan(&request, &inventory, &guard, &cancel);
    generation_stop.store(true, Ordering::Release);
    let _ = generation_thread.join();
    let guard_is_current = guard.finish().is_ok();

    match operation {
        Ok((completed, file_system)) if !guard_is_current => {
            let _ = (completed, file_system);
            write_error(writer, &request.request_id, FinalError::StaleSelection)
        }
        Ok((completed, file_system)) if cancel.load(Ordering::Acquire) => {
            let _ = (completed, file_system);
            write_cancelled(writer, &request.request_id)
        }
        Ok((completed, file_system)) => {
            let Some(target_path) = completed.target_path.to_str() else {
                return write_error(writer, &request.request_id, FinalError::WorkerFailure);
            };
            let response = benchmark_protocol::encode_completed_response(
                &request.request_id,
                target_path,
                &file_system,
                &completed.xml,
            );
            match response {
                Ok(response) => write_encoded(writer, Ok(response)),
                Err(_) => write_error(writer, &request.request_id, FinalError::WorkerFailure),
            }
        }
        Err(FinalError::CleanupRequired) => {
            write_error(writer, &request.request_id, FinalError::CleanupRequired)
        }
        Err(_) if !guard_is_current => {
            write_error(writer, &request.request_id, FinalError::StaleSelection)
        }
        Err(_) if cancel.load(Ordering::Acquire) => write_cancelled(writer, &request.request_id),
        Err(error) => write_error(writer, &request.request_id, error),
    }
}

fn run_after_scan(
    request: &BenchmarkRequest,
    inventory: &ReadOnlyInventory,
    guard: &ScanGuard,
    cancel: &AtomicBool,
) -> Result<(partition11_benchmark_worker::CompletedBenchmark, String), FinalError> {
    let volume = inventory
        .volumes
        .get(
            request
                .volume_ordinal
                .checked_sub(1)
                .ok_or(FinalError::StaleSelection)? as usize,
        )
        .ok_or(FinalError::StaleSelection)?;
    let disk_identities: Vec<Option<&StorageIdentity>> = inventory
        .disks
        .iter()
        .map(|probe| match probe {
            disk_metadata::ProbeOutcome::Ready(metadata) => metadata.stable_identity.as_ref().ok(),
            disk_metadata::ProbeOutcome::Unavailable(_) => None,
        })
        .collect();
    validate_then_start(
        request,
        request.volume_ordinal,
        volume,
        &disk_identities,
        || {
            guard
                .ensure_current()
                .map_err(|_| FinalError::StaleSelection)
        },
        |target| {
            if cancel.load(Ordering::Acquire) {
                return Err(FinalError::WorkerFailure);
            }
            let engine_path = packaged_diskspd_path().ok_or(FinalError::WorkerFailure)?;
            let engine =
                VerifiedDiskSpd::verify(&engine_path).map_err(|_| FinalError::WorkerFailure)?;
            let result = run_file_benchmark(
                Path::new(&target.volume_guid_root),
                &engine,
                map_profile(request.profile_id),
                map_direction(request.direction),
                cancel,
            )
            .map_err(map_execution_error)?;
            Ok((result, target.file_system.clone()))
        },
    )
}

fn validate_then_start<T>(
    request: &BenchmarkRequest,
    volume_ordinal: u32,
    volume: &crate::volume_inventory::VolumeRecord,
    disk_identities: &[Option<&StorageIdentity>],
    mut ensure_current: impl FnMut() -> Result<(), FinalError>,
    start: impl FnOnce(&ValidatedTarget) -> Result<T, FinalError>,
) -> Result<T, FinalError> {
    ensure_current()?;
    let target =
        benchmark_protocol::revalidate_target(request, volume_ordinal, volume, disk_identities)
            .map_err(map_target_error)?;
    ensure_current()?;
    start(&target)
}

fn map_target_error(error: TargetValidationError) -> FinalError {
    match error {
        TargetValidationError::Authorization => FinalError::InvalidRequest,
        TargetValidationError::SelectionChanged
        | TargetValidationError::Ineligible
        | TargetValidationError::UnsupportedFilesystem
        | TargetValidationError::InvalidVolumeRoot
        | TargetValidationError::IdentityUnavailable
        | TargetValidationError::AmbiguousIdentity => FinalError::StaleSelection,
    }
}

fn map_execution_error(error: BenchmarkExecutionError) -> FinalError {
    match error {
        BenchmarkExecutionError::Cancelled => FinalError::WorkerFailure,
        BenchmarkExecutionError::WorkerFailure => FinalError::WorkerFailure,
        BenchmarkExecutionError::CleanupRequired => FinalError::CleanupRequired,
    }
}

fn map_profile(profile: benchmark_protocol::ProfileId) -> DiskSpdProfile {
    match profile {
        benchmark_protocol::ProfileId::Seq1mQ8T1 => DiskSpdProfile::Seq1mQ8T1,
        benchmark_protocol::ProfileId::Seq1mQ1T1 => DiskSpdProfile::Seq1mQ1T1,
        benchmark_protocol::ProfileId::Rnd4kQ32T1 => DiskSpdProfile::Rnd4kQ32T1,
        benchmark_protocol::ProfileId::Rnd4kQ1T1 => DiskSpdProfile::Rnd4kQ1T1,
    }
}

fn map_direction(direction: benchmark_protocol::Direction) -> DiskSpdDirection {
    match direction {
        benchmark_protocol::Direction::Read => DiskSpdDirection::Read,
        benchmark_protocol::Direction::Write => DiskSpdDirection::Write,
    }
}

fn packaged_diskspd_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    packaged_diskspd_path_from(&executable)
}

fn packaged_diskspd_path_from(executable: &Path) -> Option<PathBuf> {
    let resources = executable.parent()?.parent()?;
    Some(resources.join("diskspd").join("amd64").join("diskspd.exe"))
}

fn spawn_generation_observer(
    generation: ScanGeneration,
    expected_revision: u64,
    cancel: Arc<AtomicBool>,
) -> (Arc<AtomicBool>, JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let observer_stop = Arc::clone(&stop);
    let handle = thread::Builder::new()
        .name("p11-benchmark-pnp-watch".into())
        .spawn(move || {
            while !observer_stop.load(Ordering::Acquire) {
                if generation.revision() != expected_revision {
                    cancel.store(true, Ordering::Release);
                    break;
                }
                thread::sleep(GENERATION_POLL_INTERVAL);
            }
        })
        .unwrap_or_else(|_| std::process::exit(3));
    (stop, handle)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControlFrame {
    Cancel,
    Disconnect,
    Malformed,
}

fn read_control_frame(reader: &mut impl BufRead) -> ControlFrame {
    let mut bytes = Vec::with_capacity(MAX_CONTROL_BYTES);
    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(_) => return ControlFrame::Malformed,
        };
        if available.is_empty() {
            return if bytes.is_empty() {
                ControlFrame::Disconnect
            } else {
                ControlFrame::Malformed
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let frame_bytes = newline.unwrap_or(available.len());
        if bytes.len().saturating_add(frame_bytes) > MAX_CONTROL_BYTES {
            return ControlFrame::Malformed;
        }
        bytes.extend_from_slice(&available[..frame_bytes]);
        let consumed = newline.map_or(available.len(), |index| index + 1);
        reader.consume(consumed);
        if newline.is_some() {
            return if bytes == b"cancel" {
                ControlFrame::Cancel
            } else {
                ControlFrame::Malformed
            };
        }
    }
}

fn observe_control(mut reader: impl BufRead, cancel: &AtomicBool) {
    let _ = read_control_frame(&mut reader);
    cancel.store(true, Ordering::Release);
}

fn write_cancelled(writer: &mut impl Write, request_id: &str) -> i32 {
    write_encoded(
        writer,
        benchmark_protocol::encode_cancelled_response(request_id),
    )
}

fn write_error(writer: &mut impl Write, request_id: &str, error: FinalError) -> i32 {
    write_encoded(
        writer,
        benchmark_protocol::encode_error_response(request_id, error),
    )
}

fn write_encoded(writer: &mut impl Write, response: Result<Vec<u8>, RequestError>) -> i32 {
    match response {
        Ok(response) if benchmark_protocol::write_response(writer, &response).is_ok() => 0,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        packaged_diskspd_path_from, read_control_frame, validate_then_start, ControlFrame,
        FinalError,
    };
    use crate::benchmark_protocol::{parse_request, BenchmarkRequest};
    use crate::storage_identity::{
        hmac_sha256, StableIdentifier, StableIdentifierKind, StorageIdentity,
    };
    use crate::volume_eligibility::{Assessment, DriveType, EligibilityReason};
    use crate::volume_inventory::VolumeRecord;
    use std::io::{BufReader, Cursor};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn only_the_exact_cancel_frame_is_accepted() {
        let mut cancel = BufReader::new(Cursor::new(b"cancel\n"));
        assert_eq!(read_control_frame(&mut cancel), ControlFrame::Cancel);

        for invalid in [
            b"cancel".as_slice(),
            b"pause\n",
            b"cancel \n",
            b"\xff\n",
            b"cancelx\n",
        ] {
            let mut input = BufReader::new(Cursor::new(invalid));
            assert_eq!(read_control_frame(&mut input), ControlFrame::Malformed);
        }
        let oversized = vec![b'x'; 64];
        let mut input = BufReader::new(Cursor::new(oversized));
        assert_eq!(read_control_frame(&mut input), ControlFrame::Malformed);
        let mut closed = BufReader::new(Cursor::new(b""));
        assert_eq!(read_control_frame(&mut closed), ControlFrame::Disconnect);
    }

    #[test]
    fn request_and_cancel_frames_work_over_a_pipe_while_parent_keeps_it_open() {
        use std::io::Write as _;

        let (pipe_reader, mut pipe_writer) = std::io::pipe().unwrap();
        let (request_sent, request_received) = mpsc::channel();
        let (control_sent, control_received) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut input = BufReader::new(pipe_reader);
            let request = crate::benchmark_protocol::read_request_frame(&mut input).unwrap();
            request_sent.send(request.request_id).unwrap();
            control_sent.send(read_control_frame(&mut input)).unwrap();
        });

        pipe_writer.write_all(&request_frame_bytes()).unwrap();
        pipe_writer.write_all(b"\n").unwrap();
        assert_eq!(
            request_received
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            "8c6f35f8-260e-4fbd-9e4d-c91b78562f58"
        );
        pipe_writer.write_all(b"cancel\n").unwrap();
        assert_eq!(
            control_received
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            ControlFrame::Cancel
        );
        drop(pipe_writer);
        reader.join().unwrap();
    }

    #[test]
    fn eof_after_a_valid_request_is_reported_as_disconnect() {
        use std::io::Write as _;

        let (pipe_reader, mut pipe_writer) = std::io::pipe().unwrap();
        let (request_sent, request_received) = mpsc::channel();
        let (control_sent, control_received) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut input = BufReader::new(pipe_reader);
            let request = crate::benchmark_protocol::read_request_frame(&mut input).unwrap();
            request_sent.send(request.request_id).unwrap();
            control_sent.send(read_control_frame(&mut input)).unwrap();
        });

        pipe_writer.write_all(&request_frame_bytes()).unwrap();
        pipe_writer.write_all(b"\n").unwrap();
        drop(pipe_writer);
        assert_eq!(
            request_received
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            "8c6f35f8-260e-4fbd-9e4d-c91b78562f58"
        );
        assert_eq!(
            control_received
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            ControlFrame::Disconnect
        );
        reader.join().unwrap();
    }

    #[test]
    fn packaged_engine_path_is_fixed_relative_to_the_agent_resource() {
        assert_eq!(
            packaged_diskspd_path_from(Path::new(
                r"C:\Program Files\Partition 11\resources\agent\partition11-agent.exe"
            )),
            Some(PathBuf::from(
                r"C:\Program Files\Partition 11\resources\diskspd\amd64\diskspd.exe"
            ))
        );
    }

    fn identity() -> StorageIdentity {
        StorageIdentity {
            identifiers: vec![StableIdentifier {
                kind: StableIdentifierKind::Eui64,
                bytes: vec![0x5a; 8],
            }],
        }
    }

    fn request_frame_bytes() -> Vec<u8> {
        format!(
            "{{\"protocolVersion\":1,\"operation\":\"runFileBenchmark\",\"requestId\":\"8c6f35f8-260e-4fbd-9e4d-c91b78562f58\",\"volumeOrdinal\":1,\"diskOrdinal\":1,\"partitionNumber\":1,\"identityKey\":\"{}\",\"identityHash\":\"{}\",\"profileId\":\"seq1m-q8t1\",\"direction\":\"read\",\"authorizationKey\":\"{}\",\"authorizationTag\":\"{}\"}}",
            "11".repeat(32),
            "00".repeat(32),
            "33".repeat(32),
            "00".repeat(32)
        )
        .into_bytes()
    }

    fn request() -> BenchmarkRequest {
        let mut request = parse_request(&request_frame_bytes()).unwrap();
        request.identity_hash =
            hmac_sha256(&request.identity_key, &identity().canonical_bytes()).unwrap();
        request.authorization_tag =
            hmac_sha256(&request.authorization_key, &request.authorization_message()).unwrap();
        request
    }

    fn eligible_volume() -> VolumeRecord {
        VolumeRecord {
            mount_point: Some("E:\\".into()),
            volume_guid_path: Some("\\\\?\\Volume{12345678-1234-1234-1234-123456789abc}\\".into()),
            drive_type: DriveType::Fixed,
            file_system: Some("NTFS".into()),
            total_bytes: Some(8 * 1024 * 1024 * 1024),
            available_bytes: Some(4 * 1024 * 1024 * 1024),
            read_only: false,
            assessment: Assessment {
                eligible: true,
                reason: EligibilityReason::Eligible,
                disk_ordinal: Some(1),
                partition_number: Some(1),
            },
        }
    }

    #[test]
    fn failed_authorization_staleness_or_eligibility_never_calls_the_start_side_effect() {
        let selected = identity();
        let request = request();
        let identities = [Some(&selected)];
        let starts = AtomicUsize::new(0);

        let mut bad_auth = request.clone();
        bad_auth.authorization_tag[0] ^= 1;
        let result = validate_then_start(
            &bad_auth,
            1,
            &eligible_volume(),
            &identities,
            || Ok(()),
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert_eq!(result, Err(FinalError::InvalidRequest));

        let stale = validate_then_start(
            &request,
            2,
            &eligible_volume(),
            &identities,
            || Ok(()),
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert_eq!(stale, Err(FinalError::StaleSelection));

        let mut low_space = eligible_volume();
        low_space.available_bytes = Some(1024);
        let ineligible = validate_then_start(
            &request,
            1,
            &low_space,
            &identities,
            || Ok(()),
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert_eq!(ineligible, Err(FinalError::StaleSelection));

        let mut unsupported = eligible_volume();
        unsupported.file_system = Some("exFAT".into());
        let unsupported = validate_then_start(
            &request,
            1,
            &unsupported,
            &identities,
            || Ok(()),
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert_eq!(unsupported, Err(FinalError::StaleSelection));

        let pnp_stale = validate_then_start(
            &request,
            1,
            &eligible_volume(),
            &identities,
            || Err(FinalError::StaleSelection),
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert_eq!(pnp_stale, Err(FinalError::StaleSelection));

        let expired_scan = validate_then_start(
            &request,
            1,
            &eligible_volume(),
            &identities,
            || Err(FinalError::StaleSelection),
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );
        assert_eq!(expired_scan, Err(FinalError::StaleSelection));
        assert_eq!(starts.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn pnp_change_after_target_revalidation_still_prevents_the_start_side_effect() {
        use std::cell::Cell;

        let selected = identity();
        let request = request();
        let identities = [Some(&selected)];
        let current_checks = Cell::new(0);
        let starts = AtomicUsize::new(0);

        let result = validate_then_start(
            &request,
            1,
            &eligible_volume(),
            &identities,
            || {
                let next = current_checks.get() + 1;
                current_checks.set(next);
                if next == 1 {
                    Ok(())
                } else {
                    Err(FinalError::StaleSelection)
                }
            },
            |_| {
                starts.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        );

        assert_eq!(result, Err(FinalError::StaleSelection));
        assert_eq!(current_checks.get(), 2);
        assert_eq!(starts.load(Ordering::Relaxed), 0);
    }
}
