use std::ffi::{c_void, OsStr, OsString};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

type RawHandle = *mut c_void;

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
const PROC_THREAD_ATTRIBUTE_JOB_LIST: usize = 0x0002_000D;
const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
const ERROR_BROKEN_PIPE: u32 = 109;
const MAX_PIPE_READ: usize = 16 * 1024;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
const WAIT_FAILED: u32 = u32::MAX;
const JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS: u32 = 1;
const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: u32 = 0x0000_0008;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const MAX_ACTIVE_PROCESSES: u32 = 4;
const MAX_COMMAND_LINE_CHARS: usize = 32_767;
const MAX_ARGUMENTS: usize = 64;
const MAX_WARMUP_SECONDS: u32 = 15;
const MIN_MEASUREMENT_SECONDS: u32 = 2;
const MAX_MEASUREMENT_SECONDS: u32 = 60;
const TERMINATION_GRACE: Duration = Duration::from_secs(15);
const MAX_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_STDERR_BYTES: usize = 16 * 1024;
const TERMINATE_EXIT_CODE: u32 = 0xE000_0011;

#[link(name = "Kernel32")]
extern "system" {
    fn CloseHandle(handle: RawHandle) -> i32;
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> RawHandle;
    fn SetInformationJobObject(
        job: RawHandle,
        information_class: u32,
        information: *const c_void,
        information_length: u32,
    ) -> i32;
    fn CreateProcessW(
        application_name: *const u16,
        command_line: *mut u16,
        process_attributes: *mut c_void,
        thread_attributes: *mut c_void,
        inherit_handles: i32,
        creation_flags: u32,
        environment: *mut c_void,
        current_directory: *const u16,
        startup_info: *const StartupInfoW,
        process_information: *mut ProcessInformation,
    ) -> i32;
    fn ResumeThread(thread: RawHandle) -> u32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: RawHandle, exit_code: *mut u32) -> i32;
    fn TerminateProcess(process: RawHandle, exit_code: u32) -> i32;
    fn TerminateJobObject(job: RawHandle, exit_code: u32) -> i32;
    fn QueryInformationJobObject(
        job: RawHandle,
        information_class: u32,
        information: *mut c_void,
        information_length: u32,
        return_length: *mut u32,
    ) -> i32;
    fn Sleep(milliseconds: u32);
    fn CreatePipe(
        read_pipe: *mut RawHandle,
        write_pipe: *mut RawHandle,
        attributes: *mut SecurityAttributes,
        size: u32,
    ) -> i32;
    fn SetHandleInformation(handle: RawHandle, mask: u32, flags: u32) -> i32;
    fn PeekNamedPipe(
        pipe: RawHandle,
        buffer: *mut c_void,
        buffer_size: u32,
        bytes_read: *mut u32,
        bytes_available: *mut u32,
        bytes_left: *mut u32,
    ) -> i32;
    fn ReadFile(
        file: RawHandle,
        buffer: *mut c_void,
        bytes_to_read: u32,
        bytes_read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetLastError() -> u32;
    fn CreateFileW(
        path: *const u16,
        desired_access: u32,
        share_mode: u32,
        attributes: *const c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: RawHandle,
    ) -> RawHandle;
    fn InitializeProcThreadAttributeList(
        attribute_list: *mut c_void,
        attribute_count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        attribute_list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *const c_void,
        size: usize,
        previous_value: *mut c_void,
        return_size: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(attribute_list: *mut c_void);
}

#[repr(C)]
struct BasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
struct BasicAccountingInformation {
    total_user_time: i64,
    total_kernel_time: i64,
    this_period_total_user_time: i64,
    this_period_total_kernel_time: i64,
    total_page_fault_count: u32,
    total_processes: u32,
    active_processes: u32,
    total_terminated_processes: u32,
}

#[repr(C)]
struct ExtendedLimitInformation {
    basic_limit_information: BasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    security_descriptor: *mut c_void,
    inherit_handle: i32,
}

#[repr(C)]
struct StartupInfoW {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved2_size: u16,
    reserved2: *mut u8,
    std_input: RawHandle,
    std_output: RawHandle,
    std_error: RawHandle,
}

#[repr(C)]
struct StartupInfoExW {
    startup_info: StartupInfoW,
    attribute_list: *mut c_void,
}

#[repr(C)]
struct ProcessInformation {
    process: RawHandle,
    thread: RawHandle,
    process_id: u32,
    thread_id: u32,
}

#[derive(Debug)]
pub(crate) enum ProcessError {
    InvalidCommand,
    InvalidWorkload,
    TimedOut,
    Cancelled,
    OutputLimitExceeded,
    Undrained,
    Io(io::Error),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCommand => formatter.write_str("invalid process command"),
            Self::InvalidWorkload => formatter.write_str("workload exceeds fixed process limits"),
            Self::TimedOut => formatter.write_str("bounded process timed out"),
            Self::Cancelled => formatter.write_str("bounded process was cancelled"),
            Self::OutputLimitExceeded => {
                formatter.write_str("bounded process output exceeded its limit")
            }
            Self::Undrained => {
                formatter.write_str("owned process job did not drain before its deadline")
            }
            Self::Io(error) => write!(formatter, "Windows process operation failed: {error}"),
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProcessError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) struct ProcessCommand {
    executable: PathBuf,
    args: Vec<OsString>,
}

impl ProcessCommand {
    #[cfg(test)]
    pub(crate) fn test_fixture(
        executable: PathBuf,
        args: Vec<OsString>,
    ) -> Result<Self, ProcessError> {
        validate_command(&executable, &args)?;
        Ok(Self { executable, args })
    }
}

pub(crate) struct ProcessLimits {
    execution_timeout: Duration,
    termination_grace: Duration,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}

impl ProcessLimits {
    pub(crate) fn from_workload(
        warmup_seconds: u32,
        measurement_seconds: u32,
    ) -> Result<Self, ProcessError> {
        if warmup_seconds > MAX_WARMUP_SECONDS
            || !(MIN_MEASUREMENT_SECONDS..=MAX_MEASUREMENT_SECONDS).contains(&measurement_seconds)
        {
            return Err(ProcessError::InvalidWorkload);
        }

        Ok(Self {
            execution_timeout: Duration::from_secs(u64::from(warmup_seconds + measurement_seconds)),
            termination_grace: TERMINATION_GRACE,
            max_stdout_bytes: MAX_STDOUT_BYTES,
            max_stderr_bytes: MAX_STDERR_BYTES,
        })
    }
}

pub(crate) struct ProcessOutput {
    pub(crate) exit_code: u32,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaunchMilestone {
    CreatedSuspendedInJob,
    Resumed,
    PrimaryExitedWithActiveJob,
    AbortDrained,
}

pub(crate) fn run_process(
    command: ProcessCommand,
    limits: ProcessLimits,
    cancel: &AtomicBool,
) -> Result<ProcessOutput, ProcessError> {
    run_process_inner(command, limits, cancel, |_| {})
}

#[cfg(test)]
fn run_process_with_observer(
    command: ProcessCommand,
    limits: ProcessLimits,
    cancel: &AtomicBool,
    observe: impl FnMut(LaunchMilestone),
) -> Result<ProcessOutput, ProcessError> {
    run_process_inner(command, limits, cancel, observe)
}

fn run_process_inner(
    command: ProcessCommand,
    limits: ProcessLimits,
    cancel: &AtomicBool,
    mut observe: impl FnMut(LaunchMilestone),
) -> Result<ProcessOutput, ProcessError> {
    if cancel.load(Ordering::Acquire) {
        return Err(ProcessError::Cancelled);
    }

    let job = Job::create()?;
    let mut pipes = ChildPipes::create()?;
    let mut child = SuspendedChild::create(&command, &pipes, job.raw())?;
    pipes.close_child_handles();
    observe(LaunchMilestone::CreatedSuspendedInJob);

    let thread = child
        .thread
        .as_ref()
        .ok_or_else(|| ProcessError::Io(io::Error::other("primary thread handle is closed")))?;
    if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
        let error = io::Error::last_os_error();
        job.terminate_and_drain(limits.termination_grace)?;
        return Err(ProcessError::Io(error));
    }
    child.thread.take();
    observe(LaunchMilestone::Resumed);

    let (stdout, stderr) = match job.wait_for_output(
        &mut pipes,
        &limits,
        cancel,
        child.process.raw(),
        &mut observe,
    ) {
        Ok(output) => output,
        Err(error) => {
            job.terminate_and_drain(limits.termination_grace)?;
            observe(LaunchMilestone::AbortDrained);
            return Err(error);
        }
    };

    let mut exit_code = 0;
    if unsafe { GetExitCodeProcess(child.process.raw(), &mut exit_code) } == 0 {
        return Err(ProcessError::Io(io::Error::last_os_error()));
    }
    child.process_exited = true;

    Ok(ProcessOutput {
        exit_code,
        stdout,
        stderr,
    })
}
fn validate_command(executable: &Path, args: &[OsString]) -> Result<(), ProcessError> {
    if !executable.is_absolute()
        || !executable
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || executable
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || args.len() > MAX_ARGUMENTS
        || has_nul(executable.as_os_str())
        || args.iter().any(|argument| has_nul(argument))
    {
        return Err(ProcessError::InvalidCommand);
    }
    Ok(())
}

fn has_nul(value: &OsStr) -> bool {
    value.encode_wide().any(|unit| unit == 0)
}

fn wide_nul(value: &OsStr) -> Result<Vec<u16>, ProcessError> {
    if has_nul(value) {
        return Err(ProcessError::InvalidCommand);
    }
    Ok(value.encode_wide().chain(std::iter::once(0)).collect())
}

fn encode_command_line(executable: &OsStr, args: &[OsString]) -> Result<Vec<u16>, ProcessError> {
    if has_nul(executable) || args.len() > MAX_ARGUMENTS || args.iter().any(|arg| has_nul(arg)) {
        return Err(ProcessError::InvalidCommand);
    }

    let mut output = Vec::new();
    append_quoted_argument(&mut output, executable, true)?;
    for argument in args {
        output.push(u16::from(b' '));
        append_quoted_argument(&mut output, argument, false)?;
    }

    if output.len() + 1 > MAX_COMMAND_LINE_CHARS {
        return Err(ProcessError::InvalidCommand);
    }
    output.push(0);
    Ok(output)
}

fn append_quoted_argument(
    output: &mut Vec<u16>,
    argument: &OsStr,
    force_quotes: bool,
) -> Result<(), ProcessError> {
    let units = argument.encode_wide().collect::<Vec<_>>();
    let needs_quotes = force_quotes
        || units.is_empty()
        || units.iter().any(|unit| {
            *unit == u16::from(b'"') || *unit == u16::from(b' ') || *unit == u16::from(b'\t')
        });
    if !needs_quotes {
        output.extend(units);
        if output.len() + 1 > MAX_COMMAND_LINE_CHARS {
            return Err(ProcessError::InvalidCommand);
        }
        return Ok(());
    }

    output.push(u16::from(b'"'));
    let mut slashes = 0usize;
    for unit in units {
        if unit == u16::from(b'\\') {
            slashes = slashes.checked_add(1).ok_or(ProcessError::InvalidCommand)?;
        } else if unit == u16::from(b'"') {
            output.extend(std::iter::repeat_n(u16::from(b'\\'), slashes * 2 + 1));
            output.push(unit);
            slashes = 0;
        } else {
            output.extend(std::iter::repeat_n(u16::from(b'\\'), slashes));
            output.push(unit);
            slashes = 0;
        }
        if output.len() + slashes + 2 > MAX_COMMAND_LINE_CHARS {
            return Err(ProcessError::InvalidCommand);
        }
    }
    output.extend(std::iter::repeat_n(u16::from(b'\\'), slashes * 2));
    output.push(u16::from(b'"'));
    if output.len() + 1 > MAX_COMMAND_LINE_CHARS {
        return Err(ProcessError::InvalidCommand);
    }
    Ok(())
}
struct Handle(RawHandle);

impl Handle {
    fn from_raw(raw: RawHandle) -> io::Result<Self> {
        if raw.is_null() || raw as isize == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw))
        }
    }

    fn raw(&self) -> RawHandle {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
            self.0 = null_mut();
        }
    }
}

