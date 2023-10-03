use dabba::{
    log::Logger, registry::RegistryClient, sandbox::Sandbox, slirp::PortMapping, storage::Storage,
    util,
};
use log::LevelFilter;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::register(LevelFilter::Info)?;

    log::info!("Creating base directory");
    std::fs::create_dir_all(util::get_base_path())?;

    let mut args = std::env::args().skip(1);

    let image = args.next().expect("no image passed!");
    let tag = args.next().expect("no tag passed!");

    let mut ports = Vec::<PortMapping>::new();
    let mut env = Vec::<String>::new();

    loop {
        if let Some(arg) = args.next() {
            match arg.as_str() {
                "-p" => {
                    if let Some(mapping) = args.next() {
                        ports.push(PortMapping::from_str(&mapping));
                        continue;
                    }
                }
                "-e" => {
                    if let Some(env_var) = args.next() {
                        env.push(env_var);
                        continue;
                    }
                }
                _ => panic!("Invalid arg: {arg}"),
            }
        }

        // The inner matches will continue the loop if valid data exists
        break;
    }

    log::info!("Using image: '{image}', tag: '{tag}'");
    let registry = RegistryClient::new(&image, &tag);

    let manifest = registry.get_manifest()?;
    log::info!("Image Manifest: {manifest:#?}");

    let config = registry.get_image_config(&manifest)?;
    log::info!("Image Config: {config:#?}");

    let storage = Storage::new(Path::new(&format!("{}/storage", util::get_base_path())))?;
    let layers = storage.download_layers(&registry, &manifest)?;

    Sandbox::spawn(&layers, &config.config, &ports, &env)?;

    Ok(())
}
