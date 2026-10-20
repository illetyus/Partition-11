//! Pure bounded protocol between the long-lived diagnostic and one-shot worker.
//!
//! The request intentionally carries only a fresh SetupAPI path/instance pair.
//! It never carries an IOCTL, operation selector, or renderer-controlled data.

use crate::storage_identity::{StableIdentifier, StableIdentifierKind, StorageIdentity};
use partition11_core::windows_layout::{
    DriveLayout, LayoutStyle, PartitionRange, PartitionType, MAX_ENTRIES,
};
use std::collections::HashSet;

pub(crate) const MAX_WIRE_BYTES: usize = 65_536;
const MAX_PATH_UNITS: usize = 16_384;
const MAX_INSTANCE_ID_UNITS: usize = 2_047;
const MAGIC: &[u8; 4] = b"P11W";
const VERSION: u16 = 3;
const REQUEST_KIND: u16 = 1;
const REPLY_KIND: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerRequest {
    pub path: Vec<u16>,
    pub instance_id: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireError {
    AccessDenied,
    DeviceRemoved,
    Unsupported,
    Malformed,
    IdentityChanged,
    LayoutTooLarge,
    TimedOut,
    Cancelled,
    WorkerFailed,
    Win32(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WireSectorSizes {
    pub logical: Option<u32>,
    pub physical: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WireMetadata {
    pub device_number: u32,
    pub capacity_bytes: Result<u64, WireError>,
    pub sectors: Result<WireSectorSizes, WireError>,
    pub bus_type: Result<Option<u32>, WireError>,
    pub stable_identity: Result<StorageIdentity, WireError>,
    pub partition_layout: Result<DriveLayout, WireError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerReply {
    Ready(WireMetadata),
    Unavailable(WireError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolError {
    Oversize,
    Truncated,
    BadMagic,
    BadVersion,
    WrongMessage,
    TrailingBytes,
    InvalidField,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProtocolError::Oversize)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| ProtocolError::Truncated)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| ProtocolError::Truncated)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| ProtocolError::Truncated)?,
        ))
    }

    fn finish(self) -> Result<(), ProtocolError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProtocolError::TrailingBytes)
        }
    }
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_header(out: &mut Vec<u8>, kind: u16) {
    out.extend_from_slice(MAGIC);
    write_u16(out, VERSION);
    write_u16(out, kind);
}

fn read_header(reader: &mut Reader<'_>, expected_kind: u16) -> Result<(), ProtocolError> {
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(ProtocolError::BadMagic);
    }
    if reader.u16()? != VERSION {
        return Err(ProtocolError::BadVersion);
    }
    if reader.u16()? != expected_kind {
        return Err(ProtocolError::WrongMessage);
    }
    Ok(())
}

fn validate_utf16(value: &[u16], max_units: usize) -> Result<(), ProtocolError> {
    if value.is_empty() {
        return Err(ProtocolError::InvalidField);
    }
    if value.len() > max_units {
        return Err(ProtocolError::Oversize);
    }
    if value.contains(&0) {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn read_utf16(
    reader: &mut Reader<'_>,
    units: usize,
    max_units: usize,
) -> Result<Vec<u16>, ProtocolError> {
    if units == 0 {
        return Err(ProtocolError::InvalidField);
    }
    if units > max_units {
        return Err(ProtocolError::Oversize);
    }
    let byte_count = units.checked_mul(2).ok_or(ProtocolError::Oversize)?;
    let bytes = reader.take(byte_count)?;
    let mut value = Vec::with_capacity(units);
    for pair in bytes.chunks_exact(2) {
        value.push(u16::from_le_bytes([pair[0], pair[1]]));
    }
    validate_utf16(&value, max_units)?;
    Ok(value)
}

pub(crate) fn encode_request(request: &WorkerRequest) -> Result<Vec<u8>, ProtocolError> {
    validate_utf16(&request.path, MAX_PATH_UNITS)?;
    validate_utf16(&request.instance_id, MAX_INSTANCE_ID_UNITS)?;
    let mut out = Vec::with_capacity(12 + (request.path.len() + request.instance_id.len()) * 2);
    write_header(&mut out, REQUEST_KIND);
    write_u16(&mut out, request.path.len() as u16);
    write_u16(&mut out, request.instance_id.len() as u16);
    for unit in request.path.iter().chain(&request.instance_id) {
        write_u16(&mut out, *unit);
    }
    if out.len() > MAX_WIRE_BYTES {
        return Err(ProtocolError::Oversize);
    }
    Ok(out)
}

pub(crate) fn decode_request(bytes: &[u8]) -> Result<WorkerRequest, ProtocolError> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(ProtocolError::Oversize);
    }
    let mut reader = Reader::new(bytes);
    read_header(&mut reader, REQUEST_KIND)?;
    let path_units = usize::from(reader.u16()?);
    let instance_units = usize::from(reader.u16()?);
    let path = read_utf16(&mut reader, path_units, MAX_PATH_UNITS)?;
    let instance_id = read_utf16(&mut reader, instance_units, MAX_INSTANCE_ID_UNITS)?;
    reader.finish()?;
    Ok(WorkerRequest { path, instance_id })
}

