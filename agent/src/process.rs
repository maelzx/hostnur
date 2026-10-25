//! Bounded subprocess execution primitive.
//!
//! The agent runs an agent-selected program with an explicit argument array —
//! never a shell string. Execution has a hard deadline and bounded output;
//! timeouts terminate the whole process group. A fixed policy restricts which
//! programs may be run for privileged operations. Callers must have already
//! resolved and validated every argument; this module does not interpret shell
//! syntax.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

/// Upper bound for a single execution's captured output (256 KiB).
pub const MAX_OUTPUT_BYTES: usize = 262_144;
/// Upper bound for an execution deadline.
pub const MAX_DEADLINE: Duration = Duration::from_secs(3_600);

thread_local! {
    /// Wall-clock deadline of the operation running on this thread, if any.
    static OPERATION_DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Run `work` with an operation deadline: every `run` on this thread is capped
/// at the time remaining, and none starts once it has passed. Restores the
/// previous deadline afterwards, so nesting keeps the outer bound.
pub fn with_deadline<R>(deadline: Option<Instant>, work: impl FnOnce() -> R) -> R {
    struct Restore(Option<Instant>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OPERATION_DEADLINE.with(|cell| cell.set(self.0));
        }
    }

    let previous = OPERATION_DEADLINE.with(Cell::get);
    let effective = match (previous, deadline) {
        (Some(outer), Some(inner)) => Some(outer.min(inner)),
        (outer, inner) => outer.or(inner),
    };
    OPERATION_DEADLINE.with(|cell| cell.set(effective));
    let _restore = Restore(previous);
    work()
}

/// The command deadline after applying this thread's operation deadline.
/// `None` means the operation deadline has already passed.
fn effective_deadline(requested: Duration) -> Option<Duration> {
    match OPERATION_DEADLINE.with(Cell::get) {
        None => Some(requested),
        Some(deadline) => {
            let remaining = deadline.saturating_duration_since(Instant::now());
            (!remaining.is_zero()).then(|| requested.min(remaining))
        }
    }
}

/// A fully resolved process invocation.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// Absolute path to the program. No shell is used.
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    /// Minimal environment; the inherited environment is never passed through.
    pub env: BTreeMap<String, String>,
    pub deadline: Duration,
    pub output_cap_bytes: usize,
    /// When set and the agent runs as root, the child runs as this uid/gid.
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    /// Optional bounded bytes written to the child's stdin (then closed).
    pub stdin: Option<Vec<u8>>,
}

impl CommandSpec {
    pub fn new(program: impl Into<PathBuf>, deadline: Duration) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            working_dir: None,
            env: BTreeMap::new(),
            deadline,
            output_cap_bytes: 65_536,
            uid: None,
            gid: None,
            stdin: None,
        }
    }
}

/// The bounded result of an execution.
#[derive(Debug, Clone)]
pub struct ProcessOutcome {
    /// Process-group leader pid (also the child pid).
    pub pid: u32,
    /// Exit code when the process exited normally.
    pub exit_code: Option<i32>,
    /// True when the deadline elapsed and the process group was terminated.
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration: Duration,
}

impl ProcessOutcome {
    pub fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }
}

/// Execution failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    /// The program path or spec failed validation.
    Invalid(String),
    /// The child could not be spawned.
    Spawn(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Invalid(message) => write!(f, "invalid command: {message}"),
            ProcessError::Spawn(message) => write!(f, "spawn failed: {message}"),
        }
    }
}

impl std::error::Error for ProcessError {}

/// Fixed allowlist of privileged programs and argument validation.
#[derive(Debug, Clone, Default)]
pub struct ProcessPolicy {
    allowed_programs: Vec<PathBuf>,
}

