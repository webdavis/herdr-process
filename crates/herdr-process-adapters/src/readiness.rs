use std::{io, os::fd::RawFd, time::Duration};

#[derive(Default)]
pub struct Readiness {
    descriptors: Vec<libc::pollfd>,
}

impl Readiness {
    pub fn readable(&mut self, fd: RawFd) {
        self.push(fd, libc::POLLIN);
    }

    pub fn writable(&mut self, fd: RawFd) {
        self.push(fd, libc::POLLOUT);
    }

    fn push(&mut self, fd: RawFd, events: i16) {
        self.descriptors.push(libc::pollfd {
            fd,
            events,
            revents: 0,
        });
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        let timeout = timeout.map_or(-1, |t| i32::try_from(t.as_millis()).unwrap_or(i32::MAX));
        let count = unsafe {
            libc::poll(
                self.descriptors.as_mut_ptr(),
                self.descriptors.len() as libc::nfds_t,
                timeout,
            )
        };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        Ok(())
    }
}
