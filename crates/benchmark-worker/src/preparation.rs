//! Bounded initialization of the exact, journaled benchmark scratch lease.

use crate::ScratchLease;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const INITIALIZATION_BYTES: u64 = 1024 * 1024 * 1024;
const INITIALIZATION_CHUNK_BYTES: usize = 1024 * 1024;
const INITIALIZATION_RATE_BYTES_PER_SECOND: u64 = 100_000_000;
const INITIALIZATION_CHECKPOINT_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_PREPARATION_TIME: Duration = Duration::from_secs(30);
const MAX_PACING_SLEEP: Duration = Duration::from_millis(25);

pub(crate) fn prepare_scratch(
    lease: &mut ScratchLease,
    cancel: &AtomicBool,
    deadline: Instant,
) -> io::Result<()> {
    prepare_scratch_with_size(lease, cancel, deadline, INITIALIZATION_BYTES)
}

pub(super) fn prepare_scratch_with_size(
    lease: &mut ScratchLease,
    cancel: &AtomicBool,
    deadline: Instant,
    bytes_to_write: u64,
) -> io::Result<()> {
    prepare_with_writer(lease, cancel, deadline, bytes_to_write, |lease, buffer| {
        lease.write_initialization_chunk(buffer)
    })
}

fn prepare_with_writer(
    lease: &mut ScratchLease,
    cancel: &AtomicBool,
    deadline: Instant,
    bytes_to_write: u64,
    write_chunk: impl FnMut(&mut ScratchLease, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    prepare_with_writer_at(
        lease,
        cancel,
        deadline,
        bytes_to_write,
        write_chunk,
        Instant::now,
    )
}

fn prepare_with_writer_at(
    lease: &mut ScratchLease,
    cancel: &AtomicBool,
    deadline: Instant,
    bytes_to_write: u64,
    mut write_chunk: impl FnMut(&mut ScratchLease, &[u8]) -> io::Result<()>,
    mut now: impl FnMut() -> Instant,
) -> io::Result<()> {
    lease.retain_for_recovery();
    if bytes_to_write != lease.size_bytes() || bytes_to_write == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scratch initialization size does not match its lease",
        ));
    }
    check_preparation_state_at(cancel, deadline, now())?;
    lease.begin_initialization()?;

    let started = now();
    let mut buffer = vec![0_u8; INITIALIZATION_CHUNK_BYTES];
    let mut written = 0_u64;
    while written < bytes_to_write {
        check_preparation_state_at(cancel, deadline, now())?;
        let chunk_bytes = (bytes_to_write - written).min(buffer.len() as u64) as usize;
        let chunk = &mut buffer[..chunk_bytes];
        ScratchLease::fill_initialization_buffer(chunk)?;
        write_chunk(lease, chunk)?;
        written = written
            .checked_add(chunk_bytes as u64)
            .ok_or_else(|| io::Error::other("scratch initialization byte count overflowed"))?;

        // A synchronous write that returns after its deadline is known to have
        // stopped. Do not start another write or launch DiskSpd after that.
        check_preparation_state_at(cancel, deadline, now())?;
        if written % INITIALIZATION_CHECKPOINT_BYTES == 0 || written == bytes_to_write {
            lease.verify()?;
        }
        wait_until_rate_allows(written, started, cancel, deadline, &mut now)?;
    }

    if written != lease.size_bytes() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "scratch initialization did not cover the exact leased file",
        ));
    }
    check_preparation_state_at(cancel, deadline, now())?;
    lease.finish_initialization()
}

fn check_preparation_state_at(
    cancel: &AtomicBool,
    deadline: Instant,
    now: Instant,
) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "scratch initialization was cancelled",
        ));
    }
    if now >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "scratch initialization exceeded its deadline",
        ));
    }
    Ok(())
}

