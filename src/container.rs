use crate::error::{ClientError, Error, Result};
use crate::utils;
use bollard::models::ImageInspect;
use bollard::query_parameters::{ListImagesOptionsBuilder, TagImageOptionsBuilder};
use bollard::{Docker, plugin::ImageSummary};
use chrono::{DateTime, Utc};
use futures_lite::stream::StreamExt;
use log::{debug, error, trace, warn};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, Mutex, mpsc::SyncSender};

// All aliases for one image. All repos, tags, digests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ImageCollection {
    /// Other repository name:tag pairs.
    /// "localhost" -> untagged, use id
    pub image_aliases: Vec<RepositoryInfo>, // TODO Can be empty!
    pub image_digests: Vec<String>, // TODO not parsed
    pub size_bytes: Option<i64>,
    /// https://docs.rs/bollard/latest/bollard/config/struct.ImageSummary.html#structfield.id
    pub id: String, // This is an unique id on Podman, one digest on Docker. Can be used to interact locally.
    pub date: DateTime<Utc>,
}

// TODO impl get Image from RepositoryInfo. Can't correlate digest
impl ImageCollection {
    #[must_use]
    pub fn get_id_value(&self) -> String {
        let id = self.image_digests.first().unwrap_or(&self.id);
        id.split("sha256:").last().unwrap_or("").to_string()
    }

    // Short version of the same digest
    #[must_use]
    pub fn get_short_id(&self) -> String {
        self.get_id_value().chars().take(12).collect()
    }

    #[must_use]
    pub fn to_images(&self) -> Vec<Image> {
        let mut images = Vec::new();
        for repo_info in &self.image_aliases {
            images.push(Image {
                repository_info: Some(repo_info.clone()),
                size_bytes: self.size_bytes,
                id: self.get_id_value(),
                digests: self.image_digests.clone(),
                date: self.date,
            });
        }
        images
    }
}

// An specific single image from ImageCollection. User can interact with this (retag, delete tag, upload to registry, ...)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub repository_info: Option<RepositoryInfo>, // Optional repository name:tag pair
    pub size_bytes: Option<i64>,
    //pub digest: String, // Full sha256:... Do not use to refer to an image/ as an id.
    pub id: String, // One of many digests. Do not use to interact with the image.
    pub digests: Vec<String>, // Full repo@sha256:... digests. Do not use to refer to an image/ as an id
    pub date: DateTime<Utc>,
}

// Actually image (repository + tag) info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, Ord, PartialOrd, Eq)]
pub struct RepositoryInfo {
    pub repository: String,
    pub tag: String,
}

impl RepositoryInfo {
    #[must_use]
    pub fn get_image(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
    }

    #[must_use]
    pub fn get_stripped_repository(&self) -> String {
        let mut repo_name = self.repository.clone();
        // Strip domain from repo name
        let mut parts = repo_name.split('/');
        let part_count = parts.clone().collect::<Vec<_>>().len();

        if part_count > 1
            && let Some(first) = parts.next()
        {
            // Is domain (crude)
            if first.contains('.') || first.contains("localhost") || first.contains(':') {
                let mut remainder = String::new();
                for part in parts {
                    let _ = write!(remainder, "{part}/");
                }
                remainder.pop(); // remove trailing '/'
                repo_name = remainder;
            }
        }
        trace!("Stripped repo parsed name: {repo_name:?}");
        repo_name
    }

    pub fn try_from_repo_tag(repo_tag: String) -> Result<Self> {
        let mut repo_tag = repo_tag;
        // Safety: if no ':', we assume the tag to be "latest", and append it
        if !repo_tag.contains(':') {
            repo_tag += ":latest";
        }

        // repo:tag. repo may also contain ":"
        let mut repo_parts: Vec<&str> = repo_tag.split(':').collect();
        let Some(tag_str) = repo_parts.pop() else {
            return Err(Error::Program(
                "invalid image format ({repo_tag})".to_string(),
            ));
        };
        let mut repository_str = String::new();
        for part in repo_parts {
            let _ = write!(repository_str, "{part}:");
        }
        repository_str.pop(); // remove trailing ':'

        if repository_str.starts_with("localhost/") {
            // TODO this is metadata: locally tagged image
            repository_str = repository_str
                .strip_prefix("localhost/")
                .unwrap()
                .to_string();
        }
        Ok(RepositoryInfo {
            repository: repository_str,
            tag: tag_str.to_string(),
        })
    }
}

