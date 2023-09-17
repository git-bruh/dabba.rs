use dabba::{container::*, log::Logger, slirp::SlirpHelper, util};
use log::LevelFilter;
use std::os::fd::AsRawFd;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO
    //   - Hostname

    Logger::register(LevelFilter::Trace)?;

    log::info!("prctl(PR_SET_NO_NEW_PRIVS)");
    no_new_privs()?;

    let ready_pipe = util::pipe_ownedfd()?;

    let cb = Box::new(|| {
        die_with_parent().expect("prctl failed!");

        let mut ready = [0];
        nix::unistd::read(ready_pipe.0.as_raw_fd(), &mut ready).expect("failed to read()");

        match ready[0] {
            0 => {
                log::warn!("Main process failed to initialize, exiting!");
                return 1;
            }
            1 => {
                log::info!("Main process successfully initialized!");
            }
            _ => {
                log::warn!("Got invalid status '{}'!", ready[0]);
                return 1;
            }
        }

        log::info!("Closing all non-essential FDs");
        close_fds();

        log::info!("Setting up UID and GID mappings");
        root().unwrap();

        log::info!("Performing the mounting dance");
        mounts(std::env::args().nth(1).expect("no root").as_str()).unwrap();

        log::info!("Setting up new session");
        new_session().expect("failed to setup session");

        Command::new("sh").status().unwrap();

        0
    });

    log::info!("Setting up namespace");

    let pid = clone_unshare(cb)?;

    match SlirpHelper::spawn(pid) {
        Ok(mut slirp) => {
            if let Err(e) = slirp.wait_until_ready() {
                log::warn!("Failed to wait for slirp: {e}");
                nix::unistd::write(ready_pipe.1.as_raw_fd(), &[0])?;
            } else {
                nix::unistd::write(ready_pipe.1.as_raw_fd(), &[1])?;
            }

            wait_for_completion(pid)?;
            slirp.notify_exit_and_wait()?;
        }
        Err(err) => {
            log::warn!("Failed to spawn slirp: {err}");
            nix::unistd::write(ready_pipe.1.as_raw_fd(), &[0])?;
        }
    }

    Ok(())
}
