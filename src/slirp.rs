use crate::util;
use nix::poll::{PollFd, PollFlags};
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use serde::Serialize;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

/// Helper for spawning slirp4netns and performing IPC with pipes
pub struct SlirpHelper {
    /// Pipe for waiting for slirp4netns readiness
    ready_pipe: (OwnedFd, OwnedFd),
    /// The child process handle
    slirp: Child,
    socket_path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct PortMapping {
    proto: String,
    host_port: u16,
    guest_port: u16,
}

#[derive(Debug, Serialize)]
struct HostFwdRequest {
    execute: String,
    arguments: PortMapping,
}

/// XXX this should implement the FromStr trait instead
impl PortMapping {
    // HOST:GUEST/PROTO
    // 8080:80/tcp
    pub fn from_str(mapping: &str) -> Self {
        log::info!("Parsing mapping: {mapping}");

        let mut port_proto = mapping.split('/');

        let ports = port_proto.next().expect("no ports passed");
        let proto = port_proto.next().expect("no protocol passed");

        let mut host_guest = ports.split(':');

        let host = host_guest.next().expect("no host port");
        let guest = host_guest.next().expect("no guest port");

        Self {
            proto: match proto {
                "tcp" | "udp" => proto.to_string(),
                _ => panic!("Invalid protocol: {proto}"),
            },
            host_port: host.parse::<u16>().expect("host port overflows!"),
            guest_port: guest.parse::<u16>().expect("guest port overflows!"),
        }
    }
}

impl SlirpHelper {
    /// Get the relevant namespace paths from /proc
    /// (user_namespace, network_namespace)
    fn get_ns_paths(sandbox_pid: Pid) -> (String, String) {
        let proc_ns = format!("/proc/{sandbox_pid}/ns");

        (format!("{proc_ns}/user"), format!("{proc_ns}/net"))
    }

    /// Spawn a slirp4netns instance for the given `sandbox_pid`, but doesn't
    /// implicitly wait for readiness, must call `wait_until_ready`
    pub fn spawn(sandbox_pid: Pid, socket_path: &Path) -> Result<Self, std::io::Error> {
        let ready_pipe = util::pipe_ownedfd()?;
        let (userns_path, netns_path) = Self::get_ns_paths(sandbox_pid);

        // TODO maybe take in arbritary handles for stdout and err
        let slirp = Command::new("slirp4netns")
            .args([
                "--configure",
                // Don't allow connecting to programs listening on the host
                "--disable-host-loopback",
                "--ready-fd",
                ready_pipe.1.as_raw_fd().to_string().as_str(),
                "--userns-path",
                userns_path.as_str(),
                "--netns-type=path",
                netns_path.as_str(),
                "--api-socket",
                socket_path.to_str().expect("invalid utf-8!"),
                "tap0",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            ready_pipe,
            slirp,
            socket_path: socket_path.to_path_buf(),
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

    /// Expost a port to the host network
    pub fn expose_port(&self, mapping: &PortMapping) -> Result<(), std::io::Error> {
        let mut stream = UnixStream::connect(&self.socket_path)?;

        let request = HostFwdRequest {
            execute: "add_hostfwd".to_string(),
            arguments: mapping.clone(),
        };

        log::info!("Sending host port forwarding request: {request:#?}");

        let request = serde_json::to_string(&request).expect("unreachable (unserializable type)");
        stream.write_all(&request.into_bytes())?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        let parsed: serde_json::Value =
            serde_json::from_str(&response).expect("slirp sent invalid JSON!");
        log::info!("Received JSON from slirp: {parsed}");

        // The "return" key indicates success
        if parsed.get("return").is_some() {
            return Ok(());
        }

        if let Some(error) = parsed.get("error") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Slirp Error: {error}"),
            ));
        }

        panic!("Got invalid JSON! {parsed}");
    }

    /// Signal slirp4netns to exit
    pub fn notify_exit_and_wait(&mut self) -> Result<(), std::io::Error> {
        // Send a signal only if the process has not exited yet
        if self.slirp.try_wait()?.is_none() {
            nix::sys::signal::kill(
                Pid::from_raw(
                    self.slirp
                        .id()
                        .try_into()
                        .expect("unreachable, PID would overflow"),
                ),
                Signal::SIGTERM,
            )?;
        }

        self.slirp.wait()?;

        Ok(())
    }

    /// Notify `slirp4netns` to exit and wait for the process to end
    pub fn output(&mut self) -> Result<Output, std::io::Error> {
        self.notify_exit_and_wait()?;
        util::wait_with_output(&mut self.slirp)
    }
}

impl Drop for SlirpHelper {
    fn drop(&mut self) {
        if let Err(err) = self.notify_exit_and_wait() {
            log::warn!("Failed to wait for slirp: {err}");
        }
    }
}
