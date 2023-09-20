use crate::{
    cgroups::{CGroup, CGroupConfig},
    idmap_helper,
    ipc::{ChildEvent, Ipc, ParentEvent},
    mount_helper,
    mount_helper::MountType,
    slirp::SlirpHelper,
    util,
};
use nix::sched::{CloneCb, CloneFlags};
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::path::Path;

pub struct Sandbox {
    pub pid: Pid,
    /// Just store a binding for cleanup
    _cgroup: CGroup,
    slirp: SlirpHelper,
}

impl Sandbox {
    /// Prevent ourselves from gaining any further privileges, say
    /// through executing setuid programs like `sudo` or `doas`
    fn drop_privs() -> Result<(), std::io::Error> {
        const CAP_SYS_ADMIN: u32 = 21;

        nix::sys::prctl::set_no_new_privs()?;

        // Prevents usage of umount() in the container, possibly unmasking
        // a bind mount made by us over an existig directory
        // XXX nix doesn't provide a safe wrapper for PR_CAPBSET_DROP
        let ret = unsafe { libc::prctl(libc::PR_CAPBSET_DROP, CAP_SYS_ADMIN, 0, 0, 0) };

        if ret == -1 {
            return Err(nix::Error::from_i32(nix::errno::errno()).into());
        }

        Ok(())
    }

    /// Ensure that the child process is killed with SIGKILL when the parent
    /// container process exits
    fn die_with_parent() -> nix::Result<()> {
        nix::sys::prctl::set_pdeathsig(Signal::SIGKILL)
    }

    /// Sets the hostname of the container
    fn hostname() -> nix::Result<()> {
        nix::unistd::sethostname("container")
    }

    /// Perform the mounting dance
    fn mount_and_pivot(root: &Path) -> Result<(), std::io::Error> {
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
    fn new_session() -> nix::Result<Pid> {
        nix::unistd::setsid()
    }

    fn setup_child_inner(root: &Path) -> Result<(), std::io::Error> {
        log::info!("Spawned sandbox!");

        log::info!("Ensuring that child dies with parent");
        Self::die_with_parent()?;

        log::info!("Setting hostname");
        Self::hostname()?;

        log::info!("Performing the mounting dance");
        Self::mount_and_pivot(root)?;

        log::info!("Setting up new session");
        Self::new_session()?;

        log::info!("Dropping privileges");
        Self::drop_privs()?;

        Ok(())
    }

    fn setup_child(ipc: &Ipc, root: &Path) -> isize {
        match ipc.recv_in_child().expect("failed to recv from parent!") {
            ParentEvent::CGroupFailure => {
                log::warn!("Parent reported failure in CGroup setup, exiting");
                return 1;
            }
            ParentEvent::SlirpFailure => {
                log::warn!("Parent reported failure in slirp setup, exiting");
                return 1;
            }
            ParentEvent::UidGidMapFailure => {
                log::warn!("Parent reported failed mappings, exiting");
                return 1;
            }
            ParentEvent::InitSuccess => {
                log::info!("Received success event from parent, continuing setup")
            }
        }

        if let Err(err) = Self::setup_child_inner(root) {
            log::error!("Failed to setup sandbox: {err}");

            ipc.send_from_child(ChildEvent::InitFailed)
                .expect("failed to send from child!");

            return 1;
        }

        ipc.send_from_child(ChildEvent::InitSuccess)
            .expect("failed to send from child!");

        // TODO confirm if this can lead to a (harmless) double-close of the
        // IPC pipes in the sandbox process
        log::info!("Closing FDs");
        util::close_fds().expect("shouldn't fail to open /proc/self/fd");

        0
    }

    /// Set up the namespace
    /// Just use clone() rather than fork() + unshare()
    /// as propagation of PID namespaces requires another fork()
    /// > The calling process is not moved
    /// > into the new namespace.  The first child created by the calling
    /// > process will have the process ID 1 and will assume the role of
    /// > init(1) in the new namespace.
    pub fn spawn(
        base_cgroup: &Path,
        root: &Path,
        mut user_cb: CloneCb,
    ) -> Result<Self, std::io::Error> {
        // Must be static, otherwise a stack use-after-free will occur
        // as the memory is only valid for the duration of the function
        // TODO heap allocate this
        static mut STACK: [u8; 1024 * 1024] = [0_u8; 1024 * 1024];

        let ipc = Ipc::new()?;

        let mut cgroup = CGroup::new(
            base_cgroup,
            CGroupConfig {
                mem: String::from("500m"),
            },
        )?;

        log::info!("Spawning sandbox!");

        let pid = unsafe {
            nix::sched::clone(
                Box::new(|| {
                    let status = Self::setup_child(&ipc, root);

                    if status != 0 {
                        return status;
                    }

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

        log::info!("Launched sandbox with pid {pid}");

        if let Err(err) = cgroup.enforce(pid) {
            log::warn!("Failed to setup cgroups: {err}");
            ipc.send_from_parent(ParentEvent::CGroupFailure)
                .expect("failed to send from parent!");

            nix::sys::wait::waitpid(pid, None).expect("failed to wait!");
            return Err(err);
        }

        if let Err(err) = idmap_helper::setup_maps(pid) {
            log::warn!("Failed to setup UID GID mappings: {err}");
            ipc.send_from_parent(ParentEvent::UidGidMapFailure)
                .expect("failed to send from parent!");

            nix::sys::wait::waitpid(pid, None).expect("failed to wait!");
            return Err(err);
        }

        let slirp = match SlirpHelper::spawn(pid) {
            Ok(slirp) => slirp,
            Err(err) => {
                log::warn!("Failed to setup Slirp: {err}");
                ipc.send_from_parent(ParentEvent::SlirpFailure)
                    .expect("failed to send from parent!");

                nix::sys::wait::waitpid(pid, None).expect("failed to wait!");
                return Err(err);
            }
        };

        ipc.send_from_parent(ParentEvent::InitSuccess)
            .expect("failed to send from parent!");

        match ipc.recv_in_parent().expect("failed to recv in parent!") {
            ChildEvent::InitFailed => {
                log::warn!("Child notified failure in parent");
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "child init failed",
                ))
            }
            ChildEvent::InitSuccess => Ok(Self {
                pid,
                _cgroup: cgroup,
                slirp,
            }),
        }
    }

    pub fn wait(self) -> Result<(), std::io::Error> {
        let status = nix::sys::wait::waitpid(self.pid, None)?;
        log::info!("Sandbox exited with status {status:?}");

        let output = self.slirp.notify_exit_and_wait()?;

        log::info!(
            "Slirp Output\nStdOut\n{}\nStdErr\n{}",
            std::str::from_utf8(&output.stdout).expect("invalid stdout utf8!"),
            std::str::from_utf8(&output.stderr).expect("invalid stderr utf8!")
        );

        Ok(())
    }
}
