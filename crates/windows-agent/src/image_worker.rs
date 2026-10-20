use partition11_ext_reader::{
    validate_local_file_path, ExtDirectoryEntry, ExtEntryType, ExtImageError, ExtImageReader,
    MAX_EXPORT_BYTES,
};
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::PathBuf;

const REQUEST_MAGIC: &[u8; 8] = b"P11IMG02";
const RESPONSE_MAGIC: &[u8; 8] = b"P11IMR02";
const MAX_HOST_PATH_BYTES: usize = 32 * 1024;
const MAX_INNER_PATH_BYTES: usize = 4096;
const MAX_REQUEST_BYTES: usize = 8
    + 1
    + 4
    + MAX_HOST_PATH_BYTES
    + 4
    + MAX_INNER_PATH_BYTES
    + 4
    + MAX_HOST_PATH_BYTES
    + 4
    + MAX_HOST_PATH_BYTES;

#[derive(Debug, PartialEq, Eq)]
enum Request {
    List {
        image_path: PathBuf,
        internal_path: Vec<u8>,
    },
    Export {
        image_path: PathBuf,
        internal_path: Vec<u8>,
        destination_path: PathBuf,
        staging_path: PathBuf,
    },
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum WorkerStatus {
    Success = 0,
    InvalidRequest = 1,
    InvalidImage = 2,
    UnsupportedFilesystem = 3,
    PathUnavailable = 4,
    FileTooLarge = 5,
    OutputExists = 6,
    OutputFailed = 7,
    NeedsRecovery = 8,
}

enum WorkerResponse {
    Directory(Vec<ExtDirectoryEntry>),
    Exported(u64),
    Error(WorkerStatus),
}

fn decode_request(input: &[u8]) -> Result<Request, ()> {
    let mut cursor = 0;
    if take(input, &mut cursor, REQUEST_MAGIC.len())? != REQUEST_MAGIC {
        return Err(());
    }
    let operation = take(input, &mut cursor, 1)?[0];
    let image_path = decode_path(read_field(input, &mut cursor, MAX_HOST_PATH_BYTES)?)?;
    let internal_path = read_field(input, &mut cursor, MAX_INNER_PATH_BYTES)?.to_vec();
    if internal_path.is_empty() || internal_path[0] != b'/' || internal_path.contains(&0) {
        return Err(());
    }
    let destination_bytes = read_field(input, &mut cursor, MAX_HOST_PATH_BYTES)?;
    let staging_bytes = read_field(input, &mut cursor, MAX_HOST_PATH_BYTES)?;
    if cursor != input.len() {
        return Err(());
    }

    match operation {
        1 if destination_bytes.is_empty() && staging_bytes.is_empty() => Ok(Request::List {
            image_path,
            internal_path,
        }),
        2 if !destination_bytes.is_empty() && !staging_bytes.is_empty() => {
            let destination_path = decode_path(destination_bytes)?;
            let staging_path = decode_path(staging_bytes)?;
            if destination_path == staging_path
                || destination_path.parent() != staging_path.parent()
            {
                return Err(());
            }
            Ok(Request::Export {
                image_path,
                internal_path,
                destination_path,
                staging_path,
            })
        }
        _ => Err(()),
    }
}

fn decode_path(bytes: &[u8]) -> Result<PathBuf, ()> {
    if bytes.is_empty() || bytes.len() > MAX_HOST_PATH_BYTES || bytes.contains(&0) {
        return Err(());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    Ok(PathBuf::from(text))
}

fn read_field<'a>(input: &'a [u8], cursor: &mut usize, max: usize) -> Result<&'a [u8], ()> {
    let length_bytes = take(input, cursor, 4)?;
    let length = u32::from_le_bytes(length_bytes.try_into().map_err(|_| ())?) as usize;
    if length > max {
        return Err(());
    }
    take(input, cursor, length)
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], ()> {
    let end = cursor.checked_add(length).ok_or(())?;
    let value = input.get(*cursor..end).ok_or(())?;
    *cursor = end;
    Ok(value)
}

