//! Narrow one-shot JSON protocol for Electron's read-only disk preview.
//!
//! Request IDs and an ephemeral Electron-generated HMAC key cross the process
//! boundary. Device paths, instance IDs, serials, handles and transient Windows
//! device numbers stay inside the agent; only keyed identity fingerprints leave
//! it. The parser accepts a tiny JSON subset and rejects escapes and unknown
//! keys so the protocol surface remains auditable.

use crate::disk_metadata::{MetadataError, ProbeOutcome};
use crate::storage_identity::{hmac_sha256, StorageIdentity};
use partition11_core::windows_layout::LayoutStyle;
use std::io::{Read, Write};

pub(crate) const MAX_REQUEST_BYTES: usize = 1_024;
pub(crate) const MAX_RESPONSE_BYTES: usize = 256 * 1_024;
const PROTOCOL_VERSION: u64 = 3;
const OPERATION: &str = "inspectPhysicalDisks";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InventoryRequest {
    pub request_id: String,
    pub identity_key: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestError {
    Oversize,
    Malformed,
}

struct Parser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn whitespace(&mut self) {
        while self
            .bytes
            .get(self.offset)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.offset += 1;
        }
    }

    fn byte(&mut self, expected: u8) -> Result<(), RequestError> {
        self.whitespace();
        if self.bytes.get(self.offset) == Some(&expected) {
            self.offset += 1;
            Ok(())
        } else {
            Err(RequestError::Malformed)
        }
    }

    fn string(&mut self) -> Result<&'a str, RequestError> {
        self.whitespace();
        if self.bytes.get(self.offset) != Some(&b'"') {
            return Err(RequestError::Malformed);
        }
        self.offset += 1;
        let start = self.offset;
        while let Some(&byte) = self.bytes.get(self.offset) {
            match byte {
                b'"' => {
                    let value = std::str::from_utf8(&self.bytes[start..self.offset])
                        .map_err(|_| RequestError::Malformed)?;
                    self.offset += 1;
                    return Ok(value);
                }
                // The accepted schema contains ASCII only and needs no JSON
                // escapes. Reject controls, backslashes and non-ASCII bytes.
                0x20..=0x7e if byte != b'\\' => self.offset += 1,
                _ => return Err(RequestError::Malformed),
            }
        }
        Err(RequestError::Malformed)
    }

    fn version(&mut self) -> Result<u64, RequestError> {
        self.whitespace();
        let start = self.offset;
        while self.bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        if self.offset == start || (self.offset - start > 1 && self.bytes[start] == b'0') {
            return Err(RequestError::Malformed);
        }
        std::str::from_utf8(&self.bytes[start..self.offset])
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(RequestError::Malformed)
    }

    fn finish(self) -> Result<(), RequestError> {
        let mut parser = self;
        parser.whitespace();
        if parser.offset == parser.bytes.len() {
            Ok(())
        } else {
            Err(RequestError::Malformed)
        }
    }
}

pub(crate) fn read_request(reader: &mut impl Read) -> Result<InventoryRequest, RequestError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| RequestError::Malformed)?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(RequestError::Oversize);
    }
    parse_request(&bytes)
}

fn valid_request_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn parse_identity_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    let mut key = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16)? as u8;
        let low = (pair[1] as char).to_digit(16)? as u8;
        key[index] = (high << 4) | low;
    }
    Some(key)
}

