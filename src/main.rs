use dabba::{idmap_helper, log::Logger, sandbox::Sandbox, slirp::SlirpHelper};
use log::LevelFilter;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::register(LevelFilter::Trace)?;

    let cb = || {
        Command::new("sh").status().unwrap();

        0_isize
    };

    let sandbox = Sandbox::spawn(
        Path::new(std::env::args().nth(1).expect("no root passed!").as_str()),
        Box::new(cb),
    )?;

    idmap_helper::setup_maps(sandbox.pid)?;

    let mut slirp = SlirpHelper::spawn(sandbox.pid)?;

    sandbox.wait()?;
    slirp.notify_exit_and_wait()?;

    Ok(())
}
