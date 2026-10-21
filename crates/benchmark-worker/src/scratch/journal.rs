use super::{invalid_data, FileId, ObjectIdentity};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::io;
use std::path::PathBuf;
use std::ptr::{null, null_mut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LeaseState {
    Prepared,
    Ready,
    Running,
    CleanupPending,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LeaseRecord {
    pub(super) lease_id: String,
    pub(super) volume_root: PathBuf,
    pub(super) root_identity: FileId,
    pub(super) scratch_path: PathBuf,
    pub(super) file_identity: Option<FileId>,
    pub(super) expected_size: u64,
    pub(super) created_at_unix_ms: u64,
    pub(super) state: LeaseState,
}

impl LeaseRecord {
    pub(super) fn prepared(
        lease_id: String,
        root_identity: &ObjectIdentity,
        scratch_path: PathBuf,
        expected_size: u64,
        created_at_unix_ms: u64,
    ) -> Self {
        Self {
            lease_id,
            volume_root: root_identity.final_path.clone(),
            root_identity: root_identity.file_id,
            scratch_path,
            file_identity: None,
            expected_size,
            created_at_unix_ms,
            state: LeaseState::Prepared,
        }
    }

    pub(super) fn ready(&mut self, file_identity: FileId) {
        self.file_identity = Some(file_identity);
        self.state = LeaseState::Ready;
    }
}

const MAX_RECORD_BYTES: usize = 4096;
const FIELD_NAMES: [&str; 11] = [
    "version",
    "lease_id",
    "volume_root",
    "root_volume_serial",
    "root_file_id",
    "scratch_path",
    "file_volume_serial",
    "file_id",
    "expected_size",
    "created_unix_ms",
    "state",
];

#[cfg(test)]
fn encode_record(record: &LeaseRecord) -> io::Result<Vec<u8>> {
    encode_record_with_policy(record, false)
}

fn encode_record_with_policy(
    record: &LeaseRecord,
    allow_non_volume_root: bool,
) -> io::Result<Vec<u8>> {
    validate_record(record, allow_non_volume_root)?;
    let (file_volume_serial, file_id) = match record.file_identity {
        Some(identity) => (
            format!("{:016x}", identity.volume_serial),
            encode_hex(&identity.identifier),
        ),
        None => ("-".to_string(), "-".to_string()),
    };
    let text = format!(
        "version=1\nlease_id={}\nvolume_root={}\nroot_volume_serial={:016x}\nroot_file_id={}\nscratch_path={}\nfile_volume_serial={}\nfile_id={}\nexpected_size={}\ncreated_unix_ms={}\nstate={}\n",
        record.lease_id,
        record.volume_root.display(),
        record.root_identity.volume_serial,
        encode_hex(&record.root_identity.identifier),
        record.scratch_path.display(),
        file_volume_serial,
        file_id,
        record.expected_size,
        record.created_at_unix_ms,
        state_name(record.state),
    );
    if text.len() > MAX_RECORD_BYTES {
        return Err(invalid_data("lease journal record exceeds the size limit"));
    }
    Ok(text.into_bytes())
}

#[cfg(test)]
fn decode_record(bytes: &[u8]) -> io::Result<LeaseRecord> {
    decode_record_with_policy(bytes, false)
}

fn decode_record_with_policy(bytes: &[u8], allow_non_volume_root: bool) -> io::Result<LeaseRecord> {
    if bytes.is_empty()
        || bytes.len() > MAX_RECORD_BYTES
        || bytes.contains(&0)
        || bytes.contains(&b'\r')
        || !bytes.ends_with(b"\n")
    {
        return Err(invalid_data("lease journal record framing is invalid"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid_data("lease journal record is not UTF-8"))?;
    let lines = text[..text.len() - 1].split('\n').collect::<Vec<_>>();
    if lines.len() != FIELD_NAMES.len() {
        return Err(invalid_data(
            "lease journal record has an unexpected field count",
        ));
    }
    let mut values = Vec::with_capacity(FIELD_NAMES.len());
    for (line, expected_name) in lines.iter().zip(FIELD_NAMES) {
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| invalid_data("lease journal field has no value"))?;
        if name != expected_name || value.is_empty() {
            return Err(invalid_data("lease journal field order or name is invalid"));
        }
        values.push(value);
    }
    if values[0] != "1" {
        return Err(invalid_data("unsupported lease journal schema version"));
    }
    let lease_id = values[1].to_string();
    let volume_root = PathBuf::from(values[2]);
    let root_volume_serial = parse_u64_hex(values[3])?;
    let root_file_id = decode_hex::<16>(values[4])?;
    let scratch_path = PathBuf::from(values[5]);
    let file_identity = match (values[6], values[7]) {
        ("-", "-") => None,
        (serial, file_id) if serial != "-" && file_id != "-" => Some(FileId {
            volume_serial: parse_u64_hex(serial)?,
            identifier: decode_hex::<16>(file_id)?,
        }),
        _ => return Err(invalid_data("lease journal file identity is incomplete")),
    };
    let expected_size = parse_decimal(values[8])?;
    let created_at_unix_ms = parse_decimal(values[9])?;
    let state = parse_state(values[10])?;
    let record = LeaseRecord {
        lease_id,
        volume_root,
        root_identity: FileId {
            volume_serial: root_volume_serial,
            identifier: root_file_id,
        },
        scratch_path,
        file_identity,
        expected_size,
        created_at_unix_ms,
        state,
    };
    validate_record(&record, allow_non_volume_root)?;
    Ok(record)
}

fn validate_record(record: &LeaseRecord, allow_non_volume_root: bool) -> io::Result<()> {
    if !is_lower_hex(&record.lease_id, 32)
        || record.created_at_unix_ms == 0
        || record.root_identity.identifier == [0; 16]
        || record
            .file_identity
            .is_some_and(|identity| identity.identifier == [0; 16])
    {
        return Err(invalid_data(
            "lease journal identity or timestamp is invalid",
        ));
    }
    super::validate_size(record.expected_size)?;
    let root = record
        .volume_root
        .to_str()
        .ok_or_else(|| invalid_data("lease journal volume root is not Unicode"))?;
    let scratch = record
        .scratch_path
        .to_str()
        .ok_or_else(|| invalid_data("lease journal scratch path is not Unicode"))?;
    let parent = record
        .scratch_path
        .parent()
        .ok_or_else(|| invalid_data("lease journal scratch path has no parent"))?;
    let expected_name = format!(".partition11-scratch-{}.bin", record.lease_id);
    if root
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '='))
        || scratch
            .chars()
            .any(|character| matches!(character, '\r' | '\n' | '='))
        || (!allow_non_volume_root
            && !super::windows_path_equal(
                &record.volume_root,
                PathBuf::from(super::volume_guid_prefix(&record.volume_root)?).as_path(),
            ))
        || !super::windows_path_equal(parent, &record.volume_root)
        || record
            .scratch_path
            .file_name()
            .and_then(|name| name.to_str())
            != Some(expected_name.as_str())
    {
        return Err(invalid_data(
            "lease journal paths are not canonical or bound to the lease ID",
        ));
    }
    match (record.state, record.file_identity) {
        (LeaseState::Prepared, None) => {}
        (LeaseState::Prepared, Some(_)) => {
            return Err(invalid_data(
                "prepared lease must not claim a file identity",
            ));
        }
        (_, Some(identity)) if identity.volume_serial == record.root_identity.volume_serial => {}
        (_, _) => {
            return Err(invalid_data(
                "lease journal is missing a matching file identity",
            ))
        }
    }
    Ok(())
}

fn is_lower_hex(value: &str, expected_length: usize) -> bool {
    value.len() == expected_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn decode_hex<const N: usize>(value: &str) -> io::Result<[u8; N]> {
    if !is_lower_hex(value, N * 2) {
        return Err(invalid_data(
            "lease journal hexadecimal identity is invalid",
        ));
    }
    let mut bytes = [0u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| invalid_data("lease journal hexadecimal identity is invalid"))?;
    }
    Ok(bytes)
}

fn parse_u64_hex(value: &str) -> io::Result<u64> {
    if !is_lower_hex(value, 16) {
        return Err(invalid_data("lease journal volume serial is invalid"));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| invalid_data("lease journal volume serial is invalid"))
}

fn parse_decimal(value: &str) -> io::Result<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid_data("lease journal decimal field is invalid"));
    }
    value
        .parse()
        .map_err(|_| invalid_data("lease journal decimal field is invalid"))
}