struct PipeReader {
    handle: Handle,
    eof: bool,
}

impl PipeReader {
    fn read_available(&mut self, output: &mut Vec<u8>, limit: usize) -> Result<bool, ProcessError> {
        if self.eof {
            return Ok(false);
        }
        let mut available = 0u32;
        if unsafe {
            PeekNamedPipe(
                self.handle.raw(),
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            )
        } == 0
        {
            let code = unsafe { GetLastError() };
            if code == ERROR_BROKEN_PIPE {
                self.eof = true;
                return Ok(false);
            }
            return Err(ProcessError::Io(io::Error::from_raw_os_error(code as i32)));
        }
        if available == 0 {
            return Ok(false);
        }

        let remaining = limit.saturating_sub(output.len());
        let request = available
            .min(MAX_PIPE_READ as u32)
            .min(u32::try_from(remaining.saturating_add(1)).unwrap_or(u32::MAX));
        let mut buffer = [0u8; MAX_PIPE_READ];
        let mut bytes_read = 0u32;
        if unsafe {
            ReadFile(
                self.handle.raw(),
                buffer.as_mut_ptr() as *mut c_void,
                request,
                &mut bytes_read,
                null_mut(),
            )
        } == 0
        {
            let code = unsafe { GetLastError() };
            if code == ERROR_BROKEN_PIPE {
                self.eof = true;
                return Ok(false);
            }
            return Err(ProcessError::Io(io::Error::from_raw_os_error(code as i32)));
        }
        if bytes_read == 0 {
            return Ok(false);
        }
        if bytes_read as usize > remaining {
            return Err(ProcessError::OutputLimitExceeded);
        }
        output.extend_from_slice(&buffer[..bytes_read as usize]);
        Ok(true)
    }
}

