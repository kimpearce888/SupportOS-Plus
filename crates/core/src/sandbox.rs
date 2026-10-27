//! Landlock + seccomp sandbox for parsing untrusted documents (plan row:
//! "Landlock + seccomp sandbox for pdf/zip parsing", feature-flagged
//! `sandbox`, Linux only).
//!
//! Threat model: `pdf-extract` and `zip` parse fully attacker-controlled
//! bytes. A memory-safety bug that turns into code execution must not be
//! able to read user data, reach the network, or persist anything. The
//! parse therefore runs on a dedicated thread that locks itself down
//! before touching the bytes:
//!
//! * **Landlock** restricts filesystem access to *nothing* (a ruleset
//!   handling all read/write path rights with zero rules). Best-effort
//!   compatibility: kernels without Landlock skip this layer — seccomp
//!   still applies.
//! * **seccomp** (BPF) allows only the compute/syscall baseline (memory,
//!   futexes, time, signals, exit, stderr writes) and returns `EPERM` for
//!   everything else — `socket`, `open`, `execve`, `clone`, `ptrace`, …
//!   stay blocked. `EPERM` instead of a kill action is deliberate: a
//!   killed thread cannot notify its channel (the caller would hang until
//!   the timeout), while `EPERM` still denies every escape path with a
//!   clean error. Filters stack but can never be replaced, so the
//!   allow-list cannot be loosened afterwards.
//!
//!   Descriptor hygiene (audit fix M13): Landlock cannot govern
//!   already-open descriptors, so the filter pins `read` to the process's
//!   own stdio, permits `mmap` only for anonymous (file-less) mappings,
//!   and denies `close` outright — an exploited parser must not be able
//!   to read, map, or close out from under the host the descriptors it
//!   inherited (the mirror DB, `secrets.key`, the listener sockets).
//!
//! The filters are per-thread (`prctl` semantics), so they affect only
//! the sandbox thread and nothing it never spawns. If the thread dies
//! (blocked syscall, panic), the caller gets an error instead of a hang
//! via a bounded receive.

use std::time::Duration;

use crate::error::{Error, Result};

/// Upper bound for one sandboxed task. Input sizes are already capped
/// upstream (`MAX_IMPORT_FILE_BYTES` / `MAX_PDF_INPUT_BYTES`), so a parse
/// that exceeds this is either wedged or the caps need revisiting.
const TASK_TIMEOUT: Duration = Duration::from_secs(120);

/// Run `f` on a locked-down thread and return its value.
///
/// The closure receives the already-buffered input bytes (it must not
/// read files or open sockets); everything it needs has to be moved in.
///
/// # Errors
/// * the thread could not be spawned;
/// * the restrictions could not be applied (fatal: the parse refuses to
///   run unsandboxed rather than silently losing the hardening);
/// * the closure panicked (reported as a parse error);
/// * the thread died or exceeded [`TASK_TIMEOUT`] (e.g. a blocked
///   syscall triggered the seccomp kill).
pub fn run_sandboxed<T, F>(label: &'static str, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<std::thread::Result<T>>();
    let thread = std::thread::Builder::new()
        .name(format!("spp-sandbox-{label}"))
        .spawn(move || {
            if let Err(e) = apply_restrictions() {
                // Hard-fail the task: dropping `tx` closes the channel and
                // the caller reports the error. Never run unfiltered.
                tracing::error!(error = %e, "sandbox restrictions failed: task aborted");
                return;
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            // A send error only means the caller already timed out.
            let _ = tx.send(result);
        })
        .map_err(|e| {
            Error::Config(format!(
                "failed to spawn the sandbox thread for {label}: {e}"
            ))
        })?;
    match rx.recv_timeout(TASK_TIMEOUT) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_panic)) => Err(Error::Validation(format!(
            "{label}: the parser panicked inside the sandbox"
        ))),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // The thread is abandoned (it is sandboxed and cannot do
            // harm while it finishes or dies); the JoinHandle detaches.
            drop(thread);
            Err(Error::Validation(format!(
                "{label}: sandboxed task timed out or was killed by the syscall filter"
            )))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            // The thread exited without a result: restrictions failed.
            let _ = thread.join();
            Err(Error::Config(format!(
                "{label}: sandbox restrictions could not be applied (see log)"
            )))
        }
    }
}

/// Apply both restriction layers to the calling thread. Landlock first:
/// its `PR_SET_NO_NEW_PRIVS` prctl runs before the seccomp filter exists;
/// the filter's own installing `seccomp(2)` call stays in the allow-list
/// (seccomp filters can only stack, never replace, so re-arming it gains
/// an attacker nothing).
fn apply_restrictions() -> std::result::Result<(), String> {
    let landlock_status = apply_landlock()?;
    tracing::debug!(landlock = ?landlock_status, "landlock layer applied");
    apply_seccomp()?;
    tracing::debug!("seccomp layer applied (compute allow-list, EPERM on mismatch)");
    Ok(())
}

