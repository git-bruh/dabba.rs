use rustainer::{mounts, no_new_privs, root, unshare};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO
    //   - Hostname
    //   - Mounting new pseudo-filesystems at /proc, /dev, /sys

    println!("prctl(PR_SET_NO_NEW_PRIVS)");
    no_new_privs()?;

    println!("Setting up namespace");
    unshare()?;

    println!("Setting up UID and GID mappings");
    root()?;

    println!("Performing the mounting dance");
    mounts("/")?;

    Command::new("sh").status()?;
    Ok(())
}
