//! Bounded storage identity parsing and privacy-preserving fingerprints.
//!
//! The Windows wire layouts follow Microsoft STORAGE_DEVICE_ID_DESCRIPTOR,
//! STORAGE_IDENTIFIER and STORAGE_DEVICE_DESCRIPTOR documentation.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StableIdentifierKind {
    Eui64,
    FcPhName,
    ScsiNameString,
    DeviceSerial,
}

impl StableIdentifierKind {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Eui64 => 1,
            Self::FcPhName => 2,
            Self::ScsiNameString => 3,
            Self::DeviceSerial => 4,
        }
    }

    pub(crate) fn from_tag(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Eui64),
            2 => Some(Self::FcPhName),
            3 => Some(Self::ScsiNameString),
            4 => Some(Self::DeviceSerial),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StableIdentifier {
    pub kind: StableIdentifierKind,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageIdentity {
    pub identifiers: Vec<StableIdentifier>,
}

impl StorageIdentity {
    /// Produce a deterministic, type-separated message for the keyed digest.
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = b"Partition11 storage identity v1\0".to_vec();
        for identifier in &self.identifiers {
            out.push(identifier.kind.tag());
            out.extend_from_slice(&(identifier.bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&identifier.bytes);
        }
        out
    }

    /// Treat any shared stable identifier as a collision, even if two
    /// descriptors report different additional identifiers for the same disk.
    pub(crate) fn shares_identifier(&self, other: &Self) -> bool {
        self.identifiers
            .iter()
            .any(|identifier| other.identifiers.contains(identifier))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdentityParseError {
    Malformed,
    NoStableIdentifier,
}

const DEVICE_ID_HEADER_BYTES: usize = 12;
const IDENTIFIER_HEADER_BYTES: usize = 16;
const DEVICE_DESCRIPTOR_HEADER_BYTES: usize = 36;
const MAX_DESCRIPTOR_BYTES: usize = 4096;
const MAX_IDENTIFIERS: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 128;

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, IdentityParseError> {
    let pair = bytes
        .get(offset..offset.checked_add(2).ok_or(IdentityParseError::Malformed)?)
        .ok_or(IdentityParseError::Malformed)?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, IdentityParseError> {
    let word = bytes
        .get(offset..offset.checked_add(4).ok_or(IdentityParseError::Malformed)?)
        .ok_or(IdentityParseError::Malformed)?;
    Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

/// Parse page-0x83 identifiers returned for StorageDeviceIdProperty.
/// Only device-associated EUI-64, FC-PH name and SCSI name identifiers can
/// establish identity. Vendor, port, target and group identifiers are ignored.
pub(crate) fn parse_storage_device_id_descriptor(
    bytes: &[u8],
) -> Result<StorageIdentity, IdentityParseError> {
    if bytes.len() < DEVICE_ID_HEADER_BYTES || bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(IdentityParseError::Malformed);
    }
    let version = read_u32(bytes, 0)? as usize;
    let size = read_u32(bytes, 4)? as usize;
    let count = read_u32(bytes, 8)? as usize;
    if version < DEVICE_ID_HEADER_BYTES
        || version > size
        || size < DEVICE_ID_HEADER_BYTES
        || size > bytes.len()
        || size > MAX_DESCRIPTOR_BYTES
        || count > MAX_IDENTIFIERS
    {
        return Err(IdentityParseError::Malformed);
    }
    if count == 0 {
        return Err(IdentityParseError::NoStableIdentifier);
    }

    let mut identifiers = Vec::new();
    let mut offset = DEVICE_ID_HEADER_BYTES;
    for index in 0..count {
        let header_end = offset
            .checked_add(IDENTIFIER_HEADER_BYTES)
            .ok_or(IdentityParseError::Malformed)?;
        if header_end > size {
            return Err(IdentityParseError::Malformed);
        }
        let code_set = read_u32(bytes, offset)?;
        let kind = read_u32(bytes, offset + 4)?;
        let identifier_size = read_u16(bytes, offset + 8)? as usize;
        let next_offset = read_u16(bytes, offset + 10)? as usize;
        let association = read_u32(bytes, offset + 12)?;
        if !(1..=3).contains(&code_set)
            || association > 2
            || identifier_size == 0
            || identifier_size > MAX_IDENTIFIER_BYTES
        {
            return Err(IdentityParseError::Malformed);
        }
        let identifier_end = header_end
            .checked_add(identifier_size)
            .ok_or(IdentityParseError::Malformed)?;
        if identifier_end > size {
            return Err(IdentityParseError::Malformed);
        }
        let value = &bytes[header_end..identifier_end];
        if association == 0 {
            let parsed_kind = match kind {
                2 if code_set == 1 && identifier_size == 8 => Some(StableIdentifierKind::Eui64),
                3 if code_set == 1 && identifier_size == 8 => Some(StableIdentifierKind::FcPhName),
                8 if (code_set == 2 || code_set == 3) && valid_name_identifier(value, code_set) => {
                    Some(StableIdentifierKind::ScsiNameString)
                }
                2 | 3 | 8 => return Err(IdentityParseError::Malformed),
                _ => None,
            };
            if let Some(kind) = parsed_kind {
                identifiers.push(StableIdentifier {
                    kind,
                    bytes: value.to_vec(),
                });
            }
        }

        if index + 1 == count {
            if next_offset != 0 {
                return Err(IdentityParseError::Malformed);
            }
        } else {
            let minimum = IDENTIFIER_HEADER_BYTES
                .checked_add(identifier_size)
                .ok_or(IdentityParseError::Malformed)?;
            let next = offset
                .checked_add(next_offset)
                .ok_or(IdentityParseError::Malformed)?;
            if next_offset < minimum || next <= offset || next < identifier_end || next > size {
                return Err(IdentityParseError::Malformed);
            }
            offset = next;
        }
    }
    identifiers.sort_unstable();
    if identifiers.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(IdentityParseError::Malformed);
    }
    if identifiers.is_empty() {
        return Err(IdentityParseError::NoStableIdentifier);
    }
    Ok(StorageIdentity { identifiers })
}

fn valid_name_identifier(value: &[u8], code_set: u32) -> bool {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || value.iter().any(|byte| !(0x21..=0x7e).contains(byte))
    {
        return false;
    }
    code_set == 2 || std::str::from_utf8(value).is_ok()
}

/// Extract the device serial string from a STORAGE_DEVICE_DESCRIPTOR.
/// This is a fallback because Microsoft documents serials as available only
/// for some devices; callers must still reject duplicate values in one scan.
pub(crate) fn parse_device_serial(
    bytes: &[u8],
) -> Result<Option<StorageIdentity>, IdentityParseError> {
    if bytes.len() < DEVICE_DESCRIPTOR_HEADER_BYTES || bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(IdentityParseError::Malformed);
    }
    let version = read_u32(bytes, 0)? as usize;
    let size = read_u32(bytes, 4)? as usize;
    let serial_offset = read_u32(bytes, 24)? as usize;
    if version < DEVICE_DESCRIPTOR_HEADER_BYTES
        || version > size
        || size < DEVICE_DESCRIPTOR_HEADER_BYTES
        || size > bytes.len()
        || size > MAX_DESCRIPTOR_BYTES
    {
        return Err(IdentityParseError::Malformed);
    }
    if serial_offset == 0 {
        return Ok(None);
    }
    if serial_offset < DEVICE_DESCRIPTOR_HEADER_BYTES || serial_offset >= size {
        return Err(IdentityParseError::Malformed);
    }
    let tail = &bytes[serial_offset..size];
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(IdentityParseError::Malformed)?;
    let serial = &tail[..end];
    let start = serial
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(serial.len());
    let end = serial
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |i| i + 1);
    let serial = &serial[start..end];
    if serial.is_empty()
        || serial.len() > MAX_IDENTIFIER_BYTES
        || serial.iter().any(|byte| !(0x21..=0x7e).contains(byte))
    {
        return Err(IdentityParseError::Malformed);
    }
    Ok(Some(StorageIdentity {
        identifiers: vec![StableIdentifier {
            kind: StableIdentifierKind::DeviceSerial,
            bytes: serial.to_ascii_uppercase(),
        }],
    }))
}

#[cfg(windows)]
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> Result<[u8; 32], ()> {
    use std::ffi::c_void;
    type AlgorithmHandle = *mut c_void;
    const HMAC_FLAG: u32 = 0x0000_0008;
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptOpenAlgorithmProvider(
            algorithm: *mut AlgorithmHandle,
            algorithm_id: *const u16,
            implementation: *const u16,
            flags: u32,
        ) -> i32;
        fn BCryptHash(
            algorithm: AlgorithmHandle,
            secret: *mut u8,
            secret_size: u32,
            input: *mut u8,
            input_size: u32,
            output: *mut u8,
            output_size: u32,
        ) -> i32;
        fn BCryptCloseAlgorithmProvider(algorithm: AlgorithmHandle, flags: u32) -> i32;
    }
    if key.is_empty() || key.len() > 4096 || message.len() > 4096 {
        return Err(());
    }
    let algorithm_name: [u16; 7] = [83, 72, 65, 50, 53, 54, 0];
    let mut algorithm: AlgorithmHandle = std::ptr::null_mut();
    // BCryptHash wraps create/update/finalize/destroy; HMAC requires the
    // provider's BCRYPT_ALG_HANDLE_HMAC_FLAG, per the Windows CNG API.
    let opened = unsafe {
        BCryptOpenAlgorithmProvider(
            &mut algorithm,
            algorithm_name.as_ptr(),
            std::ptr::null(),
            HMAC_FLAG,
        )
    };
    if opened < 0 || algorithm.is_null() {
        return Err(());
    }
    let mut output = [0_u8; 32];
    let hashed = unsafe {
        BCryptHash(
            algorithm,
            key.as_ptr() as *mut u8,
            key.len() as u32,
            message.as_ptr() as *mut u8,
            message.len() as u32,
            output.as_mut_ptr(),
            output.len() as u32,
        )
    };
    let closed = unsafe { BCryptCloseAlgorithmProvider(algorithm, 0) };
    if hashed < 0 || closed < 0 {
        return Err(());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Item<'a> = (u32, u32, &'a [u8], u32);

    fn push_u16(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    fn push_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn descriptor(items: &[Item<'_>]) -> Vec<u8> {
        let mut out = vec![0; DEVICE_ID_HEADER_BYTES];
        for (index, (code_set, kind, id, association)) in items.iter().enumerate() {
            let next = if index + 1 < items.len() {
                IDENTIFIER_HEADER_BYTES + id.len()
            } else {
                0
            };
            push_u32(&mut out, *code_set);
            push_u32(&mut out, *kind);
            push_u16(&mut out, id.len() as u16);
            push_u16(&mut out, next as u16);
            push_u32(&mut out, *association);
            out.extend_from_slice(id);
        }
        let size = out.len() as u32;
        out[0..4].copy_from_slice(&(DEVICE_ID_HEADER_BYTES as u32).to_le_bytes());
        out[4..8].copy_from_slice(&size.to_le_bytes());
        out[8..12].copy_from_slice(&(items.len() as u32).to_le_bytes());
        out
    }

    fn serial_descriptor(value: Option<&[u8]>) -> Vec<u8> {
        let mut out = vec![0_u8; DEVICE_DESCRIPTOR_HEADER_BYTES];
        out[0..4].copy_from_slice(&(DEVICE_DESCRIPTOR_HEADER_BYTES as u32).to_le_bytes());
        let Some(value) = value else {
            out[4..8].copy_from_slice(&(DEVICE_DESCRIPTOR_HEADER_BYTES as u32).to_le_bytes());
            return out;
        };
        let offset = out.len() as u32;
        out[24..28].copy_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(value);
        out.push(0);
        let size = out.len() as u32;
        out[4..8].copy_from_slice(&size.to_le_bytes());
        out
    }

    #[test]
    fn parses_device_scoped_eui64_identity() {
        let bytes = descriptor(&[(1, 2, &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77], 0)]);
        let parsed = parse_storage_device_id_descriptor(&bytes).unwrap();
        assert_eq!(
            parsed.identifiers,
            vec![StableIdentifier {
                kind: StableIdentifierKind::Eui64,
                bytes: vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]
            }]
        );
    }

    #[test]
    fn ignores_port_ids_and_accepts_device_scsi_name_string() {
        let bytes = descriptor(&[
            (1, 2, &[1, 2, 3, 4, 5, 6, 7, 8], 1),
            (2, 8, b"naa.6001405abc123456", 0),
        ]);
        let parsed = parse_storage_device_id_descriptor(&bytes).unwrap();
        assert_eq!(
            parsed.identifiers,
            vec![StableIdentifier {
                kind: StableIdentifierKind::ScsiNameString,
                bytes: b"naa.6001405abc123456".to_vec()
            }]
        );
    }

    #[test]
    fn rejects_duplicate_device_identifiers() {
        let id = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let bytes = descriptor(&[(1, 2, &id, 0), (1, 2, &id, 0)]);
        assert_eq!(
            parse_storage_device_id_descriptor(&bytes),
            Err(IdentityParseError::Malformed)
        );
    }

    #[test]
    fn rejects_truncated_bad_chain_and_oversized_descriptor() {
        let valid = descriptor(&[(1, 2, &[1, 2, 3, 4, 5, 6, 7, 8], 0)]);
        assert_eq!(
            parse_storage_device_id_descriptor(&valid[..valid.len() - 1]),
            Err(IdentityParseError::Malformed)
        );
        let mut bad_chain = descriptor(&[(1, 2, &[1, 2, 3, 4, 5, 6, 7, 8], 0)]);
        bad_chain[22..24].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(
            parse_storage_device_id_descriptor(&bad_chain),
            Err(IdentityParseError::Malformed)
        );
        let mut oversized = valid.clone();
        oversized[4..8].copy_from_slice(&((MAX_DESCRIPTOR_BYTES + 1) as u32).to_le_bytes());
        assert_eq!(
            parse_storage_device_id_descriptor(&oversized),
            Err(IdentityParseError::Malformed)
        );
    }

    #[test]
    fn rejects_vendor_only_or_location_dependent_identifiers() {
        let vendor = descriptor(&[(1, 0, b"VENDOR-ID", 0)]);
        assert_eq!(
            parse_storage_device_id_descriptor(&vendor),
            Err(IdentityParseError::NoStableIdentifier)
        );
        let location = descriptor(&[(1, 4, &[1, 2, 3, 4], 0)]);
        assert_eq!(
            parse_storage_device_id_descriptor(&location),
            Err(IdentityParseError::NoStableIdentifier)
        );
    }

    #[cfg(windows)]
    #[test]
    fn cng_hmac_matches_rfc_4231_sha256_vector() {
        let key = [0x0b_u8; 20];
        let digest = hmac_sha256(&key, b"Hi There").unwrap();
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
    #[test]
    fn parses_device_serial_only_when_present_and_well_formed() {
        let serial = parse_device_serial(&serial_descriptor(Some(b"  SN-1234  ")))
            .unwrap()
            .unwrap();
        assert_eq!(
            serial.identifiers[0],
            StableIdentifier {
                kind: StableIdentifierKind::DeviceSerial,
                bytes: b"SN-1234".to_vec()
            }
        );
        assert_eq!(parse_device_serial(&serial_descriptor(None)).unwrap(), None);
        let mut malformed = serial_descriptor(Some(b"SN-1234"));
        malformed.pop();
        assert_eq!(
            parse_device_serial(&malformed),
            Err(IdentityParseError::Malformed)
        );
    }
}