fn identity_hash(identity: &StorageIdentity, key: &[u8; 32]) -> Result<String, RequestError> {
    let digest =
        hmac_sha256(key, &identity.canonical_bytes()).map_err(|_| RequestError::Malformed)?;
    let mut hex = String::with_capacity(64);
    use std::fmt::Write as _;
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

pub(crate) fn parse_request(bytes: &[u8]) -> Result<InventoryRequest, RequestError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(RequestError::Oversize);
    }
    let mut parser = Parser::new(bytes);
    parser.byte(b'{')?;
    let mut version = None;
    let mut operation = None;
    let mut request_id = None;
    let mut identity_key = None;
    loop {
        let key = parser.string()?;
        parser.byte(b':')?;
        match key {
            "protocolVersion" if version.is_none() => version = Some(parser.version()?),
            "operation" if operation.is_none() => operation = Some(parser.string()?),
            "requestId" if request_id.is_none() => request_id = Some(parser.string()?),
            "identityKey" if identity_key.is_none() => identity_key = Some(parser.string()?),
            _ => return Err(RequestError::Malformed),
        }
        parser.whitespace();
        match parser.bytes.get(parser.offset) {
            Some(b',') => parser.offset += 1,
            Some(b'}') => {
                parser.offset += 1;
                break;
            }
            _ => return Err(RequestError::Malformed),
        }
    }
    parser.finish()?;
    let request_id = request_id.ok_or(RequestError::Malformed)?;
    let identity_key = parse_identity_key(identity_key.ok_or(RequestError::Malformed)?)
        .ok_or(RequestError::Malformed)?;
    if version != Some(PROTOCOL_VERSION)
        || operation != Some(OPERATION)
        || !valid_request_id(request_id)
    {
        return Err(RequestError::Malformed);
    }
    Ok(InventoryRequest {
        request_id: request_id.to_owned(),
        identity_key,
    })
}

fn error_code(error: MetadataError) -> &'static str {
    match error {
        MetadataError::AccessDenied => "access_denied",
        MetadataError::DeviceRemoved => "device_unavailable",
        MetadataError::Unsupported => "unsupported",
        MetadataError::IdentityChanged | MetadataError::Cancelled => "identity_changed",
        MetadataError::TimedOut => "scan_timeout",
        MetadataError::ElevatedProcess
        | MetadataError::MalformedResponse
        | MetadataError::LayoutTooLarge
        | MetadataError::WorkerFailed
        | MetadataError::Win32(_) => "agent_failure",
    }
}

fn bus_name(bus: u32) -> Option<&'static str> {
    match bus {
        1 => Some("scsi"),
        2 => Some("atapi"),
        3 => Some("ata"),
        4 => Some("ieee1394"),
        5 => Some("ssa"),
        6 => Some("fibre"),
        7 => Some("usb"),
        8 => Some("raid"),
        9 => Some("iscsi"),
        10 => Some("sas"),
        11 => Some("sata"),
        12 => Some("sd"),
        13 => Some("mmc"),
        14..=16 => Some("virtual"),
        17 => Some("nvme"),
        18 => Some("scm"),
        19 => Some("ufs"),
        _ => None,
    }
}

fn write_optional<T: std::fmt::Display>(out: &mut String, value: Option<T>) {
    if let Some(value) = value {
        use std::fmt::Write as _;
        let _ = write!(out, "{value}");
    } else {
        out.push_str("null");
    }
}