fn write_error(out: &mut Vec<u8>, error: WireError) {
    let tag = match error {
        WireError::AccessDenied => 1,
        WireError::DeviceRemoved => 2,
        WireError::Unsupported => 3,
        WireError::Malformed => 4,
        WireError::IdentityChanged => 5,
        WireError::LayoutTooLarge => 6,
        WireError::TimedOut => 7,
        WireError::Cancelled => 8,
        WireError::WorkerFailed => 9,
        WireError::Win32(_) => 10,
    };
    write_u8(out, tag);
    if let WireError::Win32(code) = error {
        write_u32(out, code);
    }
}

fn read_error(reader: &mut Reader<'_>) -> Result<WireError, ProtocolError> {
    match reader.u8()? {
        1 => Ok(WireError::AccessDenied),
        2 => Ok(WireError::DeviceRemoved),
        3 => Ok(WireError::Unsupported),
        4 => Ok(WireError::Malformed),
        5 => Ok(WireError::IdentityChanged),
        6 => Ok(WireError::LayoutTooLarge),
        7 => Ok(WireError::TimedOut),
        8 => Ok(WireError::Cancelled),
        9 => Ok(WireError::WorkerFailed),
        10 => Ok(WireError::Win32(reader.u32()?)),
        _ => Err(ProtocolError::InvalidField),
    }
}

fn write_result<T>(
    out: &mut Vec<u8>,
    value: &Result<T, WireError>,
    write_value: impl FnOnce(&mut Vec<u8>, &T) -> Result<(), ProtocolError>,
) -> Result<(), ProtocolError> {
    match value {
        Ok(value) => {
            write_u8(out, 0);
            write_value(out, value)
        }
        Err(error) => {
            write_u8(out, 1);
            write_error(out, *error);
            Ok(())
        }
    }
}

fn read_result<T>(
    reader: &mut Reader<'_>,
    read_value: impl FnOnce(&mut Reader<'_>) -> Result<T, ProtocolError>,
) -> Result<Result<T, WireError>, ProtocolError> {
    match reader.u8()? {
        0 => Ok(Ok(read_value(reader)?)),
        1 => Ok(Err(read_error(reader)?)),
        _ => Err(ProtocolError::InvalidField),
    }
}

fn write_option_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            write_u8(out, 1);
            write_u32(out, value);
        }
        None => write_u8(out, 0),
    }
}

fn read_option_u32(reader: &mut Reader<'_>) -> Result<Option<u32>, ProtocolError> {
    match reader.u8()? {
        0 => Ok(None),
        1 => Ok(Some(reader.u32()?)),
        _ => Err(ProtocolError::InvalidField),
    }
}

