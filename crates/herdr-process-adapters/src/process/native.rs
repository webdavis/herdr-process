use anyhow::{Result, bail, ensure};
use std::{
    mem::{size_of, zeroed},
    time::{Duration, Instant},
};

unsafe extern "C" {
    fn task_name_for_pid(task: u32, pid: i32, name: *mut u32) -> i32;
    fn task_info(task: u32, flavor: u32, info: *mut u32, count: *mut u32) -> i32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
    static mach_task_self_: u32;
    fn proc_signal_with_audittoken(token: *mut u32, signal: i32) -> i32;
}
fn info(pid: i32) -> Result<Option<libc::proc_bsdinfo>> {
    // proc_pidinfo initializes the entire structure on a successful full-sized read.
    let mut b = unsafe { zeroed::<libc::proc_bsdinfo>() };
    let n = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut b as *mut libc::proc_bsdinfo).cast(),
            size_of::<libc::proc_bsdinfo>() as i32,
        )
    };
    if n == size_of::<libc::proc_bsdinfo>() as i32 {
        return Ok(Some(b));
    }
    let e = std::io::Error::last_os_error();
    if n == 0 && e.raw_os_error() == Some(libc::ESRCH) {
        return Ok(None);
    }
    bail!("process identity unavailable for {pid}: {e}")
}
pub(super) struct TerminalJobs {
    leader: i32,
    device: u32,
}
impl TerminalJobs {
    pub fn current() -> Result<Self> {
        // The helper is the unreaped portable-pty session leader.
        let leader = unsafe { libc::getpid() };
        ensure!(
            unsafe { libc::getsid(0) } == leader,
            "guardian must be session leader"
        );
        let b = info(leader)?.ok_or_else(|| anyhow::anyhow!("guardian identity missing"))?;
        Ok(Self {
            leader,
            device: b.e_tdev,
        })
    }
    fn members(&self, end: Instant) -> Result<Vec<i32>> {
        let mut ids = vec![0i32; 64];
        loop {
            ensure!(Instant::now() < end, "terminal enumeration deadline");
            // PROC_TTY_ONLY=3 filters in the kernel; result is bytes, not a count.
            let bytes = unsafe {
                libc::proc_listpids(
                    3,
                    self.device,
                    ids.as_mut_ptr().cast(),
                    (ids.len() * 4) as i32,
                )
            };
            ensure!(bytes >= 0, "terminal enumeration denied");
            if (bytes as usize) < ids.len() * 4 {
                ensure!(bytes % 4 == 0, "invalid terminal enumeration size");
                ids.truncate(bytes as usize / 4);
                ids.retain(|pid| *pid > 0 && *pid != self.leader);
                return Ok(ids);
            }
            ensure!(ids.len() < 262144, "terminal enumeration capacity exceeded");
            ids.resize(ids.len() * 2, 0);
        }
    }
    fn signal(&self, pid: i32, signal: i32) -> Result<bool> {
        let Some(before) = info(pid)? else {
            return Ok(false);
        };
        if before.e_tdev != self.device || before.pbi_status == 5 {
            return Ok(false);
        }
        // Acquire identity before fresh ownership validation. Never signal by a stored PID.
        let mut port = 0;
        let kr = unsafe { task_name_for_pid(mach_task_self_, pid, &mut port) };
        if kr != 0 && info(pid)?.is_none() {
            return Ok(false);
        }
        ensure!(
            kr == 0,
            "task-name access denied for terminal member {pid}: {kr}"
        );
        let mut token = [0u32; 8];
        let mut count = 8;
        let kr = unsafe { task_info(port, 15, token.as_mut_ptr(), &mut count) };
        unsafe { mach_port_deallocate(mach_task_self_, port) };
        ensure!(
            kr == 0 && count == 8,
            "audit token unavailable for {pid}: {kr}"
        );
        let Some(after) = info(pid)? else {
            return Ok(false);
        };
        if before.pbi_start_tvsec != after.pbi_start_tvsec
            || before.pbi_start_tvusec != after.pbi_start_tvusec
            || after.e_tdev != self.device
            || unsafe { libc::getsid(pid) } != self.leader
        {
            return Ok(false);
        }
        let code = unsafe { proc_signal_with_audittoken(token.as_mut_ptr(), signal) };
        ensure!(
            code == 0 || code == libc::ESRCH,
            "audit-token signal {signal} denied for {pid}: {code}"
        );
        Ok(code == 0)
    }
    pub fn cleanup(&self, command: i32, status: &mut Option<i32>) -> Result<()> {
        let end = Instant::now() + Duration::from_millis(250);
        loop {
            ensure!(
                Instant::now() < end,
                "terminal job cleanup deadline exceeded"
            );
            let members = self.members(end)?;
            for pid in &members {
                self.signal(*pid, libc::SIGSTOP)?;
            }
            let stopped = self.members(end)?;
            let mut stable = true;
            for pid in &stopped {
                if let Some(b) = info(*pid)?
                    && b.pbi_status != 4
                    && b.pbi_status != 5
                {
                    stable = false;
                }
            }
            if stable {
                for pid in stopped {
                    self.signal(pid, libc::SIGKILL)?;
                }
            }
            reap(command, status)?;
            if self.members(end)?.is_empty() {
                return Ok(());
            }
            super::control::wait(-1, 0, end)?;
        }
    }
}
pub(super) fn reap(pid: i32, status: &mut Option<i32>) -> Result<()> {
    if status.is_some() {
        return Ok(());
    }
    let mut raw = 0;
    // Only the direct configured child is waited; the guardian itself stays alive.
    let n = unsafe { libc::waitpid(pid, &mut raw, libc::WNOHANG) };
    if n < 0 {
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::Interrupted {
            return Err(e.into());
        }
    }
    if n == pid {
        *status = Some(if libc::WIFEXITED(raw) {
            libc::WEXITSTATUS(raw)
        } else {
            128 + libc::WTERMSIG(raw)
        });
    }
    Ok(())
}

pub(super) fn abort_unstarted(child: &mut (dyn portable_pty::Child + Send + Sync)) -> Result<()> {
    // Start has never been sent. Keep the direct child unreaped while acquiring
    // and validating a fresh token; this path can never kill a running guardian.
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let pid = child
        .process_id()
        .ok_or_else(|| anyhow::anyhow!("helper identity unavailable"))? as i32;
    let Some(b) = info(pid)? else {
        return Ok(());
    };
    ensure!(
        unsafe { libc::getsid(pid) } == pid,
        "unstarted helper lost session ownership"
    );
    TerminalJobs {
        leader: pid,
        device: b.e_tdev,
    }
    .signal(pid, libc::SIGKILL)?;
    Ok(())
}
