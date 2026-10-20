//! Internal P2.5 file-target safety primitives.
//!
//! This crate is not packaged or connected to the Electron IPC. It does not
//! access raw disks, request elevation, or launch DiskSpd.
#![deny(unsafe_code)]

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
pub use scratch::ScratchLease;