pub(crate) fn encode_success(
    request: &InventoryRequest,
    inventory: &crate::disk_metadata::ReadOnlyInventory,
) -> Result<Vec<u8>, RequestError> {
    let mut out =
        String::with_capacity(256 + inventory.disks.len() * 192 + inventory.volumes.len() * 256);
    use std::fmt::Write as _;
    let _ = write!(
        out,
        "{{\"protocolVersion\":3,\"requestId\":\"{}\",\"ok\":true,\"disks\":[",
        request.request_id
    );
    for (index, outcome) in inventory.disks.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        let ordinal = index + 1;
        match outcome {
            ProbeOutcome::Unavailable(error) => {
                let _ = write!(
                    out,
                    "{{\"ordinal\":{ordinal},\"availability\":\"{}\",\"capacityBytes\":null,\"logicalSectorSize\":null,\"physicalSectorSize\":null,\"busType\":null,\"partitionStyle\":null,\"partitionCount\":null,\"identityHash\":null}}",
                    error_code(*error)
                );
            }
            ProbeOutcome::Ready(metadata) => {
                let capacity = metadata.capacity_bytes.as_ref().ok().copied();
                let logical = metadata
                    .sectors
                    .as_ref()
                    .ok()
                    .and_then(|sectors| sectors.logical);
                let physical = metadata
                    .sectors
                    .as_ref()
                    .ok()
                    .and_then(|sectors| sectors.physical);
                let bus = metadata
                    .bus_type
                    .as_ref()
                    .ok()
                    .and_then(|bus| bus.and_then(bus_name));
                let layout = metadata.partition_layout.as_ref().ok();
                let style = layout.map(|layout| match layout.style {
                    LayoutStyle::Mbr => "mbr",
                    LayoutStyle::Gpt => "gpt",
                    LayoutStyle::Raw => "raw",
                });
                let count = layout.map(|layout| layout.occupied.len());
                let _ = write!(
                    out,
                    "{{\"ordinal\":{ordinal},\"availability\":\"ready\",\"capacityBytes\":"
                );
                if let Some(capacity) = capacity {
                    let _ = write!(out, "\"{capacity}\"");
                } else {
                    out.push_str("null");
                }
                out.push_str(",\"logicalSectorSize\":");
                write_optional(&mut out, logical);
                out.push_str(",\"physicalSectorSize\":");
                write_optional(&mut out, physical);
                out.push_str(",\"busType\":");
                if let Some(bus) = bus {
                    let _ = write!(out, "\"{bus}\"");
                } else {
                    out.push_str("null");
                }
                out.push_str(",\"partitionStyle\":");
                if let Some(style) = style {
                    let _ = write!(out, "\"{style}\"");
                } else {
                    out.push_str("null");
                }
                out.push_str(",\"partitionCount\":");
                write_optional(&mut out, count);
                out.push_str(",\"identityHash\":");
                if let Ok(identity) = &metadata.stable_identity {
                    let hash = identity_hash(identity, &request.identity_key)?;
                    let _ = write!(out, "\"{hash}\"");
                } else {
                    out.push_str("null");
                }
                out.push('}');
            }
        }
        if out.len() > MAX_RESPONSE_BYTES {
            return Err(RequestError::Oversize);
        }
    }
    out.push_str("],\"volumes\":[");
    for (index, volume) in inventory.volumes.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        let ordinal = index + 1;
        let _ = write!(out, "{{\"ordinal\":{ordinal},\"mountPoint\":");
        if let Some(mount_point) = volume.mount_point.as_deref() {
            if !valid_drive_root(mount_point) {
                return Err(RequestError::Malformed);
            }
            let _ = write!(out, "\"{}\"", mount_point.replace('\\', "\\\\"));
        } else {
            out.push_str("null");
        }
        let _ = write!(
            out,
            ",\"driveType\":\"{}\",\"fileSystem\":",
            drive_type_name(volume.drive_type)
        );
        if let Some(file_system) = volume.file_system.as_deref().and_then(public_file_system) {
            let _ = write!(out, "\"{file_system}\"");
        } else {
            out.push_str("null");
        }
        out.push_str(",\"totalBytes\":");
        write_optional_string(&mut out, volume.total_bytes);
        out.push_str(",\"availableBytes\":");
        write_optional_string(&mut out, volume.available_bytes);
        let _ = write!(
            out,
            ",\"readOnly\":{},\"eligible\":{},\"eligibilityReason\":\"{}\",\"diskOrdinal\":",
            volume.read_only,
            volume.assessment.eligible,
            volume.assessment.reason.as_str()
        );
        write_optional(&mut out, volume.assessment.disk_ordinal);
        out.push_str(",\"partitionNumber\":");
        write_optional(&mut out, volume.assessment.partition_number);
        out.push('}');
        if out.len() > MAX_RESPONSE_BYTES {
            return Err(RequestError::Oversize);
        }
    }
    out.push_str("]}");
    if out.len() > MAX_RESPONSE_BYTES {
        return Err(RequestError::Oversize);
    }
    Ok(out.into_bytes())
}

fn valid_drive_root(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 3 && bytes[0].is_ascii_alphabetic() && bytes[1..] == *b":\\"
}

