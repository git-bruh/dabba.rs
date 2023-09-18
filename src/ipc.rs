use crate::util;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};

/// Trivial IPC using pipes
pub struct Ipc {
    // TODO convert these tuples into a concrete struct to avid
    // confusing `.0` and `.1`
    parent_pipe: (OwnedFd, OwnedFd),
    child_pipe: (OwnedFd, OwnedFd),
}

/// An event produced by the child process
#[derive(TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ChildEvent {
    // Sandbox setup successfully
    InitFailed = 0,
    // An error occured while setting up the sandbox, process has exited
    InitSuccess = 1,
}

/// An event produced by the parent process, starts from 128 to avoid
/// misinterpreting a child event as a parent event and vice-versa, making
/// debugging in case of silly bugs easier
#[derive(TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ParentEvent {
    // An error occured when setting up slirp4netns
    SlirpFailure = 1 << 7,
    // An error occured when setting up UID GID mappings
    UidGidMapFailure,
    // All good, child can go ahead with the requested command in the sandbox
    InitSuccess,
}

fn send(write_fd: RawFd, event: u8) -> Result<(), std::io::Error> {
    nix::unistd::write(write_fd, &[event])?;
    Ok(())
}

fn recv(read_fd: RawFd) -> Result<u8, std::io::Error> {
    let mut event = [std::u8::MAX];
    nix::unistd::read(read_fd, &mut event)?;

    Ok(event[0])
}

impl Ipc {
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self {
            parent_pipe: util::pipe_ownedfd()?,
            child_pipe: util::pipe_ownedfd()?,
        })
    }

    pub fn send_from_parent(&self, event: ParentEvent) -> Result<(), std::io::Error> {
        send(self.parent_pipe.1.as_raw_fd(), event.into())
    }

    pub fn send_from_child(&self, event: ChildEvent) -> Result<(), std::io::Error> {
        send(self.child_pipe.1.as_raw_fd(), event.into())
    }

    pub fn recv_in_parent(&self) -> Result<ChildEvent, std::io::Error> {
        Ok(ChildEvent::try_from(recv(self.child_pipe.0.as_raw_fd())?)
            .expect("got invalid value from pipe!"))
    }

    pub fn recv_in_child(&self) -> Result<ParentEvent, std::io::Error> {
        Ok(ParentEvent::try_from(recv(self.parent_pipe.0.as_raw_fd())?)
            .expect("got invalid value from pipe!"))
    }
}