impl TryFrom<&ImageSummary> for ImageCollection {
    type Error = Error;

    fn try_from(image: &ImageSummary) -> Result<Self> {
        let mut image_aliases = Vec::new();
        for repo_tag in image.repo_tags.clone() {
            // repo:tag (Optional)
            image_aliases.push(RepositoryInfo::try_from_repo_tag(repo_tag)?);
        }
        let image_digests = image.repo_digests.clone();
        Ok(Self {
            image_aliases,
            image_digests,
            size_bytes: Some(image.size),
            id: image.id.clone(),
            date: DateTime::from_timestamp_secs(image.created).unwrap(),
        })
    }
}

impl TryFrom<&ImageInspect> for ImageCollection {
    type Error = Error;

    fn try_from(image: &ImageInspect) -> Result<Self> {
        let mut image_aliases = Vec::new();
        if let Some(repo_tags) = image.repo_tags.clone() {
            for repo_tag in repo_tags {
                // repo:tag (Optional)
                image_aliases.push(RepositoryInfo::try_from_repo_tag(repo_tag)?);
            }
        }
        let Some(id) = image.id.clone() else {
            return Err(Error::Program("no id".to_string()));
        };
        let Some(created) = image.created.clone() else {
            // TODO fail?
            return Err(Error::Program("no date".to_string()));
        };
        let image_digests = image.repo_digests.clone().unwrap_or_default();
        Ok(Self {
            image_aliases,
            image_digests,
            size_bytes: image.size,
            id,
            date: DateTime::parse_from_rfc3339(&created).unwrap().into(),
        })
    }
}

impl Image {
    // Get one of the image's digests, HR, not preferred as internal ID
    #[must_use]
    pub fn get_id_value(&self) -> String {
        self.id.split("sha256:").last().unwrap_or("").to_string()
    }

    // Short version of the same digest
    #[must_use]
    pub fn get_short_id(&self) -> String {
        self.get_id_value().chars().take(12).collect()
    }

    // Repo name only. Human readable
    #[must_use]
    pub fn get_repo_name(&self) -> String {
        match &self.repository_info {
            Some(repo_info) => repo_info.repository.clone(),
            None => format!("<Sin nombre: {}>", self.get_short_id()),
        }
    }

    #[must_use]
    pub fn has_image_name(&self) -> bool {
        self.repository_info.is_some()
    }

    // Full image name (repo:tag or id digest)
    #[must_use]
    pub fn get_image_name(&self) -> String {
        if let Some(repo_info) = &self.repository_info {
            if repo_info.repository != "localhost" {
                return repo_info.get_image();
            }
            // "localhost" means untagged image. There might me multiple of these: show/use short_id instead
            return self.get_short_id();
        }
        self.get_id_value()
    }

    #[must_use]
    pub fn get_date_local(&self) -> String {
        utils::get_date_local(&self.date)
    }

    #[must_use]
    pub fn get_relative_date(&self) -> String {
        utils::get_relative_date(&self.date)
    }

    #[must_use]
    pub fn get_human_size(&self) -> String {
        if let Some(bytes) = self.size_bytes {
            humansize::format_size(bytes.cast_unsigned(), humansize::DECIMAL)
        } else {
            "Desconocido".to_string()
        }
    }

