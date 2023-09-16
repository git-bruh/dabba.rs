use dabba::{clone_unshare, mounts, no_new_privs, root, wait_for_completion};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO
    //   - Hostname

    println!("prctl(PR_SET_NO_NEW_PRIVS)");
    no_new_privs()?;

    let cb = Box::new(|| {
        println!("Setting up UID and GID mappings");
        root().unwrap();

        println!("Performing the mounting dance");
        mounts(std::env::args().nth(1).expect("no root").as_str()).unwrap();

        Command::new("sh").status().unwrap();

        0
    });

    println!("Setting up namespace");

    let pid = clone_unshare(cb)?;
    wait_for_completion(pid)?;

    Ok(())
}