fn wait_until_rate_allows(
    written: u64,
    started: Instant,
    cancel: &AtomicBool,
    deadline: Instant,
    now: &mut impl FnMut() -> Instant,
) -> io::Result<()> {
    let elapsed_nanos = (u128::from(written) * 1_000_000_000_u128)
        / u128::from(INITIALIZATION_RATE_BYTES_PER_SECOND);
    let required = Duration::from_nanos(
        u64::try_from(elapsed_nanos)
            .map_err(|_| io::Error::other("scratch initialization throttle overflowed"))?,
    );
    loop {
        check_preparation_state_at(cancel, deadline, now())?;
        let elapsed = now().saturating_duration_since(started);
        let remaining = required.saturating_sub(elapsed);
        if remaining.is_zero() {
            return Ok(());
        }
        thread::sleep(remaining.min(MAX_PACING_SLEEP));
    }
}

#[cfg(test)]
mod tests {
    use crate::scratch::ScratchLease;
    use std::fs::{self, File};
    use std::io::Read;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn fixture() -> (std::path::PathBuf, ScratchLease) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "p11-prepare-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let lease = ScratchLease::create_for_test(&root, 4 * 1024 * 1024).unwrap();
        (root, lease)
    }

    #[test]
    fn initializes_and_flushes_the_entire_owned_file_in_bounded_chunks() {
        let (root, mut lease) = fixture();
        let size = lease.size_bytes();
        let mut written_chunks = Vec::new();
        let result = super::prepare_with_writer(
            &mut lease,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5),
            size,
            |lease, chunk| {
                written_chunks.push(chunk.len());
                lease.write_initialization_chunk(chunk)
            },
        );
        assert!(result.is_ok(), "preparation result: {result:?}");
        assert_eq!(written_chunks, vec![super::INITIALIZATION_CHUNK_BYTES; 4]);
        let metadata = fs::metadata(lease.file_path()).unwrap();
        assert_eq!(metadata.len(), size);
        let mut file = File::open(lease.file_path()).unwrap();
        let mut sample = [0_u8; 4096];
        file.read_exact(&mut sample).unwrap();
        assert!(sample.iter().any(|byte| *byte != 0));
        drop(file);
        lease.cleanup().unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn cancellation_and_deadline_prevent_preparation_and_leave_explicit_cleanup() {
        let (root, mut lease) = fixture();
        let path = lease.file_path().to_path_buf();
        let size = lease.size_bytes();
        let cancelled = AtomicBool::new(true);
        let error = super::prepare_scratch_with_size(
            &mut lease,
            &cancelled,
            Instant::now() + Duration::from_secs(1),
            size,
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        assert!(path.exists(), "implicit Drop cleanup must stay disabled");
        lease.cleanup().unwrap();
        fs::remove_dir(root).unwrap();

        let (root, mut lease) = fixture();
        let size = lease.size_bytes();
        let error = super::prepare_scratch_with_size(
            &mut lease,
            &AtomicBool::new(false),
            Instant::now() - Duration::from_millis(1),
            size,
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        lease.cleanup().unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn a_write_that_returns_after_the_deadline_is_not_treated_as_prepared() {
        let (root, mut lease) = fixture();
        let size = lease.size_bytes();
        let before_deadline = Instant::now();
        let delayed_deadline = before_deadline + Duration::from_secs(1);
        let write_returned = std::sync::atomic::AtomicBool::new(false);
        let mut write_calls = 0;
        let error = super::prepare_with_writer_at(
            &mut lease,
            &AtomicBool::new(false),
            delayed_deadline,
            size,
            |lease, chunk| {
                write_calls += 1;
                lease.write_initialization_chunk(chunk)?;
                write_returned.store(true, std::sync::atomic::Ordering::Release);
                Ok(())
            },
            || {
                if write_returned.load(std::sync::atomic::Ordering::Acquire) {
                    delayed_deadline
                } else {
                    before_deadline
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(write_calls, 1, "late I/O must stop before a second chunk");
        lease.cleanup().unwrap();
        fs::remove_dir(root).unwrap();
    }
}
