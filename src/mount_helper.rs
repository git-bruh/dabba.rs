use nix::mount::{MntFlags, MsFlags};
use std::fs::OpenOptions;
use std::os::unix::fs::{symlink, OpenOptionsExt};
use std::path::{Path, PathBuf};

// Shuts up the type annotation errors caused by nix::mount::mount trait bounds
const NULL: Option<&str> = None;

#[derive(Debug)]
pub enum MountType {
    Dev,
    Proc,
    Sys,
    Tmp,
}

/// Prevent mounts from propagating outside/into the namespace
pub fn block_mount_propagation() -> nix::Result<()> {
    nix::mount::mount(NULL, "/", NULL, MsFlags::MS_REC | MsFlags::MS_PRIVATE, NULL)
}

/// Bind mount the container to a new directory for pivot_root()-ing
/// TODO remove this in another refactor
pub fn bind_container(container: &Path, target: &Path) -> nix::Result<()> {
    nix::mount::mount(
        Some(container),
        target,
        NULL,
        MsFlags::MS_REC | MsFlags::MS_BIND,
        NULL,
    )
}

/// Subdirectories inside `merged` are passed as the `upper` and `workdir`
/// arguments to OverlayFS, and the merged filesystem is mounted on `merged`
/// shadowing the intermediate directories
pub fn mount_image(layers: &[PathBuf], merged: &Path) -> Result<(), std::io::Error> {
    let merged = merged.to_path_buf();

    // The workdir is used internally to store tmp/intermediate files for
    // providing various guarantees such as atomicity
    let workdir = merged.join("work");
    std::fs::create_dir_all(&workdir)?;

    // This is where new files actually get written to
    let upperdir = merged.join("upper");
    std::fs::create_dir_all(&upperdir)?;

    // The first layer forms the base image and subsequent layers are mounted
    // on top of it, but the order of mounts goes from right-to-left in OverlayFS
    // mount options, so we construct the list in reverse
    // https://docs.kernel.org/filesystems/overlayfs.html#multiple-lower-layers
    // XXX We could use `fold` with a `String` but we don't mind a useless Vec
    // allocation here by collect()
    let lowerdir = layers
        .iter()
        .rev()
        .map(|lower| lower.to_str().expect("invalid utf8!"))
        .collect::<Vec<&str>>()
        .join(":")
        // Escape the digest as ':' is the delimiter for directories
        .replace("sha256:", "sha256\\:");

    let mount_args = format!(
        "lowerdir={},upperdir={},workdir={}",
        lowerdir,
        upperdir.to_str().expect("invalid utf8!"),
        workdir.to_str().expect("invalid utf8!")
    );

    log::info!("OverlayFS args: {mount_args}");

    nix::mount::mount(
        Some("overlay"),
        // Upper directory
        &merged,
        Some("overlay"),
        MsFlags::empty(),
        Some(mount_args.as_str()),
    )?;

    Ok(())
}

/// Bind `from_path` to `to_path`
/// Must mount recursively as we could otherwise access a masked directory
/// inside a container
pub fn bind(
    from_path: &Path,
    to_path: &Path,
    flags: Option<MsFlags>,
) -> Result<(), std::io::Error> {
    log::info!("Binding {from_path:?} to {to_path:?}");

    // TODO support directories
    OpenOptions::new()
        .write(true)
        // Read Only, doesn't matter as the bind mount sets permissions
        .mode(0o444)
        .create(true)
        .open(to_path)?;

    nix::mount::mount(
        Some(from_path),
        to_path,
        NULL,
        MsFlags::MS_REC | MsFlags::MS_BIND | flags.unwrap_or(MsFlags::empty()),
        NULL,
    )?;

    Ok(())
}

pub fn perform_pseudo_fs_mount(mount: MountType, path: &Path) -> Result<(), std::io::Error> {
    log::info!("Mounting {mount:?} at {path:?}");
    std::fs::create_dir_all(path)?;

    match mount {
        MountType::Dev => {
            nix::mount::mount(
                Some("dev"),
                path,
                Some("tmpfs"),
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
                // Must mount the tmpfs as read-write so that bind mounts of
                // character devices can be written to
                Some("mode=755"),
            )?;

            // Set up a traditional /dev hierarchy
            let host_dev_base = PathBuf::from("/dev");
            let container_dev_base = PathBuf::from(path);

            // We cannot call mknod() as an unprivileged user for creating
            // all these special files
            for node in ["full", "null", "random", "tty", "urandom", "zero"] {
                bind(
                    &host_dev_base.join(node),
                    &container_dev_base.join(node),
                    None,
                )?;
            }

            // Standard symlinks
            log::info!("Creating /dev stdio symlinks");
            for (fd, node) in ["stdin", "stdout", "stderr"].iter().enumerate() {
                symlink(format!("/proc/self/fd/{fd}"), container_dev_base.join(node))?;
            }

            // Some images rely on /dev/fd rather than /proc/self/fd
            log::info!("Creating /dev/fd");
            symlink("/proc/self/fd", container_dev_base.join("fd"))?;

            // Dummy directory for shared memory
            log::info!("Creating shm");
            std::fs::create_dir(container_dev_base.join("shm"))?;

            // Pseudo terminal devices
            log::info!("Creating ptmx symlink");
            symlink("pts/ptmx", container_dev_base.join("ptmx"))?;

            log::info!("Mounting devpts");

            let pts = container_dev_base.join("pts");

            std::fs::create_dir(pts)?;
            nix::mount::mount(
                Some("devpts"),
                &container_dev_base.join("pts"),
                Some("devpts"),
                MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
                // See "Mount options for devpts" in the mount(8) man-page
                // Create a private devpts instance and make it RW, but not
                // executable by all
                Some("newinstance,ptmxmode=0666"),
            )?
        }
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
