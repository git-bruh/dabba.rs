use nix::mount::{MntFlags, MsFlags};
use nix::sched::CloneFlags;
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
pub fn unshare() -> nix::Result<()> {
    nix::sched::unshare(
        CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUSER
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWCGROUP,
    )
}

/// Mounting dance
pub fn mounts(container_src: &str) -> nix::Result<()> {
    // Shuts up the type annotation errors caused by
    // nix::mount::mount trait bounds
    let null = None::<&str>;

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

    // pivot_root() without creating an intermediate directory, as
    // described in `pivot_root(2)` NOTES section
    nix::unistd::pivot_root(".", ".")?;
    nix::mount::umount2(".", MntFlags::MNT_DETACH)?;

    Ok(())
}

/// Create a new "session",
pub fn new_session() -> nix::Result<nix::unistd::Pid> {
    nix::unistd::setsid()
}
