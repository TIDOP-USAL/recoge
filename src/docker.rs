//use std::process::Command;
//use std::collections::HashMap;

use bollard::Docker;
use bollard::models::ImageInspect;
use bollard::query_parameters::ListImagesOptionsBuilder;
use chrono::{DateTime, Local, Utc};
use futures_lite::stream::StreamExt;

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

#[derive(Debug, Clone)]
pub struct Image {
    pub name: Option<String>, // Optional tag name
    pub size_bytes: u64,
    pub digest: String, // Full sha256:...
    pub date: DateTime<Utc>,
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

    pub fn get_name(&self) -> String {
        match &self.name {
            Some(name) => name.to_string(),
            None => format!("<Sin nombre: {}>", self.get_id()),
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
        humansize::format_size(self.size_bytes, humansize::DECIMAL)
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
    let Ok(docker) = docker else {
        return Err(());
    };
    println!("Connected!");
    Ok(docker)
}

pub async fn bollard_image_list() -> Vec<Image> {
    let docker = connect().await.unwrap();
    println!("{docker:?}");
    println!("Getting image list!");
    let options = ListImagesOptionsBuilder::default().all(false).build();
    let images = &docker.list_images(Some(options)).await.unwrap();

    let mut list = Vec::new();
    for image in images {
        list.push(Image {
            name: image.repo_tags.first().cloned(),
            size_bytes: image.size as u64,
            digest: image.id.clone(),
            date: DateTime::from_timestamp_secs(image.created).unwrap(),
        });
    }
    println!("{list:?}");

    list
}

pub async fn bollard_image_inspect(image: String) -> Result<ImageInspect, String> {
    let docker = connect().await.unwrap();
    println!("{docker:?}");
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
