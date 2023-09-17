use dabba::{
    clone_unshare, close_fds, mounts, new_session, no_new_privs, pipe_ownedfd, root,
    wait_for_completion, SlirpHelper,
};
use std::os::fd::AsRawFd;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO
    //   - Hostname

    println!("prctl(PR_SET_NO_NEW_PRIVS)");
    no_new_privs()?;

    let ready_pipe = pipe_ownedfd()?;

    let cb = Box::new(|| {
        let mut ready = [0];
        nix::unistd::read(ready_pipe.0.as_raw_fd(), &mut ready).expect("failed to read()");

        match ready[0] {
            0 => {
                eprintln!("Main process failed to initialize, exiting!");
                return 1;
            }
            1 => {
                println!("Main process successfully initialized!");
            }
            _ => {
                eprintln!("Got invalid status '{}'!", ready[0]);
                return 1;
            }
        }

        println!("Closing all non-essential FDs");
        close_fds();

        println!("Setting up UID and GID mappings");
        root().unwrap();

        println!("Performing the mounting dance");
        mounts(std::env::args().nth(1).expect("no root").as_str()).unwrap();

        println!("Setting up new session");
        new_session().expect("failed to setup session");

        Command::new("sh").status().unwrap();

        0
    });

    println!("Setting up namespace");

    let pid = clone_unshare(cb)?;

    match SlirpHelper::spawn(pid) {
        Ok(mut slirp) => {
            if let Err(e) = slirp.wait_until_ready() {
                eprintln!("Failed to wait for slirp: {e}");
                nix::unistd::write(ready_pipe.1.as_raw_fd(), &[0])?;
            } else {
                nix::unistd::write(ready_pipe.1.as_raw_fd(), &[1])?;
            }

            wait_for_completion(pid)?;
            slirp.notify_exit_and_wait()?;
        }
        Err(err) => {
            eprintln!("Failed to spawn slirp: {err}");
            nix::unistd::write(ready_pipe.1.as_raw_fd(), &[0])?;
        }
    }

    Ok(())
}
