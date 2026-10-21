//! One-shot, bounded authorization envelope for file-backed benchmarks.
//! Parsing and validation happen before any target scan or file side effect.

use crate::storage_identity::hmac_sha256;
use crate::storage_identity::StorageIdentity;
use crate::volume_eligibility::{DriveType, EligibilityReason};
use crate::volume_inventory::VolumeRecord;
#[cfg(test)]
use std::io::Read;
use std::io::{BufRead, Write};

pub(crate) const MAX_REQUEST_BYTES: usize = 2 * 1024;
const MAX_FINAL_XML_BYTES: usize = 1024 * 1024;
const MAX_FINAL_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const PROTOCOL_VERSION: u64 = 1;
const OPERATION: &str = "runFileBenchmark";
const MAX_VOLUME_ORDINAL: u32 = 256;
const MAX_DISK_ORDINAL: u32 = 256;
const MAX_PARTITION_NUMBER: u32 = 128;
const MIN_AVAILABLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileId {
    Seq1mQ8T1,
    Seq1mQ1T1,
    Rnd4kQ32T1,
    Rnd4kQ1T1,
}

impl ProfileId {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "seq1m-q8t1" => Some(Self::Seq1mQ8T1),
            "seq1m-q1t1" => Some(Self::Seq1mQ1T1),
            "rnd4k-q32t1" => Some(Self::Rnd4kQ32T1),
            "rnd4k-q1t1" => Some(Self::Rnd4kQ1T1),
            _ => None,
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::Seq1mQ8T1 => 1,
            Self::Seq1mQ1T1 => 2,
            Self::Rnd4kQ32T1 => 3,
            Self::Rnd4kQ1T1 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Read,
    Write,
}

impl Direction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::Read => 1,
            Self::Write => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchmarkRequest {
    pub request_id: String,
    pub volume_ordinal: u32,
    pub disk_ordinal: u32,
    pub partition_number: u32,
    pub identity_key: [u8; 32],
    pub identity_hash: [u8; 32],
    pub profile_id: ProfileId,
    pub direction: Direction,
    pub authorization_key: [u8; 32],
    pub authorization_tag: [u8; 32],
}

impl BenchmarkRequest {
    pub(crate) fn authorization_message(&self) -> Vec<u8> {
        let mut message = b"Partition11 benchmark authorization v1\0".to_vec();
        message.extend_from_slice(&(self.request_id.len() as u16).to_le_bytes());
        message.extend_from_slice(self.request_id.as_bytes());
        message.extend_from_slice(&self.volume_ordinal.to_le_bytes());
        message.extend_from_slice(&self.disk_ordinal.to_le_bytes());
        message.extend_from_slice(&self.partition_number.to_le_bytes());
        message.extend_from_slice(&self.identity_key);
        message.extend_from_slice(&self.identity_hash);
        message.push(self.profile_id.tag());
        message.push(self.direction.tag());
        message
    }

