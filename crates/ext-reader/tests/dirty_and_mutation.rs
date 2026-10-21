use partition11_ext_reader::ExtImageReader;
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempImage(PathBuf);

impl TempImage {
    fn copy_from(path: &Path) -> Self {
        for _ in 0..32 {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let target = std::env::temp_dir().join(format!(
                "partition11-ext-mutation-{}-{suffix}.img",
                std::process::id()
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
            {
                Ok(mut output) => {
                    let contents = fs::read(path).expect("read mutation source fixture");
                    output
                        .write_all(&contents)
                        .expect("copy mutation source fixture");
                    return Self(target);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create isolated mutation image: {error}"),
            }
        }
        panic!("could not allocate unique mutation image");
    }
}

impl Drop for TempImage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_byte(path: &Path, offset: u64, byte: u8) {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open isolated mutation image");
    file.seek(SeekFrom::Start(offset))
        .expect("seek within isolated mutation image");
    file.write_all(&[byte]).expect("mutate one byte");
    file.sync_data().expect("flush one-byte mutation");
}

#[test]
fn rejects_unclean_ext4_and_recovery_required_images_without_modifying_them() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ext4-e2fsprogs-1.47.3-not-clean.img");
    let before = fs::read(&path).expect("read e2fsprogs unclean fixture");

    let result = ExtImageReader::open(&path);
    assert!(result.is_err(), "unclean ext4 must fail closed");
    assert_eq!(fs::read(&path).expect("read fixture after open"), before);
}

#[test]
fn rejects_bad_superblock_magic_without_modifying_the_corrupt_copy() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ext4-e2fsprogs-1.47.3.img");
    let mut corrupt = fs::read(&fixture).expect("read clean oracle fixture");
    corrupt[1024 + 56] = 0;
    corrupt[1024 + 57] = 0;
    let temp = TempImage::copy_from(&fixture);
    fs::write(&temp.0, &corrupt).expect("write bad-magic copy");

    assert!(ExtImageReader::open(&temp.0).is_err());
    assert_eq!(fs::read(&temp.0).expect("read corrupt copy"), corrupt);
}

#[test]
fn deterministic_metadata_mutation_sweep_never_panics_or_changes_the_input() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ext4-e2fsprogs-1.47.3.img");
    let baseline = fs::read(&fixture).expect("read clean oracle fixture");
    let temp = TempImage::copy_from(&fixture);
    let bands = [
        (1024_u64, 2048_u64),
        (4096, 8192),
        (8192, 128 * 1024),
        (128 * 1024, 4 * 1024 * 1024),
        (4 * 1024 * 1024, 8 * 1024 * 1024),
    ];
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;

    for case in 0..128 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let (start, end) = bands[case % bands.len()];
        let offset = start + (state % (end - start));
        let original = baseline[offset as usize];
        let mutated = original ^ (1_u8 << ((state >> 32) & 7));
        write_byte(&temp.0, offset, mutated);

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            if let Ok(reader) = ExtImageReader::open(&temp.0) {
                let _ = reader.list_directory(b"/");
                let mut output = Vec::new();
                let _ = reader.copy_file_to(b"/known.txt", 4096, &mut output);
            }
        }));
        assert!(
            result.is_ok(),
            "reader panicked on mutation case {case} at {offset}"
        );

        write_byte(&temp.0, offset, original);
    }

    assert_eq!(
        fs::read(&temp.0).expect("read mutation image after sweep"),
        baseline
    );
}
