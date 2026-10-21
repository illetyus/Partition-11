use crate::diskspd::{build_diskspd_args, DiskSpdDirection, DiskSpdProfile, VerifiedDiskSpd};
use crate::job_process::{run_process, ProcessCommand, ProcessError, ProcessLimits, ProcessOutput};
use crate::preparation::{prepare_scratch, MAX_PREPARATION_TIME};
use crate::ScratchLease;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkExecutionError {
    Cancelled,
    WorkerFailure,
    CleanupRequired,
}

impl std::fmt::Display for BenchmarkExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "benchmark was cancelled",
            Self::WorkerFailure => "benchmark worker failed",
            Self::CleanupRequired => "scratch cleanup requires recovery",
        })
    }
}

impl std::error::Error for BenchmarkExecutionError {}

pub struct CompletedBenchmark {
    pub target_path: PathBuf,
    pub xml: String,
}

/// Owns the durable journal and scratch-file lifecycle around one contained process.
/// Only process outcomes that prove the child job is drained may explicitly clean up.
// Task 2 consumes this internally after Gate A validates the pinned engine.
#[allow(dead_code)]
pub(crate) fn execute_lease(
    mut lease: ScratchLease,
    run: impl FnOnce(&ScratchLease) -> Result<ProcessOutput, ProcessError>,
) -> Result<CompletedBenchmark, BenchmarkExecutionError> {
    lease.retain_for_recovery();
    lease
        .mark_running()
        .map_err(|_| BenchmarkExecutionError::CleanupRequired)?;

    match run(&lease) {
        Ok(output) => {
            let result = if output.exit_code == 0 {
                String::from_utf8(output.stdout).map_err(|_| BenchmarkExecutionError::WorkerFailure)
            } else {
                Err(BenchmarkExecutionError::WorkerFailure)
            };
            let target_path = lease.file_path().to_path_buf();
            lease
                .cleanup()
                .map_err(|_| BenchmarkExecutionError::CleanupRequired)?;
            result.map(|xml| CompletedBenchmark { target_path, xml })
        }
        Err(error) => {
            let outcome = match error {
                ProcessError::Undrained | ProcessError::Io(_) => {
                    return Err(BenchmarkExecutionError::CleanupRequired);
                }
                ProcessError::Cancelled => BenchmarkExecutionError::Cancelled,
                ProcessError::InvalidCommand
                | ProcessError::InvalidWorkload
                | ProcessError::TimedOut
                | ProcessError::OutputLimitExceeded => BenchmarkExecutionError::WorkerFailure,
            };
            lease
                .cleanup()
                .map_err(|_| BenchmarkExecutionError::CleanupRequired)?;
            Err(outcome)
        }
    }
}