    pub async fn is_in_local(&self) -> Result<bool> {
        let docker = Client::new().await?;
        match docker.image_inspect(self.get_image_name()).await {
            Ok(_) => Ok(true),
            Err(x) => {
                if x.is_not_found() {
                    Ok(false)
                } else {
                    Err(x)
                }
            }
        }
    }
}

pub struct Client {
    client: Docker,
}

impl Client {
    pub async fn new() -> Result<Self> {
        debug!("Connecting to Docker...");
        #[allow(unused)]
        let mut docker = Docker::connect_with_local_defaults();

        #[cfg(not(target_os = "windows"))]
        {
            if docker.is_err() {
                debug!("Connecting to Podman...");
                docker = Docker::connect_with_podman_defaults();
            }
        }
        #[cfg(target_family = "unix")]
        {
            if docker.is_err() {
                debug!("Connecting with UNIX sockets...");
                docker = Docker::connect_with_socket_defaults();
                if docker.is_err() {
                    let home_dir = match std::env::home_dir() {
                        Some(x) => {
                            let str = x.into_os_string();
                            str.to_str().unwrap_or("~").to_string()
                        }
                        None => "~".to_string(),
                    };
                    debug!("Connecting with macOS UNIX sockets...");
                    docker = Docker::connect_with_socket(
                        &format!("{home_dir}/.docker/run/docker.sock"),
                        120,
                        bollard::API_DEFAULT_VERSION,
                    );
                }
            }
        }
        let docker = match docker {
            Ok(x) => x,
            Err(x) => {
                error!("Can't connect to Docker: {x:?}");
                return Err(Error::Program(
                    "Can't connect to container services".to_string(),
                ));
            }
        };
        debug!("Connected!");
        trace!("{docker:?}");
        Ok(Self { client: docker })
    }

    // Image list, grouped by same image
    pub async fn image_collection_list(&self) -> Result<Vec<ImageCollection>> {
        let docker = &self.client;
        debug!("Getting image list...");
        let options = ListImagesOptionsBuilder::default().all(false).build();
        let images = match docker.list_images(Some(options)).await {
            Ok(images) => images,
            Err(x) => {
                let err_str = x.to_string();
                error!("Can't list local images {err_str}");
                return Err(Error::Program(err_str));
            }
        };

        let mut list = Vec::new();
        for summary in images {
            let Ok(image_collection) = ImageCollection::try_from(&summary) else {
                warn!("Error: skipping invalid image collection");
                continue;
            };
            list.push(image_collection);
        }
        trace!("Local image collections: {list:#?}");
        Ok(list)
    }

    // Ungrouped, raw image repos:tags list
    pub async fn image_list(&self) -> Result<Vec<Image>> {
        let collections = self.image_collection_list().await?;
        let mut images = Vec::new();
        for collection in collections {
            let id = match collection.image_digests.first() {
                Some(id) => id,
                None => &collection.get_short_id(),
            };
            let size = collection.size_bytes;
            trace!("Size collection {id}: {size:?}");
            images.append(&mut collection.to_images());
        }
        trace!("Local images: {images:#?}");
        Ok(images)
    }

    pub async fn image_inspect(&self, image: String) -> Result<ImageInspect> {
        let docker = &self.client;
        debug!("Inspecting image!");
        // TODO bug: Docker returns wrong size here?. Correct one on image_list.
        match docker.inspect_image(&image).await {
            Ok(image) => Ok(image.clone()),
            Err(x) => {
                let err_str = x.to_string();
                error!("Can't inspect local image: {err_str}");
                if err_str.contains("code 404") {
                    return Err(ClientError::NotFound(format!("Image {image} not found")).err());
                }
                Err(Error::Program(err_str))
            }
        }
    }

    // TODO this does not always work on Podman
    pub async fn image_add_tag(&self, image: String, new_repo_tag: String) -> Result<()> {
        let docker = &self.client;
        let repo_info = RepositoryInfo::try_from_repo_tag(new_repo_tag)?;
        let tag_options = TagImageOptionsBuilder::default()
            .repo(&repo_info.repository)
            .tag(&repo_info.tag)
            .build();
        if let Err(x) = docker.tag_image(&image, Some(tag_options)).await {
            let err_str = x.to_string();
            error!("Can't tag local image: {err_str}");
            return Err(Error::Program(err_str));
        }
        Ok(())
    }

