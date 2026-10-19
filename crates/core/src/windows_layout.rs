//! P2-I2b: bounded *pure* parser for Windows DRIVE_LAYOUT_INFORMATION_EX.
//!
//! This module does not execute IOCTLs or open any disk. The agent may use it
//! only after independent source, handle and metadata authorization.
//! Windows x64/ARM64 ABI offsets are deliberately explicit; native C layout
//! conformance and actual Windows hardware acceptance are still required.

use std::collections::HashSet;

pub const HEADER_BYTES: usize = 48;
pub const ENTRY_BYTES: usize = 144;
pub const MAX_ENTRIES: usize = 128;
pub const MAX_RESPONSE_BYTES: usize = HEADER_BYTES + ENTRY_BYTES * MAX_ENTRIES;

const MBR: u32 = 0;
const GPT: u32 = 1;
const RAW: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutStyle {
    Mbr,
    Gpt,
    Raw,
}

impl LayoutStyle {
    fn from_raw(raw: u32) -> Result<Self, LayoutError> {
        match raw {
            MBR => Ok(Self::Mbr),
            GPT => Ok(Self::Gpt),
            RAW => Ok(Self::Raw),
            _ => Err(LayoutError::InvalidStyle),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartitionRange {
    pub number: u32,
    pub offset: u64,
    pub length: u64,
    pub service: bool,
    pub partition_type: PartitionType,
}

impl PartitionRange {
    pub fn is_protected_for_benchmark(&self) -> bool {
        self.service || self.partition_type.is_protected_for_benchmark()
    }
}

/// Partition format-specific metadata returned by PARTITION_INFORMATION_EX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    Mbr {
        type_id: u8,
        boot_indicator: bool,
    },
    /// Raw Windows GUID ABI bytes (the first three GUID fields are little-endian).
    Gpt([u8; 16]),
}

impl PartitionType {
    /// Conservative boot/recovery classes that make a containing disk
    /// ineligible as a file-benchmark target.
    pub fn is_protected_for_benchmark(self) -> bool {
        match self {
            Self::Mbr {
                type_id,
                boot_indicator,
            } => boot_indicator || matches!(type_id, 0x27 | 0xef),
            Self::Gpt(guid) => matches!(
                guid,
                // EFI System Partition: C12A7328-F81F-11D2-BA4B-00A0C93EC93B
                [0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b]
                    // Microsoft Reserved: E3C9E316-0B5C-4DB8-817D-F92DF00215AE
                    | [0x16, 0xe3, 0xc9, 0xe3, 0x5c, 0x0b, 0xb8, 0x4d, 0x81, 0x7d, 0xf9, 0x2d, 0xf0, 0x02, 0x15, 0xae]
                    // Microsoft Recovery: DE94BBA4-06D1-4D40-A16A-BFD50179D6AC
                    | [0xa4, 0xbb, 0x94, 0xde, 0xd1, 0x06, 0x40, 0x4d, 0xa1, 0x6a, 0xbf, 0xd5, 0x01, 0x79, 0xd6, 0xac]
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveLayout {
    pub style: LayoutStyle,
    /// Windows may return empty MBR slots; only occupied extents are included.
    pub occupied: Vec<PartitionRange>,
    pub returned_slots: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    Truncated,
    ExcessiveCount,
    InvalidStyle,
    InvalidGeometry,
    DuplicateNumber,
    Overlap,
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, LayoutError> {
    let bytes = data.get(offset..offset + 4).ok_or(LayoutError::Truncated)?;
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| LayoutError::Truncated)?,
    ))
}

fn read_i64(data: &[u8], offset: usize) -> Result<i64, LayoutError> {
    let bytes = data.get(offset..offset + 8).ok_or(LayoutError::Truncated)?;
    Ok(i64::from_le_bytes(
        bytes.try_into().map_err(|_| LayoutError::Truncated)?,
    ))
}

/// Decode *metadata only*; caller supplies a separately verified disk size.
/// Unknown or inconsistent sector geometry is never guessed as 512 bytes.
pub fn parse_drive_layout(
    bytes: &[u8],
    disk_capacity: u64,
    logical_sector_size: Option<u32>,
) -> Result<DriveLayout, LayoutError> {
    if bytes.len() < HEADER_BYTES || bytes.len() > MAX_RESPONSE_BYTES || disk_capacity == 0 {
        return Err(LayoutError::Truncated);
    }
    let style = LayoutStyle::from_raw(read_u32(bytes, 0)?)?;
    let count = read_u32(bytes, 4)? as usize;
    if count > MAX_ENTRIES {
        return Err(LayoutError::ExcessiveCount);
    }
    let size = count
        .checked_mul(ENTRY_BYTES)
        .and_then(|n| HEADER_BYTES.checked_add(n))
        .ok_or(LayoutError::ExcessiveCount)?;
    if bytes.len() < size {
        return Err(LayoutError::Truncated);
    }
    if style == LayoutStyle::Raw && count != 0 {
        return Err(LayoutError::InvalidGeometry);
    }
    // Microsoft's documented MBR response includes vacant slots in groups
    // of four; reject truncated/odd tables rather than guessing entries.
    if style == LayoutStyle::Mbr && count % 4 != 0 {
        return Err(LayoutError::InvalidGeometry);
    }
    let sector = logical_sector_size.map(u64::from);
    if let Some(s) = sector {
        if !(512..=65536).contains(&s) || !s.is_power_of_two() {
            return Err(LayoutError::InvalidGeometry);
        }
    }
    let mut occupied = Vec::new();
    let mut numbers = HashSet::new();
    for index in 0..count {
        let start = HEADER_BYTES + index * ENTRY_BYTES;
        let entry = &bytes[start..start + ENTRY_BYTES];
        let entry_style = LayoutStyle::from_raw(read_u32(entry, 0)?)?;
        let begin = read_i64(entry, 8)?;
        let length = read_i64(entry, 16)?;
        let number = read_u32(entry, 24)?;
        let service = entry[29] != 0;
        if length == 0 {
            // Vacant MBR slots and unused GPT slots do not describe extents.
            continue;
        }
        if entry_style != style || style == LayoutStyle::Raw || begin < 0 || length < 0 {
            return Err(LayoutError::InvalidGeometry);
        }
        let offset = begin as u64;
        let size = length as u64;
        let end = offset
            .checked_add(size)
            .ok_or(LayoutError::InvalidGeometry)?;
        if offset == 0 || end > disk_capacity || number == 0 {
            return Err(LayoutError::InvalidGeometry);
        }
        if let Some(s) = sector {
            if offset % s != 0 || size % s != 0 {
                return Err(LayoutError::InvalidGeometry);
            }
        }
        if style == LayoutStyle::Mbr && entry[32] == 0 {
            return Err(LayoutError::InvalidGeometry);
        }
        if style == LayoutStyle::Gpt && entry[32..48].iter().all(|b| *b == 0) {
            return Err(LayoutError::InvalidGeometry);
        }
        let partition_type = match style {
            LayoutStyle::Mbr => PartitionType::Mbr {
                type_id: entry[32],
                boot_indicator: entry[33] != 0,
            },
            LayoutStyle::Gpt => PartitionType::Gpt(
                entry[32..48]
                    .try_into()
                    .map_err(|_| LayoutError::Truncated)?,
            ),
            LayoutStyle::Raw => return Err(LayoutError::InvalidGeometry),
        };
        if !numbers.insert(number) {
            return Err(LayoutError::DuplicateNumber);
        }
        occupied.push(PartitionRange {
            number,
            offset,
            length: size,
            service,
            partition_type,
        });
    }
    occupied.sort_by_key(|part| part.offset);
    for pair in occupied.windows(2) {
        if pair[0].offset + pair[0].length > pair[1].offset {
            return Err(LayoutError::Overlap);
        }
    }
    Ok(DriveLayout {
        style,
        occupied,
        returned_slots: count as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(style: u32, count: usize) -> Vec<u8> {
        let mut bytes = vec![0_u8; HEADER_BYTES + ENTRY_BYTES * count];
        bytes[..4].copy_from_slice(&style.to_le_bytes());
        bytes[4..8].copy_from_slice(&(count as u32).to_le_bytes());
        bytes
    }

    fn entry(b: &mut [u8], i: usize, style: u32, number: u32, offset: i64, len: i64) {
        let start = HEADER_BYTES + ENTRY_BYTES * i;
        b[start..start + 4].copy_from_slice(&style.to_le_bytes());
        b[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        b[start + 16..start + 24].copy_from_slice(&len.to_le_bytes());
        b[start + 24..start + 28].copy_from_slice(&number.to_le_bytes());
        match style {
            MBR => b[start + 32] = 0x83,
            GPT => b[start + 32] = 1,
            _ => {}
        }
    }

    #[test]
    fn parses_mbr_with_unused_slots() {
        let mut bytes = fixture(MBR, 4);
        entry(&mut bytes, 0, MBR, 1, 4096, 8192);
        let r = parse_drive_layout(&bytes, 65_536, Some(512)).unwrap();
        assert_eq!(r.style, LayoutStyle::Mbr);
        assert_eq!(r.returned_slots, 4);
        assert_eq!(r.occupied.len(), 1);
        assert_eq!(r.occupied[0].offset, 4096);
    }

    #[test]
    fn parses_gpt_and_orders_by_offset() {
        let mut bytes = fixture(GPT, 2);
        entry(&mut bytes, 0, GPT, 2, 16384, 4096);
        entry(&mut bytes, 1, GPT, 1, 4096, 4096);
        let r = parse_drive_layout(&bytes, 65_536, Some(4096)).unwrap();
        assert_eq!(r.occupied[0].number, 1);
        assert_eq!(r.occupied[1].number, 2);
    }

    #[test]
    fn rejects_truncated_unknown_and_excessive() {
        assert_eq!(
            parse_drive_layout(&[], 65_536, None),
            Err(LayoutError::Truncated)
        );
        let mut bytes = fixture(GPT, 1);
        assert_eq!(
            parse_drive_layout(&bytes[..bytes.len() - 1], 65_536, None),
            Err(LayoutError::Truncated)
        );
        bytes[4..8].copy_from_slice(&129_u32.to_le_bytes());
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, None),
            Err(LayoutError::ExcessiveCount)
        );
        bytes[..4].copy_from_slice(&55_u32.to_le_bytes());
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, None),
            Err(LayoutError::InvalidStyle)
        );
    }

    #[test]
    fn rejects_overlap_duplicates_and_out_of_bounds() {
        let mut bytes = fixture(GPT, 2);
        entry(&mut bytes, 0, GPT, 1, 4096, 8192);
        entry(&mut bytes, 1, GPT, 2, 8192, 4096);
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, Some(512)),
            Err(LayoutError::Overlap)
        );
        entry(&mut bytes, 1, GPT, 1, 16384, 4096);
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, Some(512)),
            Err(LayoutError::DuplicateNumber)
        );
        entry(&mut bytes, 1, GPT, 2, 65_536, 4096);
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, Some(512)),
            Err(LayoutError::InvalidGeometry)
        );
    }

    #[test]
    fn rejects_misaligned_negative_and_wrapped_ranges() {
        let mut bytes = fixture(GPT, 1);
        entry(&mut bytes, 0, GPT, 1, 512, 4096);
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, Some(4096)),
            Err(LayoutError::InvalidGeometry)
        );
        entry(&mut bytes, 0, GPT, 1, -4096, 4096);
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, None),
            Err(LayoutError::InvalidGeometry)
        );
        entry(&mut bytes, 0, GPT, 1, i64::MAX, i64::MAX);
        assert_eq!(
            parse_drive_layout(&bytes, 65_536, None),
            Err(LayoutError::InvalidGeometry)
        );
    }

    #[test]
    fn raw_layout_only_accepts_zero_partition_count() {
        assert_eq!(
            parse_drive_layout(&fixture(RAW, 0), 65_536, None)
                .unwrap()
                .occupied
                .len(),
            0
        );
        assert_eq!(
            parse_drive_layout(&fixture(RAW, 1), 65_536, None),
            Err(LayoutError::InvalidGeometry)
        );
    }

    #[test]
    fn identifies_boot_and_recovery_partition_types_for_benchmark_protection() {
        let mut bytes = fixture(GPT, 4);
        let types = [
            // Windows GUID memory layout for EFI System, Microsoft Reserved,
            // Microsoft Recovery, then an ordinary Basic Data partition.
            [
                0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e,
                0xc9, 0x3b,
            ],
            [
                0x16, 0xe3, 0xc9, 0xe3, 0x5c, 0x0b, 0xb8, 0x4d, 0x81, 0x7d, 0xf9, 0x2d, 0xf0, 0x02,
                0x15, 0xae,
            ],
            [
                0xa4, 0xbb, 0x94, 0xde, 0xd1, 0x06, 0x40, 0x4d, 0xa1, 0x6a, 0xbf, 0xd5, 0x01, 0x79,
                0xd6, 0xac,
            ],
            [
                0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26,
                0x99, 0xc7,
            ],
        ];
        for (index, partition_type) in types.iter().enumerate() {
            let offset = 4096 + index as i64 * 8192;
            entry(&mut bytes, index, GPT, index as u32 + 1, offset, 4096);
            let start = HEADER_BYTES + ENTRY_BYTES * index;
            bytes[start + 32..start + 48].copy_from_slice(partition_type);
        }

        let layout = parse_drive_layout(&bytes, 65_536, Some(512)).unwrap();
        assert!(layout.occupied[0].is_protected_for_benchmark());
        assert!(layout.occupied[1].is_protected_for_benchmark());
        assert!(layout.occupied[2].is_protected_for_benchmark());
        assert!(!layout.occupied[3].is_protected_for_benchmark());

        let mut mbr = fixture(MBR, 4);
        entry(&mut mbr, 0, MBR, 1, 4096, 4096);
        let start = HEADER_BYTES;
        mbr[start + 32] = 0x27; // Windows recovery partition.
        let recovery = parse_drive_layout(&mbr, 65_536, Some(512)).unwrap();
        assert!(recovery.occupied[0].is_protected_for_benchmark());

        mbr[start + 32] = 0x07; // Ordinary NTFS/exFAT data partition type.
        let ordinary = parse_drive_layout(&mbr, 65_536, Some(512)).unwrap();
        assert!(!ordinary.occupied[0].is_protected_for_benchmark());

        mbr[start + 32] = 0xef; // MBR EFI system partition.
        let efi = parse_drive_layout(&mbr, 65_536, Some(512)).unwrap();
        assert!(efi.occupied[0].is_protected_for_benchmark());

        mbr[start + 32] = 0x07;
        mbr[start + 33] = 1; // Active/bootable MBR partition.
        let boot = parse_drive_layout(&mbr, 65_536, Some(512)).unwrap();
        assert!(boot.occupied[0].is_protected_for_benchmark());

        let mut service = fixture(GPT, 1);
        entry(&mut service, 0, GPT, 1, 4096, 4096);
        service[HEADER_BYTES + 29] = 1;
        let service_layout = parse_drive_layout(&service, 65_536, Some(512)).unwrap();
        assert!(service_layout.occupied[0].is_protected_for_benchmark());
    }
}
