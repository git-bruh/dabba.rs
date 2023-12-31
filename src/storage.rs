use crate::registry::{Manifest, RegistryClient};
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use tar::Archive;

pub struct Storage {
    cache_dir: PathBuf,
}

impl Storage {
    pub fn new(cache_dir: &Path) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(cache_dir)?;

        Ok(Self {
            cache_dir: cache_dir.to_path_buf(),
        })
    }

    /// Download all the uncached layers for the given image
    /// Issues: Not very rpbust, downloads the whole layer in memory, etc.
    pub fn download_layers(
        &self,
        registry: &RegistryClient,
        manifest: &Manifest,
    ) -> Result<Vec<PathBuf>, ureq::Error> {
        let mut out = Vec::new();

        for layer in &manifest.layers {
            let layer = &layer.digest;

            let path = self.cache_dir.join(layer);

            if path.try_exists().expect("can't check path existence") {
                log::info!("Found layer {path:?}, skipping download");
            } else {
                log::info!("Downloading layer {layer}");

                // TODO Verify checksums
                let layer_bytes = registry.get_blob(layer)?;

                let mut gzip_reader = GzDecoder::new(&layer_bytes[..]);
                let mut archive = Archive::new(&mut gzip_reader);

                if let Err(err) = archive.unpack(&path) {
                    log::warn!("Failed to extract layer {layer}: {err}");

                    if let Err(_remove_err) = std::fs::remove_dir_all(&path) {
                        log::warn!("Failed to cleanup directory: {path:?}");
                    }

                    return Err(err.into());
                }

                log::info!("Successfully extracted layer {layer}");
            }

            out.push(path);
        }

        Ok(out)
    }
}