fn state_name(state: LeaseState) -> &'static str {
    match state {
        LeaseState::Prepared => "prepared",
        LeaseState::Ready => "ready",
        LeaseState::Running => "running",
        LeaseState::CleanupPending => "cleanup_pending",
    }
}

fn parse_state(value: &str) -> io::Result<LeaseState> {
    match value {
        "prepared" => Ok(LeaseState::Prepared),
        "ready" => Ok(LeaseState::Ready),
        "running" => Ok(LeaseState::Running),
        "cleanup_pending" => Ok(LeaseState::CleanupPending),
        _ => Err(invalid_data("lease journal state is unknown")),
    }
}

fn state_rank(state: LeaseState) -> u8 {
    match state {
        LeaseState::Prepared => 0,
        LeaseState::Ready => 1,
        LeaseState::Running => 2,
        LeaseState::CleanupPending => 3,
    }
}

fn validate_transition(previous: &LeaseRecord, next: &LeaseRecord) -> io::Result<()> {
    if previous == next {
        return Ok(());
    }
    let same_lease = previous.lease_id == next.lease_id
        && previous.volume_root == next.volume_root
        && previous.root_identity == next.root_identity
        && previous.scratch_path == next.scratch_path
        && previous.expected_size == next.expected_size
        && previous.created_at_unix_ms == next.created_at_unix_ms;
    if !same_lease {
        return Err(invalid_data(
            "journal update changed immutable lease identity",
        ));
    }
    let valid = match (previous.state, next.state) {
        (LeaseState::Prepared, LeaseState::Ready) => {
            previous.file_identity.is_none()
                && next.file_identity.is_some_and(|identity| {
                    identity.volume_serial == previous.root_identity.volume_serial
                })
        }
        (LeaseState::Ready, LeaseState::Running)
        | (LeaseState::Ready, LeaseState::CleanupPending)
        | (LeaseState::Running, LeaseState::CleanupPending) => {
            previous.file_identity.is_some() && previous.file_identity == next.file_identity
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_data(
            "journal lifecycle transition is not monotonic",
        ))
    }
}

