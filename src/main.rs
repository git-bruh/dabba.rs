use dabba::{log::Logger, sandbox::Sandbox};
use log::LevelFilter;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::register(LevelFilter::Trace)?;

    let cb = || {
        Command::new("sh").status().unwrap();

        0_isize
    };

    // The cgroup path can either be created by systemd or cgcreate, example
    // /sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/user.slice
    Sandbox::spawn(
        Path::new(std::env::args().nth(2).expect("no cgroup path!").as_str()),
        Path::new(std::env::args().nth(1).expect("no root passed!").as_str()),
        Box::new(cb),
    )?;

    Ok(())
}
