use crate::configuration::AppConfig;
use crate::container;
use crate::error::{ClientError, Error, RemoteError, Result};
use chrono::{DateTime, Utc};
use reqwest::Method;
use reqwest::RequestBuilder;
use reqwest::Url;
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

pub struct Client {
    api_base_url: String,
    username: String,
    password: String,
}

impl Client {
    pub fn new(config: &AppConfig) -> Result<Self> {
        if config.remote_url.is_empty() {
            println!("Can't create zot client without API URL"); // TODO public images support
            return Err(ClientError::Invalid("Missing URL".to_string()).err());
        }
        if config.is_missing_credentials() {
            println!("Can't create zot client without user credentials"); // TODO public images support
            return Err(ClientError::Invalid("Missing credentials".to_string()).err());
        }
        println!("Zot client created");
        let mut api_base_url = config.remote_url.clone();
        if !api_base_url.starts_with("https://") {
            api_base_url = format!("https://{api_base_url}");
        }
        let client = Self {
            api_base_url,
            username: config.user.clone(),
            password: config.password.clone(),
        };
        Ok(client)
    }

    fn client(
        &mut self,
        method: Method,
        path: &str,
        query: Option<&str>,
    ) -> Result<RequestBuilder> {
        let Ok(mut url) = Url::parse(&self.api_base_url) else {
            println!("URL parse error");
            return Err(ClientError::Invalid("URL".to_string()).err());
        };
        url.set_scheme("https").unwrap();
        url.set_path(path);
        url.set_query(query);
        println!("Using URL: {url}");
        let client = reqwest::Client::new().request(method, url.as_str());
        let client = client
            .header(
                "User-Agent",
                format!("recoge v{}", env!("CARGO_PKG_VERSION")),
            )
            .header(ACCEPT, "application/json, text/plain, */*")
            .basic_auth(&self.username, Some(&self.password));
        Ok(client)
    }

    pub async fn get_image_list(&mut self) -> Result<Vec<container::Image>> {
        let query = r"query=
{
  RepoListWithNewestImage(requestedPage: {limit: 0, offset:0, sortBy: ALPHABETIC_ASC}) {
    Results {
      Name
      LastUpdated
      Size
      Platforms {
        Os
        Arch
      }
      NewestImage {
        Digest
        Tag
      }
    }
  }
}
"
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        let Ok(response) = self
            .client(Method::GET, "/v2/_zot/ext/search", Some(&query))?
            .send()
            .await
        else {
            return Err(Error::Program("Can't reach zot".to_string()));
        };
        println!("{response:?}");
        if response.status().is_client_error() {
            return Err(Error::Program("zot client err".to_string()));
        }
        if response.status().is_server_error() {
            return Err(RemoteError::Generic("zot err".to_string()).err());
        }

        let body: RepoListWithNewestImageResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let list = body.data.RepoListWithNewestImage.Results;
        println!("List: {list:#?}");
        let mut images = Vec::new();
        for result in list {
            let Ok(image) = container::Image::try_from(&result) else {
                println!("Error: skipping invalid image");
                continue;
            };
            images.push(image);
        }
        Ok(images)
    }

    pub async fn get_image_info(&mut self, image_id: String) -> Result<container::Image> {
        let query = format!(
            r#"query=
{{
  Image(image: "{image_id}") {{
    RepoName
    Tag
    LastUpdated
    Digest
    Size
    Description
  }}
}}
"#
        )
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        let Ok(response) = self
            .client(Method::GET, "/v2/_zot/ext/search", Some(&query))?
            .send()
            .await
        else {
            return Err(Error::Program("Can't reach zot".to_string()));
        };
        println!("{response:?}");
        if response.status().is_client_error() {
            return Err(Error::Program("zot client err".to_string()));
        }
        if response.status().is_server_error() {
            return Err(RemoteError::Generic("zot err".to_string()).err());
        }

        let body: ImageResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let Some(data) = body.data else {
            // Not found
            return Err(ClientError::NotFound("Image not found".to_string()).err());
        };
        let image = container::Image::try_from(&data.Image)?;
        println!("Image: {image:#?}");
        Ok(image)
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListWithNewestImageResponse {
    pub data: RepoListWithNewestImageHolder,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListWithNewestImageHolder {
    pub RepoListWithNewestImage: RepoListWithNewestImage,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListWithNewestImage {
    pub Results: Vec<RepoListImage>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListImage {
    pub Name: String,
    pub LastUpdated: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub Size: i64,
    pub NewestImage: ImageTags,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageTags {
    pub Digest: String,
    pub Tag: String,
}

impl TryFrom<&RepoListImage> for container::Image {
    type Error = Error;

    fn try_from(image: &RepoListImage) -> Result<Self> {
        let repository_info = Some(container::RepositoryInfo {
            repository: image.Name.clone(),
            tag: image.NewestImage.Tag.clone(),
        });

        Ok(container::Image {
            repository_info,
            size_bytes: Some(image.Size),
            digest: image.NewestImage.Digest.clone(),
            date: image.LastUpdated,
        })
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageResponse {
    pub data: Option<ImageHolder>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageHolder {
    pub Image: ImageInfo, // TODO not RepoListImage
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageInfo {
    pub Description: String,
    pub LastUpdated: DateTime<Utc>,
    pub RepoName: String,
    pub Digest: String,
    pub Tag: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub Size: i64,
}

impl TryFrom<&ImageInfo> for container::Image {
    type Error = Error;

    fn try_from(image: &ImageInfo) -> Result<Self> {
        let repository_info = Some(container::RepositoryInfo {
            repository: image.RepoName.clone(),
            tag: image.Tag.clone(),
        });

        Ok(container::Image {
            repository_info,
            size_bytes: Some(image.Size),
            digest: image.Digest.clone(),
            date: image.LastUpdated,
        })
    }
}

// TODO refactor.
#[derive(Debug, Clone, Default)]
pub struct RemoteImageInfo {
    pub is_in_remote: bool,
    pub image_info: Option<container::Image>,
}

impl container::Image {
    pub async fn get_remote_info(self, client: &mut Client) -> Result<RemoteImageInfo> {
        println!("checking on remote: {self:#?}");
        let mut image_name = self.get_image_name();
        // Strip domain from repo name
        let mut parts = image_name.split('/');
        let part_count = parts.clone().collect::<Vec<_>>().len();

        if part_count > 1
            && let Some(first) = parts.next()
        {
            // Is domain (crude)
            if first.contains(".") || first.contains("localhost") {
                let mut remainder = String::new();
                for part in parts {
                    remainder += &format!("{part}/");
                }
                remainder.pop(); // remove trailing '/'
                image_name = remainder;
            }
        }
        println!("parsed name: {image_name:?}");

        match client.get_image_info(image_name).await {
            Ok(x) => Ok(RemoteImageInfo {
                is_in_remote: true,
                image_info: Some(x),
            }),
            Err(x) => {
                if x.is_not_found() {
                    Ok(RemoteImageInfo {
                        is_in_remote: false,
                        ..Default::default()
                    })
                } else {
                    Err(x)
                }
            }
        }
    }
}
