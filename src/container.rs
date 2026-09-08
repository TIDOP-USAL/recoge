//use std::process::Command;
//use std::collections::HashMap;
use crate::error::{ClientError, Error, Result};
use bollard::models::ImageInspect;
use bollard::query_parameters::ListImagesOptionsBuilder;
use bollard::{Docker, plugin::ImageSummary};
use chrono::{DateTime, Local, Utc};
use futures_lite::stream::StreamExt;
use serde::{Deserialize, Serialize};

/*
#[allow(non_snake_case, non_camel_case_types)]
#[derive(serde::Deserialize, Debug)]
pub struct NativeImage {
    pub Id: String,
    pub Size: i64, // TODO a enum. string ok en docker, bytes en podman (o virtualsize)
    pub Containers: u64,
    pub Labels: Option<HashMap<String, String>>,
    pub Digest: String,
    pub Created: Option<i64>,
    pub CreatedAt: Option<String>,
}

pub type NativeImageList = Vec<NativeImage>;

pub fn get_image_list()-> Vec<Image> {
    let mut list = Vec::new();
    let cmd = "docker image ls --format json";
    let output = if cfg!(target_os = "windows") {
    Command::new("cmd")
            .args(["/C", cmd])
            .output()
            .expect("failed to execute process")
    } else {
    Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .expect("failed to execute process")
    };
    println!("{output:?}");
    if !output.status.success() {
    println!("Error on cmd");
    return list;
    }
    let str = str::from_utf8(&output.stdout).expect("Non utf8 characters in cmd out");
    // TODO: comprobar primer caracter. Si "[": Podman., si no son líneas de Docker
    let native_list: NativeImageList = serde_json::from_str(str).expect("Can't parse");
    //println!("{native_list:?}");

    for image in native_list {
    if let Some(labels)= image.Labels  {
        let name = labels.get("org.opencontainers.image.base.name").unwrap_or(&"no label".to_string()).to_string();
        list.push({
        Image { name,
            size_bytes: image.Size,
            digest: image.Digest,
            date: format!("{:?}",image.Created),
        }
        });
    };
    }
    list
}
*/

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
    pub fn get_id_value(&self) -> String {
        self.id.split("sha256:").last().unwrap_or("").to_string()
    }

    // Short version of the same digest
    pub fn get_short_id(&self) -> String {
        self.get_id_value().chars().take(12).collect()
    }

    pub fn to_images(&self) -> Vec<Image> {
        let mut images = Vec::new();
        for repo_info in &self.image_aliases {
            images.push(Image {
                repository_info: Some(repo_info.clone()),
                size_bytes: self.size_bytes,
                id: self.id.clone(),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RepositoryInfo {
    pub repository: String,
    pub tag: String,
}

impl RepositoryInfo {
    pub fn get_image(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
    }

    pub fn get_stripped_repository(&self) -> String {
        let mut repo_name = self.repository.clone();
        // Strip domain from repo name
        let mut parts = repo_name.split('/');
        let part_count = parts.clone().collect::<Vec<_>>().len();

        if part_count > 1
            && let Some(first) = parts.next()
        {
            // Is domain (crude)
            if first.contains(".") || first.contains("localhost") || first.contains(":") {
                let mut remainder = String::new();
                for part in parts {
                    remainder += &format!("{part}/");
                }
                remainder.pop(); // remove trailing '/'
                repo_name = remainder;
            }
        }
        println!("parsed name: {repo_name:?}");
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
            repository_str += &format!("{part}:");
        }
        repository_str.pop(); // remove trailing ':'
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
        let image_digests = image.repo_digests.clone().unwrap_or(Vec::new());
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
    pub fn get_id_value(&self) -> String {
        self.id.split("sha256:").last().unwrap_or("").to_string()
    }

    // Short version of the same digest
    pub fn get_short_id(&self) -> String {
        self.get_id_value().chars().take(12).collect()
    }

    // Repo name only. Human readable
    pub fn get_repo_name(&self) -> String {
        match &self.repository_info {
            Some(repo_info) => repo_info.repository.to_string(),
            None => format!("<Sin nombre: {}>", self.get_short_id()),
        }
    }

    pub fn has_image_name(&self) -> bool {
        self.repository_info.is_some()
    }

    // Full image name (repo:tag or id digest)
    pub fn get_image_name(&self) -> String {
        if let Some(repo_info) = &self.repository_info {
            if repo_info.repository != "localhost" {
                return repo_info.get_image();
            } else {
                // "localhost" means untagged image. There might me multiple of these: show/use short_id instead
                return self.get_short_id();
            }
        }
        self.get_id_value()
    }

    pub fn get_date_local(&self) -> String {
        let date: DateTime<Local> = self.date.into();
        let formatted_date = date.format("%d/%m/%Y %H:%M").to_string();
        formatted_date
    }

    pub fn get_relative_date(&self) -> String {
        todo!()
    }

    pub fn get_human_size(&self) -> String {
        if let Some(bytes) = self.size_bytes {
            humansize::format_size(bytes as u64, humansize::DECIMAL)
        } else {
            "Desconocido".to_string()
        }
    }

    pub async fn is_in_local(&self) -> Result<bool> {
        match bollard_image_inspect(self.get_image_name()).await {
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

pub async fn connect() -> Result<Docker> {
    println!("Connecting to Docker...");
    let mut docker = Docker::connect_with_local_defaults();

    #[cfg(not(target_os = "windows"))]
    {
        if docker.is_err() {
            println!("Connecting to Podman...");
            docker = Docker::connect_with_podman_defaults();
        }
    }
    #[cfg(target_family = "unix")]
    {
        if docker.is_err() {
            println!("Connecting with UNIX sockets...");
            docker = Docker::connect_with_socket_defaults();
            if docker.is_err() {
                let home_dir = match std::env::home_dir() {
                    Some(x) => {
                        let str = x.into_os_string();
                        str.to_str().unwrap_or("~").to_string()
                    }
                    None => "~".to_string(),
                };
                println!("Connecting with macOS UNIX sockets...");
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
            println!("Can't connect: {x:?}");
            return Err(Error::Program(
                "Can't connect to container services".to_string(),
            ));
        }
    };
    println!("Connected!");
    //println!("{docker:?}");
    Ok(docker)
}

// TODO fix bug: "localhost:5000" is not correctly parsed
// Image list, grouped by same image
pub async fn bollard_image_collection_list() -> Result<Vec<ImageCollection>> {
    let docker = connect().await?;
    println!("Getting image list...");
    let options = ListImagesOptionsBuilder::default().all(false).build();
    let images = match docker.list_images(Some(options)).await {
        Ok(images) => images,
        Err(x) => {
            let err_str = x.to_string();
            eprintln!("{err_str}");
            return Err(Error::Program(err_str));
        }
    };

    let mut list = Vec::new();
    for summary in images {
        let Ok(image_collection) = ImageCollection::try_from(&summary) else {
            println!("Error: skipping invalid image collection");
            continue;
        };
        list.push(image_collection);
    }
    println!("{list:#?}");
    Ok(list)
}

// Ungrouped, raw image repos:tags list
pub async fn bollard_image_list() -> Result<Vec<Image>> {
    let collections = bollard_image_collection_list().await?;
    let mut images = Vec::new();
    for collection in collections {
        let id = collection.get_short_id();
        let size = collection.size_bytes;
        println!("Size collection {id}: {size:?}");
        images.append(&mut collection.to_images());
    }
    println!("{images:#?}");
    Ok(images)
}

pub async fn bollard_image_inspect(image: String) -> Result<ImageInspect> {
    let docker = connect().await?;
    println!("Inspecting image!");
    // TODO bug: Docker returns wrong size here?. Correct one on image_list.
    match docker.inspect_image(&image).await {
        Ok(image) => Ok(image.clone()),
        Err(x) => {
            let err_str = x.to_string();
            eprintln!("{err_str}");
            if err_str.contains("code 404") {
                return Err(ClientError::NotFound(format!("Image {image} not found")).err());
            }
            return Err(Error::Program(err_str));
        }
    }
}

#[allow(unused)]
async fn pull_image(docker: &Docker) {
    let options = bollard::query_parameters::CreateImageOptionsBuilder::default()
        .from_image("docker.io/busybox")
        .build();

    let mut a = docker.create_image(Some(options), None, None);
    while let Some(state) = a.next().await {
        println!("{state:?}");
    }
}

#[allow(unused)]
async fn push_image(docker: &Docker) {
    todo!()
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
