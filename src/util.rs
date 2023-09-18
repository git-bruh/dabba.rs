use std::os::fd::{FromRawFd, OwnedFd};

/// Wrap nix::unistd::pipe to reutrn OwnedFd's rather than
/// RawFd's as RawFd doesn't clean itself up on being dropped
pub fn pipe_ownedfd() -> nix::Result<(OwnedFd, OwnedFd)> {
    let (p1, p2) = nix::unistd::pipe()?;
    unsafe { Ok((OwnedFd::from_raw_fd(p1), OwnedFd::from_raw_fd(p2))) }
}

/// Close all FDs apart from stdin, stdout and stderr
pub fn close_fds() -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir("/proc/self/fd")? {
        match entry?.file_name().to_str() {
            Some(entry) => {
                let entry = entry.parse::<i32>().expect("non-integer FD!");

                match entry {
                    // Retain std{in, out, err}
                    0..=2 => {
                        log::info!("Not closing FD {entry}");
                    }
                    _ => {
                        log::info!("Closing FD {entry}");
                        nix::unistd::close(entry)?;
                    }
                }
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "invalid fd name!",
                ));
            }
        }
    }

    Ok(())
}
