//! Narrow one-shot JSON protocol for Electron's read-only disk preview.
//!
//! Only request IDs cross the process boundary. Device paths, instance IDs,
//! serials, DUIDs, handles and transient Windows device numbers stay inside
//! the agent. The parser intentionally accepts a tiny JSON subset and rejects
//! escapes and unknown keys so the protocol surface remains auditable.

use crate::disk_metadata::{MetadataError, ProbeOutcome};
use partition11_core::windows_layout::LayoutStyle;
use std::io::{Read, Write};

pub(crate) const MAX_REQUEST_BYTES: usize = 1_024;
pub(crate) const MAX_RESPONSE_BYTES: usize = 256 * 1_024;
const PROTOCOL_VERSION: u64 = 2;
const OPERATION: &str = "inspectPhysicalDisks";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InventoryRequest {
    pub request_id: String,
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

pub(crate) fn parse_request(bytes: &[u8]) -> Result<InventoryRequest, RequestError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(RequestError::Oversize);
    }
    let mut parser = Parser::new(bytes);
    parser.byte(b'{')?;
    let mut version = None;
    let mut operation = None;
    let mut request_id = None;
    loop {
        let key = parser.string()?;
        parser.byte(b':')?;
        match key {
            "protocolVersion" if version.is_none() => version = Some(parser.version()?),
            "operation" if operation.is_none() => operation = Some(parser.string()?),
            "requestId" if request_id.is_none() => request_id = Some(parser.string()?),
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
    if version != Some(PROTOCOL_VERSION)
        || operation != Some(OPERATION)
        || !valid_request_id(request_id)
    {
        return Err(RequestError::Malformed);
    }
    Ok(InventoryRequest {
        request_id: request_id.to_owned(),
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
        "{{\"protocolVersion\":2,\"requestId\":\"{}\",\"ok\":true,\"disks\":[",
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
                    "{{\"ordinal\":{ordinal},\"availability\":\"{}\",\"capacityBytes\":null,\"logicalSectorSize\":null,\"physicalSectorSize\":null,\"busType\":null,\"partitionStyle\":null,\"partitionCount\":null}}",
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
        "{{\"protocolVersion\":2,\"requestId\":\"{}\",\"ok\":false,\"error\":\"{code}\"}}",
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
    use partition11_core::windows_layout::{DriveLayout, LayoutStyle};

    fn request() -> InventoryRequest {
        InventoryRequest {
            request_id: "test-1234".into(),
        }
    }

    #[test]
    fn accepts_only_the_versioned_inventory_request_and_any_key_order() {
        let compact =
            br#"{"protocolVersion":2,"operation":"inspectPhysicalDisks","requestId":"test-1234"}"#;
        let reordered = br#" { "requestId" : "test-1234", "operation" : "inspectPhysicalDisks", "protocolVersion": 2 } "#;
        assert_eq!(parse_request(compact).unwrap(), request());
        assert_eq!(parse_request(reordered).unwrap(), request());
    }

    #[test]
    fn rejects_unknown_duplicate_missing_escaped_and_invalid_fields() {
        let samples: &[&[u8]] = &[
            br#"{"protocolVersion":2,"operation":"inspectPhysicalDisks","requestId":"x","path":"disk0"}"#,
            br#"{"protocolVersion":2,"protocolVersion":2,"operation":"inspectPhysicalDisks","requestId":"x"}"#,
            br#"{"protocolVersion":2,"operation":"inspectPhysicalDisks"}"#,
            br#"{"protocolVersion":2,"operation":"inspectPhysicalDisks","requestId":"bad\\u002dID"}"#,
            br#"{"protocolVersion":1,"operation":"inspectPhysicalDisks","requestId":"x"}"#,
            br#"{"protocolVersion":2,"operation":"metadata-summary","requestId":"x"}"#,
            br#"{"protocolVersion":2,"operation":"inspectPhysicalDisks","requestId":"x\ny"}"#,
            b"{\x0b\"protocolVersion\":2,\"operation\":\"inspectPhysicalDisks\",\"requestId\":\"x\"}",
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
}
