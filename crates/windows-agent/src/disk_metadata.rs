//! Parent-side scan policy. Device I/O is delegated to a bounded one-shot worker.

use crate::disk_discovery;
pub(crate) use crate::metadata_query::SectorSizes;
use crate::scan_identity::ScanGeneration;
use partition11_core::windows_layout::DriveLayout;
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataError {
    ElevatedProcess,
    AccessDenied,
    DeviceRemoved,
    Unsupported,
    MalformedResponse,
    IdentityChanged,
    LayoutTooLarge,
    TimedOut,
    Cancelled,
    WorkerFailed,
    Win32(u32),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElevatedProcess => write!(f, "elevated process rejected"),
            Self::AccessDenied => write!(f, "device access denied"),
            Self::DeviceRemoved => write!(f, "device unavailable or removed"),
            Self::Unsupported => write!(f, "metadata query unsupported"),
            Self::MalformedResponse => write!(f, "malformed metadata response"),
            Self::IdentityChanged => write!(f, "transient device identity changed"),
            Self::LayoutTooLarge => write!(f, "partition layout exceeds bounded reader limit"),
            Self::TimedOut => write!(f, "metadata request timed out"),
            Self::Cancelled => write!(f, "metadata request cancelled"),
            Self::WorkerFailed => write!(f, "metadata worker failed"),
            Self::Win32(code) => write!(f, "Win32 error {code}"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct DiskMetadata {
    pub device_number: u32,
    pub capacity_bytes: Result<u64, MetadataError>,
    pub sectors: Result<SectorSizes, MetadataError>,
    pub bus_type: Result<Option<u32>, MetadataError>,
    pub stable_identity: Result<crate::storage_identity::StorageIdentity, MetadataError>,
    pub partition_layout: Result<DriveLayout, MetadataError>,
}

#[derive(Debug)]
pub(crate) struct ReadOnlyInventory {
    pub disks: Vec<ProbeOutcome>,
    pub volumes: Vec<crate::volume_inventory::VolumeRecord>,
}

#[derive(Debug)]
pub(crate) enum ProbeOutcome {
    Ready(DiskMetadata),
    Unavailable(MetadataError),
}

struct ProbeEpoch<'a> {
    generation: &'a ScanGeneration,
    revision: u64,
}

impl<'a> ProbeEpoch<'a> {
    fn capture(generation: &'a ScanGeneration) -> Self {
        Self {
            generation,
            revision: generation.revision(),
        }
    }

    fn ensure_current(&self) -> Result<(), MetadataError> {
        if self.generation.revision() == self.revision {
            Ok(())
        } else {
            Err(MetadataError::IdentityChanged)
        }
    }

    #[cfg(test)]
    fn run<T>(
        &self,
        operation: impl FnOnce() -> Result<T, MetadataError>,
    ) -> Result<T, MetadataError> {
        self.ensure_current()?;
        let result = operation();
        self.ensure_current()?;
        result
    }
}

#[derive(Debug)]
pub(crate) enum ScanError {
    Discovery(disk_discovery::DiscoveryError),
    Security(MetadataError),
    TimedOut,
}

fn remaining_scan_time(deadline: Instant) -> Result<Duration, ScanError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(ScanError::TimedOut)
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(e) => write!(f, "Windows enumeration: {e}"),
            Self::Security(e) => write!(f, "Windows metadata policy: {e}"),
            Self::TimedOut => write!(f, "bounded disk inventory scan timed out"),
        }
    }
}

/// Return provisional metadata only to the local agent, never to Electron.
/// A later, independent SetupAPI pass rejects paths that disappeared.
pub(crate) fn inspect_discovered_devices() -> Result<ReadOnlyInventory, ScanError> {
    inspect_discovered_devices_with_timeout(Duration::from_secs(30))
}

pub(crate) fn inspect_discovered_devices_with_timeout(
    timeout: Duration,
) -> Result<ReadOnlyInventory, ScanError> {
    let deadline = Instant::now() + timeout;
    crate::metadata_query::reject_elevated_process().map_err(ScanError::Security)?;
    let generation = ScanGeneration::default();
    let notifications = crate::pnp_notifications::DiskNotifications::register(&generation)
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    let epoch = ProbeEpoch::capture(&generation);
    let starting_revision = epoch.revision;
    let before = disk_discovery::enumerate_disk_records().map_err(ScanError::Discovery)?;
    epoch.ensure_current().map_err(ScanError::Security)?;
    let snapshot = generation
        .capture(&before)
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    let mut results = Vec::with_capacity(before.len());
    for record in &before {
        let remaining = remaining_scan_time(deadline)?;
        let result = crate::metadata_worker::probe_via_worker_with_timeout(
            record,
            &generation,
            starting_revision,
            remaining,
        );
        if matches!(result, ProbeOutcome::Unavailable(MetadataError::TimedOut)) {
            return Err(ScanError::TimedOut);
        }
        results.push(result);
    }
    let volumes =
        crate::volume_inventory::inspect(&results, &generation, starting_revision, deadline)
            .map_err(|error| match error {
                MetadataError::TimedOut => ScanError::TimedOut,
                other => ScanError::Security(other),
            })?;
    remaining_scan_time(deadline)?;
    epoch.ensure_current().map_err(ScanError::Security)?;
    let after = disk_discovery::enumerate_disk_records().map_err(ScanError::Discovery)?;
    remaining_scan_time(deadline)?;
    let numbers: Vec<_> = results
        .iter()
        .filter_map(|result| match result {
            ProbeOutcome::Ready(metadata) => Some(metadata.device_number),
            ProbeOutcome::Unavailable(_) => None,
        })
        .collect();
    snapshot
        .validate(&after, &numbers)
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    notifications
        .finish()
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    remaining_scan_time(deadline)?;
    if generation.revision() != starting_revision {
        return Err(ScanError::Security(MetadataError::IdentityChanged));
    }
    snapshot
        .validate(&after, &numbers)
        .map_err(|_| ScanError::Security(MetadataError::IdentityChanged))?;
    generation.invalidate();
    Ok(ReadOnlyInventory {
        disks: results,
        volumes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn expired_scan_deadline_rejects_before_work_continues() {
        assert!(matches!(
            remaining_scan_time(Instant::now() - Duration::from_millis(1)),
            Err(ScanError::TimedOut)
        ));
        assert!(remaining_scan_time(Instant::now() + Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn cancelled_epoch_never_starts_a_later_worker() {
        let generation = ScanGeneration::default();
        let epoch = ProbeEpoch::capture(&generation);
        generation.invalidate();
        let ran = Cell::new(false);
        assert_eq!(
            epoch.run(|| {
                ran.set(true);
                Ok::<u32, MetadataError>(7)
            }),
            Err(MetadataError::IdentityChanged)
        );
        assert!(!ran.get());
    }

    #[test]
    #[ignore = "enumerates and queries physical Windows disks; reserved for the user's manual hardware acceptance"]
    fn metadata_probe_stays_local_and_non_elevated() {
        match inspect_discovered_devices() {
            Ok(inventory) => {
                assert!(inventory.disks.len() <= 256);
                assert!(inventory.volumes.len() <= crate::volume_inventory::MAX_VOLUMES);
            }
            Err(ScanError::Security(MetadataError::ElevatedProcess)) => (),
            Err(e) => panic!("unexpected native metadata error: {e}"),
        }
    }
}