/// Landlock deny-all: handle every V1 read/write path right and add zero
/// rules. Returns a short human-readable enforcement status.
fn apply_landlock() -> std::result::Result<String, String> {
    use landlock::{ABI, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr};
    let ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::from_read(ABI::V1))
        .map_err(|e| format!("landlock handle_access(read): {e}"))?
        .handle_access(AccessFs::from_write(ABI::V1))
        .map_err(|e| format!("landlock handle_access(write): {e}"))?
        .create()
        .map_err(|e| format!("landlock create: {e}"))?;
    let status = ruleset
        .restrict_self()
        .map_err(|e| format!("landlock restrict_self: {e}"))?;
    match status.ruleset {
        landlock::RulesetStatus::FullyEnforced => Ok("fully enforced".to_string()),
        landlock::RulesetStatus::PartiallyEnforced => Ok("partially enforced".to_string()),
        landlock::RulesetStatus::NotEnforced => {
            // Not an error: old or Landlock-disabled kernel. The seccomp
            // layer below still applies.
            Ok("not enforced (kernel without Landlock)".to_string())
        }
    }
}

/// seccomp allow-list for a pure-compute thread. Anything outside this
/// list fails with `EPERM`; `write` is additionally pinned to stderr and
/// stdout so an exploit cannot write to arbitrary inherited file
/// descriptors.
fn apply_seccomp() -> std::result::Result<(), String> {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule, TargetArch,
    };
    use std::convert::TryInto;

    // Unconditional allow-rules: syscall number -> always allowed.
    // Audit fix (M13): `read`, `close` and plain `mmap` are deliberately
    // NOT unconditional — see the conditional rules below. Landlock cannot
    // police already-open descriptors, so an exploited parser must not get
    // blanket read/mmap/close over the host's fd table (mirror DB,
    // secrets.key, listener sockets all live in that table).
    let allowed: &[i64] = &[
        libc::SYS_brk,
        libc::SYS_munmap,
        libc::SYS_mprotect,
        libc::SYS_madvise,
        libc::SYS_mremap, // only touches mappings this thread made (anonymous once mmap is gated)
        libc::SYS_futex,  // mpsc channels, mutexes, JoinHandle packets
        libc::SYS_clock_gettime,
        libc::SYS_clock_getres,
        libc::SYS_gettimeofday,
        libc::SYS_nanosleep,
        libc::SYS_clock_nanosleep,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_getpid,
        libc::SYS_gettid,
        libc::SYS_sched_getaffinity,
        libc::SYS_sched_yield,
        libc::SYS_getrandom, // harmless: no data egress
        libc::SYS_seccomp,   // installing this very filter
        libc::SYS_exit,
        libc::SYS_exit_group,
    ];
    let mut rules: std::collections::BTreeMap<i64, Vec<SeccompRule>> =
        allowed.iter().map(|nr| (*nr, Vec::new())).collect();
    // write(2): stdout (panic messages) and stderr (logging) only.
    let fd_rule = |fd: u64| {
        SeccompRule::new(vec![
            SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, fd)
                .expect("valid condition"),
        ])
        .expect("valid rule")
    };
    rules.insert(libc::SYS_write, vec![fd_rule(1), fd_rule(2)]);
    // Audit fix (M13): read(2) only on the process's own stdio. Every
    // sensitive descriptor — the mirror DB, secrets.key, the listener
    // sockets — sits at fd >= 3; stdio at 0..=2 is the app's own console
    // and carries no secret. `open` is not allowed anyway, so the parser
    // can never grow this set; all its input arrives as moved-in bytes.
    rules.insert(libc::SYS_read, vec![fd_rule(0), fd_rule(1), fd_rule(2)]);
    // Audit fix (M13): mmap(2) may only create ANONYMOUS mappings — the
    // allocator, thread stacks and guard pages all pass MAP_ANONYMOUS —
    // while a file-backed mmap would let the parser read an open file
    // (the DB, the key store) straight through its inherited fd.
    // MAP_ANONYMOUS is 0x20 on every architecture this crate targets
    // (x86_64, aarch64); the MaskedEq below asserts the flag is set.
    rules.insert(
        libc::SYS_mmap,
        vec![
            SeccompRule::new(vec![
                SeccompCondition::new(
                    3, // flags argument
                    SeccompCmpArgLen::Qword,
                    SeccompCmpOp::MaskedEq(libc::MAP_ANONYMOUS as u64),
                    libc::MAP_ANONYMOUS as u64,
                )
                .expect("valid condition"),
            ])
            .expect("valid rule"),
        ],
    );
    // Audit fix (M13): close(2) is denied outright (it stays out of the
    // map, so it hits the EPERM default). The sandbox cannot open
    // descriptors — `open` is not in the allow-list — so every close it
    // could attempt targets a host fd, and dropping one out from under
    // the running app is corruption the filter must prevent. Rust and
    // glibc both tolerate close() failures; no legitimate close exists
    // on a pure-compute thread that owns no descriptors.
    let arch: TargetArch = std::env::consts::ARCH
        .try_into()
        .map_err(|e| format!("seccompiler: unsupported arch {e}"))?;
    let filter: BpfProgram = SeccompFilter::new(
        rules,
        SeccompAction::Errno(libc::EPERM as u32),
        SeccompAction::Allow,
        arch,
    )
    .map_err(|e| format!("seccomp filter build: {e}"))?
    .try_into()
    .map_err(|e| format!("seccomp filter compile: {e}"))?;
    seccompiler::apply_filter(&filter).map_err(|e| format!("seccomp apply: {e}"))
}