    pub async fn pull_image(
        &self,
        image: String,
        reg_creds: (String, String),
        tx: Arc<Mutex<SyncSender<bollard::config::CreateImageInfo>>>,
        rename_name: Option<String>,
    ) -> Result<()> {
        let docker = &self.client;
        debug!("Pulling image...");
        let options = bollard::query_parameters::CreateImageOptionsBuilder::default()
            .from_image(&image)
            .build();
        debug!("Using options: {options:#?}");
        let credentials = bollard::auth::DockerCredentials {
            username: Some(reg_creds.0.clone()),
            password: Some(reg_creds.1.clone()),
            ..Default::default()
        };
        trace!("using {credentials:#?}"); // Sensitive info
        let mut a = docker.create_image(Some(options), None, Some(credentials));
        while let Some(state) = a.next().await {
            trace!("Pull state: {state:?}");
            let tx = tx.lock().unwrap();
            match state {
                Ok(info) => {
                    // TODO send timestamp?
                    match tx.send(info) {
                        Err(x) => {
                            error!("Can't send pull state: {:#?}", x.0);
                        }
                        Ok(_x) => (),
                    }
                }
                Err(x) => {
                    error!("Pull error {x:?}");
                    let error = bollard::config::CreateImageInfo {
                        error_detail: Some(bollard::config::ErrorDetail {
                            code: None,
                            message: format!("{x:?}").into(),
                        }),
                        ..Default::default()
                    };
                    if let Err(x) = tx.send(error) {
                        error!("Can't send push state: {:?}", x.0);
                    }
                    return Err(Error::Program(format!("{x:#?}")));
                }
            }
        }
        debug!("Pull OK: {image}");
        // Pull OK. Retag if required
        if let Some(new_repo_tag) = rename_name {
            debug!("Copying {image} to {new_repo_tag}");
            let r = self.image_add_tag(image, new_repo_tag).await;
            debug!("Rename result: {r:?}");
        }
        Ok(())
    }

    pub async fn push_image(
        &self,
        // should exist
        local_image: RepositoryInfo,
        // will be created/tagged if necessary
        remote_images: Vec<RepositoryInfo>,
        clean_local: bool,
        reg_creds: (String, String),
        tx: Arc<Mutex<SyncSender<bollard::config::PushImageInfo>>>,
    ) -> Result<()> {
        let docker = &self.client;
        debug!("Pushing image...");
        let local_image_name = local_image.get_image();
        let mut remote_images = remote_images;
        let mut new_local_image_names = Vec::new();
        remote_images.sort();
        remote_images.dedup();
        for remote_image in remote_images {
            let remote_image_name = remote_image.get_image();
            if local_image_name != remote_image_name {
                // Needs to be tagged before pushing
                new_local_image_names.push(remote_image_name.clone());
                if let Err(x) = self
                    .image_add_tag(local_image_name.clone(), remote_image_name.clone())
                    .await
                {
                    error!("Can't retag before pushing: {x:?}");
                    return Err(x);
                }
            }
            debug!("Pushing {local_image_name} as {remote_image_name}");

            let options = bollard::query_parameters::PushImageOptionsBuilder::default()
                .tag(&remote_image.tag)
                .build();
            debug!("Using options: {options:#?}");
            let credentials = bollard::auth::DockerCredentials {
                username: Some(reg_creds.0.clone()),
                password: Some(reg_creds.1.clone()),
                ..Default::default()
            };
            trace!("using {credentials:#?}"); // Sensitive info
            let mut a =
                docker.push_image(&remote_image.repository, Some(options), Some(credentials));
            while let Some(state) = a.next().await {
                trace!("Push state: {state:?}");
                let tx = tx.lock().unwrap();
                match state {
                    Ok(info) => {
                        // TODO send timestamp?
                        match tx.send(info) {
                            Err(x) => {
                                error!("Can't send push state: {:#?}", x.0);
                            }
                            Ok(_x) => (),
                        }
                    }
                    Err(x) => {
                        error!("Push error {x:?}");
                        let error = bollard::config::PushImageInfo {
                            error_detail: Some(bollard::config::ErrorDetail {
                                code: None,
                                message: format!("{x:?}").into(),
                            }),
                            ..Default::default()
                        };
                        if let Err(x) = tx.send(error) {
                            error!("Can't send push state: {:?}", x.0);
                        }
                        return Err(Error::Program(format!("{x:#?}")));
                    }
                }
            }
            debug!("Push OK: {remote_image_name}");
        }
        // Images were pushed
        if clean_local {
            // Remove new local images
            for extra_image in new_local_image_names {
                self.delete_image(extra_image, true, true).await?;
            }
        }
        Ok(())
    }