fn validate_layout(layout: &DriveLayout) -> Result<(), ProtocolError> {
    if layout.occupied.len() > MAX_ENTRIES
        || layout.returned_slots as usize > MAX_ENTRIES
        || layout.occupied.len() > layout.returned_slots as usize
    {
        return Err(ProtocolError::InvalidField);
    }
    match layout.style {
        LayoutStyle::Raw if layout.returned_slots != 0 || !layout.occupied.is_empty() => {
            return Err(ProtocolError::InvalidField)
        }
        LayoutStyle::Mbr if layout.returned_slots % 4 != 0 => {
            return Err(ProtocolError::InvalidField)
        }
        _ => {}
    }

    let mut numbers = HashSet::with_capacity(layout.occupied.len());
    let mut previous_end = 0_u64;
    for partition in &layout.occupied {
        let end = partition
            .offset
            .checked_add(partition.length)
            .ok_or(ProtocolError::InvalidField)?;
        if partition.number == 0
            || partition.offset == 0
            || partition.length == 0
            || partition.offset < previous_end
            || !numbers.insert(partition.number)
            || layout.style == LayoutStyle::Raw
        {
            return Err(ProtocolError::InvalidField);
        }
        match (layout.style, partition.partition_type) {
            (LayoutStyle::Mbr, PartitionType::Mbr { type_id, .. }) if type_id != 0 => {}
            (LayoutStyle::Gpt, PartitionType::Gpt(guid)) if guid.iter().any(|byte| *byte != 0) => {}
            _ => return Err(ProtocolError::InvalidField),
        }
        previous_end = end;
    }
    Ok(())
}

fn validate_storage_identity(identity: &StorageIdentity) -> Result<(), ProtocolError> {
    if identity.identifiers.is_empty()
        || identity.identifiers.len() > 64
        || identity
            .identifiers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(ProtocolError::InvalidField);
    }
    for identifier in &identity.identifiers {
        let valid = match identifier.kind {
            StableIdentifierKind::Eui64 | StableIdentifierKind::FcPhName => {
                identifier.bytes.len() == 8
            }
            StableIdentifierKind::ScsiNameString | StableIdentifierKind::DeviceSerial => {
                (1..=128).contains(&identifier.bytes.len())
                    && identifier
                        .bytes
                        .iter()
                        .all(|byte| (0x21..=0x7e).contains(byte))
            }
        };
        if !valid {
            return Err(ProtocolError::InvalidField);
        }
    }
    Ok(())
}

fn write_storage_identity(
    out: &mut Vec<u8>,
    identity: &StorageIdentity,
) -> Result<(), ProtocolError> {
    validate_storage_identity(identity)?;
    write_u16(out, identity.identifiers.len() as u16);
    for identifier in &identity.identifiers {
        write_u8(out, identifier.kind.tag());
        write_u16(out, identifier.bytes.len() as u16);
        out.extend_from_slice(&identifier.bytes);
    }
    Ok(())
}

fn read_storage_identity(reader: &mut Reader<'_>) -> Result<StorageIdentity, ProtocolError> {
    let count = reader.u16()? as usize;
    if count == 0 || count > 64 {
        return Err(ProtocolError::InvalidField);
    }
    let mut identifiers = Vec::with_capacity(count);
    for _ in 0..count {
        let kind =
            StableIdentifierKind::from_tag(reader.u8()?).ok_or(ProtocolError::InvalidField)?;
        let length = reader.u16()? as usize;
        if length == 0 || length > 128 {
            return Err(ProtocolError::InvalidField);
        }
        identifiers.push(StableIdentifier {
            kind,
            bytes: reader.take(length)?.to_vec(),
        });
    }
    let identity = StorageIdentity { identifiers };
    validate_storage_identity(&identity)?;
    Ok(identity)
}

fn validate_metadata(metadata: &WireMetadata) -> Result<(), ProtocolError> {
    if metadata.device_number == u32::MAX {
        return Err(ProtocolError::InvalidField);
    }
    if let Ok(capacity) = metadata.capacity_bytes {
        if capacity == 0 {
            return Err(ProtocolError::InvalidField);
        }
    }
    if let Ok(sectors) = metadata.sectors {
        for sector in [sectors.logical, sectors.physical].into_iter().flatten() {
            if !(512..=65_536).contains(&sector) || !sector.is_power_of_two() {
                return Err(ProtocolError::InvalidField);
            }
        }
        if let (Some(logical), Some(physical)) = (sectors.logical, sectors.physical) {
            if physical < logical || physical % logical != 0 {
                return Err(ProtocolError::InvalidField);
            }
        }
    }
    if matches!(metadata.bus_type, Ok(Some(0))) {
        return Err(ProtocolError::InvalidField);
    }
    if let Ok(identity) = &metadata.stable_identity {
        validate_storage_identity(identity)?;
    }
    if let Ok(layout) = &metadata.partition_layout {
        validate_layout(layout)?;
        let capacity = metadata
            .capacity_bytes
            .map_err(|_| ProtocolError::InvalidField)?;
        let sectors = metadata
            .sectors
            .as_ref()
            .map_err(|_| ProtocolError::InvalidField)?;
        let logical = sectors.logical.ok_or(ProtocolError::InvalidField)?;
        for partition in &layout.occupied {
            let end = partition
                .offset
                .checked_add(partition.length)
                .ok_or(ProtocolError::InvalidField)?;
            if end > capacity
                || partition.offset % u64::from(logical) != 0
                || partition.length % u64::from(logical) != 0
            {
                return Err(ProtocolError::InvalidField);
            }
        }
    }
    Ok(())
}

