use rustainer::{namespace_flags, unshare};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unshare(
        namespace_flags::Mount
            | namespace_flags::User
            | namespace_flags::Pid
            | namespace_flags::Net
            | namespace_flags::Ipc
            | namespace_flags::Uts
            | namespace_flags::CGroup,
    )?;

    Command::new("sh").status()?;
    Ok(())
}