impl ProcessPolicy {
    pub fn new(allowed_programs: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            allowed_programs: allowed_programs.into_iter().map(Into::into).collect(),
        }
    }

    pub fn allows(&self, program: &Path) -> bool {
        self.allowed_programs
            .iter()
            .any(|allowed| allowed == program)
    }

    /// Validate a spec against the fixed policy.
    pub fn validate(&self, spec: &CommandSpec) -> Result<(), ProcessError> {
        validate_spec(spec)?;
        if !self.allows(&spec.program) {
            return Err(ProcessError::Invalid(format!(
                "program `{}` is not allowlisted",
                spec.program.display()
            )));
        }
        Ok(())
    }
}

/// Validate a spec's structural safety independent of any allowlist.
pub fn validate_spec(spec: &CommandSpec) -> Result<(), ProcessError> {
    if !spec.program.is_absolute() {
        return Err(ProcessError::Invalid(
            "program must be an absolute path".into(),
        ));
    }
    if spec.program.as_os_str().is_empty() {
        return Err(ProcessError::Invalid("program must not be empty".into()));
    }
    for arg in &spec.args {
        if arg.contains('\0') {
            return Err(ProcessError::Invalid(
                "arguments must not contain NUL".into(),
            ));
        }
    }
    for (key, value) in &spec.env {
        if key.contains('\0') || value.contains('\0') {
            return Err(ProcessError::Invalid(
                "environment must not contain NUL".into(),
            ));
        }
    }
    // Half-specified identity must fail loudly: silently keeping only one of
    // uid/gid would run the child as the agent's own (possibly root) identity.
    if spec.uid.is_some() != spec.gid.is_some() {
        return Err(ProcessError::Invalid(
            "uid and gid must be set together or not at all".into(),
        ));
    }
    if let Some(working_dir) = &spec.working_dir
        && !working_dir.is_absolute()
    {
        return Err(ProcessError::Invalid(
            "working directory must be absolute".into(),
        ));
    }
    if spec.deadline.is_zero() || spec.deadline > MAX_DEADLINE {
        return Err(ProcessError::Invalid("deadline is out of range".into()));
    }
    if spec.output_cap_bytes == 0 || spec.output_cap_bytes > MAX_OUTPUT_BYTES {
        return Err(ProcessError::Invalid("output cap is out of range".into()));
    }
    Ok(())
}

