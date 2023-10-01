use dabba::{log::Logger, registry::RegistryClient, sandbox::Sandbox, storage::Storage};
use log::LevelFilter;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::register(LevelFilter::Info)?;

    let registry = RegistryClient::new("alpine", "latest");

    let manifest = registry.get_manifest()?;
    println!("{manifest:#?}");

    let config = registry.get_image_config(&manifest)?;
    println!("{config:#?}");

    let storage = Storage::new(Path::new("/tmp/storage"))?;
    storage.download_layers(&registry, &manifest)?;

    panic!("");

    let cb = || {
        Command::new("sh").status().unwrap();

        0_isize
    };

    // The cgroup path can either be created by systemd or cgcreate, example
    // /sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/user.slice
    Sandbox::spawn(
        Path::new(""),
        Path::new(std::env::args().nth(1).expect("no root passed!").as_str()),
        Box::new(cb),
    )?;

    Ok(())
}
