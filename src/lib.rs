pub mod namespace_flags {
    pub use libc::{
        CLONE_NEWCGROUP as CGroup, CLONE_NEWIPC as Ipc, CLONE_NEWNET as Net, CLONE_NEWNS as Mount,
        CLONE_NEWPID as Pid, CLONE_NEWUSER as User, CLONE_NEWUTS as Uts,
    };
}

fn errno() -> std::io::Error {
    std::io::Error::last_os_error()
}

pub fn unshare(flags: i32) -> Result<(), std::io::Error> {
    unsafe {
        if libc::unshare(flags) == -1 {
            return Err(errno());
        }
    }

    Ok(())
}
