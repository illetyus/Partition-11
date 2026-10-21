use crate::job_process::{ProcessError, ProcessOutput};
use crate::ScratchLease;
use std::path::PathBuf;

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
