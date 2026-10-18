use std::{
    fs::File,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::Path,
    time::Instant,
};

pub struct StartupWatch {
    queue: OwnedFd,
    _runtime: File,
    exited: bool,
}

impl StartupWatch {
    pub fn new(runtime: &Path, log: &File) -> io::Result<Self> {
        let runtime = File::open(runtime)?;
        let queue = check(unsafe { libc::kqueue() })?;
        let queue = unsafe { OwnedFd::from_raw_fd(queue) };
        for file in [&runtime, log] {
            let written = event(
                file.as_raw_fd() as libc::uintptr_t,
                libc::EVFILT_VNODE,
                libc::NOTE_WRITE | libc::NOTE_EXTEND,
            );
            register(&queue, &written)?;
        }
        Ok(Self {
            queue,
            _runtime: runtime,
            exited: false,
        })
    }

    pub fn child(&mut self, pid: u32) -> io::Result<()> {
        let exit = event(pid as libc::uintptr_t, libc::EVFILT_PROC, libc::NOTE_EXIT);
        match register(&self.queue, &exit) {
            Ok(()) => Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {
                self.exited = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn wait_until(&self, deadline: Instant) -> io::Result<()> {
        if self.exited {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = libc::timespec {
            tv_sec: remaining.as_secs() as libc::time_t,
            tv_nsec: libc::c_long::from(remaining.subsec_nanos()),
        };
        let mut fired = [event(0, 0, 0)];
        let count = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                std::ptr::null(),
                0,
                fired.as_mut_ptr(),
                1,
                &timeout,
            )
        };
        match check(count) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(()),
            other => other.map(|_| ()),
        }
    }
}

fn event(ident: libc::uintptr_t, filter: i16, fflags: u32) -> libc::kevent {
    libc::kevent {
        ident,
        filter,
        flags: libc::EV_ADD | libc::EV_CLEAR,
        fflags,
        data: 0,
        udata: std::ptr::null_mut(),
    }
}

fn register(queue: &OwnedFd, change: &libc::kevent) -> io::Result<()> {
    check(unsafe {
        libc::kevent(
            queue.as_raw_fd(),
            change,
            1,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
        )
    })
    .map(|_| ())
}

fn check(result: i32) -> io::Result<i32> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}
