use crate::ScratchLease;
use std::ffi::{c_void, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const MAX_WARMUP_SECONDS: u32 = 15;
const MIN_MEASUREMENT_SECONDS: u32 = 2;
const MAX_MEASUREMENT_SECONDS: u32 = 60;
const WRITE_RATE_BYTES_PER_MS: u32 = 100_000;
const SCRATCH_PREFIX: &str = ".partition11-scratch-";
const SCRATCH_SUFFIX: &str = ".bin";

const MAX_DISKSPD_BINARY_BYTES: u64 = 64 * 1024 * 1024;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const BCRYPT_SHA256_ALGORITHM: [u16; 7] = [83, 72, 65, 50, 53, 54, 0];
const PINNED_DISKSPD_SHA256: [u8; 32] = [
    0xdd, 0x4e, 0x57, 0xe1, 0xe8, 0xcc, 0xaf, 0x5d, 0x64, 0x37, 0x93, 0x8f, 0x8a, 0xab, 0x7f, 0x17,
    0xe9, 0xa1, 0xe6, 0xd8, 0xfb, 0xa8, 0xa0, 0x93, 0x00, 0x6b, 0x7c, 0xad, 0xf1, 0x6f, 0xae, 0xa2,
];

type AlgorithmHandle = *mut c_void;

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

/// A DiskSpd executable whose content hash was verified and whose open handle
/// prevents replacement while a workload is running.
pub struct VerifiedDiskSpd {
    path: PathBuf,
    _file: File,
}

impl VerifiedDiskSpd {
    pub fn verify(path: &Path) -> io::Result<Self> {
        Self::verify_with_sha256(path, &PINNED_DISKSPD_SHA256)
    }

    fn verify_with_sha256(path: &Path, expected_sha256: &[u8; 32]) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
            || path
                .file_name()
                .is_none_or(|name| !name.to_string_lossy().eq_ignore_ascii_case("diskspd.exe"))
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(invalid_binary());
        }

        let path = std::fs::canonicalize(path)?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        let mut file = options.open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || metadata.len() == 0
            || metadata.len() > MAX_DISKSPD_BINARY_BYTES
        {
            return Err(invalid_binary());
        }
        let file_size = metadata.len();
        let mut contents = Vec::with_capacity(file_size as usize);
        file.read_to_end(&mut contents)?;
        if contents.len() as u64 != file_size || sha256(&contents)? != *expected_sha256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "DiskSpd binary integrity verification failed",
            ));
        }
        Ok(Self { path, _file: file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn invalid_binary() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid DiskSpd executable")
}

fn sha256(contents: &[u8]) -> io::Result<[u8; 32]> {
    let input_size = u32::try_from(contents.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "DiskSpd binary is too large"))?;
    let mut algorithm = std::ptr::null_mut();
    let opened = unsafe {
        BCryptOpenAlgorithmProvider(
            &mut algorithm,
            BCRYPT_SHA256_ALGORITHM.as_ptr(),
            std::ptr::null(),
            0,
        )
    };
    if opened < 0 || algorithm.is_null() {
        return Err(io::Error::other("Windows SHA-256 provider unavailable"));
    }
    let mut digest = [0_u8; 32];
    let hashed = unsafe {
        BCryptHash(
            algorithm,
            std::ptr::null_mut(),
            0,
            contents.as_ptr() as *mut u8,
            input_size,
            digest.as_mut_ptr(),
            digest.len() as u32,
        )
    };
    let closed = unsafe { BCryptCloseAlgorithmProvider(algorithm, 0) };
    if hashed < 0 || closed < 0 {
        return Err(io::Error::other("Windows SHA-256 verification failed"));
    }
    Ok(digest)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiskSpdProfile {
    Seq1mQ8T1,
    Seq1mQ1T1,
    Rnd4kQ32T1,
    Rnd4kQ1T1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiskSpdDirection {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyError {
    InvalidTarget,
    InvalidWorkload,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTarget => {
                formatter.write_str("benchmark target is not an owned scratch file")
            }
            Self::InvalidWorkload => {
                formatter.write_str("benchmark workload exceeds fixed policy limits")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

/// Builds only fixed DiskSpd arguments for a currently verified ScratchLease.
///
/// The renderer never supplies paths, switches, block sizes, queue depths or rates. Raw
/// devices, drive letters, UNC paths and arbitrary files cannot pass through this API.
pub fn build_diskspd_args(
    lease: &ScratchLease,
    profile: DiskSpdProfile,
    direction: DiskSpdDirection,
    warmup_seconds: u32,
    measurement_seconds: u32,
) -> Result<Vec<OsString>, PolicyError> {
    lease.verify().map_err(|_| PolicyError::InvalidTarget)?;
    build_args(
        lease.file_path(),
        profile,
        direction,
        warmup_seconds,
        measurement_seconds,
    )
}

fn build_args(
    target: &Path,
    profile: DiskSpdProfile,
    direction: DiskSpdDirection,
    warmup_seconds: u32,
    measurement_seconds: u32,
) -> Result<Vec<OsString>, PolicyError> {
    if warmup_seconds > MAX_WARMUP_SECONDS
        || !(MIN_MEASUREMENT_SECONDS..=MAX_MEASUREMENT_SECONDS).contains(&measurement_seconds)
    {
        return Err(PolicyError::InvalidWorkload);
    }
    if !is_owned_scratch_path(target) {
        return Err(PolicyError::InvalidTarget);
    }

    let (block_size, queue_depth, random) = match profile {
        DiskSpdProfile::Seq1mQ8T1 => ("1M", 8, false),
        DiskSpdProfile::Seq1mQ1T1 => ("1M", 1, false),
        DiskSpdProfile::Rnd4kQ32T1 => ("4K", 32, true),
        DiskSpdProfile::Rnd4kQ1T1 => ("4K", 1, true),
    };
    let write = direction == DiskSpdDirection::Write;
    let mut args = vec![
        OsString::from(format!("-b{block_size}")),
        OsString::from(format!("-o{queue_depth}")),
        OsString::from("-t1"),
        OsString::from(if write { "-w100" } else { "-w0" }),
        OsString::from(format!("-W{warmup_seconds}")),
        OsString::from(format!("-d{measurement_seconds}")),
    ];
    if write {
        args.push(OsString::from(format!("-g{WRITE_RATE_BYTES_PER_MS}")));
    }
    args.push(OsString::from(if write { "-Suw" } else { "-Su" }));
    args.push(OsString::from("-L"));
    args.push(OsString::from("-Rxml"));
    if random {
        args.push(OsString::from("-r4K"));
    }
    args.push(target.as_os_str().to_os_string());
    Ok(args)
}

fn is_owned_scratch_path(target: &Path) -> bool {
    let Some(value) = target.to_str() else {
        return false;
    };
    let bytes = value.as_bytes();
    let prefix = r"\\?\Volume{".as_bytes();
    let Some(rest) = bytes.strip_prefix(prefix) else {
        return false;
    };
    if rest.len() < 38 || &rest[36..38] != b"}\\" || !valid_guid(&rest[..36]) {
        return false;
    }
    let Ok(relative) = std::str::from_utf8(&rest[38..]) else {
        return false;
    };
    let components: Vec<&str> = relative.split('\\').collect();
    if components.is_empty()
        || components.iter().any(|component| {
            component.is_empty()
                || matches!(*component, "." | "..")
                || component.chars().any(|character| {
                    matches!(character, ':' | '/' | '*' | '?' | '"' | '<' | '>' | '|')
                })
        })
    {
        return false;
    }
    let Some(file_name) = components.last() else {
        return false;
    };
    let Some(name) = file_name.strip_prefix(SCRATCH_PREFIX) else {
        return false;
    };
    let Some(random) = name.strip_suffix(SCRATCH_SUFFIX) else {
        return false;
    };
    random.len() == 32
        && random
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_guid(value: &[u8]) -> bool {
    value.len() == 36
        && value.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{build_args, DiskSpdDirection, DiskSpdProfile, PolicyError, VerifiedDiskSpd};
    use crate::scratch::ScratchLease;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TARGET: &str = r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\.partition11-scratch-00112233445566778899aabbccddeeff.bin";

    fn strings(args: Vec<std::ffi::OsString>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    #[test]
    fn builds_fixed_read_arguments_for_an_owned_file_target() {
        assert_eq!(
            strings(
                build_args(
                    Path::new(TARGET),
                    DiskSpdProfile::Seq1mQ8T1,
                    DiskSpdDirection::Read,
                    5,
                    15,
                )
                .unwrap()
            ),
            vec!["-b1M", "-o8", "-t1", "-w0", "-W5", "-d15", "-Su", "-L", "-Rxml", TARGET,]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn writes_are_rate_capped_and_write_through_with_no_file_create_switch() {
        let args = strings(
            build_args(
                Path::new(TARGET),
                DiskSpdProfile::Rnd4kQ32T1,
                DiskSpdDirection::Write,
                0,
                2,
            )
            .unwrap(),
        );
        assert_eq!(args.last().map(String::as_str), Some(TARGET));
        assert!(args.iter().any(|arg| arg == "-g100000"));
        assert!(args.iter().any(|arg| arg == "-Suw"));
        assert!(args.iter().any(|arg| arg == "-r4K"));
        assert!(!args.iter().any(|arg| arg.starts_with("-c")));
    }

    #[test]
    fn rejects_raw_device_drive_unc_and_non_owned_file_targets() {
        for target in [
            r"\\.\PhysicalDrive0",
            r"E:\",
            r"E:\.partition11-scratch-00112233445566778899aabbccddeeff.bin",
            r"\\server\share\.partition11-scratch-00112233445566778899aabbccddeeff.bin",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\.partition11-scratch-../payload.bin",
            r"\\?\Volume{not-a-guid}\.partition11-scratch-00112233445566778899aabbccddeeff.bin",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdeé}\.partition11-scratch-00112233445566778899aabbccddeeff.bin",
        ] {
            assert!(
                matches!(
                    build_args(
                        Path::new(target),
                        DiskSpdProfile::Seq1mQ1T1,
                        DiskSpdDirection::Read,
                        5,
                        15,
                    ),
                    Err(PolicyError::InvalidTarget)
                ),
                "accepted unsafe target: {target}"
            );
        }
    }

    #[test]
    fn rejects_workload_values_outside_fixed_limits() {
        for (warmup, duration) in [(16, 15), (0, 1), (0, 61)] {
            assert!(matches!(
                build_args(
                    Path::new(TARGET),
                    DiskSpdProfile::Seq1mQ1T1,
                    DiskSpdDirection::Read,
                    warmup,
                    duration,
                ),
                Err(PolicyError::InvalidWorkload)
            ));
        }
    }

    struct TempRoot(std::path::PathBuf);

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn verifies_the_application_pinned_executable_without_caller_supplied_hash() {
        let path = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../third_party/diskspd/amd64/diskspd.exe"),
        )
        .unwrap();
        let verified = VerifiedDiskSpd::verify(&path).unwrap();
        assert_eq!(verified.path(), path);
    }

    #[test]
    fn verifies_the_pinned_binary_hash_and_blocks_replacement_while_open() {
        const SHA256_ABC: [u8; 32] = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("partition11-engine-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let _root = TempRoot(root.clone());
        let path = root.join("diskspd.exe");
        let moved = root.join("diskspd-moved.exe");
        fs::write(&path, b"abc").unwrap();

        assert!(VerifiedDiskSpd::verify_with_sha256(&path, &[0; 32]).is_err());
        let verified = VerifiedDiskSpd::verify_with_sha256(&path, &SHA256_ABC).unwrap();
        assert_eq!(verified.path(), path);
        assert!(fs::rename(&path, &moved).is_err());
        drop(verified);
        fs::rename(&path, &moved).unwrap();
    }
    #[test]
    fn public_builder_uses_a_verified_scratch_lease_target() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("partition11-policy-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let _root = TempRoot(root.clone());
        let lease = ScratchLease::create_for_test(&root, 4 * 1024 * 1024).unwrap();
        let args = super::build_diskspd_args(
            &lease,
            DiskSpdProfile::Seq1mQ1T1,
            DiskSpdDirection::Read,
            0,
            2,
        )
        .unwrap();
        assert_eq!(
            args.last().unwrap().as_os_str(),
            lease.file_path().as_os_str()
        );
    }
    #[test]
    fn all_four_profiles_map_to_fixed_block_and_queue_depths() {
        let expected = [
            (DiskSpdProfile::Seq1mQ8T1, "-b1M", "-o8"),
            (DiskSpdProfile::Seq1mQ1T1, "-b1M", "-o1"),
            (DiskSpdProfile::Rnd4kQ32T1, "-b4K", "-o32"),
            (DiskSpdProfile::Rnd4kQ1T1, "-b4K", "-o1"),
        ];
        for (profile, block_size, queue_depth) in expected {
            let args = strings(
                build_args(Path::new(TARGET), profile, DiskSpdDirection::Read, 0, 2).unwrap(),
            );
            assert_eq!(args[0], block_size);
            assert_eq!(args[1], queue_depth);
        }
    }
}
