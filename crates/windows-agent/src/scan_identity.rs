//! P2-I2c-1: ephemeral, fail-closed Windows disk inventory guard.
//!
//! No handles, serials, paths or disk numbers leave the local agent. This
//! protects a single diagnostic scan against observable topology changes,
//! duplicate locators / temporary OS numbers, and stale scan lifetimes.
//!
//! IMPORTANT: A PnP path/number can be reused after a fast remove/replug.
//! Equality across two enumerations is NOT a durable physical identity.
//! Later P2-I2c work must add Windows PnP instance/storage identifiers and
//! event-driven invalidation before any renderer or data-access capability.

use std::collections::HashSet;
use std::time::{Duration, Instant};

const MAX_INTERFACES: usize = 256;
const MAX_PATH_UNITS: usize = 16 * 1024;
const MAX_SCAN_AGE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanIdentityError {
    InvalidLocator,
    DuplicateLocator,
    InventoryChanged,
    DuplicateDeviceNumber,
    Expired,
}

pub(crate) struct ScanSnapshot {
    original: HashSet<Vec<u16>>,
    started: Instant,
}

fn unique_paths(paths: &[Vec<u16>]) -> Result<HashSet<Vec<u16>>, ScanIdentityError> {
    if paths.len() > MAX_INTERFACES {
        return Err(ScanIdentityError::InvalidLocator);
    }
    let mut seen = HashSet::with_capacity(paths.len());
    for path in paths {
        if path.is_empty()
            || path.len() > MAX_PATH_UNITS
            || path.contains(&0)
        {
            return Err(ScanIdentityError::InvalidLocator);
        }
        if !seen.insert(path.clone()) {
            return Err(ScanIdentityError::DuplicateLocator);
        }
    }
    Ok(seen)
}

fn verify_scan(
    initial: &HashSet<Vec<u16>>,
    current: &[Vec<u16>],
    disk_numbers: &[u32],
    elapsed: Duration,
) -> Result<(), ScanIdentityError> {
    if elapsed > MAX_SCAN_AGE {
        return Err(ScanIdentityError::Expired);
    }
    let latest = unique_paths(current)?;
    if *initial != latest {
        // Any appearance, removal or path reassignment invalidates the
        // whole aggregate; reordering without changes is acceptable.
        return Err(ScanIdentityError::InventoryChanged);
    }
    let unique_numbers: HashSet<_> = disk_numbers.iter().copied().collect();
    if disk_numbers.contains(&u32::MAX) || unique_numbers.len() != disk_numbers.len() {
        return Err(ScanIdentityError::DuplicateDeviceNumber);
    }
    Ok(())
}

impl ScanSnapshot {
    pub(crate) fn capture(paths: &[Vec<u16>]) -> Result<Self, ScanIdentityError> {
        Ok(Self {
            original: unique_paths(paths)?,
            started: Instant::now(),
        })
    }

    pub(crate) fn validate(
        &self,
        current: &[Vec<u16>],
        disk_numbers: &[u32],
    ) -> Result<(), ScanIdentityError> {
        verify_scan(&self.original, current, disk_numbers, self.started.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    fn baseline() -> Vec<Vec<u16>> {
        vec![path("pnp-disk-a"), path("pnp-disk-b")]
    }

    #[test]
    fn reordered_inventory_with_distinct_numbers_is_accepted() {
        let original = baseline();
        let snapshot = ScanSnapshot::capture(&original).unwrap();
        let reversed = vec![path("pnp-disk-b"), path("pnp-disk-a")];
        assert_eq!(
            snapshot.validate(&reversed, &[3, 7]),
            Ok(())
        );
    }

    #[test]
    fn removing_replugging_or_reassigning_path_invalidates_every_result() {
        let original = baseline();
        let snap = ScanSnapshot::capture(&original).unwrap();
        assert_eq!(
            snap.validate(&[path("pnp-disk-a")], &[3]),
            Err(ScanIdentityError::InventoryChanged)
        );
        assert_eq!(
            snap.validate(&[path("pnp-disk-a"), path("pnp-disk-c")], &[3, 7]),
            Err(ScanIdentityError::InventoryChanged)
        );
        assert_eq!(
            snap.validate(
                &[path("pnp-disk-a"), path("pnp-disk-b"), path("new-disk")],
                &[3, 7, 8]
            ),
            Err(ScanIdentityError::InventoryChanged)
        );
    }

    #[test]
    fn duplicate_and_invalid_locators_fail_closed() {
        assert!(matches!(
            ScanSnapshot::capture(&[path("same"), path("same")]),
            Err(ScanIdentityError::DuplicateLocator)
        ));
        assert!(matches!(
            ScanSnapshot::capture(&[vec![]]),
            Err(ScanIdentityError::InvalidLocator)
        ));
        assert!(matches!(
            ScanSnapshot::capture(&[vec![1, 0, 2]]),
            Err(ScanIdentityError::InvalidLocator)
        ));
        let snapshot = ScanSnapshot::capture(&baseline()).unwrap();
        assert_eq!(
            snapshot.validate(&[path("pnp-disk-a"), path("pnp-disk-a")], &[3, 7]),
            Err(ScanIdentityError::DuplicateLocator)
        );
    }

    #[test]
    fn reused_or_duplicate_os_disk_numbers_are_ambiguous() {
        let original = baseline();
        let snap = ScanSnapshot::capture(&original).unwrap();
        assert_eq!(
            snap.validate(&original, &[11, 11]),
            Err(ScanIdentityError::DuplicateDeviceNumber)
        );
        assert_eq!(
            snap.validate(&original, &[u32::MAX]),
            Err(ScanIdentityError::DuplicateDeviceNumber)
        );
        // These numbers are valid only for this one scan; equal numbers in
        // successive scans do not imply identical physical disks.
        assert_eq!(snap.validate(&original, &[11, 12]), Ok(()));
    }

    #[test]
    fn elapsed_scans_expire_without_external_timers() {
        let original = unique_paths(&baseline()).unwrap();
        assert_eq!(
            verify_scan(&original, &baseline(), &[1, 2], Duration::from_secs(121)),
            Err(ScanIdentityError::Expired)
        );
        assert_eq!(
            verify_scan(&original, &baseline(), &[1, 2], Duration::from_secs(120)),
            Ok(())
        );
    }
}
