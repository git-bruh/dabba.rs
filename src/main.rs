use dabba::{log::Logger, registry::RegistryClient, sandbox::Sandbox, storage::Storage};
use log::LevelFilter;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::register(LevelFilter::Info)?;

    let mut args = std::env::args().skip(1);

    let image = args.next().expect("no image passed!");
    let tag = args.next().expect("no tag passed!");

    log::info!("Using image: '{image}', tag: '{tag}'");
    let registry = RegistryClient::new(&image, &tag);

    let manifest = registry.get_manifest()?;
    log::info!("Image Manifest: {manifest:#?}");

    let config = registry.get_image_config(&manifest)?;
    log::info!("Image Config: {config:#?}");

    let storage = Storage::new(Path::new("/tmp/storage"))?;
    let layers = storage.download_layers(&registry, &manifest)?;

    Sandbox::spawn(&layers, &config.config)?;

    Ok(())
}
