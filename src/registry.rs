use crate::util;
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
pub struct Manifest {
    schemaVersion: i32,
    mediaType: String,
    config: ManifestConfig,
    layers: Vec<ManifestConfig>,
}

/// Inner struct for list
#[derive(Debug, Deserialize)]
pub struct ImagePlatform {
    architecture: String,
    os: String,
}

/// Inner struct for list
#[derive(Debug, Deserialize)]
pub struct ImageIndex {
    mediaType: String,
    size: i32,
    digest: String,
    platform: ImagePlatform,
}

/// application/vnd.docker.distribution.manifest.list.v2+json
#[derive(Debug, Deserialize)]
pub struct ManifestListV2 {
    mediaType: String,
    manifests: Vec<ImageIndex>,
}

/// application/vnd.oci.image.index.v1+json
#[derive(Debug, Deserialize)]
pub struct ImageIndexV1 {
    mediaType: String,
    manifests: Vec<ImageIndex>,
}

pub struct RegistryClient {
    image: String,
    tag: String,
}

impl RegistryClient {
    const AUTH_URL: &'static str = "https://auth.docker.io";
    const AUTH_SERVICE: &'static str = "registry.docker.io";

    const REGISTRY_URL: &'static str = "https://registry-1.docker.io/v2";

    const MANIFEST_V2: &'static str = "application/vnd.docker.distribution.manifest.v2+json";
    const MANIFEST_LIST_V2: &'static str =
        "application/vnd.docker.distribution.manifest.list.v2+json";

    const IMAGE_INDEX_V1: &'static str = "application/vnd.oci.image.index.v1+json";
    const MANIFEST_V1: &'static str = "application/vnd.oci.image.manifest.v1+json";

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

    fn get_manifest_inner(
        &self,
        enable_oci: bool,
        digest: Option<&str>,
    ) -> Result<String, ureq::Error> {
        // /library/alpine/manifests/latest
        // /library/ubuntu/manifests/sha256:b492494d8e0113c4ad3fe4528a4b5ff89faa5331f7d52c5c138196f69ce176a6
        let mut request = ureq::get(
            format!(
                "{}/{}/manifests/{}",
                Self::REGISTRY_URL,
                &self.image,
                digest.unwrap_or(&self.tag),
            )
            .as_str(),
        )
        .set(
            "Authorization",
            format!("Bearer {}", self.get_token()?).as_str(),
        )
        .set("Accept", Self::MANIFEST_V2)
        .set("Accept", Self::MANIFEST_LIST_V2);

        // For some reason, Docker's registry starts sending us legacy DOCKER
        // V2.1 manifests https://docs.docker.com/registry/spec/manifest-v2-1/
        // When we request OCI-compliant ones, so we just special case this
        // for when we know we will get OCI-compliant responses
        request = if enable_oci {
            request
                .set("Accept", Self::IMAGE_INDEX_V1)
                .set("Accept", Self::MANIFEST_V1)
        } else {
            request
        };

        Ok(request.call()?.into_string()?)
    }

    pub fn get_manifest(&self) -> Result<Manifest, ureq::Error> {
        let manifest = self.get_manifest_inner(false, None)?;
        let parsed: serde_json::Value = util::serde_deserialize_or_err(&manifest)?;

        println!("{parsed:#?}");

        if let serde_json::Value::String(token) = &parsed["mediaType"] {
            log::info!("Got JSON: {parsed:#?}");

            match token.as_str() {
                Self::MANIFEST_V2 => {
                    return Ok(util::serde_deserialize_or_err(&manifest)?);
                }
                Self::MANIFEST_LIST_V2 | Self::IMAGE_INDEX_V1 => {
                    let manifest_list: ImageIndexV1 = util::serde_deserialize_or_err(&manifest)?;
                    let manifests: Vec<&ImageIndex> = manifest_list
                        .manifests
                        .iter()
                        .filter(|manifest_item| {
                            manifest_item.platform.architecture == "amd64"
                                && manifest_item.platform.os == "linux"
                        })
                        .collect();

                    // https://github.com/moby/moby/issues/45077
                    // https://github.com/moby/moby/issues/43126#issuecomment-1406280316
                    // We must fetch the manifest again, specifying the digest in the request
                    // $ docker buildx imagetools inspect ubuntu:latest
                    // Name:      docker.io/library/ubuntu:latest
                    // MediaType: application/vnd.oci.image.index.v1+json
                    // Digest:    sha256:aabed3296a3d45cede1dc866a24476c4d7e093aa806263c27ddaadbdce3c1054
                    //
                    // Manifests:
                    //   Name:      docker.io/library/ubuntu:latest@sha256:b492494d8e0113c4ad3fe4528a4b5ff89faa5331f7d52c5c138196f69ce176a6
                    //   MediaType: application/vnd.oci.image.manifest.v1+json
                    //   Platform:  linux/amd64
                    if let Some(final_manifest) = manifests.first() {
                        return Ok(util::serde_deserialize_or_err(&self.get_manifest_inner(
                            true,
                            Some(final_manifest.digest.as_str()),
                        )?)?);
                    }

                    panic!("No relevant manifests found!");
                }
                media_type => {
                    panic!("Got invalid media type {media_type}");
                }
            }
        }

        panic!("Invalid manifest!");
    }
}