/// Initializes and benchmarks one exclusively owned scratch file using only
/// the pinned engine and fixed workload policy.
pub fn run_file_benchmark(
    root: &Path,
    engine: &VerifiedDiskSpd,
    profile: DiskSpdProfile,
    direction: DiskSpdDirection,
    cancel: &AtomicBool,
) -> Result<CompletedBenchmark, BenchmarkExecutionError> {
    if cancel.load(Ordering::Acquire) {
        return Err(BenchmarkExecutionError::Cancelled);
    }
    let mut lease = ScratchLease::create(root, 1024 * 1024 * 1024)
        .map_err(|_| BenchmarkExecutionError::WorkerFailure)?;
    let args = match build_diskspd_args(&lease, profile, direction, 5, 15) {
        Ok(args) => args,
        Err(_) => {
            return Err(if lease.cleanup().is_ok() {
                BenchmarkExecutionError::WorkerFailure
            } else {
                BenchmarkExecutionError::CleanupRequired
            });
        }
    };
    let command = match ProcessCommand::verified_diskspd(engine, args) {
        Ok(command) => command,
        Err(_) => {
            return Err(if lease.cleanup().is_ok() {
                BenchmarkExecutionError::WorkerFailure
            } else {
                BenchmarkExecutionError::CleanupRequired
            });
        }
    };
    let limits = match ProcessLimits::from_workload(5, 15) {
        Ok(limits) => limits,
        Err(_) => {
            return Err(if lease.cleanup().is_ok() {
                BenchmarkExecutionError::WorkerFailure
            } else {
                BenchmarkExecutionError::CleanupRequired
            });
        }
    };

    if let Err(error) = prepare_scratch(&mut lease, cancel, Instant::now() + MAX_PREPARATION_TIME) {
        let was_cancelled = error.kind() == std::io::ErrorKind::Interrupted;
        return Err(if lease.cleanup().is_ok() {
            if was_cancelled {
                BenchmarkExecutionError::Cancelled
            } else {
                BenchmarkExecutionError::WorkerFailure
            }
        } else {
            BenchmarkExecutionError::CleanupRequired
        });
    }
    if cancel.load(Ordering::Acquire) {
        return Err(if lease.cleanup().is_ok() {
            BenchmarkExecutionError::Cancelled
        } else {
            BenchmarkExecutionError::CleanupRequired
        });
    }

    let result = execute_lease(lease, |_| run_process(command, limits, cancel));
    if cancel.load(Ordering::Acquire) {
        return Err(BenchmarkExecutionError::Cancelled);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job_process::{run_process, ProcessCommand, ProcessError, ProcessLimits};
    use crate::scratch::ScratchLease;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    fn fixture() -> (PathBuf, PathBuf, ScratchLease) {
        let root = std::env::temp_dir().join(format!(
            "p11-execution-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let journal = root.join("journal");
        let lease =
            ScratchLease::create_journaled_for_test(&root, &journal, 4 * 1024 * 1024).unwrap();
        (root, journal, lease)
    }

    #[test]
    fn running_is_durable_before_contained_child_and_cleanup_follows_drain() {
        let (root, journal, lease) = fixture();
        let target = lease.file_path().to_path_buf();
        let cancel = AtomicBool::new(false);
        let result = execute_lease(lease, |lease| {
            assert!(lease.file_path().exists());
            let record = std::fs::read_dir(&journal)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| path.extension().is_some_and(|ext| ext == "json"))
                .unwrap();
            assert!(std::fs::read_to_string(record)
                .unwrap()
                .ends_with("state=running\n"));
            let executable = std::env::var_os("ComSpec")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
            let command = ProcessCommand::test_fixture(
                executable,
                vec![
                    OsString::from("/d"),
                    OsString::from("/c"),
                    OsString::from("echo harmless-fixture"),
                ],
            )
            .unwrap();
            run_process(
                command,
                ProcessLimits::from_workload(0, 2).unwrap(),
                &cancel,
            )
        })
        .unwrap();
        assert_eq!(result.target_path, target);
        assert!(result.xml.contains("harmless-fixture"));
        assert!(!target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepared_lease_is_flushed_before_child_launch_and_cleaned_after_drain() {
        let (root, journal, mut lease) = fixture();
        let target = lease.file_path().to_path_buf();
        let size = lease.size_bytes();
        crate::preparation::prepare_scratch_with_size(
            &mut lease,
            &AtomicBool::new(false),
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            size,
        )
        .unwrap();

        let prepared_record = std::fs::read_dir(&journal)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|ext| ext == "json"))
            .unwrap();
        assert!(std::fs::read_to_string(prepared_record)
            .unwrap()
            .ends_with("state=ready\n"));
        let mut prepared = std::fs::File::open(&target).unwrap();
        assert_eq!(prepared.metadata().unwrap().len(), size);
        let mut sample = [0_u8; 4096];
        std::io::Read::read_exact(&mut prepared, &mut sample).unwrap();
        assert!(sample.iter().any(|byte| *byte != 0));
        drop(prepared);

        let result = execute_lease(lease, |lease| {
            let running_record = std::fs::read_dir(&journal)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| path.extension().is_some_and(|ext| ext == "json"))
                .unwrap();
            assert!(std::fs::read_to_string(running_record)
                .unwrap()
                .ends_with("state=running\n"));
            assert_eq!(std::fs::metadata(lease.file_path()).unwrap().len(), size);

            let executable = std::env::var_os("ComSpec")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
            let command = ProcessCommand::test_fixture(
                executable,
                vec![
                    OsString::from("/d"),
                    OsString::from("/c"),
                    OsString::from("echo prepared-fixture"),
                ],
            )
            .unwrap();
            run_process(
                command,
                ProcessLimits::from_workload(0, 2).unwrap(),
                &AtomicBool::new(false),
            )
        })
        .unwrap();
        assert_eq!(result.target_path, target);
        assert!(result.xml.contains("prepared-fixture"));
        assert!(!target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uncertain_job_retains_file_and_running_journal_for_exact_recovery() {
        let (root, journal, lease) = fixture();
        let target = lease.file_path().to_path_buf();
        assert!(matches!(
            execute_lease(lease, |_| Err(ProcessError::Undrained)),
            Err(BenchmarkExecutionError::CleanupRequired)
        ));
        assert!(
            target.exists(),
            "Drop must never delete while a process could still be active"
        );
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        assert!(!target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn drained_cancel_cleans_before_reporting_cancelled() {
        let (root, journal, lease) = fixture();
        let target = lease.file_path().to_path_buf();
        assert!(matches!(
            execute_lease(lease, |_| Err(ProcessError::Cancelled)),
            Err(BenchmarkExecutionError::Cancelled)
        ));
        assert!(!target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nonzero_exit_cleans_before_reporting_worker_failure() {
        let (root, journal, lease) = fixture();
        let target = lease.file_path().to_path_buf();
        let output = crate::job_process::ProcessOutput {
            exit_code: 1,
            stdout: Vec::new(),
            stderr: b"fixture failure".to_vec(),
        };
        assert!(matches!(
            execute_lease(lease, |_| Ok(output)),
            Err(BenchmarkExecutionError::WorkerFailure)
        ));
        assert!(!target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uncertain_native_io_error_retains_file_until_recovery() {
        let (root, journal, lease) = fixture();
        let target = lease.file_path().to_path_buf();
        assert!(matches!(
            execute_lease(lease, |_| Err(ProcessError::Io(std::io::Error::other(
                "simulated uncertain native failure"
            )))),
            Err(BenchmarkExecutionError::CleanupRequired)
        ));
        assert!(target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        assert!(!target.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn panic_during_process_execution_never_deletes_the_lease() {
        let (root, journal, lease) = fixture();
        let target = lease.file_path().to_path_buf();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute_lease(lease, |_| panic!("simulated supervisor failure"))
        }));
        assert!(panic.is_err());
        assert!(target.exists());
        assert_eq!(
            crate::scratch::recover_test_journal(&journal),
            crate::ScratchRecoveryStatus::Clean
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