struct ChildPipes {
    stdout: PipeReader,
    stderr: PipeReader,
    stdin: Option<Handle>,
    stdout_writer: Option<Handle>,
    stderr_writer: Option<Handle>,
}

impl ChildPipes {
    fn create() -> Result<Self, ProcessError> {
        let (stdout_reader, stdout_writer) = create_inheritable_pipe()?;
        let (stderr_reader, stderr_writer) = create_inheritable_pipe()?;
        let attributes = SecurityAttributes {
            length: size_of::<SecurityAttributes>() as u32,
            security_descriptor: null_mut(),
            inherit_handle: 1,
        };
        let nul = wide_nul(OsStr::new("NUL"))?;
        let stdin = Handle::from_raw(unsafe {
            CreateFileW(
                nul.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &attributes as *const SecurityAttributes as *const c_void,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        })?;

        Ok(Self {
            stdout: PipeReader {
                handle: stdout_reader,
                eof: false,
            },
            stderr: PipeReader {
                handle: stderr_reader,
                eof: false,
            },
            stdin: Some(stdin),
            stdout_writer: Some(stdout_writer),
            stderr_writer: Some(stderr_writer),
        })
    }

    fn child_handles(&self) -> Result<[RawHandle; 3], ProcessError> {
        Ok([
            self.stdin
                .as_ref()
                .ok_or_else(|| ProcessError::Io(io::Error::other("child stdin is closed")))?
                .raw(),
            self.stdout_writer
                .as_ref()
                .ok_or_else(|| ProcessError::Io(io::Error::other("child stdout is closed")))?
                .raw(),
            self.stderr_writer
                .as_ref()
                .ok_or_else(|| ProcessError::Io(io::Error::other("child stderr is closed")))?
                .raw(),
        ])
    }

    fn close_child_handles(&mut self) {
        self.stdin.take();
        self.stdout_writer.take();
        self.stderr_writer.take();
    }
}

fn create_inheritable_pipe() -> Result<(Handle, Handle), ProcessError> {
    let mut attributes = SecurityAttributes {
        length: size_of::<SecurityAttributes>() as u32,
        security_descriptor: null_mut(),
        inherit_handle: 1,
    };
    let mut read_pipe = null_mut();
    let mut write_pipe = null_mut();
    if unsafe { CreatePipe(&mut read_pipe, &mut write_pipe, &mut attributes, 0) } == 0 {
        return Err(ProcessError::Io(io::Error::last_os_error()));
    }
    let reader = Handle::from_raw(read_pipe)?;
    let writer = Handle::from_raw(write_pipe)?;
    if unsafe { SetHandleInformation(reader.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(ProcessError::Io(io::Error::last_os_error()));
    }
    Ok((reader, writer))
}

struct ProcessAttributeList {
    _storage: Vec<usize>,
    pointer: *mut c_void,
    initialized: bool,
}

impl ProcessAttributeList {
    fn new(handles: &[RawHandle], jobs: &[RawHandle]) -> Result<Self, ProcessError> {
        let mut required_size = 0usize;
        unsafe {
            let _ = InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut required_size);
        }
        if required_size == 0 {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }
        let words = required_size.div_ceil(size_of::<usize>());
        let mut storage = vec![0usize; words];
        let pointer = storage.as_mut_ptr() as *mut c_void;
        if unsafe { InitializeProcThreadAttributeList(pointer, 2, 0, &mut required_size) } == 0 {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }
        let list = Self {
            _storage: storage,
            pointer,
            initialized: true,
        };
        if unsafe {
            UpdateProcThreadAttribute(
                list.pointer,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                handles.as_ptr() as *const c_void,
                std::mem::size_of_val(handles),
                null_mut(),
                null_mut(),
            )
        } == 0
        {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }
        if unsafe {
            UpdateProcThreadAttribute(
                list.pointer,
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST,
                jobs.as_ptr() as *const c_void,
                std::mem::size_of_val(jobs),
                null_mut(),
                null_mut(),
            )
        } == 0
        {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }
        Ok(list)
    }
}

impl Drop for ProcessAttributeList {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { DeleteProcThreadAttributeList(self.pointer) };
            self.initialized = false;
        }
    }
}
struct Job {
    handle: Handle,
}