fn write_layout(out: &mut Vec<u8>, layout: &DriveLayout) -> Result<(), ProtocolError> {
    validate_layout(layout)?;
    let style = match layout.style {
        LayoutStyle::Mbr => 0,
        LayoutStyle::Gpt => 1,
        LayoutStyle::Raw => 2,
    };
    write_u8(out, style);
    write_u32(out, layout.returned_slots);
    write_u16(out, layout.occupied.len() as u16);
    for partition in &layout.occupied {
        write_u32(out, partition.number);
        write_u64(out, partition.offset);
        write_u64(out, partition.length);
        write_u8(out, u8::from(partition.service));
        match partition.partition_type {
            PartitionType::Mbr {
                type_id,
                boot_indicator,
            } => {
                write_u8(out, type_id);
                write_u8(out, u8::from(boot_indicator));
            }
            PartitionType::Gpt(guid) => out.extend_from_slice(&guid),
        }
    }
    Ok(())
}

fn read_layout(reader: &mut Reader<'_>) -> Result<DriveLayout, ProtocolError> {
    let style = match reader.u8()? {
        0 => LayoutStyle::Mbr,
        1 => LayoutStyle::Gpt,
        2 => LayoutStyle::Raw,
        _ => return Err(ProtocolError::InvalidField),
    };
    let returned_slots = reader.u32()?;
    let count = usize::from(reader.u16()?);
    if count > MAX_ENTRIES || returned_slots as usize > MAX_ENTRIES {
        return Err(ProtocolError::InvalidField);
    }
    let mut occupied = Vec::with_capacity(count);
    for _ in 0..count {
        let number = reader.u32()?;
        let offset = reader.u64()?;
        let length = reader.u64()?;
        let service = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err(ProtocolError::InvalidField),
        };
        let partition_type = match style {
            LayoutStyle::Mbr => {
                let type_id = reader.u8()?;
                let boot_indicator = match reader.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(ProtocolError::InvalidField),
                };
                PartitionType::Mbr {
                    type_id,
                    boot_indicator,
                }
            }
            LayoutStyle::Gpt => PartitionType::Gpt(
                reader
                    .take(16)?
                    .try_into()
                    .map_err(|_| ProtocolError::Truncated)?,
            ),
            LayoutStyle::Raw => return Err(ProtocolError::InvalidField),
        };
        occupied.push(PartitionRange {
            number,
            offset,
            length,
            service,
            partition_type,
        });
    }
    let layout = DriveLayout {
        style,
        occupied,
        returned_slots,
    };
    validate_layout(&layout)?;
    Ok(layout)
}

fn write_metadata(out: &mut Vec<u8>, metadata: &WireMetadata) -> Result<(), ProtocolError> {
    validate_metadata(metadata)?;
    write_u32(out, metadata.device_number);
    write_result(out, &metadata.capacity_bytes, |out, value| {
        write_u64(out, *value);
        Ok(())
    })?;
    write_result(out, &metadata.sectors, |out, value| {
        write_option_u32(out, value.logical);
        write_option_u32(out, value.physical);
        Ok(())
    })?;
    write_result(out, &metadata.bus_type, |out, value| {
        write_option_u32(out, *value);
        Ok(())
    })?;
    write_result(out, &metadata.stable_identity, write_storage_identity)?;
    write_result(out, &metadata.partition_layout, write_layout)
}

