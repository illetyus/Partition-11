//! Pure, fail-closed policy for associating an OS volume with this scan's
//! read-only disk metadata. This module grants no write capability.

use crate::disk_metadata::ProbeOutcome;
use partition11_core::windows_layout::{DriveLayout, LayoutStyle, PartitionRange, PartitionType};

pub(crate) const GIB: u64 = 1024 * 1024 * 1024;
/// The initial standard benchmark file is 1 GiB. Keep another GiB available
/// after allocation so an eligible volume is never selected at the edge.
pub(crate) const MIN_AVAILABLE_BYTES: u64 = 2 * GIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DriveType {
    Unknown,
    NoRoot,
    Removable,
    Fixed,
    Remote,
    Cdrom,
    Ramdisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VolumeExtent {
    pub disk_number: u32,
    pub starting_offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VolumeFacts {
    /// Mounted paths returned by Windows. Only one drive-letter root is
    /// eligible; folder mount points are deliberately not target paths.
    pub mount_roots: Vec<String>,
    pub drive_type: DriveType,
    pub file_system: Option<String>,
    pub read_only: bool,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub is_system_volume: bool,
    pub system_volume_identity_known: bool,
    pub system_disk_number: Option<u32>,
    pub extents_known: bool,
    pub extents: Vec<VolumeExtent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EligibilityReason {
    Eligible,
    SystemVolume,
    SystemVolumeUnknown,
    NoDriveLetterMount,
    AmbiguousMount,
    UnsupportedDriveType,
    UnsupportedFileSystem,
    ReadOnly,
    MissingSpaceMetadata,
    InsufficientSpace,
    MultipleExtents,
    MissingExtents,
    InvalidExtent,
    UnknownDisk,
    AmbiguousDisk,
    BootDisk,
    PartitionUnmapped,
    ProtectedPartition,
    UnsupportedPartition,
}

impl EligibilityReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::SystemVolume => "system_volume",
            Self::SystemVolumeUnknown => "system_volume_unknown",
            Self::NoDriveLetterMount => "no_drive_letter_mount",
            Self::AmbiguousMount => "ambiguous_mount",
            Self::UnsupportedDriveType => "unsupported_drive_type",
            Self::UnsupportedFileSystem => "unsupported_filesystem",
            Self::ReadOnly => "read_only",
            Self::MissingSpaceMetadata => "missing_space_metadata",
            Self::InsufficientSpace => "insufficient_space",
            Self::MultipleExtents => "multiple_extents",
            Self::MissingExtents => "missing_extents",
            Self::InvalidExtent => "invalid_extent",
            Self::UnknownDisk => "unknown_disk",
            Self::AmbiguousDisk => "ambiguous_disk",
            Self::BootDisk => "boot_disk",
            Self::PartitionUnmapped => "partition_unmapped",
            Self::ProtectedPartition => "protected_partition",
            Self::UnsupportedPartition => "unsupported_partition",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Assessment {
    pub eligible: bool,
    pub reason: EligibilityReason,
    pub disk_ordinal: Option<u32>,
    pub partition_number: Option<u32>,
}

impl Assessment {
    fn denied(reason: EligibilityReason) -> Self {
        Self {
            eligible: false,
            reason,
            disk_ordinal: None,
            partition_number: None,
        }
    }
}

pub(crate) fn is_drive_letter_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

fn supported_file_system(name: &str) -> bool {
    ["NTFS", "REFS", "EXFAT", "FAT", "FAT32"]
        .iter()
        .any(|allowed| name.eq_ignore_ascii_case(allowed))
}

fn is_efi_partition(partition: &PartitionRange) -> bool {
    matches!(
        partition.partition_type,
        PartitionType::Gpt([
            0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e,
            0xc9, 0x3b,
        ])
    )
}

fn disk_has_boot_partition(layout: &DriveLayout) -> bool {
    layout.occupied.iter().any(|partition| {
        is_efi_partition(partition)
            || matches!(
                partition.partition_type,
                PartitionType::Mbr {
                    boot_indicator: true,
                    ..
                }
            )
            || matches!(
                partition.partition_type,
                PartitionType::Mbr { type_id: 0xef, .. }
            )
    })
}

fn is_basic_data_partition(partition: &PartitionRange) -> bool {
    match partition.partition_type {
        PartitionType::Mbr {
            type_id,
            boot_indicator: false,
        } => {
            matches!(type_id, 0x07 | 0x0b | 0x0c)
        }
        PartitionType::Gpt(guid) => {
            guid == [
                0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26,
                0x99, 0xc7,
            ]
        }
        PartitionType::Mbr {
            boot_indicator: true,
            ..
        } => false,
    }
}

pub(crate) fn mapping_matches_disk_layout(extent: VolumeExtent, disks: &[ProbeOutcome]) -> bool {
    if extent.length == 0 {
        return false;
    }
    let Some(extent_end) = extent.starting_offset.checked_add(extent.length) else {
        return false;
    };
    let mut matching_disks = disks.iter().filter_map(|outcome| match outcome {
        ProbeOutcome::Ready(metadata) if metadata.device_number == extent.disk_number => {
            Some(metadata)
        }
        _ => None,
    });
    let Some(metadata) = matching_disks.next() else {
        return false;
    };
    if matching_disks.next().is_some()
        || metadata
            .capacity_bytes
            .as_ref()
            .ok()
            .is_none_or(|capacity| extent_end > *capacity)
    {
        return false;
    }
    let Ok(layout) = &metadata.partition_layout else {
        return false;
    };
    let mut matching_partitions = layout.occupied.iter().filter(|partition| {
        partition.offset <= extent.starting_offset
            && partition
                .offset
                .checked_add(partition.length)
                .is_some_and(|partition_end| extent_end <= partition_end)
    });
    matching_partitions.next().is_some() && matching_partitions.next().is_none()
}

pub(crate) fn assess(volume: &VolumeFacts, disks: &[ProbeOutcome]) -> Assessment {
    let deny = Assessment::denied;
    if !volume.system_volume_identity_known {
        return deny(EligibilityReason::SystemVolumeUnknown);
    }
    if volume.is_system_volume {
        return deny(EligibilityReason::SystemVolume);
    }
    if !volume
        .mount_roots
        .iter()
        .any(|root| is_drive_letter_root(root))
    {
        return deny(EligibilityReason::NoDriveLetterMount);
    }
    if volume.mount_roots.len() != 1 || !is_drive_letter_root(&volume.mount_roots[0]) {
        return deny(EligibilityReason::AmbiguousMount);
    }
    if !matches!(volume.drive_type, DriveType::Fixed | DriveType::Removable) {
        return deny(EligibilityReason::UnsupportedDriveType);
    }
    if !volume
        .file_system
        .as_deref()
        .is_some_and(supported_file_system)
    {
        return deny(EligibilityReason::UnsupportedFileSystem);
    }
    if volume.read_only {
        return deny(EligibilityReason::ReadOnly);
    }
    let (Some(total_bytes), Some(available_bytes)) = (volume.total_bytes, volume.available_bytes)
    else {
        return deny(EligibilityReason::MissingSpaceMetadata);
    };
    if total_bytes == 0 || available_bytes > total_bytes {
        return deny(EligibilityReason::MissingSpaceMetadata);
    }
    if available_bytes < MIN_AVAILABLE_BYTES {
        return deny(EligibilityReason::InsufficientSpace);
    }
    if !volume.extents_known {
        return deny(EligibilityReason::MissingExtents);
    }
    if volume.extents.len() != 1 {
        return deny(EligibilityReason::MultipleExtents);
    }
    let extent = volume.extents[0];
    if extent.length == 0 || extent.starting_offset.checked_add(extent.length).is_none() {
        return deny(EligibilityReason::InvalidExtent);
    }
    if volume.system_disk_number == Some(extent.disk_number) {
        return deny(EligibilityReason::SystemVolume);
    }
    let extent_end = extent.starting_offset + extent.length;
    let mut matching_disks =
        disks
            .iter()
            .enumerate()
            .filter_map(|(index, outcome)| match outcome {
                ProbeOutcome::Ready(metadata) if metadata.device_number == extent.disk_number => {
                    Some((index as u32 + 1, metadata))
                }
                _ => None,
            });
    let Some((disk_ordinal, metadata)) = matching_disks.next() else {
        return deny(EligibilityReason::UnknownDisk);
    };
    if matching_disks.next().is_some() {
        return deny(EligibilityReason::AmbiguousDisk);
    }
    let Ok(capacity_bytes) = metadata.capacity_bytes else {
        return deny(EligibilityReason::UnknownDisk);
    };
    if extent_end > capacity_bytes {
        return deny(EligibilityReason::InvalidExtent);
    }
    let Ok(layout) = &metadata.partition_layout else {
        return deny(EligibilityReason::UnknownDisk);
    };
    if disk_has_boot_partition(layout) {
        return deny(EligibilityReason::BootDisk);
    }
    let mut matching_partitions = layout.occupied.iter().filter(|partition| {
        partition.offset <= extent.starting_offset
            && partition
                .offset
                .checked_add(partition.length)
                .is_some_and(|partition_end| extent_end <= partition_end)
    });
    let Some(partition) = matching_partitions.next() else {
        return deny(EligibilityReason::PartitionUnmapped);
    };
    if matching_partitions.next().is_some() {
        return deny(EligibilityReason::PartitionUnmapped);
    }
    if partition.is_protected_for_benchmark() {
        return deny(EligibilityReason::ProtectedPartition);
    }
    if !is_basic_data_partition(partition) || layout.style == LayoutStyle::Raw {
        return deny(EligibilityReason::UnsupportedPartition);
    }
    Assessment {
        eligible: true,
        reason: EligibilityReason::Eligible,
        disk_ordinal: Some(disk_ordinal),
        partition_number: Some(partition.number),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk_metadata::{DiskMetadata, MetadataError, ProbeOutcome};
    use crate::metadata_query::SectorSizes;
    use partition11_core::windows_layout::{
        DriveLayout, LayoutStyle, PartitionRange, PartitionType,
    };

    fn basic_data_gpt() -> PartitionType {
        PartitionType::Gpt([
            0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26,
            0x99, 0xc7,
        ])
    }

    fn partition(number: u32, offset: u64, length: u64, kind: PartitionType) -> PartitionRange {
        PartitionRange {
            number,
            offset,
            length,
            service: false,
            partition_type: kind,
        }
    }

    fn disk(layout: DriveLayout) -> ProbeOutcome {
        ProbeOutcome::Ready(DiskMetadata {
            device_number: 4,
            capacity_bytes: Ok(20 * GIB),
            sectors: Ok(SectorSizes {
                logical: Some(512),
                physical: Some(4096),
            }),
            bus_type: Ok(Some(7)),
            partition_layout: Ok(layout),
        })
    }

    fn volume() -> VolumeFacts {
        VolumeFacts {
            mount_roots: vec!["E:\\".to_owned()],
            drive_type: DriveType::Fixed,
            file_system: Some("NTFS".to_owned()),
            read_only: false,
            total_bytes: Some(10 * GIB),
            available_bytes: Some(4 * GIB),
            is_system_volume: false,
            system_volume_identity_known: true,
            system_disk_number: Some(0),
            extents_known: true,
            extents: vec![VolumeExtent {
                disk_number: 4,
                starting_offset: 1024 * 1024,
                length: 8 * GIB,
            }],
        }
    }

    fn data_disk() -> ProbeOutcome {
        disk(DriveLayout {
            style: LayoutStyle::Gpt,
            occupied: vec![partition(1, 1024 * 1024, 8 * GIB, basic_data_gpt())],
            returned_slots: 1,
        })
    }

    #[test]
    fn accepts_one_mounted_supported_volume_mapped_to_basic_data_partition() {
        let result = assess(&volume(), &[data_disk()]);
        assert!(result.eligible);
        assert_eq!(result.disk_ordinal, Some(1));
        assert_eq!(result.partition_number, Some(1));
        assert_eq!(result.reason, EligibilityReason::Eligible);
    }

    #[test]
    fn rejects_windows_volume_even_when_its_partition_is_basic_data() {
        let mut facts = volume();
        facts.is_system_volume = true;
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::SystemVolume
        );
    }

    #[test]
    fn rejects_volumes_spanning_multiple_disk_extents() {
        let mut facts = volume();
        facts.extents.push(VolumeExtent {
            disk_number: 5,
            starting_offset: 0,
            length: GIB,
        });
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::MultipleExtents
        );
    }

    #[test]
    fn rejects_disk_with_efi_or_active_mbr_boot_partition() {
        let efi = PartitionType::Gpt([
            0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e,
            0xc9, 0x3b,
        ]);
        let mut layout = match data_disk() {
            ProbeOutcome::Ready(metadata) => metadata.partition_layout.unwrap(),
            ProbeOutcome::Unavailable(_) => unreachable!(),
        };
        layout.occupied.push(partition(2, 9 * GIB, GIB, efi));
        assert_eq!(
            assess(&volume(), &[disk(layout)]).reason,
            EligibilityReason::BootDisk
        );

        let active_mbr = PartitionType::Mbr {
            type_id: 0x07,
            boot_indicator: true,
        };
        let mut facts = volume();
        facts.extents[0].disk_number = 4;
        assert_eq!(
            assess(
                &facts,
                &[disk(DriveLayout {
                    style: LayoutStyle::Mbr,
                    occupied: vec![partition(1, 1024 * 1024, 8 * GIB, active_mbr)],
                    returned_slots: 1,
                })]
            )
            .reason,
            EligibilityReason::BootDisk
        );
    }

    #[test]
    fn denies_exact_recovery_and_microsoft_reserved_targets() {
        let recovery = PartitionType::Gpt([
            0xa4, 0xbb, 0x94, 0xde, 0xd1, 0x06, 0x40, 0x4d, 0xa1, 0x6a, 0xbf, 0xd5, 0x01, 0x79,
            0xd6, 0xac,
        ]);
        let mut facts = volume();
        facts.extents[0].length = GIB;
        assert_eq!(
            assess(
                &facts,
                &[disk(DriveLayout {
                    style: LayoutStyle::Gpt,
                    occupied: vec![partition(1, 1024 * 1024, GIB, recovery)],
                    returned_slots: 1,
                })]
            )
            .reason,
            EligibilityReason::ProtectedPartition
        );

        let msr = PartitionType::Gpt([
            0x16, 0xe3, 0xc9, 0xe3, 0x5c, 0x0b, 0xb8, 0x4d, 0x81, 0x7d, 0xf9, 0x2d, 0xf0, 0x02,
            0x15, 0xae,
        ]);
        assert_eq!(
            assess(
                &volume(),
                &[disk(DriveLayout {
                    style: LayoutStyle::Gpt,
                    occupied: vec![partition(1, 1024 * 1024, 8 * GIB, msr)],
                    returned_slots: 1,
                })]
            )
            .reason,
            EligibilityReason::ProtectedPartition
        );
    }

    #[test]
    fn permits_data_volume_on_disk_that_only_contains_a_separate_msr() {
        let msr = PartitionType::Gpt([
            0x16, 0xe3, 0xc9, 0xe3, 0x5c, 0x0b, 0xb8, 0x4d, 0x81, 0x7d, 0xf9, 0x2d, 0xf0, 0x02,
            0x15, 0xae,
        ]);
        let layout = DriveLayout {
            style: LayoutStyle::Gpt,
            occupied: vec![
                partition(1, 1024 * 1024, GIB, msr),
                partition(2, 2 * GIB, 8 * GIB, basic_data_gpt()),
            ],
            returned_slots: 2,
        };
        let mut facts = volume();
        facts.extents[0].starting_offset = 2 * GIB;
        assert!(assess(&facts, &[disk(layout)]).eligible);
    }

    #[test]
    fn rejects_unsupported_filesystems_read_only_and_unsupported_drive_types() {
        let mut facts = volume();
        facts.file_system = Some("ext4".to_owned());
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::UnsupportedFileSystem
        );
        facts = volume();
        facts.read_only = true;
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::ReadOnly
        );
        facts = volume();
        facts.drive_type = DriveType::Remote;
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::UnsupportedDriveType
        );
    }

    #[test]
    fn rejects_missing_ambiguous_mount_or_insufficient_free_space() {
        let mut facts = volume();
        facts.mount_roots.clear();
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::NoDriveLetterMount
        );
        facts = volume();
        facts.mount_roots.push("F:\\".to_owned());
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::AmbiguousMount
        );
        facts = volume();
        facts.available_bytes = Some(2 * GIB - 1);
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::InsufficientSpace
        );
    }

    #[test]
    fn rejects_unmatched_ambiguous_or_overflowing_extents() {
        let mut facts = volume();
        facts.extents[0].disk_number = 99;
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::UnknownDisk
        );
        facts = volume();
        facts.extents[0].starting_offset = u64::MAX - 5;
        facts.extents[0].length = 10;
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::InvalidExtent
        );
    }

    #[test]
    fn denies_metadata_without_layout_or_with_duplicate_temporary_disk_number() {
        let unavailable = ProbeOutcome::Unavailable(MetadataError::Unsupported);
        assert_eq!(
            assess(&volume(), &[unavailable]).reason,
            EligibilityReason::UnknownDisk
        );
        let second = disk(DriveLayout {
            style: LayoutStyle::Gpt,
            occupied: vec![partition(2, 1024 * 1024, 8 * GIB, basic_data_gpt())],
            returned_slots: 1,
        });
        assert_eq!(
            assess(&volume(), &[data_disk(), second]).reason,
            EligibilityReason::AmbiguousDisk
        );
    }

    #[test]
    fn rejects_data_volume_on_the_windows_system_disk() {
        let mut facts = volume();
        facts.system_disk_number = Some(4);
        let result = assess(&facts, &[data_disk()]);
        assert!(!result.eligible);
        assert_eq!(result.reason, EligibilityReason::SystemVolume);
    }

    #[test]
    fn rejects_folder_only_and_aliased_volume_mounts() {
        let mut facts = volume();
        facts.mount_roots = vec!["E:\\Mounted\\".to_owned()];
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::NoDriveLetterMount
        );
        facts.mount_roots = vec!["E:\\".to_owned(), "C:\\Mounted\\".to_owned()];
        assert_eq!(
            assess(&facts, &[data_disk()]).reason,
            EligibilityReason::AmbiguousMount
        );
    }

    #[test]
    fn mapping_must_resolve_to_one_live_partition_range() {
        let mapping = volume().extents[0];
        assert!(mapping_matches_disk_layout(mapping, &[data_disk()]));
        assert!(!mapping_matches_disk_layout(
            VolumeExtent {
                starting_offset: 0,
                ..mapping
            },
            &[data_disk()]
        ));
        assert!(!mapping_matches_disk_layout(
            mapping,
            &[data_disk(), data_disk()]
        ));
    }
}