fn write_optional_string(out: &mut String, value: Option<u64>) {
    if let Some(value) = value {
        use std::fmt::Write as _;
        let _ = write!(out, "\"{value}\"");
    } else {
        out.push_str("null");
    }
}

fn drive_type_name(value: crate::volume_eligibility::DriveType) -> &'static str {
    use crate::volume_eligibility::DriveType;
    match value {
        DriveType::Unknown => "unknown",
        DriveType::NoRoot => "no_root",
        DriveType::Removable => "removable",
        DriveType::Fixed => "fixed",
        DriveType::Remote => "remote",
        DriveType::Cdrom => "cdrom",
        DriveType::Ramdisk => "ramdisk",
    }
}

fn public_file_system(value: &str) -> Option<&'static str> {
    ["NTFS", "ReFS", "exFAT", "FAT", "FAT32"]
        .into_iter()
        .find(|allowed| value.eq_ignore_ascii_case(allowed))
}

pub(crate) fn encode_failure(request: &InventoryRequest, code: &'static str) -> Vec<u8> {
    format!(
        "{{\"protocolVersion\":3,\"requestId\":\"{}\",\"ok\":false,\"error\":\"{code}\"}}",
        request.request_id
    )
    .into_bytes()
}

pub(crate) fn write_response(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk_metadata::DiskMetadata;
    use crate::metadata_query::SectorSizes;
    use crate::volume_eligibility::{Assessment, DriveType, EligibilityReason};
    use crate::volume_inventory::VolumeRecord;
    use partition11_core::windows_layout::{DriveLayout, LayoutStyle};

    fn request() -> InventoryRequest {
        InventoryRequest {
            request_id: "test-1234".into(),
            identity_key: [0x11; 32],
        }
    }

    #[test]
    fn accepts_only_the_versioned_inventory_request_and_any_key_order() {
        let compact =
            br#"{"protocolVersion":3,"operation":"inspectPhysicalDisks","requestId":"test-1234","identityKey":"1111111111111111111111111111111111111111111111111111111111111111"}"#;
        let reordered = br#" { "identityKey":"1111111111111111111111111111111111111111111111111111111111111111", "requestId" : "test-1234", "operation" : "inspectPhysicalDisks", "protocolVersion": 3 } "#;
        assert_eq!(parse_request(compact).unwrap(), request());
        assert_eq!(parse_request(reordered).unwrap(), request());
    }

    #[test]
    fn rejects_unknown_duplicate_missing_escaped_and_invalid_fields() {
        let samples: &[&[u8]] = &[
            br#"{"protocolVersion":3,"operation":"inspectPhysicalDisks","requestId":"x","path":"disk0"}"#,
            br#"{"protocolVersion":3,"protocolVersion":3,"operation":"inspectPhysicalDisks","requestId":"x"}"#,
            br#"{"protocolVersion":3,"operation":"inspectPhysicalDisks"}"#,
            br#"{"protocolVersion":3,"operation":"inspectPhysicalDisks","requestId":"bad\\u002dID"}"#,
            br#"{"protocolVersion":1,"operation":"inspectPhysicalDisks","requestId":"x"}"#,
            br#"{"protocolVersion":3,"operation":"metadata-summary","requestId":"x"}"#,
            br#"{"protocolVersion":3,"operation":"inspectPhysicalDisks","requestId":"x\ny"}"#,
            b"{\x0b\"protocolVersion\":3,\"operation\":\"inspectPhysicalDisks\",\"requestId\":\"x\"}",
        ];
        for sample in samples {
            assert_eq!(parse_request(sample), Err(RequestError::Malformed));
        }
    }

    #[test]
    fn request_size_is_bounded() {
        assert_eq!(
            parse_request(&vec![b' '; MAX_REQUEST_BYTES + 1]),
            Err(RequestError::Oversize)
        );
    }

    #[test]
    fn response_contains_only_allowlisted_snapshot_metadata() {
        let metadata = DiskMetadata {
            device_number: 987_654,
            capacity_bytes: Ok(1_000_000),
            sectors: Ok(SectorSizes {
                logical: Some(512),
                physical: Some(4096),
            }),
            bus_type: Ok(Some(17)),
            stable_identity: Err(MetadataError::Unsupported),
            partition_layout: Ok(DriveLayout {
                style: LayoutStyle::Gpt,
                occupied: Vec::new(),
                returned_slots: 128,
            }),
        };
        let bytes = encode_success(
            &request(),
            &crate::disk_metadata::ReadOnlyInventory {
                disks: vec![ProbeOutcome::Ready(metadata)],
                volumes: Vec::new(),
            },
        )
        .unwrap();
        let json = String::from_utf8(bytes).unwrap();
        assert!(json.contains("\"capacityBytes\":\"1000000\""));
        assert!(json.contains("\"busType\":\"nvme\""));
        assert!(json.contains("\"partitionStyle\":\"gpt\""));
        assert!(!json.contains("device_number"));
        assert!(!json.contains("987654"));
        assert!(!json.contains("serial"));
        assert!(!json.contains("path"));
        assert!(!json.contains("instance"));
        assert!(json.contains("\"volumes\":[]"));
    }

    #[test]
    fn response_does_not_expose_internal_volume_guid_paths() {
        let marker = "\\\\?\\Volume{deadbeef-dead-beef-dead-beefdeadbeef}\\";
        let inventory = crate::disk_metadata::ReadOnlyInventory {
            disks: Vec::new(),
            volumes: vec![VolumeRecord {
                mount_point: Some("E:\\".into()),
                volume_guid_path: Some(marker.into()),
                drive_type: DriveType::Fixed,
                file_system: Some("NTFS".into()),
                total_bytes: Some(10_000),
                available_bytes: Some(5_000),
                read_only: false,
                assessment: Assessment {
                    eligible: true,
                    reason: EligibilityReason::Eligible,
                    disk_ordinal: Some(1),
                    partition_number: Some(1),
                },
            }],
        };
        let json = String::from_utf8(encode_success(&request(), &inventory).unwrap()).unwrap();
        assert!(!json.contains(marker));
        assert!(!json.contains("deadbeef"));
        assert!(!json.contains("volumeGuidPath"));
        assert!(json.contains("\"mountPoint\":\"E:\\\\\""));
    }

    #[test]
    fn response_emits_a_request_keyed_identity_hash_without_the_raw_identifier() {
        let metadata = DiskMetadata {
            device_number: 7,
            capacity_bytes: Ok(1_000_000),
            sectors: Ok(SectorSizes {
                logical: Some(512),
                physical: Some(4096),
            }),
            bus_type: Ok(Some(17)),
            stable_identity: Ok(crate::storage_identity::StorageIdentity {
                identifiers: vec![crate::storage_identity::StableIdentifier {
                    kind: crate::storage_identity::StableIdentifierKind::Eui64,
                    bytes: vec![0, 1, 2, 3, 4, 5, 6, 7],
                }],
            }),
            partition_layout: Ok(DriveLayout {
                style: LayoutStyle::Gpt,
                occupied: Vec::new(),
                returned_slots: 128,
            }),
        };
        let inventory = crate::disk_metadata::ReadOnlyInventory {
            disks: vec![ProbeOutcome::Ready(metadata)],
            volumes: Vec::new(),
        };
        let first = encode_success(&request(), &inventory).unwrap();
        let first = String::from_utf8(first).unwrap();
        assert!(first.contains(
            "\"identityHash\":\"a96560f078aaa53ce68d4abf16f5ec11f5775c96f9781cd97577d3cebebe6888\""
        ), "response: {first}");
        assert!(!first.contains("0001020304050607"));

        let mut second_request = request();
        second_request.identity_key = [0x22; 32];
        let second =
            String::from_utf8(encode_success(&second_request, &inventory).unwrap()).unwrap();
        assert_ne!(first, second);
    }
}