/// Run a command with a deadline and bounded output.
///
/// On timeout the entire process group is killed with SIGKILL and
/// `timed_out` is set; the recorded outcome reflects the actual termination.
pub fn run(spec: &CommandSpec) -> Result<ProcessOutcome, ProcessError> {
    validate_spec(spec)?;

    // An operation past its deadline starts no further commands; report it the
    // same way as a command that ran out of time.
    let Some(deadline) = effective_deadline(spec.deadline) else {
        return Ok(ProcessOutcome {
            pid: 0,
            exit_code: None,
            timed_out: true,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            duration: Duration::ZERO,
        });
    };

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .env_clear()
        .envs(&spec.env)
        .stdin(if spec.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(working_dir) = &spec.working_dir {
        command.current_dir(working_dir);
    }
    // New process group so the whole tree can be signalled together.
    command.process_group(0);

    if let (Some(uid), Some(gid)) = (spec.uid, spec.gid) {
        // Drop the process identity in the forked child before exec. The
        // closure only calls async-signal-safe syscalls. Supplementary groups
        // are cleared *before* dropping to the target GID/UID so the child
        // never inherits the agent's extra groups.
        //
        // SAFETY: pre_exec runs between fork and exec; setgroups/setgid/setuid
        // are async-signal-safe and the closure captures only Copy values.
        unsafe {
            command.pre_exec(move || {
                if libc::setgroups(0, std::ptr::null()) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::setgid(gid) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::setuid(uid) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|err| ProcessError::Spawn(err.to_string()))?;
    let pid = child.id();

    // Feed bounded stdin (e.g. a SQL dump) then close it, so the child never
    // deadlocks waiting for input and the payload cannot be unbounded.
    if let Some(bytes) = &spec.stdin
        && let Some(mut stdin) = child.stdin.take()
    {
        let payload = bytes.clone();
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = stdin.write_all(&payload);
            let _ = stdin.flush();
        });
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let cap = spec.output_cap_bytes;
    let stdout_reader = spawn_reader(stdout, cap);
    let stderr_reader = spawn_reader(stderr, cap);

    let mut timed_out = false;
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {
                if started.elapsed() >= deadline {
                    timed_out = true;
                    let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
                    let status = child
                        .wait()
                        .map_err(|err| ProcessError::Spawn(err.to_string()))?;
                    break status.code();
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(err) => return Err(ProcessError::Spawn(err.to_string())),
        }
    };

    let (stdout, stdout_truncated) = join_reader(stdout_reader);
    let (stderr, stderr_truncated) = join_reader(stderr_reader);

    Ok(ProcessOutcome {
        pid,
        exit_code,
        timed_out,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        duration: started.elapsed(),
    })
}

type ReaderHandle = std::thread::JoinHandle<(Vec<u8>, bool)>;

fn spawn_reader<R: Read + Send + 'static>(reader: Option<R>, cap: usize) -> ReaderHandle {
    std::thread::spawn(move || match reader {
        Some(reader) => read_capped(reader, cap),
        None => (Vec::new(), false),
    })
}

fn join_reader(handle: ReaderHandle) -> (Vec<u8>, bool) {
    handle.join().unwrap_or_else(|_| (Vec::new(), false))
}

/// Read to EOF, keeping only the first `cap` bytes so the child never blocks.
fn read_capped<R: Read>(mut reader: R, cap: usize) -> (Vec<u8>, bool) {
    let mut kept = Vec::with_capacity(cap.min(8192));
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if kept.len() < cap {
                    let remaining = cap - kept.len();
                    let take = remaining.min(read);
                    kept.extend_from_slice(&buffer[..take]);
                    if take < read {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    (kept, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_program() {
        let mut spec = CommandSpec::new("echo", Duration::from_secs(1));
        spec.args = vec!["hi".into()];
        assert!(matches!(
            validate_spec(&spec),
            Err(ProcessError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_out_of_range_limits() {
        let mut spec = CommandSpec::new("/bin/echo", Duration::from_secs(1));
        spec.output_cap_bytes = 0;
        assert!(validate_spec(&spec).is_err());
        spec.output_cap_bytes = 1024;
        spec.deadline = Duration::ZERO;
        assert!(validate_spec(&spec).is_err());
    }

    #[test]
    fn policy_restricts_programs() {
        let policy = ProcessPolicy::new(["/usr/bin/systemctl"]);
        let blocked = CommandSpec::new("/bin/sh", Duration::from_secs(1));
        assert!(policy.validate(&blocked).is_err());

        let allowed = CommandSpec::new("/usr/bin/systemctl", Duration::from_secs(1));
        assert!(policy.validate(&allowed).is_ok());
    }

    #[test]
    fn operation_deadline_caps_a_running_command() {
        let mut spec = CommandSpec::new("/bin/sleep", Duration::from_secs(30));
        spec.args = vec!["5".into()];
        let started = Instant::now();
        let outcome = with_deadline(Some(Instant::now() + Duration::from_millis(200)), || {
            run(&spec)
        })
        .expect("sleep runs");
        assert!(outcome.timed_out);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn passed_operation_deadline_starts_nothing() {
        let spec = CommandSpec::new("/bin/true", Duration::from_secs(5));
        let outcome = with_deadline(Some(Instant::now()), || run(&spec)).expect("no spawn error");
        assert!(outcome.timed_out);
        assert_eq!(outcome.pid, 0);
        assert!(!outcome.succeeded());
    }

    #[test]
    fn deadline_is_restored_and_nesting_keeps_the_tighter_bound() {
        let outer = Instant::now() + Duration::from_secs(1);
        with_deadline(Some(outer), || {
            with_deadline(Some(outer + Duration::from_secs(60)), || {
                assert_eq!(OPERATION_DEADLINE.with(Cell::get), Some(outer));
            });
            assert_eq!(OPERATION_DEADLINE.with(Cell::get), Some(outer));
        });
        assert_eq!(OPERATION_DEADLINE.with(Cell::get), None);
    }
}
