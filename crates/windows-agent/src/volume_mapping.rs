//! Bounded, read-only mapping from drive-letter volumes to Windows partitions.
//!
//! Opening a volume device for IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS requires
//! an administrator token, which this application explicitly rejects. The
//! local Win32_LogicalDiskToPartition association provides the same mapping
//! metadata without opening a volume handle. Its result is treated as
//! untrusted and must match this scan's independently queried disk layout.

use crate::volume_eligibility::VolumeExtent;

const MAX_MAPPINGS: usize = 4096;
const MAX_OUTPUT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DrivePartitionMapping {
    pub drive_root: String,
    pub extent: VolumeExtent,
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MappingError {
    Unavailable,
    Malformed,
    TimedOut,
    Cancelled,
}

pub(crate) fn parse_mappings(bytes: &[u8]) -> Result<Vec<DrivePartitionMapping>, ()> {
    if bytes.len() > MAX_OUTPUT_BYTES || bytes.contains(&0) {
        return Err(());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if !text.is_ascii() {
        return Err(());
    }
    let mut mappings = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if mappings.len() == MAX_MAPPINGS {
            return Err(());
        }
        let mut fields = line.split('|');
        let drive = fields.next().ok_or(())?;
        let disk_number = fields.next().ok_or(())?.parse::<u32>().map_err(|_| ())?;
        let starting_offset = fields.next().ok_or(())?.parse::<u64>().map_err(|_| ())?;
        let length = fields.next().ok_or(())?.parse::<u64>().map_err(|_| ())?;
        if fields.next().is_some()
            || drive.len() != 2
            || !drive.as_bytes()[0].is_ascii_alphabetic()
            || drive.as_bytes()[1] != b':'
            || length == 0
            || starting_offset.checked_add(length).is_none()
        {
            return Err(());
        }
        let letter = drive.as_bytes()[0].to_ascii_uppercase() as char;
        mappings.push(DrivePartitionMapping {
            drive_root: format!("{letter}:\\"),
            extent: VolumeExtent {
                disk_number,
                starting_offset,
                length,
            },
        });
    }
    mappings.sort_unstable_by(|left, right| {
        (
            &left.drive_root,
            left.extent.disk_number,
            left.extent.starting_offset,
            left.extent.length,
        )
            .cmp(&(
                &right.drive_root,
                right.extent.disk_number,
                right.extent.starting_offset,
                right.extent.length,
            ))
    });
    Ok(mappings)
}

#[cfg(windows)]
mod windows {
    use super::{parse_mappings, DrivePartitionMapping, MappingError, MAX_OUTPUT_BYTES};
    use crate::scan_identity::ScanGeneration;
    use std::ffi::OsString;
    use std::io::Read;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    const QUERY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$culture = [Globalization.CultureInfo]::InvariantCulture
try {
    $drives = @(Get-CimInstance -Namespace 'root/cimv2' -ClassName 'Win32_LogicalDisk' -OperationTimeoutSec 3 -ErrorAction Stop)
    $count = 0
    foreach ($drive in $drives) {
        $driveId = [string]$drive.DeviceID
        if ($driveId -notmatch '^[A-Za-z]:$') { continue }
        $driveType = [uint32]$drive.DriveType
        if ($driveType -ne 2 -and $driveType -ne 3) { continue }
        $partitions = @(Get-CimAssociatedInstance -InputObject $drive -Association 'Win32_LogicalDiskToPartition' -Namespace 'root/cimv2' -OperationTimeoutSec 3 -ErrorAction Stop)
        foreach ($partition in $partitions) {
            if ($null -eq $partition.DiskIndex -or $null -eq $partition.StartingOffset -or $null -eq $partition.Size) { throw 'incomplete partition mapping' }
            $diskIndex = [uint32]$partition.DiskIndex
            $startingOffset = [uint64]$partition.StartingOffset
            $length = [uint64]$partition.Size
            if ($length -eq 0) { throw 'empty partition mapping' }
            $count++
            if ($count -gt 4096) { throw 'mapping limit exceeded' }
            $line = $driveId.ToUpperInvariant() + '|' + $diskIndex.ToString($culture) + '|' + $startingOffset.ToString($culture) + '|' + $length.ToString($culture)
            [Console]::Out.WriteLine($line)
        }
    }
    exit 0
}
catch {
    exit 20
}
"#;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemDirectoryW(path: *mut u16, size: u32) -> u32;
    }

    fn powershell_path() -> Result<PathBuf, MappingError> {
        let mut buffer = vec![0_u16; 512];
        let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 || length as usize >= buffer.len() {
            return Err(MappingError::Unavailable);
        }
        let mut path = PathBuf::from(OsString::from_wide(&buffer[..length as usize]));
        path.push("WindowsPowerShell");
        path.push("v1.0");
        path.push("powershell.exe");
        if path.is_file() {
            Ok(path)
        } else {
            Err(MappingError::Unavailable)
        }
    }

    fn read_bounded(mut pipe: impl Read) -> Result<Vec<u8>, ()> {
        let mut output = Vec::new();
        let mut chunk = [0_u8; 4096];
        let mut oversized = false;
        loop {
            let count = pipe.read(&mut chunk).map_err(|_| ())?;
            if count == 0 {
                break;
            }
            if output.len().saturating_add(count) > MAX_OUTPUT_BYTES {
                oversized = true;
            } else if !oversized {
                output.extend_from_slice(&chunk[..count]);
            }
        }
        if oversized {
            Err(())
        } else {
            Ok(output)
        }
    }

    fn terminate(child: &mut Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    pub(crate) fn query(
        generation: &ScanGeneration,
        revision: u64,
        scan_deadline: Instant,
    ) -> Result<Vec<DrivePartitionMapping>, MappingError> {
        let executable = powershell_path()?;
        let mut child = Command::new(executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                QUERY_SCRIPT,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| MappingError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(MappingError::Unavailable)?;
        let reader = thread::spawn(move || read_bounded(stdout));
        let process_deadline = Instant::now() + QUERY_TIMEOUT;
        let deadline = process_deadline.min(scan_deadline);
        let status = loop {
            if generation.revision() != revision {
                terminate(&mut child);
                let _ = reader.join();
                return Err(MappingError::Cancelled);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(_) => {
                    terminate(&mut child);
                    let _ = reader.join();
                    return Err(MappingError::Unavailable);
                }
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                let _ = reader.join();
                return Err(MappingError::TimedOut);
            }
            thread::sleep(POLL_INTERVAL);
        };
        let bytes = reader
            .join()
            .map_err(|_| MappingError::Unavailable)?
            .map_err(|_| MappingError::Malformed)?;
        if !status.success() {
            return Err(MappingError::Unavailable);
        }
        if generation.revision() != revision {
            return Err(MappingError::Cancelled);
        }
        if Instant::now() >= scan_deadline {
            return Err(MappingError::TimedOut);
        }
        parse_mappings(&bytes).map_err(|_| MappingError::Malformed)
    }
}

#[cfg(windows)]
pub(crate) use windows::query;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_drive_mappings_and_sorts_them() {
        let parsed = parse_mappings(b"e:|2|4096|8192\r\nC:|0|1048576|4096\r\n").unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].drive_root, "C:\\");
        assert_eq!(parsed[0].extent.disk_number, 0);
        assert_eq!(parsed[1].drive_root, "E:\\");
    }

    #[test]
    fn rejects_malformed_or_overflowing_mapping_rows() {
        assert!(parse_mappings(b"C:|1|4096|8192|extra\n").is_err());
        assert!(parse_mappings(b"C:|1|18446744073709551615|1\n").is_err());
        assert!(parse_mappings(b"\\\\server|1|0|4096\n").is_err());
        assert!(parse_mappings(b"C:|1|0|0\n").is_err());
        assert!(parse_mappings(b"C:|x|0|4096\n").is_err());
    }

    #[test]
    fn rejects_non_ascii_nul_and_oversized_output() {
        assert!(parse_mappings(b"C:|0|0|4096\0").is_err());
        assert!(parse_mappings("Ç:|0|0|4096\n".as_bytes()).is_err());
        assert!(parse_mappings(&vec![b'a'; MAX_OUTPUT_BYTES + 1]).is_err());
    }
}