fn encode_response(response: WorkerResponse) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(RESPONSE_MAGIC);
    match response {
        WorkerResponse::Error(status) => output.push(status as u8),
        WorkerResponse::Directory(entries) => {
            output.push(WorkerStatus::Success as u8);
            output.push(1);
            output.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            for entry in entries {
                output.push(match entry.kind {
                    ExtEntryType::File => 1,
                    ExtEntryType::Directory => 2,
                    ExtEntryType::Symlink => 3,
                    ExtEntryType::Other => 4,
                });
                output.extend_from_slice(&entry.size_bytes.to_le_bytes());
                output.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
                output.extend_from_slice(&entry.name);
            }
        }
        WorkerResponse::Exported(bytes) => {
            output.push(WorkerStatus::Success as u8);
            output.push(2);
            output.extend_from_slice(&bytes.to_le_bytes());
        }
    }
    output
}

fn map_open_error(error: ExtImageError) -> WorkerStatus {
    match error {
        ExtImageError::FilesystemNeedsRecovery => WorkerStatus::NeedsRecovery,
        ExtImageError::UnsupportedFilesystemType | ExtImageError::UnsupportedFeatureBits { .. } => {
            WorkerStatus::UnsupportedFilesystem
        }
        _ => WorkerStatus::InvalidImage,
    }
}

fn list_directory(image_path: &std::path::Path, internal_path: &[u8]) -> WorkerResponse {
    let reader = match ExtImageReader::open(image_path) {
        Ok(reader) => reader,
        Err(error) => return WorkerResponse::Error(map_open_error(error)),
    };
    match reader.list_directory(internal_path) {
        Ok(entries) => WorkerResponse::Directory(entries),
        Err(_) => WorkerResponse::Error(WorkerStatus::PathUnavailable),
    }
}

fn export_file(
    image_path: &std::path::Path,
    internal_path: &[u8],
    destination_path: &std::path::Path,
    staging_path: &std::path::Path,
) -> WorkerResponse {
    if validate_local_file_path(destination_path).is_err()
        || validate_local_file_path(staging_path).is_err()
        || destination_path == staging_path
        || destination_path.parent() != staging_path.parent()
    {
        return WorkerResponse::Error(WorkerStatus::OutputFailed);
    }
    let reader = match ExtImageReader::open(image_path) {
        Ok(reader) => reader,
        Err(error) => return WorkerResponse::Error(map_open_error(error)),
    };
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let mut output = match options.open(staging_path) {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return WorkerResponse::Error(WorkerStatus::OutputExists);
        }
        Err(_) => return WorkerResponse::Error(WorkerStatus::OutputFailed),
    };

    let bytes = match reader.copy_file_to(internal_path, MAX_EXPORT_BYTES, &mut output) {
        Ok(bytes) if output.sync_all().is_ok() => bytes,
        Err(ExtImageError::FileTooLarge { .. }) => {
            drop(output);
            let _ = std::fs::remove_file(staging_path);
            return WorkerResponse::Error(WorkerStatus::FileTooLarge);
        }
        _ => {
            drop(output);
            let _ = std::fs::remove_file(staging_path);
            return WorkerResponse::Error(WorkerStatus::OutputFailed);
        }
    };
    drop(output);

    match win32_file_move::move_no_replace(staging_path, destination_path) {
        Ok(()) => WorkerResponse::Exported(bytes),
        Err(_) => {
            let destination_exists = std::fs::symlink_metadata(destination_path).is_ok();
            let _ = std::fs::remove_file(staging_path);
            WorkerResponse::Error(if destination_exists {
                WorkerStatus::OutputExists
            } else {
                WorkerStatus::OutputFailed
            })
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod win32_file_move {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    pub(super) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let from: Vec<u16> = from
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let to: Vec<u16> = to
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let moved = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if moved == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(windows))]
mod win32_file_move {
    use std::io;
    use std::path::Path;

    pub(super) fn move_no_replace(_from: &Path, _to: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows export commit required",
        ))
    }
}
fn process_request(request: Request) -> WorkerResponse {
    match request {
        Request::List {
            image_path,
            internal_path,
        } => list_directory(&image_path, &internal_path),
        Request::Export {
            image_path,
            internal_path,
            destination_path,
            staging_path,
        } => export_file(
            &image_path,
            &internal_path,
            &destination_path,
            &staging_path,
        ),
    }
}

