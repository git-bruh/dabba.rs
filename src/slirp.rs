use crate::util;
use nix::poll::{PollFd, PollFlags};
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::{Child, Command, Output, Stdio};

/// Helper for spawning slirp4netns and performing IPC with pipes
pub struct SlirpHelper {
    /// Pipe for waiting for slirp4netns readiness
    ready_pipe: (OwnedFd, OwnedFd),
    /// Pipe to notify slirp4netns to exit
    exit_pipe: (OwnedFd, OwnedFd),
    /// The child process handle
    slirp: Child,
}

impl SlirpHelper {
    /// Get the relevant namespace paths from /proc
    fn get_ns_paths(sandbox_pid: Pid) -> (String, String) {
        let proc_ns = format!("/proc/{sandbox_pid}/ns");

        (format!("{proc_ns}/user"), format!("{proc_ns}/net"))
    }

    /// Spawn a slirp4netns instance for the given `sandbox_pid`, but doesn't
    /// implicitly wait for readiness, must call `wait_until_ready`
    pub fn spawn(sandbox_pid: Pid) -> Result<Self, std::io::Error> {
        let ready_pipe = util::pipe_ownedfd()?;
        let exit_pipe = util::pipe_ownedfd()?;

        let (userns_path, netns_path) = Self::get_ns_paths(sandbox_pid);

        // TODO maybe take in arbritary handles for stdout and err
        let slirp = Command::new("slirp4netns")
            .args([
                "--configure",
                // This has to be investigated, for some reason slirp expects
                // to receive a POLLHUP event which doesn't get triggered on
                // closing the write end of the FD, so we use signals for now
                // "--exit-fd",
                // exit_pipe.0.as_raw_fd().to_string().as_str(),
                "--ready-fd",
                ready_pipe.1.as_raw_fd().to_string().as_str(),
                "--userns-path",
                userns_path.as_str(),
                "--netns-type=path",
                netns_path.as_str(),
                "tap0",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            ready_pipe,
            exit_pipe,
            slirp,
        })
    }

    /// Wait for activity on the notification FD to ensure that `slirp4netns`
    /// has initialized
    pub fn wait_until_ready(&self) -> Result<(), std::io::Error> {
        const TIMEOUT: i32 = 1000;

        let read_fd = &self.ready_pipe.0;

        let mut pollfd = [PollFd::new(read_fd, PollFlags::POLLIN)];
        nix::poll::poll(&mut pollfd, TIMEOUT)?;

        if (pollfd[0].revents().expect("failed to get revents!") & PollFlags::POLLIN)
            != PollFlags::POLLIN
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Did not receive POLLIN event after {TIMEOUT}ms"),
            ));
        }

        let mut notification_buf = [0];
        nix::unistd::read(read_fd.as_raw_fd(), &mut notification_buf)?;

        if notification_buf[0] != b'1' {
            log::warn!("Expected '1', got '{}'", notification_buf[0]);
        }

        Ok(())
    }

    /// ~~Write to the exit pipe, notifying `slirp4netns` to exit~~
    fn notify_exit(&self) -> Result<(), std::io::Error> {
        let write_fd = &self.exit_pipe.1;
        nix::unistd::write(write_fd.as_raw_fd(), &[b'1'])?;

        // TODO remove this once we figure out why writing to the pipe
        // doesn't wake up slirp
        nix::sys::signal::kill(
            Pid::from_raw(
                self.slirp
                    .id()
                    .try_into()
                    .expect("unreachable, PID would overflow"),
            ),
            Signal::SIGTERM,
        )?;

        Ok(())
    }

    /// Notify `slirp4netns` to exit and wait for the process to end
    pub fn notify_exit_and_wait(self) -> Result<Output, std::io::Error> {
        self.notify_exit()?;
        self.slirp.wait_with_output()
    }
}
