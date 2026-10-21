//! Internal P2.5 file-target safety primitives.
//!
//! This crate is not packaged or connected to the Electron IPC. It does not
//! access raw disks, request elevation, or launch DiskSpd.
#![deny(unsafe_code)]

#[cfg(windows)]
mod execution;

#[cfg(windows)]
#[allow(unsafe_code)]
mod diskspd;
#[cfg(windows)]
#[allow(dead_code, unsafe_code)]
mod job_process;
#[cfg(windows)]
#[allow(unsafe_code)]
mod scratch;
#[cfg(windows)]
pub use diskspd::{
    build_diskspd_args, DiskSpdDirection, DiskSpdProfile, PolicyError, VerifiedDiskSpd,
};
#[cfg(windows)]
pub use execution::{BenchmarkExecutionError, CompletedBenchmark};
#[cfg(windows)]
pub use scratch::ScratchLease;

/// Sanitized outcome of reconciling application-owned scratch leases.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScratchRecoveryStatus {
    Clean,
    CleanupRequired,
    Deferred,
}

/// Reconciles only exact leases recorded in the protected per-user journal.
#[cfg(windows)]
pub fn recover_scratch_leases() -> ScratchRecoveryStatus {
    scratch::recover_pending_leases()
}