impl Job {
    fn create() -> Result<Self, ProcessError> {
        let handle = Handle::from_raw(unsafe { CreateJobObjectW(null(), null()) })?;
        let mut information: ExtendedLimitInformation = unsafe { zeroed() };
        information.basic_limit_information.limit_flags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        information.basic_limit_information.active_process_limit = MAX_ACTIVE_PROCESSES;

        if unsafe {
            SetInformationJobObject(
                handle.raw(),
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                &information as *const ExtendedLimitInformation as *const c_void,
                size_of::<ExtendedLimitInformation>() as u32,
            )
        } == 0
        {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }
        Ok(Self { handle })
    }

    fn raw(&self) -> RawHandle {
        self.handle.raw()
    }

    fn process_counts(&self) -> Result<(u32, u32), ProcessError> {
        let mut information: BasicAccountingInformation = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.raw(),
                JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS,
                &mut information as *mut BasicAccountingInformation as *mut c_void,
                size_of::<BasicAccountingInformation>() as u32,
                null_mut(),
            )
        } == 0
        {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }
        Ok((information.active_processes, information.total_processes))
    }

    fn wait_for_output(
        &self,
        pipes: &mut ChildPipes,
        limits: &ProcessLimits,
        cancel: &AtomicBool,
        primary_process: RawHandle,
        observe: &mut impl FnMut(LaunchMilestone),
    ) -> Result<(Vec<u8>, Vec<u8>), ProcessError> {
        let deadline = Instant::now() + limits.execution_timeout;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut primary_exit_observed = false;
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(ProcessError::Cancelled);
            }
            pipes
                .stdout
                .read_available(&mut stdout, limits.max_stdout_bytes)?;
            pipes
                .stderr
                .read_available(&mut stderr, limits.max_stderr_bytes)?;

            let (active_processes, total_processes) = self.process_counts()?;
            if active_processes == 0 {
                loop {
                    let stdout_progress = pipes
                        .stdout
                        .read_available(&mut stdout, limits.max_stdout_bytes)?;
                    let stderr_progress = pipes
                        .stderr
                        .read_available(&mut stderr, limits.max_stderr_bytes)?;
                    if !stdout_progress && !stderr_progress {
                        break;
                    }
                }
                return Ok((stdout, stderr));
            }
            if !primary_exit_observed {
                match unsafe { WaitForSingleObject(primary_process, 0) } {
                    WAIT_OBJECT_0 => {
                        primary_exit_observed = true;
                        if total_processes > 1 {
                            observe(LaunchMilestone::PrimaryExitedWithActiveJob);
                        }
                    }
                    WAIT_TIMEOUT => {}
                    WAIT_FAILED => return Err(ProcessError::Io(io::Error::last_os_error())),
                    _ => {
                        return Err(ProcessError::Io(io::Error::other(
                            "unexpected process wait result",
                        )))
                    }
                }
            }
            if Instant::now() >= deadline {
                return Err(ProcessError::TimedOut);
            }
            unsafe { Sleep(10) };
        }
    }

    fn terminate_and_drain(&self, grace: Duration) -> Result<(), ProcessError> {
        let _ = unsafe { TerminateJobObject(self.raw(), TERMINATE_EXIT_CODE) };
        let deadline = Instant::now() + grace;
        loop {
            match self.process_counts() {
                Ok((0, _)) => return Ok(()),
                Ok(_) => {}
                Err(_) => return Err(ProcessError::Undrained),
            }
            if Instant::now() >= deadline {
                return Err(ProcessError::Undrained);
            }
            unsafe { Sleep(10) };
        }
    }
}

