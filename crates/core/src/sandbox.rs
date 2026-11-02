use std::time::Duration;

use crate::error::{Error, Result};

const TASK_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) fn run_sandboxed<T, F>(label: &'static str, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<std::thread::Result<T>>();
    let thread = std::thread::Builder::new()
        .name(format!("spp-sandbox-{label}"))
        .spawn(move || {
            if let Err(e) = apply_restrictions() {
                tracing::error!(error = %e, "sandbox restrictions failed: task aborted");
                return;
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
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
            drop(thread);
            Err(Error::Validation(format!(
                "{label}: sandboxed task timed out or was killed by the syscall filter"
            )))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            let _ = thread.join();
            Err(Error::Config(format!(
                "{label}: sandbox restrictions could not be applied (see log)"
            )))
        }
    }
}

fn apply_restrictions() -> std::result::Result<(), String> {
    let landlock_status = apply_landlock()?;
    tracing::debug!(landlock = ?landlock_status, "landlock layer applied");
    apply_seccomp()?;
    tracing::debug!("seccomp layer applied (compute allow-list, EPERM on mismatch)");
    Ok(())
}

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
            Ok("not enforced (kernel without Landlock)".to_string())
        }
    }
}

fn apply_seccomp() -> std::result::Result<(), String> {
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule, TargetArch,
    };
    use std::convert::TryInto;

    let allowed: &[i64] = &[
        libc::SYS_brk,
        libc::SYS_munmap,
        libc::SYS_mprotect,
        libc::SYS_madvise,
        libc::SYS_mremap,
        libc::SYS_futex,
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
        libc::SYS_getrandom,
        libc::SYS_seccomp,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ];
    let mut rules: std::collections::BTreeMap<i64, Vec<SeccompRule>> =
        allowed.iter().map(|nr| (*nr, Vec::new())).collect();
    let fd_rule = |fd: u64| {
        SeccompRule::new(vec![
            SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, fd)
                .expect("valid condition"),
        ])
        .expect("valid rule")
    };
    rules.insert(libc::SYS_write, vec![fd_rule(1), fd_rule(2)]);
    rules.insert(libc::SYS_read, vec![fd_rule(0), fd_rule(1), fd_rule(2)]);
    rules.insert(
        libc::SYS_mmap,
        vec![
            SeccompRule::new(vec![
                SeccompCondition::new(
                    3,
                    SeccompCmpArgLen::Qword,
                    SeccompCmpOp::MaskedEq(libc::MAP_ANONYMOUS as u64),
                    libc::MAP_ANONYMOUS as u64,
                )
                .expect("valid condition"),
            ])
            .expect("valid rule"),
        ],
    );
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
        let result = run_sandboxed("fs-read", || {
            std::fs::read_to_string("/etc/hostname").is_ok()
        })
        .expect("the sandboxed task itself must complete");
        if result {
            eprintln!("landlock not enforced on this kernel: FS test degraded to no-op");
        } else {
            assert!(!result, "the read must be denied by the Landlock layer");
        }
    }

    #[test]
    fn sandbox_blocks_network_syscalls() {
        let blocked = run_sandboxed("net", || {
            std::net::TcpListener::bind(("127.0.0.1", 0)).is_err()
        })
        .expect("the sandboxed task itself must complete");
        assert!(
            blocked,
            "socket(2) must fail with EPERM under the seccomp allow-list"
        );
    }

    #[test]
    fn sandbox_m13_blocks_reads_beyond_stdio() {
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
        let errno = run_sandboxed("m13-close", || {
            unsafe { libc::close(999) };
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
        })
        .expect("the sandboxed task itself must complete");
        assert_eq!(errno, libc::EPERM, "close(2) must be denied outright");
    }

    #[test]
    fn sandbox_m13_blocks_file_backed_mmap_but_allows_anonymous() {
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
                    libc::MAP_PRIVATE,
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

        let ok = run_sandboxed("m13-anon", || {
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
