use crate::util;
use serde::Deserialize;

/// application/vnd.docker.container.image.v1+json
/// application/vnd.docker.image.rootfs.diff.tar.gzip
#[derive(Debug, Deserialize)]
pub struct ManifestConfig {
    #[serde(rename(deserialize = "mediaType"))]
    pub media_type: String,
    pub size: i32,
    pub digest: String,
}

/// https://docs.docker.com/registry/spec/manifest-v2-2
/// application/vnd.docker.distribution.manifest.v2+json
#[derive(Debug, Deserialize)]
pub struct Manifest {
    #[serde(rename(deserialize = "schemaVersion"))]
    pub schema_version: i32,
    #[serde(rename(deserialize = "mediaType"))]
    pub media_type: String,
    config: ManifestConfig,
    /// The first layer is the base image, and subsequent layers must be
    /// mounted on top of it
    pub layers: Vec<ManifestConfig>,
}

/// Inner struct for list
#[derive(Debug, Deserialize)]
pub struct ImagePlatform {
    pub architecture: String,
    pub os: String,
}

/// Inner struct for list
#[derive(Debug, Deserialize)]
pub struct ImageIndex {
    #[serde(rename(deserialize = "mediaType"))]
    pub media_type: String,
    pub size: i32,
    pub digest: String,
    pub platform: ImagePlatform,
}

/// application/vnd.docker.distribution.manifest.list.v2+json
#[derive(Deserialize)]
pub struct ManifestListV2 {
    #[serde(rename(deserialize = "mediaType"))]
    pub media_type: String,
    pub manifests: Vec<ImageIndex>,
}

/// application/vnd.oci.image.index.v1+json
#[derive(Debug, Deserialize)]
pub struct ImageIndexV1 {
    #[serde(rename(deserialize = "mediaType"))]
    pub media_type: String,
    pub manifests: Vec<ImageIndex>,
}

/*
  "config": {
    "Hostname": "",
    "Domainname": "",
    "User": "",
    "AttachStdin": false,
    "AttachStdout": false,
    "AttachStderr": false,
    "ExposedPorts": {
      "80/tcp": {}
    },
    "Tty": false,
    "OpenStdin": false,
    "StdinOnce": false,
    "Env": [
      "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
      "NGINX_VERSION=1.25.2",
      "NJS_VERSION=0.8.0",
      "PKG_RELEASE=1~bookworm"
    ],
    "Cmd": [
      "nginx",
      "-g",
      "daemon off;"
    ],
    "Image": "sha256:3b2f458929e18623de347479de73236067715ac36681cf720ad29da0382df8fb",
    "Volumes": null,
    "WorkingDir": "",
    "Entrypoint": [
      "/docker-entrypoint.sh"
    ],
    "OnBuild": null,
    "Labels": {
      "maintainer": "NGINX Docker Maintainers <docker-maint@nginx.com>"
    },
    "StopSignal": "SIGQUIT"
  }
*/
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ImageConfigRuntime {
    /// 8080/tcp, 8080/udp, 8080
    pub exposed_ports: Option<serde_json::Value>,
    pub env: Vec<String>,
    pub working_dir: Option<String>,
    /// Arguments to pass to the binary, treated as Entrypoint if it's absent
    pub cmd: Vec<String>,
    /// Binary to execute
    pub entrypoint: Option<Vec<String>>,
    pub stop_signal: Option<String>,
}

/// application/vnd.oci.image.config.v1+json
#[derive(Debug, Deserialize)]
pub struct ImageConfig {
    pub architecture: String,
    pub os: String,
    pub config: ImageConfigRuntime,
    // We don't care about the rest of the fields for now
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

        panic!("Invalid manifest: {manifest}");
    }

    /// Parses and return's the image config for running the container
    pub fn get_image_config(&self, manifest: &Manifest) -> Result<ImageConfig, ureq::Error> {
        let config = self.get_blob(&manifest.config.digest)?;
        Ok(util::serde_result_to_ureq(serde_json::from_slice(&config))?)
    }
}