pub fn run_stdio_worker() -> i32 {
    let mut input = Vec::with_capacity(1024);
    if io::stdin()
        .lock()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .is_err()
        || input.len() > MAX_REQUEST_BYTES
    {
        return 2;
    }
    let response = match decode_request(&input) {
        Ok(request) => encode_response(process_request(request)),
        Err(()) => encode_response(WorkerResponse::Error(WorkerStatus::InvalidRequest)),
    };
    match io::stdout().lock().write_all(&response) {
        Ok(()) => 0,
        Err(_) => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_request, Request};
    #[cfg(windows)]
    use partition11_ext_reader::validate_local_file_path;
    use std::path::PathBuf;

    fn encoded_request(operation: u8, image: &str, inner: &[u8], destination: &str) -> Vec<u8> {
        let mut bytes = b"P11IMG02".to_vec();
        bytes.push(operation);
        put_bytes(&mut bytes, image.as_bytes());
        put_bytes(&mut bytes, inner);
        put_bytes(&mut bytes, destination.as_bytes());
        let staging = if destination.is_empty() {
            ""
        } else {
            r"D:\exports\.p11-fixture.partial"
        };
        put_bytes(&mut bytes, staging.as_bytes());
        bytes
    }
    fn encoded_export_request(
        image: &str,
        inner: &[u8],
        destination: &str,
        staging: &str,
    ) -> Vec<u8> {
        let mut bytes = b"P11IMG02".to_vec();
        bytes.push(2);
        put_bytes(&mut bytes, image.as_bytes());
        put_bytes(&mut bytes, inner);
        put_bytes(&mut bytes, destination.as_bytes());
        put_bytes(&mut bytes, staging.as_bytes());
        bytes
    }
    fn put_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
        output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        output.extend_from_slice(bytes);
    }

    #[cfg(windows)]
    struct TempExportDir(PathBuf);

    #[cfg(windows)]
    impl Drop for TempExportDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join(".p11-test.partial"));
            let _ = std::fs::remove_file(self.0.join("target.bin"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[cfg(windows)]
    fn temp_export_dir() -> TempExportDir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let path = std::env::temp_dir().join(format!(
            "p11-image-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("create isolated temp directory");
        TempExportDir(path)
    }

    #[cfg(windows)]
    fn independent_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace crates directory")
            .join("ext-reader/tests/fixtures/ext4-e2fsprogs-1.47.3.img")
    }

    #[cfg(windows)]
    #[test]
    fn refuses_device_and_network_namespaces_as_export_destinations() {
        for path in [
            r"\\.\PhysicalDrive0",
            r"\\server\share\out.img",
            r"C:\images\NUL.txt",
        ] {
            assert!(validate_local_file_path(std::path::Path::new(path)).is_err());
        }
    }
    #[test]
    fn decodes_a_bounded_list_request_with_raw_internal_path_bytes() {
        let encoded = encoded_request(1, r"D:\images\fixture.img", b"/dir/\xff", "");
        let request = decode_request(&encoded).expect("valid listing request");
        match request {
            Request::List {
                image_path,
                internal_path,
            } => {
                assert_eq!(image_path.to_string_lossy(), r"D:\images\fixture.img");
                assert_eq!(internal_path, b"/dir/\xff");
            }
            _ => panic!("wrong request kind"),
        }
    }

    #[test]
    fn export_protocol_requires_a_distinct_staging_sibling() {
        let encoded = encoded_export_request(
            r"D:\images\fixture.img",
            b"/hello.txt",
            r"D:\exports\hello.txt",
            r"D:\exports\.p11-123.partial",
        );
        let decoded = decode_request(&encoded).expect("valid export request");
        match decoded {
            Request::Export {
                destination_path,
                staging_path,
                ..
            } => {
                assert_eq!(destination_path, PathBuf::from(r"D:\exports\hello.txt"));
                assert_eq!(staging_path, PathBuf::from(r"D:\exports\.p11-123.partial"));
            }
            _ => panic!("wrong request kind"),
        }

        let different_directory = encoded_export_request(
            r"D:\images\fixture.img",
            b"/hello.txt",
            r"D:\exports\hello.txt",
            r"D:\temp\.p11-123.partial",
        );
        assert!(decode_request(&different_directory).is_err());

        let same_path = encoded_export_request(
            r"D:\images\fixture.img",
            b"/hello.txt",
            r"D:\exports\hello.txt",
            r"D:\exports\hello.txt",
        );
        assert!(decode_request(&same_path).is_err());
    }

    #[test]
    fn rejects_trailing_bytes_bad_magic_and_invalid_inner_paths() {
        let mut trailing = encoded_request(1, r"D:\images\fixture.img", b"/", "");
        trailing.push(0);
        assert!(decode_request(&trailing).is_err());

        let mut bad_magic = encoded_request(1, r"D:\images\fixture.img", b"/", "");
        bad_magic[0] = b'X';
        assert!(decode_request(&bad_magic).is_err());

        for path in [b"relative".as_slice(), b"/bad\0name", b""] {
            let invalid = encoded_request(1, r"D:\images\fixture.img", path, "");
            assert!(decode_request(&invalid).is_err());
        }
    }

    #[test]
    fn recovery_required_status_has_a_stable_wire_code() {
        use super::{encode_response, WorkerResponse, WorkerStatus};
        assert_eq!(
            encode_response(WorkerResponse::Error(WorkerStatus::NeedsRecovery)),
            b"P11IMR02\x08"
        );
    }

    #[test]
    fn rejects_oversized_lengths_and_mismatched_operations() {
        let too_long = encoded_request(1, &"x".repeat(32_769), b"/", "");
        assert!(decode_request(&too_long).is_err());

        let export_without_destination = encoded_request(2, r"D:\images\fixture.img", b"/file", "");
        assert!(decode_request(&export_without_destination).is_err());

        let list_with_destination =
            encoded_request(1, r"D:\images\fixture.img", b"/", r"D:\out.bin");
        assert!(decode_request(&list_with_destination).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn atomic_stage_commit_does_not_replace_an_existing_destination() {
        use super::win32_file_move::move_no_replace;
        let directory = temp_export_dir();
        let staging = directory.0.join(".p11-test.partial");
        let destination = directory.0.join("target.bin");
        std::fs::write(&staging, b"complete export").expect("write staged bytes");
        std::fs::write(&destination, b"preserve existing").expect("write existing destination");

        let result = move_no_replace(&staging, &destination);

        assert!(
            result.is_err(),
            "commit must fail when destination already exists"
        );
        assert_eq!(
            std::fs::read(&destination).expect("read destination"),
            b"preserve existing"
        );
        assert_eq!(
            std::fs::read(&staging).expect("read staged file"),
            b"complete export"
        );
    }

    #[cfg(windows)]
    #[test]
    fn exports_known_image_content_through_stage_before_committing_destination() {
        use super::{export_file, WorkerResponse};

        let directory = temp_export_dir();
        let staging = directory.0.join(".p11-test.partial");
        let destination = directory.0.join("target.bin");

        let response = export_file(
            &independent_fixture_path(),
            b"/known.txt",
            &destination,
            &staging,
        );

        match response {
            WorkerResponse::Exported(bytes) => assert_eq!(bytes, 36),
            _ => panic!("expected successful staged export"),
        }
        assert_eq!(
            std::fs::read(&destination).expect("read committed output"),
            b"e2fsprogs 1.47.3 independent oracle\n"
        );
        assert!(
            !staging.exists(),
            "committed stage is moved to the destination"
        );
    }

    #[cfg(windows)]
    #[test]
    fn export_collision_preserves_both_foreign_stage_and_existing_destination() {
        use super::{export_file, WorkerResponse, WorkerStatus};

        let directory = temp_export_dir();
        let staging = directory.0.join(".p11-test.partial");
        let destination = directory.0.join("target.bin");
        std::fs::write(&staging, b"foreign stage").expect("create colliding stage file");
        std::fs::write(&destination, b"existing destination").expect("create existing destination");

        let response = export_file(
            &independent_fixture_path(),
            b"/known.txt",
            &destination,
            &staging,
        );

        assert!(matches!(
            response,
            WorkerResponse::Error(WorkerStatus::OutputExists)
        ));
        assert_eq!(
            std::fs::read(&staging).expect("read foreign stage"),
            b"foreign stage"
        );
        assert_eq!(
            std::fs::read(&destination).expect("read existing destination"),
            b"existing destination"
        );
    }
}
