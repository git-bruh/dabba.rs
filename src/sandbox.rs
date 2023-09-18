use crate::{mount_helper, mount_helper::MountType};
use nix::sched::{CloneCb, CloneFlags};
use nix::sys::eventfd::EfdFlags;
use nix::sys::signal::Signal;
use nix::unistd::{Pid, Uid};
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;

pub struct Sandbox {
    pub pid: Pid,
}

impl Sandbox {
    /// Prevent ourselves from gaining any further privileges, say
    /// through executing setuid programs like `sudo` or `doas`
    pub fn no_new_privs() -> nix::Result<()> {
        nix::sys::prctl::set_no_new_privs()
    }

    /// Ensure that the child process is killed with SIGKILL when the parent
    /// container process exits
    pub fn die_with_parent() -> nix::Result<()> {
        nix::sys::prctl::set_pdeathsig(Signal::SIGKILL)
    }

    /// Sets the hostname of the container
    pub fn hostname() -> nix::Result<()> {
        nix::unistd::sethostname("container")
    }

    /// Pretent to be root inside the container by creating the appropriate
    /// UID and GID mappings
    pub fn uid_gid_mappings(uid: Uid) -> Result<(), std::io::Error> {
        let mapping = format!("0 {} 1", uid);
        log::info!("UID GID mappings: {mapping}");

        let write = |sub_file, contents: &str| -> Result<(), std::io::Error> {
            let mut file = File::create(format!("/proc/self/{sub_file}"))?;
            file.write_all(contents.as_bytes())?;

            Ok(())
        };

        write("uid_map", mapping.as_str())?;

        /* Unprivileged writes to `gid_map` are not possible unless
         * we disable setgroups() completely */
        write("setgroups", "deny")?;

        write("gid_map", mapping.as_str())?;

        Ok(())
    }

    /// Perform the mounting dance
    pub fn mount_and_pivot(root: &Path) -> Result<(), std::io::Error> {
        let target = Path::new("/tmp");

        log::info!("Blocking mount propagataion");
        mount_helper::block_mount_propagation()?;

        log::info!("Mounting the container at {target:?}");
        mount_helper::bind_container(root, target)?;

        // `chdir` into the target so we can use relative paths for
        // mounting rather than constructing new sub-paths each time
        nix::unistd::chdir(target)?;

        mount_helper::perform_pseudo_fs_mount(MountType::Dev, Path::new("dev"))?;
        mount_helper::perform_pseudo_fs_mount(MountType::Proc, Path::new("proc"))?;
        mount_helper::perform_pseudo_fs_mount(MountType::Sys, Path::new("sys"))?;
        mount_helper::perform_pseudo_fs_mount(MountType::Tmp, Path::new("tmp"))?;
        mount_helper::perform_pseudo_fs_mount(MountType::Tmp, Path::new("run"))?;

        log::info!("Performing pivot root");
        mount_helper::pivot(target)?;

        Ok(())
    }

    /// Create a new "session", preventing exploits involving
    /// ioctl()'s on the tty outside the sandbox
    pub fn new_session() -> nix::Result<Pid> {
        nix::unistd::setsid()
    }

    fn setup(uid: Uid, root: &Path) -> Result<(), std::io::Error> {
        log::info!("Spawned sandbox!");

        log::info!("Ensuring that child dies with parent");
        Self::die_with_parent()?;

        // log::info!("Closing FDs");
        // util::close_fds()?;

        log::info!("Setting hostname");
        Self::hostname()?;

        log::info!("Setting up UID GID mappings");
        Self::uid_gid_mappings(uid)?;

        log::info!("Performing the mounting dance");
        Self::mount_and_pivot(root)?;

        log::info!("Setting up new session");
        Self::new_session()?;

        Ok(())
    }

    /// Set up the namespace
    /// Just use clone() rather than fork() + unshare()
    /// as propagation of PID namespaces requires another fork()
    /// > The calling process is not moved
    /// > into the new namespace.  The first child created by the calling
    /// > process will have the process ID 1 and will assume the role of
    /// > init(1) in the new namespace.
    pub fn spawn(root: &Path, mut user_cb: CloneCb) -> Result<Self, std::io::Error> {
        // Must be static, otherwise a stack use-after-free will occur
        // as the memory is only valid for the duration of the function
        // TODO heap allocate this
        static mut STACK: [u8; 1024 * 1024] = [0_u8; 1024 * 1024];

        // XXX maybe swap this for something a bit rusty
        let efd = nix::sys::eventfd::eventfd(0, EfdFlags::empty())?;

        log::info!("Spawning sandbox!");

        let uid = Uid::current();

        let pid = unsafe {
            nix::sched::clone(
                Box::new(|| {
                    if let Err(err) = Self::setup(uid, root) {
                        log::error!("Failed to setup sandbox: {err}");

                        nix::unistd::write(efd.as_raw_fd(), &1_u64.to_ne_bytes())
                            .expect("failed to write!");
                        return 1;
                    }

                    nix::unistd::write(efd.as_raw_fd(), &2_u64.to_ne_bytes())
                        .expect("failed to write!");
                    user_cb()
                }),
                &mut STACK,
                CloneFlags::CLONE_NEWNS
                    | CloneFlags::CLONE_NEWUSER
                    | CloneFlags::CLONE_NEWPID
                    | CloneFlags::CLONE_NEWNET
                    | CloneFlags::CLONE_NEWIPC
                    | CloneFlags::CLONE_NEWUTS
                    | CloneFlags::CLONE_NEWCGROUP,
                Some(Signal::SIGCHLD as i32),
            )?
        };

        let mut ev64 = [0_u8; 8];
        nix::unistd::read(efd.as_raw_fd(), &mut ev64).expect("failed to read!");

        match u64::from_ne_bytes(ev64) {
            1 => {
                log::error!("Failed to setup sandbox, cleaning up child process");
                nix::sys::wait::waitpid(pid, None)?;

                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "sandbox setup failed",
                ))
            }
            2 => {
                log::info!("Successfully initialized sandbox and entered user_cb()");
                Ok(Self { pid })
            }
            _ => {
                panic!("Got invalid u64 value from container!")
            }
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        nix::sys::wait::waitpid(self.pid, None).expect("failed to wait!");
    }
}
