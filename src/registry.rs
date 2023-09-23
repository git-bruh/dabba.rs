use serde::Deserialize;

/// application/vnd.docker.container.image.v1+json
/// application/vnd.docker.image.rootfs.diff.tar.gzip
#[derive(Debug, Deserialize)]
pub struct ManifestConfig {
    mediaType: String,
    size: i32,
    digest: String,
}

/// https://docs.docker.com/registry/spec/manifest-v2-2
/// application/vnd.docker.distribution.manifest.v2+json
#[derive(Debug, Deserialize)]
pub struct ManifestV2 {
    schemaVersion: i32,
    mediaType: String,
    config: ManifestConfig,
    layers: Vec<ManifestConfig>,
}

pub struct RegistryClient {
    image: String,
    tag: String,
}

impl RegistryClient {
    const AUTH_URL: &'static str = "https://auth.docker.io";
    const AUTH_SERVICE: &'static str = "registry.docker.io";

    const REGISTRY_URL: &'static str = "https://registry-1.docker.io/v2";

    pub fn new(image: &str, tag: &str) -> Self {
        Self {
            image: {
                // The `library/` prefix is used for official images
                if !image.contains('/') {
                    format!("library/{image}")
                } else {
                    String::from(image)
                }
            },
            tag: String::from(tag),
        }
    }

    fn get_token(&self) -> Result<String, ureq::Error> {
        let mut json: serde_json::Value = ureq::get(
            format!(
                "{}/token?service={}&scope=repository:{}:pull",
                Self::AUTH_URL,
                Self::AUTH_SERVICE,
                self.image
            )
            .as_str(),
        )
        .call()?
        .into_json()?;

        if let serde_json::Value::String(token) = json["token"].take() {
            return Ok(token);
        }

        panic!("Invalid token!");
    }

    pub fn get_blob(&self, digest: &str) -> Result<Vec<u8>, ureq::Error> {
        let blob =
            ureq::get(format!("{}/{}/blobs/{}", Self::REGISTRY_URL, self.image, digest,).as_str())
                .set(
                    "Authorization",
                    format!("Bearer {}", self.get_token()?).as_str(),
                )
                .call()?;

        assert!(blob.has("Content-Length"));

        if let Some(len) = blob.header("Content-Length") {
            let mut bytes: Vec<u8> = Vec::with_capacity(len.parse().expect("failed to parse int"));
            blob.into_reader().read_to_end(&mut bytes)?;

            return Ok(bytes);
        }

        panic!("No Content-Length!");
    }

    pub fn get_manifest(&self) -> Result<ManifestV2, ureq::Error> {
        // /library/alpine/manifests/latest
        let manifest: ManifestV2 = ureq::get(
            format!(
                "{}/{}/manifests/{}",
                Self::REGISTRY_URL,
                self.image,
                self.tag
            )
            .as_str(),
        )
        .set(
            "Authorization",
            format!("Bearer {}", self.get_token()?).as_str(),
        )
        .set(
            "Accept",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .call()?
        .into_json()?;

        Ok(manifest)
    }
}