fn parse_temp_name(name: &str) -> Option<(&str, &str)> {
    let value = name.strip_prefix('.')?;
    let (lease_id, nonce) = value.split_once(".tmp-")?;
    (is_lower_hex(lease_id, 32) && is_lower_hex(nonce, 32)).then_some((lease_id, nonce))
}

const JOURNAL_DIRECTORY_NAME: &str = "Partition11.BenchmarkLeases";
const JOURNAL_LOCK_NAME: &str = ".journal.lock";
const MAX_JOURNAL_RECORDS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveryResult {
    Clean,
    CleanupRequired,
    Deferred,
}

pub(super) struct LeaseJournal {
    directory: PathBuf,
    directory_identity: ObjectIdentity,
    _directory_handle: super::Handle,
    _lock_handle: super::Handle,
    // Non-volume roots are accepted only by the explicit cfg(test) constructor.
    allow_non_volume_root: bool,
}

impl LeaseJournal {
    pub(super) fn open_default() -> io::Result<Self> {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| invalid_data("Local AppData is unavailable"))?;
        super::validate_root_argument(&local_app_data)?;
        Self::open_at(&local_app_data.join(JOURNAL_DIRECTORY_NAME))
    }

    pub(super) fn open_at(directory: &std::path::Path) -> io::Result<Self> {
        Self::open_at_with_policy(directory, false)
    }

    #[cfg(test)]
    pub(super) fn open_at_for_test(directory: &std::path::Path) -> io::Result<Self> {
        Self::open_at_with_policy(directory, true)
    }

    fn open_at_with_policy(
        directory: &std::path::Path,
        allow_non_volume_root: bool,
    ) -> io::Result<Self> {
        let parent = directory
            .parent()
            .ok_or_else(|| invalid_data("journal directory has no parent"))?;
        super::validate_root_argument(parent)?;
        let parent_handle = super::open_directory(parent, super::FILE_READ_ATTRIBUTES)?;
        let parent_identity = super::query_identity(parent_handle.raw())?;
        super::validate_directory(parent_handle.raw(), &parent_identity)?;
        let directory_name = directory
            .file_name()
            .ok_or_else(|| invalid_data("journal directory name is missing"))?;
        let expected_path = parent_identity.final_path.join(directory_name);
        let directory_wide = super::wide_path(&expected_path)?;
        let (_descriptor, security_attributes) =
            super::protected_user_system_security_attributes()?;
        if unsafe {
            super::CreateDirectoryW(
                directory_wide.as_ptr(),
                &security_attributes as *const super::SecurityAttributes as *const c_void,
            )
        } == 0
        {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(super::ERROR_ALREADY_EXISTS) {
                return Err(error);
            }
        }

        let directory_handle = super::open_directory(
            &expected_path,
            super::FILE_READ_ATTRIBUTES | super::READ_CONTROL,
        )?;
        let directory_identity = super::query_identity(directory_handle.raw())?;
        super::validate_directory(directory_handle.raw(), &directory_identity)?;
        super::validate_restricted_file_dacl(directory_handle.raw())?;
        if !super::windows_path_equal(&expected_path, &directory_identity.final_path) {
            return Err(invalid_data("journal directory path changed while opening"));
        }

        let lock_path = directory_identity.final_path.join(JOURNAL_LOCK_NAME);
        let lock_wide = super::wide_path(&lock_path)?;
        let (_lock_descriptor, lock_attributes) =
            super::protected_user_system_security_attributes()?;
        let lock_raw = unsafe {
            super::CreateFileW(
                lock_wide.as_ptr(),
                super::GENERIC_READ | super::GENERIC_WRITE | super::FILE_READ_ATTRIBUTES,
                0,
                &lock_attributes as *const super::SecurityAttributes as *const c_void,
                super::OPEN_ALWAYS,
                super::FILE_ATTRIBUTE_NORMAL | super::FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        let lock_handle = super::Handle::from_raw(lock_raw)?;
        super::validate_restricted_file_dacl(lock_handle.raw())?;
        let lock_identity = super::query_identity(lock_handle.raw())?;
        super::validate_owned_regular_file(lock_handle.raw(), &lock_identity)?;
        if !super::windows_path_equal(&lock_identity.final_path, &lock_path) {
            return Err(invalid_data("journal lock path changed while opening"));
        }

        Ok(Self {
            directory: directory_identity.final_path.clone(),
            directory_identity,
            _directory_handle: directory_handle,
            _lock_handle: lock_handle,
            allow_non_volume_root,
        })
    }

    pub(super) fn ensure_empty(&self) -> io::Result<()> {
        let records = self.read_records()?;
        if records.is_empty() {
            Ok(())
        } else {
            Err(invalid_data(
                "unresolved scratch journal blocks a new benchmark",
            ))
        }
    }

    pub(super) fn persist_new(&self, record: &LeaseRecord) -> io::Result<()> {
        if record.state != LeaseState::Prepared || record.file_identity.is_some() {
            return Err(invalid_data("new journal entries must start prepared"));
        }
        if !self.read_records()?.is_empty() {
            return Err(invalid_data(
                "unresolved scratch journal blocks a new benchmark",
            ));
        }
        self.write_new_record(record)
    }

    pub(super) fn persist(&self, record: &LeaseRecord) -> io::Result<()> {
        self.validate_directory()?;
        let records = self.read_records()?;
        if records
            .iter()
            .any(|existing| existing.lease_id != record.lease_id)
        {
            return Err(invalid_data(
                "another unresolved scratch lease blocks this update",
            ));
        }
        let current = self.read_record(&record.lease_id)?;
        validate_transition(&current, record)?;
        if current == *record {
            return Ok(());
        }
        let temp_path = self.write_temp_record(record)?;
        self.promote_temp(&temp_path, record, Some(&current))?;
        let persisted = self.read_record(&record.lease_id)?;
        if persisted != *record {
            return Err(invalid_data(
                "journal record changed during atomic persistence",
            ));
        }
        Ok(())
    }

    fn write_new_record(&self, record: &LeaseRecord) -> io::Result<()> {
        self.validate_directory()?;
        if self.read_record_if_present(&record.lease_id)?.is_some() {
            return Err(invalid_data("duplicate scratch lease ID"));
        }
        let temp_path = self.write_temp_record(record)?;
        self.promote_temp(&temp_path, record, None)?;
        let persisted = self.read_record(&record.lease_id)?;
        if persisted != *record {
            return Err(invalid_data(
                "journal record changed during atomic insertion",
            ));
        }
        Ok(())
    }

    fn write_temp_record(&self, record: &LeaseRecord) -> io::Result<PathBuf> {
        self.validate_directory()?;
        let bytes = encode_record_with_policy(record, self.allow_non_volume_root)?;
        let temp_path = self.unique_temp_path(&record.lease_id)?;
        let temp_wide = super::wide_path(&temp_path)?;
        let (_descriptor, security_attributes) =
            super::protected_user_system_security_attributes()?;
        let raw = unsafe {
            super::CreateFileW(
                temp_wide.as_ptr(),
                super::GENERIC_WRITE | super::FILE_READ_ATTRIBUTES,
                0,
                &security_attributes as *const super::SecurityAttributes as *const c_void,
                super::CREATE_NEW,
                super::FILE_ATTRIBUTE_NORMAL | super::FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        let mut temp_handle = super::Handle::from_raw(raw)?;
        let mut bytes_written = 0u32;
        if unsafe {
            super::WriteFile(
                temp_handle.raw(),
                bytes.as_ptr() as *const c_void,
                bytes.len() as u32,
                &mut bytes_written,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if bytes_written as usize != bytes.len()
            || unsafe { super::FlushFileBuffers(temp_handle.raw()) } == 0
        {
            return Err(io::Error::last_os_error());
        }
        temp_handle.close()?;
        Ok(temp_path)
    }

    fn promote_temp(
        &self,
        temp_path: &std::path::Path,
        record: &LeaseRecord,
        expected_existing: Option<&LeaseRecord>,
    ) -> io::Result<()> {
        self.validate_directory()?;
        match expected_existing {
            Some(expected) => {
                let current = self.read_record(&record.lease_id)?;
                if current != *expected {
                    return Err(invalid_data("journal changed before atomic replacement"));
                }
                validate_transition(expected, record)?;
            }
            None => {
                if self.read_record_if_present(&record.lease_id)?.is_some() {
                    return Err(invalid_data("duplicate scratch lease ID"));
                }
                if record.state != LeaseState::Prepared || record.file_identity.is_some() {
                    return Err(invalid_data("new journal entries must start prepared"));
                }
            }
        }
        let source_wide = super::wide_path(temp_path)?;
        let destination_wide = super::wide_path(&self.record_path(&record.lease_id))?;
        let flags = super::MOVEFILE_WRITE_THROUGH
            | if expected_existing.is_some() {
                super::MOVEFILE_REPLACE_EXISTING
            } else {
                0
            };
        if unsafe { super::MoveFileExW(source_wide.as_ptr(), destination_wide.as_ptr(), flags) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn read_record(&self, lease_id: &str) -> io::Result<LeaseRecord> {
        self.validate_directory()?;
        let path = self.record_path(lease_id);
        self.read_record_at(&path, lease_id)
    }

    fn read_record_at(&self, path: &std::path::Path, lease_id: &str) -> io::Result<LeaseRecord> {
        self.validate_directory()?;
        let wide = super::wide_path(path)?;
        let raw = unsafe {
            super::CreateFileW(
                wide.as_ptr(),
                super::GENERIC_READ | super::FILE_READ_ATTRIBUTES | super::READ_CONTROL,
                0,
                null(),
                super::OPEN_EXISTING,
                super::FILE_ATTRIBUTE_NORMAL | super::FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        let handle = super::Handle::from_raw(raw)?;
        super::validate_restricted_file_dacl(handle.raw())?;
        self.read_record_from_handle(handle.raw(), path, lease_id)
    }

    fn read_record_if_present(&self, lease_id: &str) -> io::Result<Option<LeaseRecord>> {
        let path = self.record_path(lease_id);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => self.read_record(lease_id).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn read_record_from_handle(
        &self,
        handle: super::RawHandle,
        path: &std::path::Path,
        lease_id: &str,
    ) -> io::Result<LeaseRecord> {
        let identity = super::query_identity(handle)?;
        if !super::windows_path_equal(&identity.final_path, path) {
            return Err(invalid_data("journal record path changed while opening"));
        }
        let (attributes, standard) = super::query_attributes_and_standard(handle)?;
        super::validate_owned_file_metadata(attributes, &standard)?;
        if standard.end_of_file < 0 || standard.end_of_file as usize > MAX_RECORD_BYTES {
            return Err(invalid_data("journal record size is invalid"));
        }
        let mut bytes = vec![0u8; standard.end_of_file as usize];
        let mut bytes_read = 0u32;
        if unsafe {
            super::ReadFile(
                handle,
                bytes.as_mut_ptr() as *mut c_void,
                bytes.len() as u32,
                &mut bytes_read,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if bytes_read as usize != bytes.len() {
            return Err(invalid_data("journal record was read incompletely"));
        }
        let record = decode_record_with_policy(&bytes, self.allow_non_volume_root)?;
        if record.lease_id != lease_id {
            return Err(invalid_data("journal filename does not match its lease ID"));
        }
        Ok(record)
    }

    pub(super) fn remove_record(&self, expected_record: &LeaseRecord) -> io::Result<()> {
        self.remove_journal_file(
            &self.record_path(&expected_record.lease_id),
            &expected_record.lease_id,
            expected_record,
        )
    }

    fn remove_journal_file(
        &self,
        path: &std::path::Path,
        lease_id: &str,
        expected_record: &LeaseRecord,
    ) -> io::Result<()> {
        self.validate_directory()?;
        let wide = super::wide_path(path)?;
        let raw = unsafe {
            super::CreateFileW(
                wide.as_ptr(),
                super::DELETE
                    | super::GENERIC_READ
                    | super::FILE_READ_ATTRIBUTES
                    | super::READ_CONTROL,
                0,
                null(),
                super::OPEN_EXISTING,
                super::FILE_ATTRIBUTE_NORMAL | super::FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        let mut handle = super::Handle::from_raw(raw)?;
        super::validate_restricted_file_dacl(handle.raw())?;
        let identity = super::query_identity(handle.raw())?;
        if !super::windows_path_equal(&identity.final_path, path) {
            return Err(invalid_data("journal record path changed before removal"));
        }
        let record = self.read_record_from_handle(handle.raw(), path, lease_id)?;
        if record != *expected_record {
            return Err(invalid_data("journal record changed before removal"));
        }
        super::set_delete_disposition(handle.raw())?;
        handle.close()?;
        super::ensure_path_absent(path)
    }

    pub(super) fn recover(&self) -> RecoveryResult {
        self.recover_with_policy(false)
    }

    #[cfg(test)]
    fn recover_test_records(&self) -> RecoveryResult {
        self.recover_with_policy(true)
    }

    fn recover_with_policy(&self, allow_non_volume_root: bool) -> RecoveryResult {
        let records = match self.read_records_with_policy(allow_non_volume_root) {
            Ok(records) => records,
            Err(_) => return RecoveryResult::CleanupRequired,
        };
        let mut result = RecoveryResult::Clean;
        for record in records {
            if record.state == LeaseState::Prepared || record.file_identity.is_none() {
                result = RecoveryResult::CleanupRequired;
                continue;
            }
            match self.recover_record(&record) {
                RecoveryResult::Clean => {}
                RecoveryResult::Deferred if result == RecoveryResult::Clean => {
                    result = RecoveryResult::Deferred;
                }
                RecoveryResult::Deferred => {}
                RecoveryResult::CleanupRequired => result = RecoveryResult::CleanupRequired,
            }
        }
        result
    }

    fn recover_record(&self, record: &LeaseRecord) -> RecoveryResult {
        let root_handle =
            match super::open_directory(&record.volume_root, super::FILE_READ_ATTRIBUTES) {
                Ok(handle) => handle,
                Err(_) => return RecoveryResult::Deferred,
            };
        let root_identity = match super::query_identity(root_handle.raw()) {
            Ok(identity) => identity,
            Err(_) => return RecoveryResult::Deferred,
        };
        if root_identity.file_id != record.root_identity
            || !super::windows_path_equal(&root_identity.final_path, &record.volume_root)
            || super::validate_directory(root_handle.raw(), &root_identity).is_err()
        {
            return RecoveryResult::CleanupRequired;
        }
        let Some(file_id) = record.file_identity else {
            return RecoveryResult::CleanupRequired;
        };
        let delete_handle = match super::open_file_for_delete(&record.scratch_path) {
            Ok(handle) => handle,
            Err(error) if error.raw_os_error() == Some(2) => {
                return if self.remove_record(record).is_ok() {
                    RecoveryResult::Clean
                } else {
                    RecoveryResult::CleanupRequired
                };
            }
            Err(error) if matches!(error.raw_os_error(), Some(3) | Some(21)) => {
                return RecoveryResult::Deferred;
            }
            Err(_) => return RecoveryResult::CleanupRequired,
        };
        let expected = ObjectIdentity {
            file_id,
            final_path: record.scratch_path.clone(),
        };
        if super::validate_regular_file(delete_handle.raw(), &expected, record.expected_size)
            .is_err()
            || super::validate_restricted_file_dacl(delete_handle.raw()).is_err()
        {
            return RecoveryResult::CleanupRequired;
        }
        let volume_prefix = match super::volume_guid_prefix(&record.volume_root) {
            Ok(prefix) => prefix,
            Err(_) => return RecoveryResult::CleanupRequired,
        };
        if super::ensure_same_volume(&root_identity, &expected, &volume_prefix).is_err() {
            return RecoveryResult::CleanupRequired;
        }
        let mut delete_handle = delete_handle;
        if super::set_delete_disposition(delete_handle.raw()).is_err()
            || delete_handle.close().is_err()
            || super::ensure_path_absent(&record.scratch_path).is_err()
        {
            return RecoveryResult::CleanupRequired;
        }
        if self.remove_record(record).is_err() {
            return RecoveryResult::CleanupRequired;
        }
        RecoveryResult::Clean
    }

    fn read_records(&self) -> io::Result<Vec<LeaseRecord>> {
        self.read_records_with_policy(false)
    }

    fn read_records_with_policy(
        &self,
        allow_non_volume_root: bool,
    ) -> io::Result<Vec<LeaseRecord>> {
        let allow_non_volume_root = allow_non_volume_root || self.allow_non_volume_root;
        self.validate_directory()?;
        self.reconcile_temps()?;
        let mut records = Vec::new();
        for entry in std::fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid_data("journal contains a non-Unicode entry"))?;
            if name == JOURNAL_LOCK_NAME {
                continue;
            }
            let lease_id = name
                .strip_suffix(".json")
                .filter(|id| is_lower_hex(id, 32))
                .ok_or_else(|| invalid_data("journal contains an unexpected entry"))?;
            let record = self.read_record(lease_id)?;
            if !allow_non_volume_root
                && !super::windows_path_equal(
                    &record.volume_root,
                    PathBuf::from(super::volume_guid_prefix(&record.volume_root)?).as_path(),
                )
            {
                return Err(invalid_data("journal target is not a volume root"));
            }
            records.push(record);
            if records.len() > MAX_JOURNAL_RECORDS {
                return Err(invalid_data("journal contains too many unresolved leases"));
            }
        }
        Ok(records)
    }

    fn reconcile_temps(&self) -> io::Result<()> {
        self.validate_directory()?;
        let mut groups: BTreeMap<String, Vec<(PathBuf, LeaseRecord)>> = BTreeMap::new();
        let mut temp_count = 0usize;
        for entry in std::fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid_data("journal contains a non-Unicode entry"))?;
            if name == JOURNAL_LOCK_NAME || name.ends_with(".json") {
                continue;
            }
            let Some((lease_id, _nonce)) = parse_temp_name(&name) else {
                continue;
            };
            temp_count += 1;
            if temp_count > MAX_JOURNAL_RECORDS * 4 {
                return Err(invalid_data("journal contains too many temporary records"));
            }
            let path = entry.path();
            let record = self.read_record_at(&path, lease_id)?;
            groups
                .entry(lease_id.to_string())
                .or_default()
                .push((path, record));
        }

        for (lease_id, mut temps) in groups {
            let existing = self.read_record_if_present(&lease_id)?;
            if existing.is_none()
                && temps
                    .iter()
                    .any(|(_, record)| record.state != LeaseState::Prepared)
            {
                return Err(invalid_data(
                    "journal transition temp has no committed predecessor",
                ));
            }
            temps.sort_by_key(|(_, record)| state_rank(record.state));
            let mut lineage = Vec::with_capacity(temps.len() + usize::from(existing.is_some()));
            if let Some(record) = existing.as_ref() {
                lineage.push(record.clone());
            }
            lineage.extend(temps.iter().map(|(_, record)| record.clone()));
            lineage.sort_by_key(|record| state_rank(record.state));
            for pair in lineage.windows(2) {
                validate_transition(&pair[0], &pair[1])?;
            }
            let desired = lineage
                .last()
                .ok_or_else(|| invalid_data("journal temp group is empty"))?;
            let promote = existing.as_ref() != Some(desired);
            let promoted_path = if promote {
                let (path, _) = temps
                    .iter()
                    .find(|(_, record)| record == desired)
                    .ok_or_else(|| invalid_data("latest journal transition temp is missing"))?;
                self.promote_temp(path, desired, existing.as_ref())?;
                Some(path.clone())
            } else {
                None
            };
            for (path, record) in temps {
                if promoted_path.as_ref() == Some(&path) {
                    continue;
                }
                self.remove_journal_file(&path, &lease_id, &record)?;
            }
        }
        self.validate_directory()
    }

    fn validate_directory(&self) -> io::Result<()> {
        super::validate_directory(self._directory_handle.raw(), &self.directory_identity)?;
        super::validate_restricted_file_dacl(self._directory_handle.raw())
    }

    fn record_path(&self, lease_id: &str) -> PathBuf {
        self.directory.join(format!("{lease_id}.json"))
    }

    fn unique_temp_path(&self, lease_id: &str) -> io::Result<PathBuf> {
        let random_name = super::random_file_name()?;
        let nonce = random_name
            .strip_prefix(super::SCRATCH_FILE_PREFIX)
            .and_then(|name| name.strip_suffix(".bin"))
            .ok_or_else(|| invalid_data("could not create a journal temp name"))?;
        Ok(self.directory.join(format!(".{lease_id}.tmp-{nonce}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_record, encode_record, LeaseJournal, LeaseRecord, LeaseState};
    use crate::scratch::{FileId, ObjectIdentity};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_JOURNAL_ROOT: AtomicU64 = AtomicU64::new(1);

    fn test_root() -> PathBuf {
        let ticks = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "p11-journal-test-{}-{ticks}-{}",
            std::process::id(),
            NEXT_JOURNAL_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::canonicalize(root).unwrap()
    }

    fn ready_record() -> LeaseRecord {
        let lease_id = "0123456789abcdef0123456789abcdef".to_string();
        let volume_root = PathBuf::from("\\\\?\\Volume{01234567-89ab-cdef-0123-456789abcdef}\\");
        let root_identity = ObjectIdentity {
            file_id: FileId {
                volume_serial: 0x1122_3344_5566_7788,
                identifier: [0x11; 16],
            },
            final_path: volume_root.clone(),
        };
        let mut record = LeaseRecord::prepared(
            lease_id.clone(),
            &root_identity,
            volume_root.join(format!(".partition11-scratch-{lease_id}.bin")),
            8 * 1024 * 1024,
            1_800_000_000_000,
        );
        record.ready(FileId {
            volume_serial: root_identity.file_id.volume_serial,
            identifier: [0x22; 16],
        });
        record
    }

    fn prepared_record() -> LeaseRecord {
        let mut record = ready_record();
        record.file_identity = None;
        record.state = LeaseState::Prepared;
        record
    }

    #[test]
    fn journal_record_round_trips_exact_identity_and_state() {
        let record = ready_record();
        let encoded = encode_record(&record).unwrap();
        assert_eq!(decode_record(&encoded).unwrap(), record);
    }

    #[test]
    fn journal_parser_rejects_unknown_fields_and_missing_file_identity() {
        let encoded = encode_record(&ready_record()).unwrap();
        let text = String::from_utf8(encoded).unwrap();
        let with_unknown = text.replace("state=ready\n", "unexpected=1\nstate=ready\n");
        assert!(decode_record(with_unknown.as_bytes()).is_err());

        let mut record = ready_record();
        record.file_identity = None;
        record.state = LeaseState::Running;
        assert!(encode_record(&record).is_err());
    }

    #[test]
    fn journal_persists_replaces_and_removes_only_the_named_record() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let mut record = prepared_record();

        journal.persist_new(&record)?;
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        assert!(journal.ensure_empty().is_err());
        assert!(
            LeaseJournal::open_at(&directory).is_err(),
            "journal lock must be exclusive"
        );

        record.ready(FileId {
            volume_serial: record.root_identity.volume_serial,
            identifier: [0x22; 16],
        });
        journal.persist(&record)?;
        record.state = LeaseState::Running;
        journal.persist(&record)?;
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        let mut stale_record = record.clone();
        stale_record.state = LeaseState::Ready;
        assert!(journal.remove_record(&stale_record).is_err());
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        journal.remove_record(&record)?;
        assert!(journal.ensure_empty().is_ok());
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn recovery_never_guesses_a_prepared_file_id() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let record = prepared_record();
        journal.persist_new(&record)?;

        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::CleanupRequired
        );
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::CleanupRequired
        );
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn recovery_defers_an_unavailable_volume_and_keeps_its_record() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let mut record = prepared_record();
        journal.persist_new(&record)?;
        record.ready(FileId {
            volume_serial: record.root_identity.volume_serial,
            identifier: [0x22; 16],
        });
        journal.persist(&record)?;

        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::Deferred
        );
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn duplicate_initial_lease_id_never_replaces_the_committed_record() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let record = prepared_record();
        journal.persist_new(&record)?;

        assert!(journal.persist_new(&record).is_err());
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn recovery_promotes_a_flushed_initial_temp_and_keeps_it_fail_closed() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let record = prepared_record();
        let temp = journal.write_temp_record(&record)?;

        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::CleanupRequired
        );
        assert_eq!(journal.read_record(&record.lease_id)?, record);
        assert!(!temp.exists());
        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::CleanupRequired,
            "recovery remains idempotently fail-closed for an unresolved Prepared lease"
        );
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn recovery_promotes_only_a_valid_next_state_temp() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let prepared = prepared_record();
        journal.persist_new(&prepared)?;
        let mut ready = prepared;
        ready.ready(FileId {
            volume_serial: ready.root_identity.volume_serial,
            identifier: [0x22; 16],
        });
        let temp = journal.write_temp_record(&ready)?;

        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::Deferred
        );
        assert_eq!(journal.read_record(&ready.lease_id)?, ready);
        assert!(!temp.exists());
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn malformed_journal_temp_is_not_promoted_or_deleted() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let temp = journal.write_temp_record(&prepared_record())?;
        fs::write(&temp, b"truncated")?;

        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::CleanupRequired
        );
        assert!(temp.exists());
        drop(journal);
        fs::remove_dir_all(root)
    }

    #[test]
    fn journal_temp_with_untrusted_default_acl_is_not_promoted_or_deleted() -> std::io::Result<()> {
        let root = test_root();
        let directory = root.join("journal");
        let journal = LeaseJournal::open_at(&directory)?;
        let record = prepared_record();
        let temp = directory.join(format!(".{}.tmp-{}", record.lease_id, "a".repeat(32)));
        fs::write(&temp, encode_record(&record)?)?;

        assert_eq!(
            journal.recover_test_records(),
            super::RecoveryResult::CleanupRequired
        );
        assert!(temp.exists());
        assert!(journal.read_record_if_present(&record.lease_id)?.is_none());
        drop(journal);
        fs::remove_dir_all(root)
    }
}
