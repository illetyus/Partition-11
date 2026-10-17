//! Read-only, bounded decoding of MBR and GPT partition-table bytes.
//!
//! No file or device handles are opened here. Callers must supply a sector
//! snapshot and a verified device capacity. This module has no write API.
#![forbid(unsafe_code)]

const MAX_GPT_TABLE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableError {
    InvalidSectorSize,
    Truncated,
    InvalidSignature,
    InvalidHeader,
    InvalidGeometry,
    ChecksumMismatch,
    OverlappingPartitions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbrPartition {
    pub slot: u8,
    pub bootable: bool,
    pub type_code: u8,
    pub start_lba: u64,
    pub sector_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GptHeader {
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub partition_array_lba: u64,
    pub entry_count: u32,
    pub entry_size: u32,
    pub partition_array_crc32: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GptPartition {
    pub slot: u32,
    /// The GUID is retained in on-disk byte order, not formatted as a string.
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub first_lba: u64,
    pub last_lba: u64,
    pub attributes: u64,
}

fn sector_size(sector: &[u8]) -> Result<u64, TableError> {
    match sector.len() {
        512 | 4096 => Ok(sector.len() as u64),
        _ => Err(TableError::InvalidSectorSize),
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, TableError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(TableError::Truncated)?;
    let mut copy = [0; 4];
    copy.copy_from_slice(raw);
    Ok(u32::from_le_bytes(copy))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, TableError> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or(TableError::Truncated)?;
    let mut copy = [0; 8];
    copy.copy_from_slice(raw);
    Ok(u64::from_le_bytes(copy))
}

/// Standard IEEE CRC-32, not CRC32C. GPT uses this polynomial.
fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut checksum = !0u32;
    for byte in bytes {
        let mut next = (checksum ^ u32::from(*byte)) & 0xff;
        for _ in 0..8 {
            next = (next >> 1)
                ^ if next & 1 == 1 { 0xedb8_8320 } else { 0 };
        }
        checksum = (checksum >> 8) ^ next;
    }
    !checksum
}

/// Decode only four primary MBR entries. Extended/logical partitions are NOT
/// followed; hybrid/protective entries are returned as plain entries.
pub fn parse_mbr(
    sector: &[u8],
    disk_sectors: u64,
) -> Result<Vec<MbrPartition>, TableError> {
    sector_size(sector)?;
    if disk_sectors < 2 {
        return Err(TableError::InvalidGeometry);
    }
    if sector[510] != 0x55 || sector[511] != 0xaa {
        return Err(TableError::InvalidSignature);
    }

    let mut parts = Vec::new();
    for slot in 0..4usize {
        let entry = &sector[446 + slot * 16..446 + (slot + 1) * 16];
        let boot_flag = entry[0];
        if boot_flag != 0 && boot_flag != 0x80 {
            return Err(TableError::InvalidHeader);
        }
        let type_code = entry[4];
        let start_lba = u64::from(read_u32(entry, 8)?);
        let sector_count = u64::from(read_u32(entry, 12)?);
        if type_code == 0 {
            if start_lba != 0 || sector_count != 0 {
                return Err(TableError::InvalidHeader);
            }
            continue;
        }
        if start_lba == 0 || sector_count == 0 {
            return Err(TableError::InvalidGeometry);
        }
        let end = start_lba
            .checked_add(sector_count)
            .ok_or(TableError::InvalidGeometry)?;
        if end > disk_sectors {
            return Err(TableError::InvalidGeometry);
        }
        parts.push(MbrPartition {
            slot: slot as u8,
            bootable: boot_flag == 0x80,
            type_code,
            start_lba,
            sector_count,
        });
    }
    // A hybrid/protective MBR may legitimately overlap by design. The caller
    // must distinguish such layouts; this parser never authorizes writes.
    Ok(parts)
}

/// Validate a GPT header sector using the expected physical LBA and sector
/// count. Does not prove the entry array is valid; call parse_gpt_entries.
pub fn parse_gpt_header(
    sector: &[u8],
    disk_sectors: u64,
    expected_lba: u64,
) -> Result<GptHeader, TableError> {
    let logical_sector_bytes = sector_size(sector)?;
    if disk_sectors < 34 {
        return Err(TableError::InvalidGeometry);
    }
    if &sector[0..8] != b"EFI PART" {
        return Err(TableError::InvalidSignature);
    }
    if read_u32(sector, 8)? != 0x0001_0000 {
        return Err(TableError::InvalidHeader);
    }
    let size = read_u32(sector, 12)? as usize;
    if size < 92 || size > sector.len() || read_u32(sector, 20)? != 0 {
        return Err(TableError::InvalidHeader);
    }
    let stored_crc = read_u32(sector, 16)?;
    let mut data = sector[..size].to_vec();
    data[16..20].fill(0);
    if crc32_ieee(&data) != stored_crc {
        return Err(TableError::ChecksumMismatch);
    }

    let current_lba = read_u64(sector, 24)?;
    let backup_lba = read_u64(sector, 32)?;
    let first_usable_lba = read_u64(sector, 40)?;
    let last_usable_lba = read_u64(sector, 48)?;
    let partition_array_lba = read_u64(sector, 72)?;
    let entry_count = read_u32(sector, 80)?;
    let entry_size = read_u32(sector, 84)?;
    let partition_array_crc32 = read_u32(sector, 88)?;

    if current_lba != expected_lba
        || current_lba >= disk_sectors
        || backup_lba >= disk_sectors
        || current_lba == backup_lba
        || first_usable_lba < 2
        || first_usable_lba > last_usable_lba
        || last_usable_lba >= disk_sectors
    {
        return Err(TableError::InvalidGeometry);
    }

    if entry_count == 0 || entry_size < 128 || entry_size % 8 != 0 {
        return Err(TableError::InvalidHeader);
    }
    let bytes = u64::from(entry_count)
        .checked_mul(u64::from(entry_size))
        .ok_or(TableError::InvalidGeometry)?;
    if bytes > MAX_GPT_TABLE_BYTES {
        return Err(TableError::InvalidGeometry);
    }
    let array_sectors = bytes.div_ceil(logical_sector_bytes);
    let array_end = partition_array_lba
        .checked_add(array_sectors)
        .ok_or(TableError::InvalidGeometry)?;
    if partition_array_lba < 2
        || array_end > disk_sectors
        || (partition_array_lba <= current_lba && current_lba < array_end)
        || (partition_array_lba <= backup_lba && backup_lba < array_end)
    {
        return Err(TableError::InvalidGeometry);
    }

    Ok(GptHeader {
        current_lba,
        backup_lba,
        first_usable_lba,
        last_usable_lba,
        partition_array_lba,
        entry_count,
        entry_size,
        partition_array_crc32,
    })
}

/// Validate the entire GPT array checksum, bounds and overlapping used entries.
/// No partition name decoding, filesystem reads, or disk writes are performed.
pub fn parse_gpt_entries(
    header: &GptHeader,
    array: &[u8],
) -> Result<Vec<GptPartition>, TableError> {
    let array_len = usize::try_from(
        u64::from(header.entry_count)
            .checked_mul(u64::from(header.entry_size))
            .ok_or(TableError::InvalidGeometry)?,
    )
    .map_err(|_| TableError::InvalidGeometry)?;
    if array_len as u64 > MAX_GPT_TABLE_BYTES {
        return Err(TableError::InvalidGeometry);
    }
    let array_data = array.get(..array_len).ok_or(TableError::Truncated)?;
    if crc32_ieee(array_data) != header.partition_array_crc32 {
        return Err(TableError::ChecksumMismatch);
    }
    let mut result = Vec::new();
    for (slot, raw) in array_data.chunks_exact(header.entry_size as usize).enumerate() {
        if raw[0..16].iter().all(|byte| *byte == 0) {
            if raw.iter().any(|byte| *byte != 0) {
                return Err(TableError::InvalidHeader);
            }
            continue;
        }
        let mut type_guid = [0u8; 16];
        type_guid.copy_from_slice(&raw[..16]);
        let mut unique_guid = [0u8; 16];
        unique_guid.copy_from_slice(&raw[16..32]);
        let first_lba = read_u64(raw, 32)?;
        let last_lba = read_u64(raw, 40)?;
        if unique_guid.iter().all(|byte| *byte == 0)
            || first_lba < header.first_usable_lba
            || last_lba > header.last_usable_lba
            || first_lba > last_lba
        {
            return Err(TableError::InvalidGeometry);
        }
        result.push(GptPartition {
            slot: slot as u32,
            type_guid,
            unique_guid,
            first_lba,
            last_lba,
            attributes: read_u64(raw, 48)?,
        });
    }
    let mut ranges: Vec<_> = result
        .iter()
        .map(|entry| (entry.first_lba, entry.last_lba))
        .collect();
    ranges.sort_unstable_by_key(|range| range.0);
    if ranges.windows(2).any(|pair| pair[0].1 >= pair[1].0) {
        return Err(TableError::OverlappingPartitions);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mbr() -> [u8; 512] {
        let mut sector = [0u8; 512];
        sector[510] = 0x55;
        sector[511] = 0xaa;
        sector[446] = 0x80;
        sector[450] = 0x83;
        sector[454..458].copy_from_slice(&2048u32.to_le_bytes());
        sector[458..462].copy_from_slice(&4096u32.to_le_bytes());
        sector
    }

    fn make_gpt(entries: &[u8]) -> [u8; 512] {
        let mut sector = [0u8; 512];
        sector[..8].copy_from_slice(b"EFI PART");
        sector[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        sector[12..16].copy_from_slice(&92u32.to_le_bytes());
        sector[24..32].copy_from_slice(&1u64.to_le_bytes());
        sector[32..40].copy_from_slice(&99u64.to_le_bytes());
        sector[40..48].copy_from_slice(&34u64.to_le_bytes());
        sector[48..56].copy_from_slice(&66u64.to_le_bytes());
        sector[72..80].copy_from_slice(&2u64.to_le_bytes());
        sector[80..84].copy_from_slice(&4u32.to_le_bytes());
        sector[84..88].copy_from_slice(&128u32.to_le_bytes());
        sector[88..92].copy_from_slice(&crc32_ieee(entries).to_le_bytes());
        let crc = crc32_ieee(&sector[..92]);
        sector[16..20].copy_from_slice(&crc.to_le_bytes());
        sector
    }

    #[test]
    fn crc_uses_gpt_ieee_polynomial() {
        assert_eq!(crc32_ieee(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn decodes_bounded_primary_mbr_without_io() {
        let result = parse_mbr(&mbr(), 20_000).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_lba, 2048);
        assert_eq!(result[0].sector_count, 4096);
        assert!(result[0].bootable);
    }

    #[test]
    fn rejects_mbr_invalid_signature_and_out_of_bounds() {
        let mut sector = mbr();
        sector[510] = 0;
        assert_eq!(parse_mbr(&sector, 20_000), Err(TableError::InvalidSignature));
        assert_eq!(parse_mbr(&mbr(), 6000), Err(TableError::InvalidGeometry));
        assert_eq!(parse_mbr(&mbr()[..128], 20_000), Err(TableError::InvalidSectorSize));
    }

    #[test]
    fn validates_gpt_header_and_empty_partition_array() {
        let entries = vec![0u8; 4 * 128];
        let sector = make_gpt(&entries);
        let header = parse_gpt_header(&sector, 100, 1).unwrap();
        assert_eq!(header.entry_count, 4);
        assert!(parse_gpt_entries(&header, &entries).unwrap().is_empty());

        let mut invalid = sector;
        invalid[40] ^= 1;
        assert_eq!(
            parse_gpt_header(&invalid, 100, 1),
            Err(TableError::ChecksumMismatch)
        );
    }

    #[test]
    fn validates_gpt_partition_bounds_and_overlaps() {
        let mut entries = vec![0u8; 4 * 128];
        entries[0] = 1;
        entries[16] = 2;
        entries[32..40].copy_from_slice(&40u64.to_le_bytes());
        entries[40..48].copy_from_slice(&50u64.to_le_bytes());
        let header = parse_gpt_header(&make_gpt(&entries), 100, 1).unwrap();
        let parts = parse_gpt_entries(&header, &entries).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].first_lba, 40);

        let mut corrupt = entries.clone();
        corrupt[32] ^= 1;
        assert_eq!(
            parse_gpt_entries(&header, &corrupt),
            Err(TableError::ChecksumMismatch)
        );

        let second = 128;
        entries[second] = 3;
        entries[second + 16] = 4;
        entries[second + 32..second + 40].copy_from_slice(&45u64.to_le_bytes());
        entries[second + 40..second + 48].copy_from_slice(&55u64.to_le_bytes());
        let header = parse_gpt_header(&make_gpt(&entries), 100, 1).unwrap();
        assert_eq!(
            parse_gpt_entries(&header, &entries),
            Err(TableError::OverlappingPartitions)
        );
    }

    #[test]
    fn accepts_full_4kn_header_and_rejects_impossible_geometry() {
        let entries = vec![0u8; 4 * 128];
        let small = make_gpt(&entries);
        let mut sector = [0u8; 4096];
        sector[..512].copy_from_slice(&small);
        let header = parse_gpt_header(&sector, 100, 1).unwrap();
        assert_eq!(header.current_lba, 1);
        assert_eq!(
            parse_gpt_header(&sector, 50, 1),
            Err(TableError::InvalidGeometry)
        );
        assert_eq!(
            parse_gpt_header(&sector, 100, 4),
            Err(TableError::InvalidGeometry)
        );
    }
}
