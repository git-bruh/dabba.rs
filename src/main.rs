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

    Sandbox::spawn(Path::new("/"), Box::new(cb))?;

    Ok(())
}