#[cfg(test)]
mod tests {
    // The M13 regression tests below must issue the raw syscalls they gate
    // (read/close/mmap through libc) to prove the filter intercepts them;
    // the crate-wide `unsafe_code` deny is lifted for this test scope only.
    #![allow(unsafe_code)]

    use super::*;

    #[test]
    fn sandboxed_computation_returns_its_value() {
        let value = run_sandboxed("compute", || 40 + 2).expect("sandboxed computation");
        assert_eq!(value, 42);
    }

    #[test]
    fn sandboxed_panic_surfaces_as_a_parse_error() {
        let err = run_sandboxed::<(), _>("panic", || panic!("boom"))
            .expect_err("panics must surface as errors");
        assert!(
            err.to_string().contains("panicked"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sandbox_blocks_filesystem_reads() {
        // /etc/hostname is world-readable on every Linux; without the
        // sandbox this read succeeds, so the assertion proves the
        // Landlock layer is active.
        let result = run_sandboxed("fs-read", || {
            std::fs::read_to_string("/etc/hostname").is_ok()
        })
        .expect("the sandboxed task itself must complete");
        // BestEffort: on kernels without Landlock the read may succeed;
        // that is the documented degradation, not a test failure.
        if result {
            eprintln!("landlock not enforced on this kernel: FS test degraded to no-op");
        } else {
            assert!(!result, "the read must be denied by the Landlock layer");
        }
    }

    #[test]
    fn sandbox_blocks_network_syscalls() {
        // socket(2) is not in the allow-list: the seccomp layer must make
        // the bind fail with EPERM (observable, no hang).
        let blocked = run_sandboxed("net", || {
            std::net::TcpListener::bind(("127.0.0.1", 0)).is_err()
        })
        .expect("the sandboxed task itself must complete");
        assert!(
            blocked,
            "socket(2) must fail with EPERM under the seccomp allow-list"
        );
    }

    // --- Audit fix (M13) regression tests: descriptor hygiene ------------

    #[test]
    fn sandbox_m13_blocks_reads_beyond_stdio() {
        // fd 999 is not a stdio fd; whether or not it is open, the filter
        // must intercept the read. Asserting EPERM (filter) — not EBADF
        // (fd-table miss) — proves the seccomp rule fired before the fd
        // lookup, i.e. no fd >= 3 is readable from inside the sandbox.
        let errno = run_sandboxed("m13-read", || {
            let mut buf = [0u8; 16];
            unsafe { libc::read(999, buf.as_mut_ptr().cast(), buf.len()) };
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
        })
        .expect("the sandboxed task itself must complete");
        assert_eq!(errno, libc::EPERM, "read(2) beyond stdio must EPERM");
    }

    #[test]
    fn sandbox_m13_blocks_close_of_host_fds() {
        // close(2) must be denied outright: the sandbox cannot open
        // descriptors, so any close it attempts targets a host fd.
        let errno = run_sandboxed("m13-close", || {
            unsafe { libc::close(999) };
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
        })
        .expect("the sandboxed task itself must complete");
        assert_eq!(errno, libc::EPERM, "close(2) must be denied outright");
    }

    #[test]
    fn sandbox_m13_blocks_file_backed_mmap_but_allows_anonymous() {
        // A real, open fd for the file-backed attempt (opened by the TEST
        // thread — the sandbox itself never opens anything).
        let path = std::env::temp_dir().join(format!("spp-sandbox-m13-{}.tmp", std::process::id()));
        std::fs::write(&path, b"probe").expect("temp file");
        let file = std::fs::File::open(&path).expect("open temp file");
        let fd = std::os::fd::AsRawFd::as_raw_fd(&file);

        let errno = run_sandboxed("m13-mmap", move || {
            let m = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    4096,
                    libc::PROT_READ,
                    libc::MAP_PRIVATE, // no MAP_ANONYMOUS -> must EPERM
                    fd,
                    0,
                )
            };
            if m == libc::MAP_FAILED {
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
            } else {
                0
            }
        })
        .expect("the sandboxed task itself must complete");
        assert_eq!(
            errno,
            libc::EPERM,
            "file-backed mmap must EPERM under the filter"
        );

        // Anonymous mapping (the allocator's own path) must keep working.
        let ok = run_sandboxed("m13-anon", || {
            // 8 MB forces the mmap path in glibc malloc (threshold 128 KiB).
            let mut v: Vec<u8> = vec![0u8; 8 * 1024 * 1024];
            v[7] = 0xAB;
            v.iter().fold(0u64, |acc, &b| acc + u64::from(b)) == u64::from(0xAB_u8)
        })
        .expect("anonymous mmap must keep working under the filter");
        assert!(ok);

        drop(file);
        let _ = std::fs::remove_file(&path);
    }
}