    /// Must be checked before revalidation or any filesystem side effect.
    pub(crate) fn authorization_is_valid(&self) -> bool {
        hmac_sha256(&self.authorization_key, &self.authorization_message())
            .is_ok_and(|expected| constant_time_eq(&expected, &self.authorization_tag))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedTarget {
    pub volume_guid_root: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub disk_ordinal: u32,
    pub partition_number: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetValidationError {
    Authorization,
    SelectionChanged,
    Ineligible,
    UnsupportedFilesystem,
    InvalidVolumeRoot,
    IdentityUnavailable,
    AmbiguousIdentity,
}

/// Rebinds the one-shot request to the latest complete disk and volume scan.
/// The caller must run this after consent and before it creates a scratch file.
/// NTFS is the only enabled filesystem until the delete-on-close CI probe proves
/// the lease lifecycle on other filesystems.
pub(crate) fn revalidate_target(
    request: &BenchmarkRequest,
    volume_ordinal: u32,
    volume: &VolumeRecord,
    disk_identities: &[Option<&StorageIdentity>],
) -> Result<ValidatedTarget, TargetValidationError> {
    if !request.authorization_is_valid() {
        return Err(TargetValidationError::Authorization);
    }
    if volume_ordinal != request.volume_ordinal
        || volume.assessment.disk_ordinal != Some(request.disk_ordinal)
        || volume.assessment.partition_number != Some(request.partition_number)
    {
        return Err(TargetValidationError::SelectionChanged);
    }
    if !volume.assessment.eligible
        || volume.assessment.reason != EligibilityReason::Eligible
        || volume.read_only
        || !matches!(volume.drive_type, DriveType::Fixed | DriveType::Removable)
        || !volume
            .mount_point
            .as_deref()
            .is_some_and(crate::volume_eligibility::is_drive_letter_root)
    {
        return Err(TargetValidationError::Ineligible);
    }
    let volume_guid_root = volume
        .volume_guid_path
        .as_deref()
        .filter(|path| crate::volume_inventory::valid_volume_guid_path(path))
        .ok_or(TargetValidationError::InvalidVolumeRoot)?
        .to_owned();
    let file_system = volume
        .file_system
        .as_deref()
        .filter(|file_system| file_system.eq_ignore_ascii_case("NTFS"))
        .ok_or(TargetValidationError::UnsupportedFilesystem)?;
    let total_bytes = volume
        .total_bytes
        .ok_or(TargetValidationError::Ineligible)?;
    let available_bytes = volume
        .available_bytes
        .ok_or(TargetValidationError::Ineligible)?;
    if total_bytes == 0 || available_bytes > total_bytes || available_bytes < MIN_AVAILABLE_BYTES {
        return Err(TargetValidationError::Ineligible);
    }

    if disk_identities.is_empty() || disk_identities.iter().any(Option::is_none) {
        return Err(TargetValidationError::IdentityUnavailable);
    }
    let selected_index = request
        .disk_ordinal
        .checked_sub(1)
        .ok_or(TargetValidationError::SelectionChanged)? as usize;
    let selected_identity = disk_identities
        .get(selected_index)
        .and_then(|identity| *identity)
        .ok_or(TargetValidationError::IdentityUnavailable)?;
    if disk_identities
        .iter()
        .enumerate()
        .any(|(index, candidate)| {
            index != selected_index
                && candidate.is_some_and(|candidate| selected_identity.shares_identifier(candidate))
        })
    {
        return Err(TargetValidationError::AmbiguousIdentity);
    }
    let current_hash = hmac_sha256(&request.identity_key, &selected_identity.canonical_bytes())
        .map_err(|_| TargetValidationError::IdentityUnavailable)?;
    if !constant_time_eq(&current_hash, &request.identity_hash) {
        return Err(TargetValidationError::SelectionChanged);
    }

    Ok(ValidatedTarget {
        volume_guid_root,
        file_system: file_system.to_owned(),
        total_bytes,
        available_bytes,
        disk_ordinal: request.disk_ordinal,
        partition_number: request.partition_number,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestError {
    Oversize,
    Malformed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FinalError {
    InvalidRequest,
    StaleSelection,
    WorkerFailure,
    CleanupRequired,
}

impl FinalError {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::StaleSelection => "stale_selection",
            Self::WorkerFailure => "worker_failure",
            Self::CleanupRequired => "cleanup_required",
        }
    }
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
                0x20..=0x7e if byte != b'\\' => self.offset += 1,
                _ => return Err(RequestError::Malformed),
            }
        }
        Err(RequestError::Malformed)
    }

    fn integer(&mut self) -> Result<u32, RequestError> {
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
            .and_then(|value| value.parse::<u32>().ok())
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

fn valid_request_id(value: &str) -> bool {
    (36..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn parse_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16)? as u8;
        let low = (pair[1] as char).to_digit(16)? as u8;
        bytes[index] = (high << 4) | low;
    }
    Some(bytes)
}

pub(crate) fn parse_request(bytes: &[u8]) -> Result<BenchmarkRequest, RequestError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(RequestError::Oversize);
    }
    let mut parser = Parser::new(bytes);
    parser.byte(b'{')?;
    let mut version = None;
    let mut operation = None;
    let mut request_id = None;
    let mut volume_ordinal = None;
    let mut disk_ordinal = None;
    let mut partition_number = None;
    let mut identity_key = None;
    let mut identity_hash = None;
    let mut profile_id = None;
    let mut direction = None;
    let mut authorization_key = None;
    let mut authorization_tag = None;
    loop {
        let key = parser.string()?;
        parser.byte(b':')?;
        match key {
            "protocolVersion" if version.is_none() => version = Some(parser.integer()? as u64),
            "operation" if operation.is_none() => operation = Some(parser.string()?),
            "requestId" if request_id.is_none() => request_id = Some(parser.string()?),
            "volumeOrdinal" if volume_ordinal.is_none() => volume_ordinal = Some(parser.integer()?),
            "diskOrdinal" if disk_ordinal.is_none() => disk_ordinal = Some(parser.integer()?),
            "partitionNumber" if partition_number.is_none() => {
                partition_number = Some(parser.integer()?)
            }
            "identityKey" if identity_key.is_none() => identity_key = Some(parser.string()?),
            "identityHash" if identity_hash.is_none() => identity_hash = Some(parser.string()?),
            "profileId" if profile_id.is_none() => profile_id = Some(parser.string()?),
            "direction" if direction.is_none() => direction = Some(parser.string()?),
            "authorizationKey" if authorization_key.is_none() => {
                authorization_key = Some(parser.string()?)
            }
            "authorizationTag" if authorization_tag.is_none() => {
                authorization_tag = Some(parser.string()?)
            }
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
    let volume_ordinal = volume_ordinal.ok_or(RequestError::Malformed)?;
    let disk_ordinal = disk_ordinal.ok_or(RequestError::Malformed)?;
    let partition_number = partition_number.ok_or(RequestError::Malformed)?;
    let profile_id = ProfileId::parse(profile_id.ok_or(RequestError::Malformed)?)
        .ok_or(RequestError::Malformed)?;
    let direction = Direction::parse(direction.ok_or(RequestError::Malformed)?)
        .ok_or(RequestError::Malformed)?;
    if version != Some(PROTOCOL_VERSION)
        || operation != Some(OPERATION)
        || !valid_request_id(request_id)
        || !(1..=MAX_VOLUME_ORDINAL).contains(&volume_ordinal)
        || !(1..=MAX_DISK_ORDINAL).contains(&disk_ordinal)
        || !(1..=MAX_PARTITION_NUMBER).contains(&partition_number)
    {
        return Err(RequestError::Malformed);
    }
    Ok(BenchmarkRequest {
        request_id: request_id.to_owned(),
        volume_ordinal,
        disk_ordinal,
        partition_number,
        identity_key: parse_hex_32(identity_key.ok_or(RequestError::Malformed)?)
            .ok_or(RequestError::Malformed)?,
        identity_hash: parse_hex_32(identity_hash.ok_or(RequestError::Malformed)?)
            .ok_or(RequestError::Malformed)?,
        profile_id,
        direction,
        authorization_key: parse_hex_32(authorization_key.ok_or(RequestError::Malformed)?)
            .ok_or(RequestError::Malformed)?,
        authorization_tag: parse_hex_32(authorization_tag.ok_or(RequestError::Malformed)?)
            .ok_or(RequestError::Malformed)?,
    })
}

#[cfg(test)]
fn read_request(reader: &mut impl Read) -> Result<BenchmarkRequest, RequestError> {
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

/// Reads exactly one newline-terminated request without waiting for the parent
/// to close stdin. Bytes after the first newline remain available as control
/// frames for the duration of the operation.
pub(crate) fn read_request_frame(
    reader: &mut impl BufRead,
) -> Result<BenchmarkRequest, RequestError> {
    let mut bytes = Vec::with_capacity(MAX_REQUEST_BYTES.min(512));
    loop {
        let available = reader.fill_buf().map_err(|_| RequestError::Malformed)?;
        if available.is_empty() {
            return Err(RequestError::Malformed);
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let frame_bytes = newline.unwrap_or(available.len());
        if bytes.len().saturating_add(frame_bytes) > MAX_REQUEST_BYTES {
            return Err(RequestError::Oversize);
        }
        bytes.extend_from_slice(&available[..frame_bytes]);
        let consumed = newline.map_or(available.len(), |index| index + 1);
        reader.consume(consumed);
        if newline.is_some() {
            return parse_request(&bytes);
        }
    }
}

pub(crate) fn write_response(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

pub(crate) fn encode_completed_response(
    request_id: &str,
    target_path: &str,
    file_system: &str,
    xml: &str,
) -> Result<Vec<u8>, RequestError> {
    if !valid_request_id(request_id) || target_path.len() > 32 * 1024 || file_system != "NTFS" {
        return Err(RequestError::Malformed);
    }
    if xml.len() > MAX_FINAL_XML_BYTES {
        return Err(RequestError::Oversize);
    }
    let mut output = String::with_capacity(xml.len().min(MAX_FINAL_RESPONSE_BYTES));
    output.push_str("{\"protocolVersion\":1,\"requestId\":");
    push_json_string(&mut output, request_id);
    output.push_str(",\"status\":\"completed\",\"ownedTargetPath\":");
    push_json_string(&mut output, target_path);
    output.push_str(",\"filesystem\":\"NTFS\",\"xml\":");
    push_json_string(&mut output, xml);
    output.push('}');
    if output.len() > MAX_FINAL_RESPONSE_BYTES {
        return Err(RequestError::Oversize);
    }
    Ok(output.into_bytes())
}

pub(crate) fn encode_cancelled_response(request_id: &str) -> Result<Vec<u8>, RequestError> {
    encode_terminal_response(request_id, None)
}

pub(crate) fn encode_error_response(
    request_id: &str,
    error: FinalError,
) -> Result<Vec<u8>, RequestError> {
    encode_terminal_response(request_id, Some(error))
}

fn encode_terminal_response(
    request_id: &str,
    error: Option<FinalError>,
) -> Result<Vec<u8>, RequestError> {
    if !valid_request_id(request_id) {
        return Err(RequestError::Malformed);
    }
    let mut output = String::with_capacity(128);
    output.push_str("{\"protocolVersion\":1,\"requestId\":");
    push_json_string(&mut output, request_id);
    match error {
        Some(error) => {
            output.push_str(",\"status\":\"error\",\"error\":");
            push_json_string(&mut output, error.code());
        }
        None => output.push_str(",\"status\":\"cancelled\""),
    }
    output.push('}');
    Ok(output.into_bytes())
}

fn push_json_string(output: &mut String, value: &str) {
    use std::fmt::Write as _;

    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", character as u32)
                    .expect("writing a JSON escape to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_identity::{StableIdentifier, StableIdentifierKind, StorageIdentity};
    use crate::volume_eligibility::{Assessment, DriveType, EligibilityReason};
    use crate::volume_inventory::VolumeRecord;

    fn request_bytes(extra: &str) -> Vec<u8> {
        format!(
            "{{\"protocolVersion\":1,\"operation\":\"runFileBenchmark\",\"requestId\":\"8c6f35f8-260e-4fbd-9e4d-c91b78562f58\",\"volumeOrdinal\":1,\"diskOrdinal\":2,\"partitionNumber\":3,\"identityKey\":\"{}\",\"identityHash\":\"{}\",\"profileId\":\"seq1m-q8t1\",\"direction\":\"read\",\"authorizationKey\":\"{}\",\"authorizationTag\":\"{}\"{extra}}}",
            "11".repeat(32),
            "22".repeat(32),
            "33".repeat(32),
            "44".repeat(32),
        )
        .into_bytes()
    }

    fn identity(byte: u8) -> StorageIdentity {
        StorageIdentity {
            identifiers: vec![StableIdentifier {
                kind: StableIdentifierKind::Eui64,
                bytes: vec![byte; 8],
            }],
        }
    }

    fn eligible_volume() -> VolumeRecord {
        VolumeRecord {
            mount_point: Some("E:\\".into()),
            volume_guid_path: Some("\\\\?\\Volume{12345678-1234-1234-1234-123456789abc}\\".into()),
            drive_type: DriveType::Fixed,
            file_system: Some("NTFS".into()),
            total_bytes: Some(10 * 1024 * 1024 * 1024),
            available_bytes: Some(4 * 1024 * 1024 * 1024),
            read_only: false,
            assessment: Assessment {
                eligible: true,
                reason: EligibilityReason::Eligible,
                disk_ordinal: Some(2),
                partition_number: Some(3),
            },
        }
    }

    fn request_for(identity: &StorageIdentity) -> BenchmarkRequest {
        let mut request = parse_request(&request_bytes("")).unwrap();
        request.identity_hash =
            hmac_sha256(&request.identity_key, &identity.canonical_bytes()).unwrap();
        request.authorization_tag =
            hmac_sha256(&request.authorization_key, &request.authorization_message()).unwrap();
        request
    }

    fn to_hex(value: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut output = String::with_capacity(value.len() * 2);
        for byte in value {
            let _ = write!(output, "{byte:02x}");
        }
        output
    }

    #[test]
    fn accepts_the_exact_bounded_one_run_request() {
        let request = parse_request(&request_bytes("")).unwrap();
        assert_eq!(request.request_id, "8c6f35f8-260e-4fbd-9e4d-c91b78562f58");
        assert_eq!(request.volume_ordinal, 1);
        assert_eq!(request.disk_ordinal, 2);
        assert_eq!(request.partition_number, 3);
        assert_eq!(request.profile_id, ProfileId::Seq1mQ8T1);
        assert_eq!(request.direction, Direction::Read);
    }

    #[test]
    fn rejects_unknown_duplicate_missing_path_and_unbounded_fields() {
        let samples = [
            request_bytes(",\"path\":\"\\\\.\\PhysicalDrive0\""),
            request_bytes(",\"profileId\":\"seq1m-q8t1\""),
            request_bytes(",\"durationSeconds\":900"),
        ];
        for sample in samples {
            assert_eq!(parse_request(&sample), Err(RequestError::Malformed));
        }
        assert_eq!(parse_request(b"{}"), Err(RequestError::Malformed));
    }

    #[test]
    fn rejects_invalid_profiles_directions_and_oversize_frames() {
        let invalid_profile = String::from_utf8(request_bytes(""))
            .unwrap()
            .replace("seq1m-q8t1", "custom");
        let invalid_direction = String::from_utf8(request_bytes(""))
            .unwrap()
            .replace("\"read\"", "\"both\"");
        let invalid_ordinal = String::from_utf8(request_bytes(""))
            .unwrap()
            .replace("\"volumeOrdinal\":1", "\"volumeOrdinal\":257");
        let invalid_partition = String::from_utf8(request_bytes(""))
            .unwrap()
            .replace("\"partitionNumber\":3", "\"partitionNumber\":0");
        let invalid_identity = String::from_utf8(request_bytes(""))
            .unwrap()
            .replace(&"22".repeat(32), &"AA".repeat(32));
        let invalid_id = String::from_utf8(request_bytes(""))
            .unwrap()
            .replace("8c6f35f8-260e-4fbd-9e4d-c91b78562f58", "short");
        assert_eq!(
            parse_request(invalid_profile.as_bytes()),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            parse_request(invalid_direction.as_bytes()),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            parse_request(invalid_ordinal.as_bytes()),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            parse_request(invalid_partition.as_bytes()),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            parse_request(invalid_identity.as_bytes()),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            parse_request(invalid_id.as_bytes()),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            parse_request(&vec![b' '; MAX_REQUEST_BYTES + 1]),
            Err(RequestError::Oversize)
        );
    }

    #[test]
    fn authorization_tag_binds_the_target_and_workload() {
        let mut request = parse_request(&request_bytes("")).unwrap();
        request.authorization_tag =
            hmac_sha256(&request.authorization_key, &request.authorization_message()).unwrap();
        assert_eq!(
            to_hex(&request.authorization_tag),
            "ce971eb08282bd2a3d4c527184083fc77abe86fa5e218930f7cc2dab5ac3d5d5"
        );
        assert!(request.authorization_is_valid());
        request.authorization_tag[0] ^= 1;
        assert!(!request.authorization_is_valid());
        request.authorization_tag =
            hmac_sha256(&request.authorization_key, &request.authorization_message()).unwrap();
        request.partition_number += 1;
        assert!(!request.authorization_is_valid());
    }

    #[test]
    fn bounds_stdin_and_terminates_response_frames() {
        let mut input = std::io::Cursor::new(request_bytes(""));
        assert!(read_request(&mut input).is_ok());
        let mut oversized = std::io::Cursor::new(vec![b' '; MAX_REQUEST_BYTES + 2]);
        assert_eq!(read_request(&mut oversized), Err(RequestError::Oversize));

        let mut output = Vec::new();
        write_response(&mut output, b"{} ").unwrap();
        assert_eq!(output, b"{} \n");
    }

    #[test]
    fn reads_one_bounded_newline_request_without_waiting_for_eof_or_consuming_cancel() {
        use std::io::BufRead as _;

        let mut bytes = request_bytes("");
        bytes.push(b'\n');
        bytes.extend_from_slice(b"cancel\n");
        let mut input = std::io::BufReader::new(std::io::Cursor::new(bytes));

        let request = read_request_frame(&mut input).unwrap();
        assert_eq!(request.request_id, "8c6f35f8-260e-4fbd-9e4d-c91b78562f58");
        assert_eq!(input.fill_buf().unwrap(), b"cancel\n");
    }

    #[test]
    fn newline_request_frame_is_bounded_even_when_the_stream_stays_open() {
        let mut bytes = vec![b' '; MAX_REQUEST_BYTES + 1];
        bytes.push(b'\n');
        let mut input = std::io::BufReader::new(std::io::Cursor::new(bytes));
        assert_eq!(read_request_frame(&mut input), Err(RequestError::Oversize));
    }

    #[test]
    fn final_responses_have_exact_fields_and_escape_only_bounded_values() {
        let completed = encode_completed_response(
            "8c6f35f8-260e-4fbd-9e4d-c91b78562f58",
            "\\\\?\\Volume{12345678-1234-1234-1234-123456789abc}\\.partition11-scratch-a.bin",
            "NTFS",
            "<Results note=\"safe\">\n</Results>",
        )
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&completed).unwrap(),
            "{\"protocolVersion\":1,\"requestId\":\"8c6f35f8-260e-4fbd-9e4d-c91b78562f58\",\"status\":\"completed\",\"ownedTargetPath\":\"\\\\\\\\?\\\\Volume{12345678-1234-1234-1234-123456789abc}\\\\.partition11-scratch-a.bin\",\"filesystem\":\"NTFS\",\"xml\":\"<Results note=\\\"safe\\\">\\n</Results>\"}"
        );
        assert_eq!(
            std::str::from_utf8(
                &encode_error_response(
                    "8c6f35f8-260e-4fbd-9e4d-c91b78562f58",
                    FinalError::StaleSelection,
                )
                .unwrap()
            )
            .unwrap(),
            "{\"protocolVersion\":1,\"requestId\":\"8c6f35f8-260e-4fbd-9e4d-c91b78562f58\",\"status\":\"error\",\"error\":\"stale_selection\"}"
        );
        assert_eq!(
            std::str::from_utf8(
                &encode_cancelled_response("8c6f35f8-260e-4fbd-9e4d-c91b78562f58").unwrap()
            )
            .unwrap(),
            "{\"protocolVersion\":1,\"requestId\":\"8c6f35f8-260e-4fbd-9e4d-c91b78562f58\",\"status\":\"cancelled\"}"
        );
        assert_eq!(
            encode_completed_response("bad", "path", "NTFS", "xml"),
            Err(RequestError::Malformed)
        );
        assert_eq!(
            encode_completed_response(
                "8c6f35f8-260e-4fbd-9e4d-c91b78562f58",
                "\\\\?\\Volume{12345678-1234-1234-1234-123456789abc}\\scratch.bin",
                "NTFS",
                &"x".repeat(MAX_FINAL_XML_BYTES + 1),
            ),
            Err(RequestError::Oversize)
        );
    }

    #[test]
    fn accepts_only_a_fresh_unique_identity_on_a_supported_eligible_volume() {
        let other = identity(1);
        let selected = identity(2);
        let request = request_for(&selected);
        let identities = [Some(&other), Some(&selected)];
        let target = revalidate_target(&request, 1, &eligible_volume(), &identities).unwrap();
        assert_eq!(
            target.volume_guid_root, "\\\\?\\Volume{12345678-1234-1234-1234-123456789abc}\\",
            "the validated target must retain the volume-GUID root used by ScratchLease"
        );
        assert_eq!(target.file_system, "NTFS");
        assert_eq!(target.available_bytes, 4 * 1024 * 1024 * 1024);
    }

    #[test]
    fn rejects_missing_or_malformed_volume_guid_roots() {
        let selected = identity(2);
        let request = request_for(&selected);
        let identities = [Some(&selected)];
        for invalid_root in [None, Some("E:\\"), Some("\\\\?\\Volume{not-a-guid}\\")] {
            let mut volume = eligible_volume();
            volume.volume_guid_path = invalid_root.map(str::to_owned);
            assert_eq!(
                revalidate_target(&request, 1, &volume, &identities),
                Err(TargetValidationError::InvalidVolumeRoot)
            );
        }
    }

    #[test]
    fn rejects_selection_drift_in_mapping_identity_eligibility_space_or_filesystem() {
        let other = identity(1);
        let selected = identity(2);
        let request = request_for(&selected);
        let identities = [Some(&other), Some(&selected)];

        let mut changed_mapping = eligible_volume();
        changed_mapping.assessment.partition_number = Some(4);
        assert_eq!(
            revalidate_target(&request, 1, &changed_mapping, &identities),
            Err(TargetValidationError::SelectionChanged)
        );

        let mut ineligible = eligible_volume();
        ineligible.assessment.eligible = false;
        assert_eq!(
            revalidate_target(&request, 1, &ineligible, &identities),
            Err(TargetValidationError::Ineligible)
        );

        let mut no_space = eligible_volume();
        no_space.available_bytes = Some(1024);
        assert_eq!(
            revalidate_target(&request, 1, &no_space, &identities),
            Err(TargetValidationError::Ineligible)
        );

        let mut unsupported_fs = eligible_volume();
        unsupported_fs.file_system = Some("exFAT".into());
        assert_eq!(
            revalidate_target(&request, 1, &unsupported_fs, &identities),
            Err(TargetValidationError::UnsupportedFilesystem)
        );

        assert_eq!(
            revalidate_target(&request, 1, &eligible_volume(), &[Some(&other), None]),
            Err(TargetValidationError::IdentityUnavailable)
        );

        let duplicate = StorageIdentity {
            identifiers: vec![
                selected.identifiers[0].clone(),
                StableIdentifier {
                    kind: StableIdentifierKind::ScsiNameString,
                    bytes: vec![9; 8],
                },
            ],
        };
        assert_eq!(
            revalidate_target(
                &request,
                1,
                &eligible_volume(),
                &[Some(&selected), Some(&duplicate)]
            ),
            Err(TargetValidationError::AmbiguousIdentity)
        );

        let mut stale_identity_request = request.clone();
        stale_identity_request.identity_hash = hmac_sha256(
            &stale_identity_request.identity_key,
            &other.canonical_bytes(),
        )
        .unwrap();
        stale_identity_request.authorization_tag = hmac_sha256(
            &stale_identity_request.authorization_key,
            &stale_identity_request.authorization_message(),
        )
        .unwrap();
        assert_eq!(
            revalidate_target(&stale_identity_request, 1, &eligible_volume(), &identities),
            Err(TargetValidationError::SelectionChanged)
        );

        let mut bad_mac = request;
        bad_mac.authorization_tag[0] ^= 1;
        assert_eq!(
            revalidate_target(&bad_mac, 1, &eligible_volume(), &identities),
            Err(TargetValidationError::Authorization)
        );
    }
}
