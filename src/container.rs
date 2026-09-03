//use std::process::Command;
//use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub repository_info: Option<RepositoryInfo>, // Optional repository name:tag
    pub size_bytes: Option<i64>,
    pub digest: String, // Full sha256:...
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub repository: String,
    pub tag: String,
}

impl TryFrom<&ImageSummary> for Image {
    type Error = String;

    fn try_from(image: &ImageSummary) -> Result<Self, Self::Error> {
        let mut repository_info = None;
        if let Some(image) = image.repo_tags.first().cloned() {
            // repo:tag (Optional)
            let mut repo_parts = image.split(':');
            let Some(repository_str) = repo_parts.next() else {
                return Err("invalid img".to_string());
            };
            let Some(tag_str) = repo_parts.last() else {
                return Err("invalid img".to_string());
            };
            repository_info = Some(RepositoryInfo {
                repository: repository_str.to_string(),
                tag: tag_str.to_string(),
            });
        }
        Ok(Image {
            repository_info,
            size_bytes: Some(image.size),
            digest: image.id.clone(),
            date: DateTime::from_timestamp_secs(image.created).unwrap(),
        })
    }
}

impl TryFrom<&ImageInspect> for Image {
    type Error = String;

    fn try_from(image: &ImageInspect) -> Result<Self, Self::Error> {
        let mut repository_info = None;
        if let Some(repo_tags) = image.repo_tags.clone()
            && let Some(image) = repo_tags.first().cloned()
        {
            // repo:tag (Optional)
            let mut repo_parts = image.split(':');
            let Some(repository_str) = repo_parts.next() else {
                return Err("invalid img".to_string());
            };
            let Some(tag_str) = repo_parts.last() else {
                return Err("invalid img".to_string());
            };
            repository_info = Some(RepositoryInfo {
                repository: repository_str.to_string(),
                tag: tag_str.to_string(),
            });
        }
        let Some(digest) = image.id.clone() else {
            return Err("no digest".to_string());
        };
        let Some(created) = image.created.clone() else {
            // TODO fail?
            return Err("no date".to_string());
        };

        Ok(Image {
            repository_info,
            size_bytes: image.size,
            digest,
            date: DateTime::parse_from_rfc3339(&created).unwrap().into(),
        })
    }
}

impl Image {
    pub fn get_digest_value(&self) -> String {
        let Some(a) = self.digest.split(':').last() else {
            return String::new();
        };
        a.to_string()
    }

    pub fn get_id(&self) -> String {
        let Some(a) = self.digest.split(':').last() else {
            return String::new();
        };
        let a: String = a.chars().take(12).collect();
        a
    }

    // Repo name only. Human readable
    pub fn get_repo_name(&self) -> String {
        match &self.repository_info {
            Some(repo_info) => repo_info.repository.to_string(),
            None => format!("<Sin nombre: {}>", self.get_id()),
        }
    }

    // Full image name (repo:tag or digest)
    pub fn get_image_name(&self) -> String {
        if let Some(repo_info) = &self.repository_info {
            format!("{}:{}", repo_info.repository, repo_info.tag)
        } else {
            self.get_digest_value()
        }
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
}

pub async fn connect() -> Result<Docker, ()> {
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
            return Err(());
        }
    };
    println!("Connected!");
    //println!("{docker:?}");
    Ok(docker)
}

pub async fn bollard_image_list() -> Result<Vec<Image>, String> {
    let docker = match connect().await {
        Ok(x) => x,
        Err(()) => {
            return Err(format!("No Docker connection"));
        }
    };
    println!("Getting image list!");
    let options = ListImagesOptionsBuilder::default().all(false).build();
    let images = match docker.list_images(Some(options)).await {
        Ok(images) => images,
        Err(x) => {
            let err_str = x.to_string();
            eprintln!("{err_str}");
            return Err(err_str);
        }
    };

    let mut list = Vec::new();
    for summary in images {
        let Ok(image) = Image::try_from(&summary) else {
            continue;
        };
        list.push(image);
    }
    //println!("{list:?}");

    Ok(list)
}

pub async fn bollard_image_inspect(image: String) -> Result<ImageInspect, String> {
    let docker = match connect().await {
        Ok(x) => x,
        Err(()) => {
            return Err(format!("No Docker connection"));
        }
    };
    println!("Inspecting image!");
    let image = &docker.inspect_image(&image).await;
    match image {
        Ok(image) => Ok(image.clone()),
        Err(x) => {
            let err_str = x.to_string();
            eprintln!("{err_str}");
            return Err(err_str);
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