    pub async fn delete_image(
        &self,
        image: String,
        force: bool,
        noprune: bool, // Do not delete untagged parent images
    ) -> Result<Vec<bollard::config::ImageDeleteResponseItem>> {
        let docker = &self.client;
        debug!("Deleting image");
        let options = bollard::query_parameters::RemoveImageOptions {
            force,
            noprune,
            ..Default::default()
        };
        debug!("Using options: {options:#?}");
        let result = match docker.remove_image(&image, Some(options), None).await {
            Ok(x) => x,
            Err(x) => {
                // TODO error parsing
                error!("Error while deleting {image}: {x:?}");
                return Err(Error::Program(format!("{x:?}")));
            }
        };
        debug!("Delete OK: {result:?}");
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repositoryinfo_strip_works() {
        let mut info = RepositoryInfo {
            repository: "domain.com/one/two".to_string(),
            tag: String::new(),
        };
        assert_eq!(&info.get_stripped_repository(), "one/two");
        info.repository = "domain.com:443/one/two".to_string();
        assert_eq!(&info.get_stripped_repository(), "one/two");
        info.repository = "192.168.1.1/one/two".to_string();
        assert_eq!(&info.get_stripped_repository(), "one/two");
        info.repository = "192.168.1.1:5000/one/two".to_string();
        assert_eq!(&info.get_stripped_repository(), "one/two");
        info.repository = "localhost/one/two".to_string();
        assert_eq!(&info.get_stripped_repository(), "one/two");
        info.repository = "one/two".to_string();
        assert_eq!(&info.get_stripped_repository(), "one/two");
    }

    #[test]
    fn repositoryinfo_from_image_works() {
        // Usual cases
        let info =
            RepositoryInfo::try_from_repo_tag("domain.com/one/two:v1.0".to_string()).unwrap();
        assert_eq!(info.repository, "domain.com/one/two");
        assert_eq!(info.tag, "v1.0");
        let info = RepositoryInfo::try_from_repo_tag("one/two:v1.0".to_string()).unwrap();
        assert_eq!(info.repository, "one/two");
        assert_eq!(info.tag, "v1.0");
        // Repo contains port
        let info =
            RepositoryInfo::try_from_repo_tag("domain.com:5000/one/two:v1.0".to_string()).unwrap();
        assert_eq!(info.repository, "domain.com:5000/one/two");
        assert_eq!(info.tag, "v1.0");
        let info =
            RepositoryInfo::try_from_repo_tag("192.168.1.1:5000/one/two:v1.0".to_string()).unwrap();
        assert_eq!(info.repository, "192.168.1.1:5000/one/two");
        assert_eq!(info.tag, "v1.0");
        // "latest" test
        let info = RepositoryInfo::try_from_repo_tag("domain.com/one/two".to_string()).unwrap();
        assert_eq!(info.repository, "domain.com/one/two");
        assert_eq!(info.tag, "latest");
    }
}
