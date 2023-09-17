use nix::mount::{MntFlags, MsFlags};
use nix::poll::{PollFd, PollFlags};
use nix::sched::CloneFlags;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::fs::File;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::{Child, Command};

/// Wrap nix::unistd::pipe to reutrn OwnedFd's rather than
/// RawFd's as RawFd doesn't clean itself up on being dropped
pub fn pipe_ownedfd() -> nix::Result<(OwnedFd, OwnedFd)> {
    let (p1, p2) = nix::unistd::pipe()?;
    unsafe { Ok((OwnedFd::from_raw_fd(p1), OwnedFd::from_raw_fd(p2))) }
}

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
        let ready_pipe = pipe_ownedfd()?;
        let exit_pipe = pipe_ownedfd()?;

        let (userns_path, netns_path) = Self::get_ns_paths(sandbox_pid);

        let slirp = Command::new("slirp4netns")
            .args([
                "--configure",
                "--exit-fd",
                exit_pipe.0.as_raw_fd().to_string().as_str(),
                "--ready-fd",
                ready_pipe.1.as_raw_fd().to_string().as_str(),
                "--userns-path",
                userns_path.as_str(),
                "--netns-type=path",
                netns_path.as_str(),
                "tap0",
            ])
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
            eprintln!("Expected '1', got '{}'", notification_buf[0]);
        }

        Ok(())
    }

    /// Write to the exit pipe, notifying `slirp4netns` to exit
    fn notify_exit(&self) -> Result<(), std::io::Error> {
        let write_fd = &self.exit_pipe.1;
        nix::unistd::write(write_fd.as_raw_fd(), &[b'1'])?;

        Ok(())
    }

    /// Notify `slirp4netns` to exit and wait for the process to end
    pub fn notify_exit_and_wait(&mut self) -> Result<(), std::io::Error> {
        self.notify_exit()?;
        self.slirp.wait()?;

        Ok(())
    }
}

/// Prevent ourselves from gaining any further privileges, say
/// through executing setuid programs like `sudo` or `doas`
pub fn no_new_privs() -> nix::Result<()> {
    nix::sys::prctl::set_no_new_privs()
}

/// Pretent to be root inside the container by creating the appropriate
/// UID and GID mappings
pub fn root() -> Result<(), std::io::Error> {
    const MAPPING: &[u8] = b"0 1000 1";

    let write = |sub_file, contents| -> Result<(), std::io::Error> {
        let mut file = File::create(format!("/proc/self/{sub_file}"))?;
        file.write_all(contents)?;

        Ok(())
    };

    write("uid_map", MAPPING)?;

    /* Unprivileged writes to `gid_map` are not possible unless
     * we disable setgroups() completely */
    write("setgroups", b"deny")?;

    write("gid_map", MAPPING)?;

    Ok(())
}

/// Set up the namespace
/// Just use clone() rather than fork() + unshare()
/// as propagation of PID namespaces requires another fork()
/// > The calling process is not moved
/// > into the new namespace.  The first child created by the calling
/// > process will have the process ID 1 and will assume the role of
/// > init(1) in the new namespace.
pub fn clone_unshare(cb: nix::sched::CloneCb) -> nix::Result<nix::unistd::Pid> {
    // Must be static, otherwise a stack use-after-free will occur
    // as the memory is only valid for the duration of the function
    static mut STACK: [u8; 1024 * 1024] = [0_u8; 1024 * 1024];

    unsafe {
        nix::sched::clone(
            cb,
            &mut STACK,
            CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWNET
                | CloneFlags::CLONE_NEWIPC
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWCGROUP,
            Some(Signal::SIGCHLD as i32),
        )
    }
}

pub fn wait_for_completion(pid: nix::unistd::Pid) -> nix::Result<nix::sys::wait::WaitStatus> {
    nix::sys::wait::waitpid(pid, None)
}

/// Mounting dance
pub fn mounts(container_src: &str) -> nix::Result<()> {
    // Shuts up the type annotation errors caused by
    // nix::mount::mount trait bounds
    let null = None::<&str>;

    // TODO
    //   - devpts (nosuid, noexec)
    //   - dev/shm (nosuid, nodev)
    //   - proc, sys (nosuid, noexec, nodev)
    // Prevent mounts from propagating outside/into the namespace
    nix::mount::mount(null, "/", null, MsFlags::MS_REC | MsFlags::MS_PRIVATE, null)?;

    // Bind mount the container to a new directory for pivot_root()-ing
    // Must mount recursively as we could otherwise access a masked directory
    // inside a container
    nix::mount::mount(
        Some(container_src),
        "/tmp",
        null,
        MsFlags::MS_REC | MsFlags::MS_BIND,
        null,
    )?;

    nix::unistd::chdir("/tmp")?;

    let dirs: [&str; 5] = ["dev", "proc", "tmp", "sys", "run"];

    for dir in dirs.iter() {
        std::fs::create_dir_all(dir).expect("failed to mkdir");
    }

    println!("Mounting /dev");
    nix::mount::mount(Some("dev"), "dev", Some("tmpfs"), MsFlags::empty(), null)?;

    println!("Mounting /proc");
    nix::mount::mount(
        Some("proc"),
        "proc",
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        null,
    )?;

    println!("Mounting /tmp");
    nix::mount::mount(Some("tmp"), "tmp", Some("tmpfs"), MsFlags::empty(), null)?;

    println!("Mounting /sys");
    nix::mount::mount(Some("sys"), "sys", Some("sysfs"), MsFlags::empty(), null)?;

    println!("Mounting /run");
    nix::mount::mount(Some("tmp"), "run", Some("tmpfs"), MsFlags::empty(), null)?;

    // pivot_root() without creating an intermediate directory, as
    // described in `pivot_root(2)` NOTES section
    nix::unistd::pivot_root(".", ".")?;
    nix::mount::umount2(".", MntFlags::MNT_DETACH)?;

    Ok(())
}

/// Create a new "session",
pub fn new_session() -> nix::Result<Pid> {
    nix::unistd::setsid()
}

/// Close all FDs apart from stdin, stdout and stderr
pub fn close_fds() {
    // Close all FDs until failure
    for fd in 3.. {
        if nix::unistd::close(fd).is_err() {
            return;
        }

        eprintln!("Closed FD '{fd}'")
    }
}