fn read_metadata(reader: &mut Reader<'_>) -> Result<WireMetadata, ProtocolError> {
    let metadata = WireMetadata {
        device_number: reader.u32()?,
        capacity_bytes: read_result(reader, |reader| reader.u64())?,
        sectors: read_result(reader, |reader| {
            Ok(WireSectorSizes {
                logical: read_option_u32(reader)?,
                physical: read_option_u32(reader)?,
            })
        })?,
        bus_type: read_result(reader, read_option_u32)?,
        stable_identity: read_result(reader, read_storage_identity)?,
        partition_layout: read_result(reader, read_layout)?,
    };
    validate_metadata(&metadata)?;
    Ok(metadata)
}

pub(crate) fn encode_reply(reply: &WorkerReply) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::new();
    write_header(&mut out, REPLY_KIND);
    match reply {
        WorkerReply::Ready(metadata) => {
            write_u8(&mut out, 1);
            write_metadata(&mut out, metadata)?;
        }
        WorkerReply::Unavailable(error) => {
            write_u8(&mut out, 2);
            write_error(&mut out, *error);
        }
    }
    if out.len() > MAX_WIRE_BYTES {
        return Err(ProtocolError::Oversize);
    }
    Ok(out)
}

pub(crate) fn decode_reply(bytes: &[u8]) -> Result<WorkerReply, ProtocolError> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(ProtocolError::Oversize);
    }
    let mut reader = Reader::new(bytes);
    read_header(&mut reader, REPLY_KIND)?;
    let reply = match reader.u8()? {
        1 => WorkerReply::Ready(read_metadata(&mut reader)?),
        2 => WorkerReply::Unavailable(read_error(&mut reader)?),
        _ => return Err(ProtocolError::InvalidField),
    };
    reader.finish()?;
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use partition11_core::windows_layout::{
        DriveLayout, LayoutStyle, PartitionRange, PartitionType,
    };

    fn utf16(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    fn sample_metadata() -> WireMetadata {
        WireMetadata {
            device_number: 7,
            capacity_bytes: Ok(1_048_576),
            sectors: Ok(WireSectorSizes {
                logical: Some(512),
                physical: Some(4096),
            }),
            bus_type: Ok(Some(17)),
            stable_identity: Ok(StorageIdentity {
                identifiers: vec![StableIdentifier {
                    kind: StableIdentifierKind::Eui64,
                    bytes: vec![0, 1, 2, 3, 4, 5, 6, 7],
                }],
            }),
            partition_layout: Ok(DriveLayout {
                style: LayoutStyle::Gpt,
                occupied: vec![PartitionRange {
                    number: 1,
                    offset: 4096,
                    length: 524_288,
                    service: false,
                    partition_type: PartitionType::Gpt([
                        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0,
                        0xc9, 0x3e, 0xc9, 0x3b,
                    ]),
                }],
                returned_slots: 1,
            }),
        }
    }

    #[test]
    fn request_round_trip_preserves_utf16_path_and_instance() {
        let request = WorkerRequest {
            path: utf16(r"\\?\USBSTOR#Disk&Å\x"),
            instance_id: utf16(r"USBSTOR\Disk&Å\7&1234"),
        };

        let encoded = encode_request(&request).expect("request encodes");
        assert_eq!(decode_request(&encoded).expect("request decodes"), request);
    }

    #[test]
    fn request_rejects_bad_magic_version_trailing_bytes_and_oversize() {
        let request = WorkerRequest {
            path: utf16(r"\\?\Disk#1"),
            instance_id: utf16("SCSI\\Disk&1"),
        };
        let mut encoded = encode_request(&request).expect("request encodes");

        let mut bad_magic = encoded.clone();
        bad_magic[0] ^= 1;
        assert!(decode_request(&bad_magic).is_err());

        let mut bad_version = encoded.clone();
        bad_version[4] ^= 1;
        assert!(decode_request(&bad_version).is_err());

        encoded.push(0);
        assert!(decode_request(&encoded).is_err());

        let too_long = WorkerRequest {
            path: vec![b'A' as u16; 16_385],
            instance_id: vec![b'B' as u16],
        };
        assert!(matches!(
            encode_request(&too_long),
            Err(ProtocolError::Oversize)
        ));

        let oversized_frame = vec![0; MAX_WIRE_BYTES + 1];
        assert!(matches!(
            decode_request(&oversized_frame),
            Err(ProtocolError::Oversize)
        ));
    }

    #[test]
    fn reply_round_trip_preserves_all_metadata_field_results() {
        let ready = WorkerReply::Ready(sample_metadata());
        let bytes = encode_reply(&ready).expect("ready reply encodes");
        let decoded = decode_reply(&bytes).expect("ready reply decodes");
        assert_eq!(decoded, ready);
        let WorkerReply::Ready(metadata) = decoded else {
            panic!("ready reply expected")
        };
        let layout = metadata.partition_layout.expect("layout preserved");
        assert!(layout.occupied[0].is_protected_for_benchmark());

        let partial = WorkerReply::Ready(WireMetadata {
            device_number: 9,
            capacity_bytes: Err(WireError::AccessDenied),
            sectors: Err(WireError::DeviceRemoved),
            bus_type: Ok(None),
            stable_identity: Err(WireError::Unsupported),
            partition_layout: Err(WireError::LayoutTooLarge),
        });
        let bytes = encode_reply(&partial).expect("partial metadata encodes");
        assert_eq!(
            decode_reply(&bytes).expect("partial metadata decodes"),
            partial
        );

        let unavailable = WorkerReply::Unavailable(WireError::Win32(1234));
        let bytes = encode_reply(&unavailable).expect("error reply encodes");
        assert_eq!(
            decode_reply(&bytes).expect("error reply decodes"),
            unavailable
        );
    }

    #[test]
    fn mbr_partition_classification_survives_worker_protocol_round_trip() {
        let mut metadata = sample_metadata();
        let layout = metadata.partition_layout.as_mut().unwrap();
        layout.style = LayoutStyle::Mbr;
        layout.returned_slots = 4;
        layout.occupied[0].partition_type = PartitionType::Mbr {
            type_id: 0x07,
            boot_indicator: true,
        };

        let bytes = encode_reply(&WorkerReply::Ready(metadata)).expect("encodes");
        let WorkerReply::Ready(decoded) = decode_reply(&bytes).expect("decodes") else {
            panic!("ready reply expected");
        };
        let layout = decoded.partition_layout.unwrap();
        assert_eq!(
            layout.occupied[0].partition_type,
            PartitionType::Mbr {
                type_id: 0x07,
                boot_indicator: true,
            }
        );
        assert!(layout.occupied[0].is_protected_for_benchmark());
    }

    #[test]
    fn worker_protocol_rejects_partition_type_for_wrong_layout_style() {
        let mut metadata = sample_metadata();
        metadata.partition_layout.as_mut().unwrap().occupied[0].partition_type =
            PartitionType::Mbr {
                type_id: 0x07,
                boot_indicator: false,
            };
        assert!(matches!(
            encode_reply(&WorkerReply::Ready(metadata)),
            Err(ProtocolError::InvalidField)
        ));
    }

    #[test]
    fn reply_rejects_more_than_128_partitions_and_truncation() {
        let mut too_many = sample_metadata();
        if let Ok(layout) = &mut too_many.partition_layout {
            layout.occupied = (0..129)
                .map(|number| PartitionRange {
                    number,
                    offset: u64::from(number) * 4096,
                    length: 2048,
                    service: false,
                    partition_type: PartitionType::Gpt([
                        0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6,
                        0xb7, 0x26, 0x99, 0xc7,
                    ]),
                })
                .collect();
            layout.returned_slots = 129;
        }
        assert!(matches!(
            encode_reply(&WorkerReply::Ready(too_many)),
            Err(ProtocolError::InvalidField)
        ));

        let mut too_many_reply =
            encode_reply(&WorkerReply::Ready(sample_metadata())).expect("encodes");
        let returned_slots_offset = 8 + 1 + 4 + 9 + 11 + 6 + 14 + 1;
        too_many_reply[returned_slots_offset..returned_slots_offset + 4]
            .copy_from_slice(&129_u32.to_le_bytes());
        assert!(matches!(
            decode_reply(&too_many_reply),
            Err(ProtocolError::InvalidField)
        ));

        let valid = encode_reply(&WorkerReply::Ready(sample_metadata())).expect("encodes");
        for end in 0..valid.len() {
            assert!(
                decode_reply(&valid[..end]).is_err(),
                "accepted prefix ending at {end}"
            );
        }
    }
}
