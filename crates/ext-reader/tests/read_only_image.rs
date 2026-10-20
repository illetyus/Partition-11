use partition11_ext_reader::{ExtEntryType, ExtImageError, ExtImageReader};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const KNOWN_CONTENT: &[u8] = b"Partition 11 ext4 read-only fixture v1";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_temp(contents: &[u8]) -> TempFile {
    for _ in 0..32 {
        let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "partition11-ext-reader-{}-{suffix}.img",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(contents)
                    .expect("write isolated image fixture");
                return TempFile(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("create isolated image fixture: {error}"),
        }
    }
    panic!("could not allocate a unique isolated image fixture");
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ext4-readonly-small.img")
}

#[test]
fn lists_and_streams_a_file_from_an_ext4_image_without_modifying_it() {
    let before = fs::read(fixture_path()).expect("read fixture before test");
    let reader = ExtImageReader::open(fixture_path().as_path()).expect("mount fixture read-only");

    let entries = reader.list_directory(b"/").expect("list ext4 root");
    let known = entries
        .iter()
        .find(|entry| entry.name == b"known.txt")
        .expect("fixture file is listed");
    assert_eq!(known.kind, ExtEntryType::File);
    assert_eq!(known.size_bytes, KNOWN_CONTENT.len() as u64);

    let mut output = Vec::new();
    let copied = reader
        .copy_file_to(b"/known.txt", KNOWN_CONTENT.len() as u64, &mut output)
        .expect("copy known file to caller-provided writer");
    assert_eq!(copied, KNOWN_CONTENT.len() as u64);
    assert_eq!(output, KNOWN_CONTENT);

    drop(reader);
    assert_eq!(
        fs::read(fixture_path()).expect("read fixture after test"),
        before
    );
}

#[test]
fn export_limit_is_checked_before_any_output_is_written() {
    let reader = ExtImageReader::open(fixture_path().as_path()).expect("mount fixture read-only");
    let mut output = Vec::new();
    assert!(matches!(
        reader.copy_file_to(b"/known.txt", 8, &mut output),
        Err(ExtImageError::FileTooLarge { .. })
    ));
    assert!(output.is_empty());
}

#[test]
fn directory_and_file_operations_reject_the_wrong_inode_kind() {
    let reader = ExtImageReader::open(fixture_path().as_path()).expect("mount fixture read-only");
    assert!(matches!(
        reader.list_directory(b"/known.txt"),
        Err(ExtImageError::NotDirectory)
    ));
    let mut output = Vec::new();
    assert!(matches!(
        reader.copy_file_to(b"/", 1024, &mut output),
        Err(ExtImageError::NotRegularFile)
    ));
    assert!(output.is_empty());
}

#[cfg(windows)]
#[test]
fn mounted_image_denies_a_concurrent_host_writer() {
    let image = fixture_path();
    let reader = ExtImageReader::open(image.as_path()).expect("mount fixture read-only");
    let writer = OpenOptions::new().write(true).open(&image);
    assert!(
        writer.is_err(),
        "a mounted image must deny concurrent writes"
    );
    drop(reader);
}
#[test]
fn rejects_relative_host_image_paths_before_file_access() {
    assert!(matches!(
        ExtImageReader::open(Path::new("tests/fixtures/ext4-readonly-small.img")),
        Err(ExtImageError::InvalidImagePath)
    ));
}
#[test]
fn rejects_too_small_and_non_ext_images() {
    let too_small = write_temp(&[0; 2047]);
    assert!(matches!(
        ExtImageReader::open(&too_small.0),
        Err(ExtImageError::ImageTooSmall)
    ));

    let not_ext = write_temp(&[0; 4096]);
    assert!(matches!(
        ExtImageReader::open(&not_ext.0),
        Err(ExtImageError::Filesystem(_))
    ));
}

#[test]
fn rejects_relative_or_oversized_image_paths_inside_the_image() {
    let reader = ExtImageReader::open(fixture_path().as_path()).expect("mount fixture read-only");
    assert!(matches!(
        reader.list_directory(b"known.txt"),
        Err(ExtImageError::InvalidImagePath)
    ));
    let long_path = vec![b'a'; 4097];
    assert!(matches!(
        reader.copy_file_to(&long_path, 1024, &mut Vec::new()),
        Err(ExtImageError::InvalidImagePath)
    ));
}
