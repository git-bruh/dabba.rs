use nix::mount::{MntFlags, MsFlags};
use std::path::Path;

// Shuts up the type annotation errors caused by nix::mount::mount trait bounds
const NULL: Option<&str> = None;

#[derive(Debug)]
pub enum MountType {
    Dev,
    DevPts,
    Proc,
    Sys,
    Tmp,
}

/// Prevent mounts from propagating outside/into the namespace
pub fn block_mount_propagation() -> nix::Result<()> {
    nix::mount::mount(NULL, "/", NULL, MsFlags::MS_REC | MsFlags::MS_PRIVATE, NULL)
}

/// Bind mount the container to a new directory for pivot_root()-ing
/// Must mount recursively as we could otherwise access a masked directory
/// inside a container
pub fn bind_container(container: &Path, target: &Path) -> nix::Result<()> {
    nix::mount::mount(
        Some(container),
        target,
        NULL,
        MsFlags::MS_REC | MsFlags::MS_BIND,
        NULL,
    )
}

pub fn perform_pseudo_fs_mount(mount: MountType, path: &Path) -> Result<(), std::io::Error> {
    log::info!("Mounting {mount:?} at {path:?}");
    std::fs::create_dir_all(path)?;

    match mount {
        MountType::Dev => nix::mount::mount(
            Some("dev"),
            path,
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
            NULL,
        )?,
        MountType::DevPts => panic!("TODO"),
        MountType::Proc => nix::mount::mount(
            Some("proc"),
            path,
            Some("proc"),
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
            NULL,
        )?,
        MountType::Sys => nix::mount::mount(
            Some("sys"),
            path,
            Some("sysfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
            NULL,
        )?,
        MountType::Tmp => nix::mount::mount(
            Some("tmp"),
            path,
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            NULL,
        )?,
    }

    Ok(())
}

/// pivot_root() without creating an intermediate directory, as
/// described in `pivot_root(2)` NOTES section
pub fn pivot(path: &Path) -> nix::Result<()> {
    nix::unistd::chdir(path)?;

    nix::unistd::pivot_root(".", ".")?;
    nix::mount::umount2(".", MntFlags::MNT_DETACH)?;

    Ok(())
}
