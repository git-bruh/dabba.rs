use nix::{dir::Dir, fcntl::OFlag, sys::stat::Mode};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::{Child, Output};

/// Wait for a process to exit and store it's output without
/// moving the object
pub fn wait_with_output(process: &mut Child) -> Result<Output, std::io::Error> {
    let status = process.wait()?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    process
        .stdout
        .take()
        .expect("failed to take stdout")
        .read_to_end(&mut stdout)?;
    process
        .stderr
        .take()
        .expect("failed to take stdin")
        .read_to_end(&mut stderr)?;

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Wrap nix::unistd::pipe to reutrn OwnedFd's rather than
/// RawFd's as RawFd doesn't clean itself up on being dropped
pub fn pipe_ownedfd() -> nix::Result<(OwnedFd, OwnedFd)> {
    let (p1, p2) = nix::unistd::pipe()?;
    unsafe { Ok((OwnedFd::from_raw_fd(p1), OwnedFd::from_raw_fd(p2))) }
}

/// Close all FDs apart from stdin, stdout and stderr
pub fn close_fds() -> Result<(), std::io::Error> {
    let dir = Dir::open("/proc/self/fd", OFlag::O_DIRECTORY, Mode::empty())?;
    let dir_fd = dir.as_raw_fd();

    for entry in dir.into_iter() {
        match entry?.file_name().to_str() {
            Ok(entry) => {
                if entry == "." || entry == ".." {
                    continue;
                }

                match entry.parse::<i32>() {
                    // Retain std{in, out, err}
                    Ok(0) => log::info!("Not closing stdin"),
                    Ok(1) => log::info!("Not closing stdout"),
                    Ok(2) => log::info!("Not closing stderr"),
                    Ok(fd) => {
                        if fd == dir_fd {
                            log::info!("Not closing dir fd {dir_fd}")
                        } else {
                            log::info!("Closing FD {fd}");
                            nix::unistd::close(fd)?;
                        }
                    }
                    Err(err) => {
                        log::warn!("Got invalid FD '{entry}': {err}");
                    }
                }
            }
            Err(err) => {
                log::warn!("Got invalid UTF8 in FD, ignoring: {err}");
            }
        }
    }

    Ok(())
}