fn terminate_process_and_wait(process: RawHandle, grace: Duration) -> Result<(), ProcessError> {
    let _ = unsafe { TerminateProcess(process, TERMINATE_EXIT_CODE) };
    let milliseconds = u32::try_from(grace.as_millis()).map_err(|_| ProcessError::Undrained)?;
    match unsafe { WaitForSingleObject(process, milliseconds) } {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT | WAIT_FAILED => Err(ProcessError::Undrained),
        _ => Err(ProcessError::Undrained),
    }
}

struct SuspendedChild {
    process: Handle,
    thread: Option<Handle>,
    process_exited: bool,
}

impl SuspendedChild {
    fn create(
        command: &ProcessCommand,
        pipes: &ChildPipes,
        job: RawHandle,
    ) -> Result<Self, ProcessError> {
        let application_name = wide_nul(command.executable.as_os_str())?;
        let mut command_line = encode_command_line(command.executable.as_os_str(), &command.args)?;
        let child_handles = pipes.child_handles()?;
        let jobs = [job];
        let attributes = ProcessAttributeList::new(&child_handles, &jobs)?;
        let mut startup_info: StartupInfoExW = unsafe { zeroed() };
        startup_info.startup_info.cb = size_of::<StartupInfoExW>() as u32;
        startup_info.startup_info.flags = STARTF_USESTDHANDLES;
        startup_info.startup_info.std_input = child_handles[0];
        startup_info.startup_info.std_output = child_handles[1];
        startup_info.startup_info.std_error = child_handles[2];
        startup_info.attribute_list = attributes.pointer;
        let mut process_information: ProcessInformation = unsafe { zeroed() };

        if unsafe {
            CreateProcessW(
                application_name.as_ptr(),
                command_line.as_mut_ptr(),
                null_mut(),
                null_mut(),
                1,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
                null_mut(),
                null(),
                &startup_info as *const StartupInfoExW as *const StartupInfoW,
                &mut process_information,
            )
        } == 0
        {
            return Err(ProcessError::Io(io::Error::last_os_error()));
        }

        let process = Handle::from_raw(process_information.process)?;
        let thread = match Handle::from_raw(process_information.thread) {
            Ok(thread) => thread,
            Err(error) => {
                let mut child = Self {
                    process,
                    thread: None,
                    process_exited: false,
                };
                child.terminate_and_wait(TERMINATION_GRACE)?;
                return Err(ProcessError::Io(error));
            }
        };

        Ok(Self {
            process,
            thread: Some(thread),
            process_exited: false,
        })
    }

