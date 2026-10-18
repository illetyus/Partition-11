//! P2-I2c-2: fail-closed, ephemeral Windows PnP inventory cross-check.
//!
//! The Windows SetupAPI device instance ID is paired with the interface path
//! within the SAME device info set. Changes to either invalidate the entire
//! diagnostic scan. No values leave the agent or become renderer handles.
//!
//! A device instance ID is an OS PnP label and can be reused for a newly
//! inserted physical disk, particularly for devices without real serials.
//! This does not prove strong identity or eliminate rapid unplug/replug races.

use crate::disk_discovery::DiskInterface;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const MAX_INTERFACES: usize = 256;
const MAX_PATH_UNITS: usize = 16 * 1024;
const MAX_INSTANCE_UNITS: usize = 2048;
const MAX_SCAN_AGE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanIdentityError {
    InvalidLocator,
    InvalidInstanceId,
    DuplicateLocator,
    DuplicateInstanceId,
    InventoryChanged,
    DuplicateDeviceNumber,
    Expired,
}

pub(crate) struct ScanSnapshot {
    original: HashMap<Vec<u16>, Vec<u16>>,
    started: Instant,
}

fn unique_records(
    records: &[DiskInterface],
) -> Result<HashMap<Vec<u16>, Vec<u16>>, ScanIdentityError> {
    if records.len() > MAX_INTERFACES {
        return Err(ScanIdentityError::InvalidLocator);
    }
    let mut by_path = HashMap::with_capacity(records.len());
    let mut ids = HashSet::with_capacity(records.len());
    for record in records {
        if record.path.is_empty()
            || record.path.len() > MAX_PATH_UNITS
            || record.path.contains(&0)
        {
            return Err(ScanIdentityError::InvalidLocator);
        }
        if record.instance_id.is_empty()
            || record.instance_id.len() > MAX_INSTANCE_UNITS
            || record.instance_id.contains(&0)
        {
            return Err(ScanIdentityError::InvalidInstanceId);
        }
        if by_path.insert(record.path.clone(), record.instance_id.clone()).is_some() {
            return Err(ScanIdentityError::DuplicateLocator);
        }
        // A conservative invariant for GUID_DEVINTERFACE_DISK only.
        // If a driver exposes two disk paths from one devnode, reject it
        // rather than pretending they are independently identifiable.
        if !ids.insert(record.instance_id.clone()) {
            return Err(ScanIdentityError::DuplicateInstanceId);
        }
    }
    Ok(by_path)
}

fn verify_scan(
    initial: &HashMap<Vec<u16>, Vec<u16>>,
    current: &[DiskInterface],
    disk_numbers: &[u32],
    elapsed: Duration,
) -> Result<(), ScanIdentityError> {
    if elapsed > MAX_SCAN_AGE {
        return Err(ScanIdentityError::Expired);
    }
    let latest = unique_records(current)?;
    if *initial != latest {
        return Err(ScanIdentityError::InventoryChanged);
    }
    let unique_numbers: HashSet<_> = disk_numbers.iter().copied().collect();
    if disk_numbers.contains(&u32::MAX) || unique_numbers.len() != disk_numbers.len() {
        return Err(ScanIdentityError::DuplicateDeviceNumber);
    }
    Ok(())
}

impl ScanSnapshot {
    pub(crate) fn capture(records: &[DiskInterface]) -> Result<Self, ScanIdentityError> {
        Ok(Self {
            original: unique_records(records)?,
            started: Instant::now(),
        })
    }

    pub(crate) fn validate(
        &self,
        current: &[DiskInterface],
        disk_numbers: &[u32],
    ) -> Result<(), ScanIdentityError> {
        verify_scan(&self.original, current, disk_numbers, self.started.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(path: &str, id: &str) -> DiskInterface {
        DiskInterface {
            path: path.encode_utf16().collect(),
            instance_id: id.encode_utf16().collect(),
        }
    }

    fn baseline() -> Vec<DiskInterface> {
        vec![disk("path-A", "instance-A"), disk("path-B", "instance-B")]
    }

    #[test]
    fn accepts_reordering_of_identical_path_instance_pairs() {
        let snap = ScanSnapshot::capture(&baseline()).unwrap();
        assert_eq!(
            snap.validate(
                &[disk("path-B", "instance-B"), disk("path-A", "instance-A")],
                &[3, 7]
            ),
            Ok(())
        );
    }

    #[test]
    fn removal_addition_and_path_reassignment_fail_closed() {
        let snap = ScanSnapshot::capture(&baseline()).unwrap();
        assert_eq!(
            snap.validate(&[disk("path-A", "instance-A")], &[3]),
            Err(ScanIdentityError::InventoryChanged)
        );
        assert_eq!(
            snap.validate(
                &[disk("path-A", "instance-A"), disk("path-C", "instance-C")],
                &[3, 7]
            ),
            Err(ScanIdentityError::InventoryChanged)
        );
        assert_eq!(
            snap.validate(
                &[disk("path-A", "instance-B"), disk("path-B", "instance-A")],
                &[3, 7]
            ),
            Err(ScanIdentityError::InventoryChanged)
        );
    }

    #[test]
    fn changed_instance_id_rejected_even_if_path_unchanged() {
        let snap = ScanSnapshot::capture(&baseline()).unwrap();
        assert_eq!(
            snap.validate(
                &[disk("path-A", "new-instance"), disk("path-B", "instance-B")],
                &[3, 7]
            ),
            Err(ScanIdentityError::InventoryChanged)
        );
        // Same instance ID after replug STILL does not prove same hardware.
        assert_eq!(snap.validate(&baseline(), &[3, 7]), Ok(()));
    }

    #[test]
    fn ambiguous_or_invalid_instance_and_path_are_rejected() {
        assert!(matches!(
            ScanSnapshot::capture(&[disk("p", "instance"), disk("p", "instance-2")]),
            Err(ScanIdentityError::DuplicateLocator)
        ));
        assert!(matches!(
            ScanSnapshot::capture(&[disk("p", "instance"), disk("q", "instance")]),
            Err(ScanIdentityError::DuplicateInstanceId)
        ));
        assert!(matches!(
            ScanSnapshot::capture(&[disk("p", "")]),
            Err(ScanIdentityError::InvalidInstanceId)
        ));
        assert!(matches!(
            ScanSnapshot::capture(&[disk("", "id")]),
            Err(ScanIdentityError::InvalidLocator)
        ));
        let mut bad = disk("p", "id");
        bad.instance_id.push(0);
        assert!(matches!(
            ScanSnapshot::capture(&[bad]),
            Err(ScanIdentityError::InvalidInstanceId)
        ));
    }

    #[test]
    fn duplicate_temporary_numbers_and_age_expire_the_whole_scan() {
        let records = baseline();
        let snap = ScanSnapshot::capture(&records).unwrap();
        assert_eq!(
            snap.validate(&records, &[11, 11]),
            Err(ScanIdentityError::DuplicateDeviceNumber)
        );
        assert_eq!(
            snap.validate(&records, &[u32::MAX]),
            Err(ScanIdentityError::DuplicateDeviceNumber)
        );
        assert_eq!(
            verify_scan(
                &unique_records(&records).unwrap(),
                &records,
                &[1, 2],
                Duration::from_secs(121)
            ),
            Err(ScanIdentityError::Expired)
        );
        assert_eq!(
            verify_scan(
                &unique_records(&records).unwrap(),
                &records,
                &[1, 2],
                Duration::from_secs(120)
            ),
            Ok(())
        );
    }
}
