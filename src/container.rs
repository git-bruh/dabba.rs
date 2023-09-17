use nix::mount::{MntFlags, MsFlags};
use nix::sched::CloneFlags;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::fs::File;
use std::io::Write;

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

    log::info!("Mounting /dev");
    nix::mount::mount(Some("dev"), "dev", Some("tmpfs"), MsFlags::empty(), null)?;

    log::info!("Mounting /proc");
    nix::mount::mount(
        Some("proc"),
        "proc",
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        null,
    )?;

    log::info!("Mounting /tmp");
    nix::mount::mount(Some("tmp"), "tmp", Some("tmpfs"), MsFlags::empty(), null)?;

    log::info!("Mounting /sys");
    nix::mount::mount(Some("sys"), "sys", Some("sysfs"), MsFlags::empty(), null)?;

    log::info!("Mounting /run");
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

        log::warn!("Closed FD '{fd}'")
    }
}