    fn terminate_and_wait(&mut self, grace: Duration) -> Result<(), ProcessError> {
        terminate_process_and_wait(self.process.raw(), grace)?;
        self.process_exited = true;
        Ok(())
    }
}

impl Drop for SuspendedChild {
    fn drop(&mut self) {
        if !self.process_exited {
            unsafe {
                let _ = TerminateProcess(self.process.raw(), TERMINATE_EXIT_CODE);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{
        encode_command_line, run_process, run_process_with_observer, LaunchMilestone,
        ProcessCommand, ProcessError, ProcessLimits,
    };
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    fn command_shell(args: &[&str]) -> ProcessCommand {
        let executable = std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
        ProcessCommand::test_fixture(executable, args.iter().map(OsString::from).collect()).unwrap()
    }

    fn limits() -> ProcessLimits {
        ProcessLimits::from_workload(0, 2).unwrap()
    }
    fn descendant_process_command() -> ProcessCommand {
        ProcessCommand::test_fixture(
            std::env::current_exe().unwrap(),
            [
                "--exact",
                "job_process::tests::descendant_process_fixture",
                "--ignored",
                "--nocapture",
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
        )
        .unwrap()
    }

    #[test]
    fn rejects_relative_executable_paths() {
        assert!(matches!(
            ProcessCommand::test_fixture(Path::new("cmd.exe").to_path_buf(), Vec::new()),
            Err(ProcessError::InvalidCommand)
        ));
    }

    #[test]
    fn quotes_windows_arguments_with_spaces_quotes_and_backslashes() {
        let command_line = encode_command_line(
            OsStr::new(r"C:\Program Files\DiskSpd\diskspd.exe"),
            &[
                OsString::from("plain"),
                OsString::from("with space"),
                OsString::from("quote\"inside"),
                OsString::from("ends with space\\"),
            ],
        )
        .unwrap();
        let command_line = OsString::from_wide(&command_line[..command_line.len() - 1]);
        assert_eq!(
            command_line,
            OsString::from(
                r#""C:\Program Files\DiskSpd\diskspd.exe" plain "with space" "quote\"inside" "ends with space\\""#,
            )
        );
    }

    #[test]
    fn rejects_workloads_outside_the_existing_policy_bounds() {
        assert!(matches!(
            ProcessLimits::from_workload(16, 2),
            Err(ProcessError::InvalidWorkload)
        ));
        assert!(matches!(
            ProcessLimits::from_workload(0, 61),
            Err(ProcessError::InvalidWorkload)
        ));
        assert!(matches!(
            ProcessLimits::from_workload(0, 1),
            Err(ProcessError::InvalidWorkload)
        ));
        assert!(ProcessLimits::from_workload(15, 60).is_ok());
    }

    #[test]
    fn captures_stdout_and_stderr() {
        let output = run_process(
            command_shell(&[
                "/d",
                "/c",
                "echo captured stdout & echo captured stderr 1>&2",
            ]),
            limits(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("captured stdout"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("captured stderr"));
    }

    #[test]
    fn rejects_stdout_and_stderr_overflow() {
        let mut stdout_limits = limits();
        stdout_limits.max_stdout_bytes = 64;
        assert!(matches!(
            run_process(
                command_shell(&[
                    "/d",
                    "/c",
                    "for /L %i in (1,1,100) do @echo stdout overflow"
                ]),
                stdout_limits,
                &AtomicBool::new(false),
            ),
            Err(ProcessError::OutputLimitExceeded)
        ));

        let mut stderr_limits = limits();
        stderr_limits.max_stderr_bytes = 64;
        assert!(matches!(
            run_process(
                command_shell(&[
                    "/d",
                    "/c",
                    "for /L %i in (1,1,100) do @echo stderr overflow 1>&2"
                ]),
                stderr_limits,
                &AtomicBool::new(false),
            ),
            Err(ProcessError::OutputLimitExceeded)
        ));
    }

    #[test]
    fn terminates_the_job_when_the_deadline_expires() {
        let mut bounded_limits = limits();
        bounded_limits.execution_timeout = Duration::from_millis(80);
        assert!(matches!(
            run_process(
                command_shell(&["/d", "/c", "ping -n 20 127.0.0.1 >nul"]),
                bounded_limits,
                &AtomicBool::new(false),
            ),
            Err(ProcessError::TimedOut)
        ));
    }

    #[test]
    fn cancellation_terminates_the_job() {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_after_delay = Arc::clone(&cancel);
        let signal = thread::spawn(move || {
            thread::sleep(Duration::from_millis(80));
            cancel_after_delay.store(true, Ordering::Release);
        });
        let result = run_process(
            command_shell(&["/d", "/c", "ping -n 20 127.0.0.1 >nul"]),
            limits(),
            &cancel,
        );
        signal.join().unwrap();
        assert!(matches!(result, Err(ProcessError::Cancelled)));
    }

    #[test]
    fn waits_for_a_descendant_to_leave_the_owned_job() {
        let started = Instant::now();
        let mut descendant_limits = limits();
        descendant_limits.execution_timeout = Duration::from_secs(10);
        let output = run_process(
            descendant_process_command(),
            descendant_limits,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(started.elapsed() >= Duration::from_secs(1));
    }

    #[test]
    fn cancellation_after_primary_exit_drains_the_remaining_descendant() {
        let cancel = AtomicBool::new(false);
        let mut observed = Vec::new();
        let mut bounded_limits = limits();
        bounded_limits.execution_timeout = Duration::from_secs(10);
        let result = run_process_with_observer(
            descendant_process_command(),
            bounded_limits,
            &cancel,
            |milestone| {
                observed.push(milestone);
                if milestone == LaunchMilestone::PrimaryExitedWithActiveJob {
                    cancel.store(true, Ordering::Release);
                }
            },
        );
        assert!(matches!(result, Err(ProcessError::Cancelled)));
        assert!(observed.contains(&LaunchMilestone::PrimaryExitedWithActiveJob));
        assert!(observed.contains(&LaunchMilestone::AbortDrained));
    }
    #[test]
    #[ignore = "launched only as the contained process-tree fixture"]
    fn descendant_process_fixture() {
        use std::process::{Command, Stdio};

        let powershell = PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        let descendant = Command::new(powershell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Milliseconds 1500",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(descendant);
    }
    #[test]
    fn runs_explicit_executable_and_propagates_exit_code() {
        let output = run_process(
            command_shell(&["/d", "/c", "exit 17"]),
            limits(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(output.exit_code, 17);
    }

    #[test]
    fn creates_child_suspended_inside_job_before_resuming_primary_thread() {
        let mut milestones = Vec::new();
        let output = run_process_with_observer(
            command_shell(&["/d", "/c", "exit 0"]),
            limits(),
            &AtomicBool::new(false),
            |milestone| milestones.push(milestone),
        )
        .unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(milestones.starts_with(&[
            LaunchMilestone::CreatedSuspendedInJob,
            LaunchMilestone::Resumed,
        ]));
    }
}
